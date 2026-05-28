const DEFAULT_REDEMPTION_WEB_BASE_URL = "/api/redemption";

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface ApiErrorPayload {
  error: string;
  message?: string;
}

export interface RedemptionOrder {
  orderId: string;
  state: RedemptionOrderState;
  userId: string;
  skuId: string;
  priceMinorUnits: number;
  pricePoints: number;
  entitlementId?: string;
  idempotencyKey: string;
  replayed: boolean;
}

export interface RedemptionEntitlement {
  entitlementId: string;
  orderId: string;
  ownerUserId: string;
  skuId: string;
  state: RedemptionEntitlementState;
  fulfillmentPayload?: Record<string, unknown> | null;
}

export interface RedemptionCatalogItem {
  skuId: string;
  title: string;
  category: string;
  description: string;
  priceMinorUnits: number;
  pricePoints: number;
  fulfillmentMode: string;
}

export interface RedemptionCatalogResponse {
  items: RedemptionCatalogItem[];
}

export interface CreateRedemptionOrderInput {
  skuId: string;
  idempotencyKey?: string;
  correlationId?: string;
}

export interface RedemptionApiOptions {
  baseUrl?: string;
  fetcher?: Fetcher;
}

export type RedemptionOrderState =
  | "submitted"
  | "accepted"
  | "committed"
  | "fulfilled"
  | "refunded"
  | "expired";

export type RedemptionEntitlementState =
  | "issued"
  | "fulfilled"
  | "consumed"
  | "refunded"
  | "expired";

export class RedemptionApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "RedemptionApiError";
    this.status = status;
    this.code = code;
  }
}

interface ApiRedemptionOrder {
  order_id: string;
  state: RedemptionOrderState;
  user_id: string;
  sku_id: string;
  price_minor_units: number;
  entitlement_id?: string | null;
  idempotency_key: string;
  replayed: boolean;
}

interface ApiRedemptionEntitlement {
  entitlement_id: string;
  order_id: string;
  owner_user_id: string;
  sku_id: string;
  state: RedemptionEntitlementState;
  fulfillment_payload?: Record<string, unknown> | null;
}

interface ApiRedemptionCatalogResponse {
  catalog: Array<{
    sku_id: string;
    title: string;
    category: string;
    description: string;
    price_minor_units: number;
    fulfillment_mode: string;
  }>;
}

export async function fetchRedemptionCatalog({
  baseUrl = DEFAULT_REDEMPTION_WEB_BASE_URL,
  fetcher = fetch,
}: RedemptionApiOptions = {}): Promise<RedemptionCatalogItem[]> {
  const response = await fetcher(apiUrl(baseUrl, "/catalog"), {
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw await makeRedemptionError(response);
  }

  const body = (await response.json()) as ApiRedemptionCatalogResponse;
  return body.catalog.map((item) => mapCatalogItem(item));
}

export async function fetchMyEntitlements({
  baseUrl = DEFAULT_REDEMPTION_WEB_BASE_URL,
  fetcher = fetch,
}: RedemptionApiOptions = {}): Promise<RedemptionEntitlement[]> {
  const response = await fetcher(apiUrl(baseUrl, "/entitlements"), {
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw await makeRedemptionError(response);
  }

  const body = (await response.json()) as { entitlements: ApiRedemptionEntitlement[] };
  return body.entitlements.map(mapEntitlement);
}

export async function createRedemptionOrder(
  input: CreateRedemptionOrderInput,
  {
    baseUrl = DEFAULT_REDEMPTION_WEB_BASE_URL,
    fetcher = fetch,
  }: RedemptionApiOptions = {},
): Promise<RedemptionOrder> {
  const response = await fetcher(apiUrl(baseUrl, "/orders"), {
    method: "POST",
    cache: "no-store",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      sku_id: input.skuId,
      idempotency_key: input.idempotencyKey ?? crypto.randomUUID(),
      correlation_id: input.correlationId,
    }),
  });
  if (!response.ok) {
    throw await makeRedemptionError(response);
  }

  const payload = (await response.json()) as ApiRedemptionOrder;
  return mapOrder(payload);
}

export async function fetchRedemptionOrder(
  orderId: string,
  { baseUrl = DEFAULT_REDEMPTION_WEB_BASE_URL, fetcher = fetch }: RedemptionApiOptions = {},
): Promise<RedemptionOrder> {
  const response = await fetcher(apiUrl(baseUrl, `/orders/${encodeURIComponent(orderId)}`), {
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw await makeRedemptionError(response);
  }

  const payload = (await response.json()) as ApiRedemptionOrder;
  return mapOrder(payload);
}

export async function cancelRedemptionOrder(
  orderId: string,
  { baseUrl = DEFAULT_REDEMPTION_WEB_BASE_URL, fetcher = fetch }: RedemptionApiOptions = {},
): Promise<RedemptionOrder> {
  const response = await fetcher(apiUrl(baseUrl, `/orders/${encodeURIComponent(orderId)}/cancel`), {
    method: "POST",
    cache: "no-store",
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw await makeRedemptionError(response);
  }

  const payload = (await response.json()) as ApiRedemptionOrder;
  return mapOrder(payload);
}

function mapOrder(payload: ApiRedemptionOrder): RedemptionOrder {
  return {
    orderId: payload.order_id,
    state: payload.state,
    userId: payload.user_id,
    skuId: payload.sku_id,
    priceMinorUnits: payload.price_minor_units,
    pricePoints: payload.price_minor_units / 100,
    entitlementId: payload.entitlement_id ?? undefined,
    idempotencyKey: payload.idempotency_key,
    replayed: payload.replayed,
  };
}

function mapEntitlement(payload: ApiRedemptionEntitlement): RedemptionEntitlement {
  return {
    entitlementId: payload.entitlement_id,
    orderId: payload.order_id,
    ownerUserId: payload.owner_user_id,
    skuId: payload.sku_id,
    state: payload.state,
    fulfillmentPayload: payload.fulfillment_payload,
  };
}

function mapCatalogItem(item: ApiRedemptionCatalogResponse["catalog"][number]): RedemptionCatalogItem {
  return {
    skuId: item.sku_id,
    title: item.title,
    category: item.category,
    description: item.description,
    priceMinorUnits: item.price_minor_units,
    pricePoints: item.price_minor_units / 100,
    fulfillmentMode: item.fulfillment_mode,
  };
}

function apiUrl(baseUrl: string, path: string): string {
  const trimmed = trimTrailingSlash(baseUrl);
  return `${trimmed}${path}`;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

async function makeRedemptionError(response: Response): Promise<RedemptionApiError> {
  const body = await response.json().catch(async () => {
    const text = await response.text();
    return { error: String(response.status), message: text };
  });
  const payload = body as ApiErrorPayload;
  const errorCode = typeof payload.error === "string" ? payload.error : "redemption_request_failed";
  const message =
    typeof payload.message === "string"
      ? payload.message
      : `Redemption request failed: ${response.status}`;

  return new RedemptionApiError(response.status, errorCode, message);
}
