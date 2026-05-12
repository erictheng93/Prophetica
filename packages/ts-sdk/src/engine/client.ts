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
  };
}
