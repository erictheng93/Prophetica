import {
  ApiError,
  createEngineClient,
  type PlaceEngineOrderRequest,
} from "@prophetica/ts-sdk";
import { NextResponse } from "next/server";
import { getViewer } from "@/lib/session";

const DEFAULT_MATCHING_ENGINE_BASE_URL = "http://127.0.0.1:8088";
const DEFAULT_ENGINE_USER_ID = "10";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

interface RouteContext {
  params: Promise<{ marketId: string }>;
}

interface PlaceOrderPayload {
  side: PlaceEngineOrderRequest["side"];
  outcome: PlaceEngineOrderRequest["outcome"];
  type: PlaceEngineOrderRequest["type"];
  price_pts: number;
  quantity: number;
  time_in_force: PlaceEngineOrderRequest["timeInForce"];
}

export async function POST(
  request: Request,
  { params }: RouteContext,
): Promise<NextResponse> {
  const { marketId } = await params;
  const payload = await readPayload(request);
  if (!payload) {
    return NextResponse.json(
      { error: "request body must be valid JSON" },
      { status: 400 },
    );
  }

  const viewer = await getViewer();
  const userId =
    viewer?.userId ??
    (process.env.NODE_ENV === "production"
      ? null
      : (process.env.PROPHETICA_ENGINE_DEV_USER_ID ?? DEFAULT_ENGINE_USER_ID));
  if (!userId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const client = createEngineClient({
    baseUrl:
      process.env.MATCHING_ENGINE_BASE_URL ?? DEFAULT_MATCHING_ENGINE_BASE_URL,
    auth: { userId },
    fetcher: fetch,
  });

  try {
    const order = await client.placeOrder({
      marketId,
      side: payload.side,
      outcome: payload.outcome,
      type: payload.type,
      pricePts: payload.price_pts,
      quantity: payload.quantity,
      timeInForce: payload.time_in_force,
      clientOrderId: request.headers.get("x-client-order-id") ?? undefined,
    });

    return NextResponse.json({
      order_id: order.orderId,
      status: order.status,
      filled_qty: order.filledQty,
      avg_price_pts: order.avgPricePts,
      remaining_qty: order.remainingQty,
      fee_pts: order.feePts,
    });
  } catch (error) {
    if (error instanceof ApiError) {
      return NextResponse.json(
        { error: error.message },
        { status: error.status },
      );
    }
    return NextResponse.json(
      { error: "matching-engine unavailable" },
      { status: 503 },
    );
  }
}

async function readPayload(
  request: Request,
): Promise<PlaceOrderPayload | null> {
  try {
    return (await request.json()) as PlaceOrderPayload;
  } catch {
    return null;
  }
}
