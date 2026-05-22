import { HttpClient, type ClientOptions } from "../runtime/fetch.js";
import { generateUuidV7 } from "../runtime/idempotency.js";
import type {
  PlaceEngineOrderRequest,
  PlaceEngineOrderResponse,
} from "./types.js";

interface ApiPlaceOrderResponse {
  order_id: string;
  status: PlaceEngineOrderResponse["status"];
  filled_qty: number;
  avg_price_pts: number;
  remaining_qty: number;
  fee_pts: number;
  fills?: ApiPlaceOrderFill[];
}

interface ApiPlaceOrderFill {
  fill_id: string;
  market_id: string;
  seq_no: number;
  price_pts: number;
  quantity: number;
  taker_order_id: string;
  maker_order_id: string;
  taker_user_id: number;
  maker_user_id: number;
  taker_side: "buy" | "sell";
  maker_side: "buy" | "sell";
  outcome: "YES" | "NO";
}

export interface EngineClient {
  placeOrder(
    request: PlaceEngineOrderRequest,
  ): Promise<PlaceEngineOrderResponse>;
}

export function createEngineClient(options: ClientOptions): EngineClient {
  const http = new HttpClient(options);

  return {
    async placeOrder(
      request: PlaceEngineOrderRequest,
    ): Promise<PlaceEngineOrderResponse> {
      const clientOrderId = request.clientOrderId ?? generateUuidV7();
      const response = await http.json<ApiPlaceOrderResponse>({
        method: "POST",
        path: `/api/v1/markets/${encodeURIComponent(request.marketId)}/orders`,
        headers: { "x-client-order-id": clientOrderId },
        body: {
          side: request.side,
          outcome: request.outcome,
          type: request.type,
          price_pts: request.pricePts,
          quantity: request.quantity,
          time_in_force: request.timeInForce,
        },
      });

      return mapPlaceOrderResponse(response);
    },
  };
}

function mapPlaceOrderResponse(
  response: ApiPlaceOrderResponse,
): PlaceEngineOrderResponse {
  return {
    orderId: response.order_id,
    status: response.status,
    filledQty: response.filled_qty,
    avgPricePts: response.avg_price_pts,
    remainingQty: response.remaining_qty,
    feePts: response.fee_pts,
    fills: (response.fills ?? []).map((fill) => ({
      fillId: fill.fill_id,
      marketId: fill.market_id,
      seqNo: fill.seq_no,
      pricePts: fill.price_pts,
      quantity: fill.quantity,
      takerOrderId: fill.taker_order_id,
      makerOrderId: fill.maker_order_id,
      takerUserId: fill.taker_user_id,
      makerUserId: fill.maker_user_id,
      takerSide: fill.taker_side,
      makerSide: fill.maker_side,
      outcome: fill.outcome,
    })),
  };
}
