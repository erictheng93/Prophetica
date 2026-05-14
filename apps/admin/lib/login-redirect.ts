const LOCAL_ORIGIN = "http://prophetica-admin.local";

function safeLocalPath(candidate: string | null | undefined): string | null {
  const value = candidate?.trim();
  if (
    !value ||
    !value.startsWith("/") ||
    value.startsWith("//") ||
    value.includes("\\")
  ) {
    return null;
  }

  try {
    const url = new URL(value, LOCAL_ORIGIN);
    if (url.origin !== LOCAL_ORIGIN) return null;
    return `${url.pathname}${url.search}${url.hash}`;
  } catch {
    return null;
  }
}

export function resolveLoginRedirect(
  intendedRedirect: string | null | undefined,
  apiRedirect: string | null | undefined,
): string {
  return safeLocalPath(intendedRedirect) ?? safeLocalPath(apiRedirect) ?? "/";
}
