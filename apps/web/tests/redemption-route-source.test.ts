import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const catalogRouteSource = readFileSync(
  join(process.cwd(), "app/api/redemption/catalog/route.ts"),
  "utf8",
);
const entitlementsRouteSource = readFileSync(
  join(process.cwd(), "app/api/redemption/entitlements/route.ts"),
  "utf8",
);
const ordersRouteSource = readFileSync(
  join(process.cwd(), "app/api/redemption/orders/route.ts"),
  "utf8",
);
const orderRouteSource = readFileSync(
  join(process.cwd(), "app/api/redemption/orders/[orderId]/route.ts"),
  "utf8",
);
const cancelRouteSource = readFileSync(
  join(process.cwd(), "app/api/redemption/orders/[orderId]/cancel/route.ts"),
  "utf8",
);

test("catalog proxy keeps auth guard and returns configured catalog payload", () => {
  assert.match(catalogRouteSource, /FALLBACK_CATALOG/);
  assert.doesNotMatch(catalogRouteSource, /devBffFallbackEnabled/);
  assert.match(
    catalogRouteSource,
    /NextResponse\.json\(\{ error: "unauthorized" \}, \{ status: 401 \}\)/,
  );
});

test("entitlements proxy forwards viewer id with bearer authorization only", () => {
  assert.doesNotMatch(entitlementsRouteSource, /x-user-id/);
  assert.doesNotMatch(entitlementsRouteSource, /x-transition-caller/);
  assert.doesNotMatch(entitlementsRouteSource, /devBffFallbackEnabled/);
  assert.match(entitlementsRouteSource, /authorization:\s*`Bearer/);
  assert.match(entitlementsRouteSource, /\/v1\/redemption\/entitlements\//);
  assert.match(
    entitlementsRouteSource,
    /NextResponse\.json\(parsed, \{ status: response\.status \}\)/,
  );
});

test("orders proxy sends body to gateway endpoint and preserves status", () => {
  assert.doesNotMatch(ordersRouteSource, /x-user-id/);
  assert.doesNotMatch(ordersRouteSource, /x-transition-caller/);
  assert.doesNotMatch(ordersRouteSource, /devBffFallbackEnabled/);
  assert.match(ordersRouteSource, /authorization:\s*`Bearer/);
  assert.match(ordersRouteSource, /sku_id:\s*skuId/);
  assert.match(ordersRouteSource, /idempotency_key:/);
  assert.match(ordersRouteSource, /correlation_id:/);
  assert.match(ordersRouteSource, /\"\/v1\/redemption\/orders\"/);
  assert.match(ordersRouteSource, /responsePayload/);
  assert.match(ordersRouteSource, /\{ status: response\.status \}/);
});

test("orders detail proxy maps /orders/:orderId path and preserves payload", () => {
  assert.doesNotMatch(orderRouteSource, /x-user-id/);
  assert.doesNotMatch(orderRouteSource, /x-transition-caller/);
  assert.doesNotMatch(orderRouteSource, /devBffFallbackEnabled/);
  assert.match(orderRouteSource, /authorization:\s*`Bearer/);
  assert.match(
    orderRouteSource,
    /\/v1\/redemption\/orders\/\$\{encodeURIComponent\(orderIdValue\)\}/,
  );
  assert.match(
    orderRouteSource,
    /Response\.json\(payload, \{ status: response\.status \}\)/,
  );
});

test("order cancel proxy maps /orders/:orderId/cancel path and preserves status", () => {
  assert.doesNotMatch(cancelRouteSource, /x-user-id/);
  assert.doesNotMatch(cancelRouteSource, /x-transition-caller/);
  assert.doesNotMatch(cancelRouteSource, /devBffFallbackEnabled/);
  assert.match(cancelRouteSource, /authorization:\s*`Bearer/);
  assert.match(
    cancelRouteSource,
    /\`\/v1\/redemption\/orders\/\$\{encodeURIComponent\(trimmedOrderId\)\}\/cancel\`/,
  );
  assert.match(
    cancelRouteSource,
    /NextResponse\.json\(payload, \{ status: response\.status \}\)/,
  );
});
