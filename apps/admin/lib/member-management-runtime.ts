export function allowsMemberMockFallback(
  env: Record<string, string | undefined> = process.env,
): boolean {
  return env.MEMBER_MANAGEMENT_ALLOW_MOCK_FALLBACK === "true";
}
