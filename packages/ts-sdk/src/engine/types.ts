export type EngineOrderSide = "buy" | "sell";
export type EngineOutcome = "YES" | "NO";
export type EngineOrderType = "LIMIT" | "IOC" | "FOK" | "POST_ONLY";
export type EngineTimeInForce = "GTC" | "IOC" | "FOK";
export type EngineOrderStatus =
  | "ACTIVE"
  | "FILLED"
  | "PARTIAL"
  | "CANCELLED"
  | "PENDING"
  | "UNSPECIFIED";

export interface PlaceEngineOrderRequest {
  marketId: string;
  side: EngineOrderSide;
  outcome: EngineOutcome;
  type: EngineOrderType;
  pricePts: number;
  quantity: number;
  timeInForce: EngineTimeInForce;
  clientOrderId?: string;
}

export interface PlaceEngineOrderResponse {
  orderId: string;
  status: EngineOrderStatus;
  filledQty: number;
  avgPricePts: number;
  remainingQty: number;
  feePts: number;
}
