terraform {
  required_version = ">= 1.8.0"

  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 5.19"
    }
  }
}

provider "cloudflare" {
  # Use CLOUDFLARE_API_TOKEN with Access: Apps and Policies Read/Write.
}

variable "cloudflare_account_id" {
  description = "Cloudflare account id that owns the Zero Trust tenant."
  type        = string
}

variable "admin_hostname" {
  description = "Admin app hostname without scheme, for example admin.prophetica.example."
  type        = string
}

variable "internal_email_domain" {
  description = "Email domain allowed to authenticate to internal admin surfaces."
  type        = string
}

variable "allowed_idp_ids" {
  description = "Optional Cloudflare Access IdP ids allowed for the admin applications."
  type        = list(string)
  default     = []
}

locals {
  common_tags = ["prophetica", "admin", "launch-gate-lg-9"]
}

resource "cloudflare_zero_trust_access_policy" "admin_internal_sso" {
  account_id = var.cloudflare_account_id
  name       = "Prophetica admin internal SSO with hardware MFA"
  decision   = "allow"

  include = [{
    email_domain = {
      domain = var.internal_email_domain
    }
  }]

  mfa_config = {
    allowed_authenticators = ["security_key"]
    mfa_disabled          = false
    session_duration      = "30m"
  }

  session_duration = "30m"
}

resource "cloudflare_zero_trust_access_policy" "public_bypass" {
  account_id = var.cloudflare_account_id
  name       = "Prophetica admin public login and affiliate bypass"
  decision   = "bypass"

  include = [{
    everyone = {}
  }]
}

resource "cloudflare_zero_trust_access_application" "admin_internal" {
  account_id                    = var.cloudflare_account_id
  name                          = "Prophetica admin internal surfaces"
  type                          = "self_hosted"
  domain                        = var.admin_hostname
  allowed_idps                  = var.allowed_idp_ids
  app_launcher_visible          = false
  auto_redirect_to_identity     = true
  options_preflight_bypass      = false
  path_cookie_attribute         = true
  service_auth_401_redirect     = true
  session_duration              = "30m"
  tags                          = local.common_tags

  policies = [{
    id         = cloudflare_zero_trust_access_policy.admin_internal_sso.id
    precedence = 1
  }]
}

resource "cloudflare_zero_trust_access_application" "admin_login_bypass" {
  account_id                = var.cloudflare_account_id
  name                      = "Prophetica admin login bypass"
  type                      = "self_hosted"
  domain                    = "${var.admin_hostname}/login*"
  app_launcher_visible      = false
  path_cookie_attribute     = true
  service_auth_401_redirect = true
  tags                      = local.common_tags

  policies = [{
    id         = cloudflare_zero_trust_access_policy.public_bypass.id
    precedence = 1
  }]
}

resource "cloudflare_zero_trust_access_application" "affiliate_portal_bypass" {
  account_id                = var.cloudflare_account_id
  name                      = "Prophetica affiliate portal bypass"
  type                      = "self_hosted"
  domain                    = "${var.admin_hostname}/me/affiliate*"
  app_launcher_visible      = false
  path_cookie_attribute     = true
  service_auth_401_redirect = true
  tags                      = local.common_tags

  policies = [{
    id         = cloudflare_zero_trust_access_policy.public_bypass.id
    precedence = 1
  }]
}

