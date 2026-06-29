const TRUSTED_PROXY_SECRET_HEADER = "x-admin-trusted-proxy-secret";

interface ProductionAccessGateOptions {
  trustedProxySecret?: string;
}

function constantTimeEqual(left: string, right: string): boolean {
  const encoder = new TextEncoder();
  const leftBytes = encoder.encode(left);
  const rightBytes = encoder.encode(right);
  const maxLength = Math.max(leftBytes.length, rightBytes.length);
  let diff = leftBytes.length ^ rightBytes.length;

  for (let index = 0; index < maxLength; index += 1) {
    diff |= (leftBytes[index] ?? 0) ^ (rightBytes[index] ?? 0);
  }

  return diff === 0;
}

export function hasTrustedProductionAccessSignal(
  headers: Headers,
  options: ProductionAccessGateOptions = {},
): boolean {
  const trustedProxySecret =
    options.trustedProxySecret ?? process.env.ADMIN_TRUSTED_PROXY_SECRET;
  if (!trustedProxySecret) {
    return false;
  }

  const suppliedSecret = headers.get(TRUSTED_PROXY_SECRET_HEADER);
  if (!suppliedSecret) {
    return false;
  }

  return constantTimeEqual(suppliedSecret, trustedProxySecret);
}
