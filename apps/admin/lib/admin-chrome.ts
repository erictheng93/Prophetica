export function isStandaloneAdminChromePath(pathname: string | null): boolean {
  if (!pathname) return false;

  return (
    pathname === "/login" ||
    pathname === "/offline" ||
    pathname === "/403" ||
    pathname === "/me/affiliate" ||
    pathname.startsWith("/me/affiliate/")
  );
}
