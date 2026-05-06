export type JsonI64 = number | string | bigint;

export function parseI64(value: JsonI64): bigint {
  if (typeof value === "bigint") {
    return value;
  }

  if (typeof value === "number") {
    if (!Number.isInteger(value)) {
      throw new TypeError(`expected integer i64, received ${value}`);
    }
    return BigInt(value);
  }

  if (!/^-?\d+$/.test(value)) {
    throw new TypeError(`expected integer i64 string, received ${value}`);
  }
  return BigInt(value);
}

export function i64ToJsonNumber(value: JsonI64): number {
  const parsed = parseI64(value);
  const numberValue = Number(parsed);
  if (!Number.isSafeInteger(numberValue)) {
    throw new RangeError(`i64 value ${parsed} cannot be sent as a JSON number safely`);
  }
  return numberValue;
}
