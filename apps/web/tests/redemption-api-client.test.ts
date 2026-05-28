import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createRedemptionOrder,
  cancelRedemptionOrder,
  fetchMyEntitlements,
  fetchRedemptionCatalog,
  fetchRedemptionOrder,
  type RedemptionOrder,
  type RedemptionEntitlement,
  type RedemptionCatalogItem,
} from "../lib/redemption-api.js";

const apiBase = "https://internal.local/api/redemption";

test("fetchRedemptionCatalog maps catalog payload from gateway facade", async () => {
  const calls: string[] = [];
  const apiCatalogItem: RedemptionCatalogItem = {
    skuId: "sku_test",
    title: "Test item",
    category: "service",
    description: "Test redemption item",
    priceMinorUnits: 12300,
    pricePoints: 123,
    fulfillmentMode: "digital_immediate",
  };
  const fetcher = async (input: string | URL) => {
    calls.push(String(input));
    return Response.json({
      catalog: [
        {
          sku_id: apiCatalogItem.skuId,
          title: apiCatalogItem.title,
          category: apiCatalogItem.category,
          description: apiCatalogItem.description,
          price_minor_units: apiCatalogItem.priceMinorUnits,
          fulfillment_mode: apiCatalogItem.fulfillmentMode,
        },
      ],
    });
  };

  const catalog = await fetchRedemptionCatalog({ baseUrl: apiBase, fetcher });
  assert.equal(calls[0], "https://internal.local/api/redemption/catalog");
  assert.equal(catalog.length, 1);
  assert.deepEqual(catalog[0], apiCatalogItem);
});

test("fetchMyEntitlements maps entitlement list to local shape", async () => {
  const calls: string[] = [];
  const entitlementItem: RedemptionEntitlement = {
    entitlementId: "ent-01",
    orderId: "order-01",
    ownerUserId: "user_01",
    skuId: "sku_01",
    state: "issued",
    fulfillmentPayload: { code: "AA-01" },
  };

  const fetcher = async (input: string | URL) => {
    calls.push(String(input));
    return Response.json({
      entitlements: [
        {
          entitlement_id: entitlementItem.entitlementId,
          order_id: entitlementItem.orderId,
          owner_user_id: entitlementItem.ownerUserId,
          sku_id: entitlementItem.skuId,
          state: entitlementItem.state,
          fulfillment_payload: entitlementItem.fulfillmentPayload,
        },
      ],
    });
  };

  const entitlements = await fetchMyEntitlements({ baseUrl: apiBase, fetcher });
  assert.equal(calls[0], "https://internal.local/api/redemption/entitlements");
  assert.deepEqual(entitlements, [entitlementItem]);
});

test("createRedemptionOrder posts mapped snake_case payload", async () => {
  const calls: string[] = [];
  let bodyText = "";
  const expected: RedemptionOrder = {
    orderId: "order-01",
    state: "submitted",
    userId: "user_01",
    skuId: "sku-01",
    priceMinorUnits: 5000,
    pricePoints: 50,
    entitlementId: "ent-01",
    idempotencyKey: "idempotency-test",
    replayed: false,
  };

  const fetcher = async (input: string | URL, init?: RequestInit) => {
    calls.push(String(input));
    bodyText = String(init?.body ?? "");
    return Response.json({
      order_id: expected.orderId,
      state: expected.state,
      user_id: expected.userId,
      sku_id: expected.skuId,
      price_minor_units: expected.priceMinorUnits,
      entitlement_id: expected.entitlementId,
      idempotency_key: expected.idempotencyKey,
      replayed: expected.replayed,
    });
  };

  const order = await createRedemptionOrder(
    {
      skuId: "sku-01",
      idempotencyKey: "idempotency-test",
      correlationId: "corr-01",
    },
    { baseUrl: apiBase, fetcher },
  );
  const body = JSON.parse(bodyText);

  assert.equal(calls[0], "https://internal.local/api/redemption/orders");
  assert.equal(body.sku_id, "sku-01");
  assert.equal(body.idempotency_key, "idempotency-test");
  assert.equal(body.correlation_id, "corr-01");
  assert.deepEqual(order, expected);
});

test("fetchRedemptionOrder GETs order endpoint by id", async () => {
  const calls: string[] = [];
  const expected: RedemptionOrder = {
    orderId: "order-01",
    state: "submitted",
    userId: "user_01",
    skuId: "sku-01",
    priceMinorUnits: 5000,
    pricePoints: 50,
    idempotencyKey: "idem-01",
    replayed: false,
  };

  const fetcher = async (input: string | URL) => {
    calls.push(String(input));
    return Response.json({
      order_id: expected.orderId,
      state: expected.state,
      user_id: expected.userId,
      sku_id: expected.skuId,
      price_minor_units: expected.priceMinorUnits,
      idempotency_key: expected.idempotencyKey,
      replayed: expected.replayed,
    });
  };

  const order = await fetchRedemptionOrder("order-01", { baseUrl: apiBase, fetcher });

  assert.equal(calls[0], "https://internal.local/api/redemption/orders/order-01");
  assert.equal(order.orderId, expected.orderId);
  assert.equal(order.pricePoints, 50);
});

test("cancelRedemptionOrder POSTs cancel endpoint", async () => {
  const calls: string[] = [];
  const expected: RedemptionOrder = {
    orderId: "order-01",
    state: "refunded",
    userId: "user-01",
    skuId: "sku-01",
    priceMinorUnits: 5000,
    pricePoints: 50,
    idempotencyKey: "idem-01",
    replayed: false,
  };

  const fetcher = async (input: string | URL, init?: RequestInit) => {
    calls.push(String(input));
    return Response.json({
      order_id: expected.orderId,
      state: expected.state,
      user_id: expected.userId,
      sku_id: expected.skuId,
      price_minor_units: expected.priceMinorUnits,
      idempotency_key: expected.idempotencyKey,
      replayed: expected.replayed,
    });
  };

  const order = await cancelRedemptionOrder("order-01", { baseUrl: apiBase, fetcher });

  assert.equal(calls[0], "https://internal.local/api/redemption/orders/order-01/cancel");
  assert.equal(order.state, "refunded");
});
