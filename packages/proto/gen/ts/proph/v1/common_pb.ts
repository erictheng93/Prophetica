export type OrderID = {
  value: Uint8Array;
};

export function isUlidBytes(orderId: OrderID): boolean {
  return orderId.value.byteLength === 16;
}
