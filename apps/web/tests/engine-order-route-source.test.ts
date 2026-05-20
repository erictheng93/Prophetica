import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const routeSource = readFileSync(
  join(process.cwd(), "app/api/engine/markets/[marketId]/orders/route.ts"),
  "utf8",
);

test("order proxy forwards browser trades to api-gateway with server bearer token", () => {
  assert.match(routeSource, /API_GATEWAY_BASE_URL/);
  assert.match(routeSource, /getUpstreamSessionToken/);
  assert.match(routeSource, /authorization", `Bearer \$\{bearerToken\}`/);
  assert.match(routeSource, /process\.env\.NODE_ENV !== "production"/);
  assert.match(
    routeSource,
    /\/v1\/markets\/\$\{encodeURIComponent\(marketId\)\}\/orders/,
  );
  assert.doesNotMatch(routeSource, /createEngineClient/);
  assert.doesNotMatch(routeSource, /MATCHING_ENGINE_BASE_URL/);
});

test("order proxy preserves idempotency and gateway rejection payloads", () => {
  assert.match(routeSource, /request\.headers\.get\("idempotency-key"\)/);
  assert.match(
    routeSource,
    /headers\.set\("idempotency-key", idempotencyKey\)/,
  );
  assert.match(
    routeSource,
    /NextResponse\.json\(responsePayload, \{ status: response\.status \}\)/,
  );
});
