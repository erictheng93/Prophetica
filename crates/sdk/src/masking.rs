//! Shared masking helpers for PII emitted by internal services.

#[must_use]
pub fn mask_email(value: &str) -> String {
    if let Some((local, domain)) = value.split_once('@')
        && let Some(first) = local.chars().next()
    {
        return format!("{first}***@{domain}");
    }

    "****".to_owned()
}
