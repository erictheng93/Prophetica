export function allowsMemberMockFallback(
  env: Record<string, string | undefined> = process.env,
): boolean {
  return env.PROPHETICA_ENV !== "production" && env.NODE_ENV !== "production";
}
