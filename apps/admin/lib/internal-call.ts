import { createHash, createHmac } from "node:crypto";

export interface InternalCallInput {
  method: string;
  path: string;
  body: string;
  actorId: string;
  role: string;
  secret?: string;
  nowSeconds?: number;
}

export interface InternalCallHeaders {
  "x-actor-id": string;
  "x-operator-role": string;
  "x-internal-call-timestamp": string;
  "x-internal-call-signature": string;
}

export function signInternalCall(input: InternalCallInput): InternalCallHeaders {
  const secret = input.secret ?? process.env.ADMIN_INTERNAL_CALL_SECRET;
  if (!secret) {
    throw new Error("ADMIN_INTERNAL_CALL_SECRET is not configured");
  }

  const timestamp = input.nowSeconds ?? Math.floor(Date.now() / 1000);
  const bodyHash = createHash("sha256").update(input.body).digest("hex");
  const canonical = [input.method, input.path, bodyHash, timestamp, input.actorId, input.role].join("\n");
  const signature = createHmac("sha256", secret).update(canonical).digest("hex");

  return {
    "x-actor-id": input.actorId,
    "x-operator-role": input.role,
    "x-internal-call-timestamp": String(timestamp),
    "x-internal-call-signature": signature,
  };
}
