export interface NavActiveCandidate {
  href: string;
  key: string;
}

export function isNavPathActive(
  pathname: string | null,
  href: string,
): boolean {
  if (!pathname) return false;
  if (href === "/") return pathname === "/";

  return pathname === href || pathname.startsWith(`${href}/`);
}

export function findActiveNavChildKey(
  pathname: string | null,
  children: readonly NavActiveCandidate[],
): string | undefined {
  return children
    .filter((child) => isNavPathActive(pathname, child.href))
    .sort((left, right) => right.href.length - left.href.length)[0]?.key;
}
