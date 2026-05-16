const CLOUDFLARE_ACCESS_BYPASS_PREFIXES = ["/login", "/me/affiliate"] as const;

export function bypassesCloudflareAccess(pathname: string): boolean {
  return CLOUDFLARE_ACCESS_BYPASS_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );
}

export function requiresCloudflareAccess(pathname: string): boolean {
  return !bypassesCloudflareAccess(pathname);
}
