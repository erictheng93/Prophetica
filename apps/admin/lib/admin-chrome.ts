// Routes listed here render their own full-page or nested chrome, so the root
// admin chrome must pass them through unchanged. Add new standalone shells here.
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
