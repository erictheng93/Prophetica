"use client";

import { useCallback, useEffect, useState } from "react";

import {
  createRedemptionOrder,
  cancelRedemptionOrder,
  fetchMyEntitlements,
  fetchRedemptionCatalog,
  fetchRedemptionOrder,
  type RedemptionCatalogItem,
  type RedemptionEntitlement,
  type RedemptionOrder,
} from "@/lib/redemption-api";

function formatPoints(points: number): string {
  return `${points.toLocaleString("zh-Hant")} points`;
}

function formatState(state: RedemptionOrder["state"]): string {
  return state.replaceAll("_", " ").replace(/^./, (char) => char.toUpperCase());
}

function orderSummary(order: RedemptionOrder): string {
  return `${order.orderId.slice(0, 8)}•${order.skuId}•${formatState(order.state)}`;
}

const CANCELLABLE_ORDER_STATES: ReadonlySet<RedemptionOrder["state"]> = new Set(
  ["submitted", "accepted", "committed"],
);

function ErrorMessage({ message }: { message: string }) {
  return (
    <p
      className="mt-3 rounded-proph-md border border-proph-feedback-warning bg-proph-canvas px-4 py-3 font-text text-[14px] tracking-[-0.224px] text-proph-feedback-warning"
      role="status"
      aria-live="polite"
    >
      {message}
    </p>
  );
}

function EmptyState({ label }: { label: string }) {
  return (
    <p className="rounded-proph-md border border-proph-hairline bg-proph-canvas-parchment px-4 py-6 text-center font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
      {label}
    </p>
  );
}

export function RedemptionClient() {
  const [catalog, setCatalog] = useState<RedemptionCatalogItem[]>([]);
  const [entitlements, setEntitlements] = useState<RedemptionEntitlement[]>([]);
  const [selectedOrder, setSelectedOrder] = useState<RedemptionOrder | null>(
    null,
  );
  const [lookupOrderId, setLookupOrderId] = useState("");
  const [statusMessage, setStatusMessage] = useState<string>("");
  const [isCatalogLoading, setIsCatalogLoading] = useState(false);
  const [isEntitlementsLoading, setIsEntitlementsLoading] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState<string | null>(null);
  const [isLookingUp, setIsLookingUp] = useState(false);
  const [error, setError] = useState<string>("");

  const clearStatus = useCallback(() => {
    setStatusMessage("");
    setError("");
  }, []);

  const loadCatalog = useCallback(async () => {
    setIsCatalogLoading(true);
    clearStatus();
    try {
      const data = await fetchRedemptionCatalog();
      setCatalog(data);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to load catalog";
      setError(message);
    } finally {
      setIsCatalogLoading(false);
    }
  }, [clearStatus]);

  const loadEntitlements = useCallback(async () => {
    setIsEntitlementsLoading(true);
    try {
      const data = await fetchMyEntitlements();
      setEntitlements(data);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to load entitlements";
      setError(message);
    } finally {
      setIsEntitlementsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadCatalog();
    void loadEntitlements();
  }, [loadCatalog, loadEntitlements]);

  const submitOrder = useCallback(
    async (skuId: string) => {
      clearStatus();
      setIsSubmitting(skuId);
      try {
        const order = await createRedemptionOrder({
          skuId,
          idempotencyKey: crypto.randomUUID(),
        });
        setSelectedOrder(order);
        setStatusMessage(`Order created: ${orderSummary(order)}`);
        await loadEntitlements();
      } catch (err) {
        const message =
          err instanceof Error ? err.message : "Failed to create order";
        setError(message);
      } finally {
        setIsSubmitting(null);
      }
    },
    [clearStatus, loadEntitlements],
  );

  const lookupOrder = useCallback(async () => {
    const orderId = lookupOrderId.trim();
    if (!orderId) {
      return;
    }

    setIsLookingUp(true);
    clearStatus();
    try {
      const order = await fetchRedemptionOrder(orderId);
      setSelectedOrder(order);
      setStatusMessage(`Order found: ${orderSummary(order)}`);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Order lookup failed";
      setError(message);
    } finally {
      setIsLookingUp(false);
    }
  }, [clearStatus, lookupOrderId]);

  const cancelOrder = useCallback(async () => {
    if (!selectedOrder) {
      return;
    }

    clearStatus();
    setIsSubmitting(selectedOrder.orderId);
    try {
      const updated = await cancelRedemptionOrder(selectedOrder.orderId);
      setSelectedOrder(updated);
      setStatusMessage(`Order canceled: ${orderSummary(updated)}`);
      await loadEntitlements();
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Order cancel failed";
      setError(message);
    } finally {
      setIsSubmitting(null);
    }
  }, [clearStatus, loadEntitlements, selectedOrder]);

  const canCancelSelectedOrder = selectedOrder
    ? CANCELLABLE_ORDER_STATES.has(selectedOrder.state)
    : false;

  return (
    <div className="space-y-8">
      <section className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8">
        <header className="mb-4">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px]">
            Catalog
          </h2>
          <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            Buy digital items and services with points.
          </p>
        </header>

        {isCatalogLoading ? (
          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            Loading catalog...
          </p>
        ) : null}

        {catalog.length === 0 && !isCatalogLoading ? (
          <EmptyState label="No redeemable items are available right now." />
        ) : null}

        <ul className="mt-6 grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {catalog.map((item) => {
            const busy = isSubmitting === item.skuId;
            const disabled = busy || Boolean(isSubmitting);

            return (
              <li
                key={item.skuId}
                className="flex flex-col rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas-parchment p-6 shadow-sm transition-all duration-200 hover:-translate-y-1 hover:border-proph-primary/40 hover:shadow-proph-card"
              >
                <div className="font-display text-[21px] font-semibold leading-[1.26] tracking-[0.196px]">
                  {item.title}
                </div>
                <div className="mt-1 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {item.category} / {item.fulfillmentMode}
                </div>
                <p className="mt-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                  {item.description}
                </p>
                <p className="mt-4 font-mono text-[17px] font-semibold leading-[1.47] tracking-[-0.374px] text-proph-primary">
                  {formatPoints(item.pricePoints)}
                </p>
                <button
                  type="button"
                  disabled={disabled}
                  onClick={() => void submitOrder(item.skuId)}
                  className="mt-4 inline-flex h-10 min-w-[132px] items-center justify-center rounded-proph-pill bg-proph-primary px-5 font-text text-[14px] font-medium leading-[1.43] tracking-[-0.224px] text-proph-on-primary transition-transform active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {busy ? "Submitting" : "Create order"}
                </button>
              </li>
            );
          })}
        </ul>
      </section>

      <section className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8">
        <header className="mb-4">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px]">
            Order lookup and action
          </h2>
          <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            Query an order by ID and request cancel.
          </p>
        </header>

        <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
          <label htmlFor="orderId" className="flex-1">
            <span className="sr-only">order id</span>
            <input
              id="orderId"
              value={lookupOrderId}
              onChange={(event) => setLookupOrderId(event.target.value)}
              placeholder="Order ID"
              className="w-full rounded-proph-md border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink placeholder:text-proph-ink-muted-48"
            />
          </label>
          <button
            type="button"
            onClick={() => void lookupOrder()}
            disabled={isLookingUp || !lookupOrderId.trim()}
            className="inline-flex h-10 min-w-[132px] items-center justify-center rounded-proph-pill bg-proph-primary px-5 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-60"
          >
            {isLookingUp ? "Loading" : "Query order"}
          </button>
        </div>

        {selectedOrder ? (
          <article className="mt-6 rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas-parchment p-6 shadow-sm">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              Order details
            </div>
            <dl className="mt-3 grid gap-2 font-text text-[14px] tracking-[-0.224px]">
              <div className="grid gap-1">
                <dt className="text-proph-ink-muted-80">Order ID</dt>
                <dd className="font-mono">{selectedOrder.orderId}</dd>
              </div>
              <div className="grid gap-1">
                <dt className="text-proph-ink-muted-80">SKU</dt>
                <dd>{selectedOrder.skuId}</dd>
              </div>
              <div className="grid gap-1">
                <dt className="text-proph-ink-muted-80">State</dt>
                <dd>{formatState(selectedOrder.state)}</dd>
              </div>
              <div className="grid gap-1">
                <dt className="text-proph-ink-muted-80">Cancellation</dt>
                <dd>
                  {canCancelSelectedOrder
                    ? "Available"
                    : "Not available for current order state"}
                </dd>
              </div>
              <div className="grid gap-1">
                <dt className="text-proph-ink-muted-80">Price</dt>
                <dd>{formatPoints(selectedOrder.pricePoints)}</dd>
              </div>
              <div className="mt-2">
                <button
                  type="button"
                  onClick={() => void cancelOrder()}
                  disabled={isSubmitting !== null || !canCancelSelectedOrder}
                  className="inline-flex h-10 min-w-[132px] items-center justify-center rounded-proph-pill bg-proph-primary/90 px-5 text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-on-primary disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {isSubmitting === selectedOrder.orderId
                    ? "Canceling"
                    : "Cancel order"}
                </button>
              </div>
            </dl>
          </article>
        ) : null}
      </section>

      <section className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card md:p-8">
        <header className="mb-4">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px]">
            My entitlements
          </h2>
          <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            Items you own now (and their fulfillment states).
          </p>
        </header>

        {isEntitlementsLoading ? (
          <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            Loading entitlements...
          </p>
        ) : null}

        {entitlements.length === 0 && !isEntitlementsLoading ? (
          <EmptyState label="No entitlements yet." />
        ) : null}

        <ul className="grid grid-cols-1 gap-3">
          {entitlements.map((entitlement) => (
            <li
              key={entitlement.entitlementId}
              className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas-parchment p-5 transition-all hover:border-proph-primary/40"
            >
              <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {entitlement.state}
              </div>
              <div className="mt-1 font-display text-[17px] leading-[1.15] tracking-[0]">
                {entitlement.skuId}
              </div>
              <div className="mt-1 font-mono text-[12px] tracking-[-0.224px] text-proph-ink-muted-80">
                {entitlement.entitlementId}
              </div>
              <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                order {entitlement.orderId}
              </div>
            </li>
          ))}
        </ul>
      </section>

      {statusMessage ? (
        <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
          {statusMessage}
        </p>
      ) : null}
      {error ? <ErrorMessage message={error} /> : null}
    </div>
  );
}
