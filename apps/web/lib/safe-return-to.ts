const DEFAULT_RETURN_TO = "/portfolio";
const MAX_RETURN_TO_LENGTH = 1024;

export function safeReturnTo(
  raw: string | null | undefined,
  fallback = DEFAULT_RETURN_TO,
): string {
  const safeFallback = isSafeRelativePath(fallback)
    ? fallback
    : DEFAULT_RETURN_TO;
  const value = raw?.trim();
  if (!value || value.length > MAX_RETURN_TO_LENGTH) {
    return safeFallback;
  }

  let decoded: string;
  try {
    decoded = decodeURIComponent(value);
  } catch {
    return safeFallback;
  }

  if (!isSafeRelativePath(decoded)) {
    return safeFallback;
  }

  return decoded;
}

function isSafeRelativePath(value: string): boolean {
  if (!value.startsWith("/") || value.startsWith("//")) {
    return false;
  }
  if (/[\u0000-\u001f\u007f]/u.test(value)) {
    return false;
  }
  try {
    const parsed = new URL(value, "https://prophetica.local");
    return parsed.origin === "https://prophetica.local";
  } catch {
    return false;
  }
}
