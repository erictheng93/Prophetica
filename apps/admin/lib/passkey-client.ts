"use client";

type JsonRecord = Record<string, unknown>;

interface PasskeyStartResponse {
  challenge_id: string;
  creation_options?: unknown;
  request_options?: unknown;
}

export function browserSupportsPasskeys(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.PublicKeyCredential !== "undefined" &&
    typeof navigator.credentials?.create === "function" &&
    typeof navigator.credentials?.get === "function"
  );
}

export async function registerPasskey(): Promise<void> {
  const start = await postPasskey<PasskeyStartResponse>("/api/auth/passkeys/register-start");
  const publicKey = decodeCreationOptions(start.creation_options);
  const credential = await navigator.credentials.create({ publicKey });
  if (!credential) throw new Error("passkey_registration_cancelled");
  await postPasskey("/api/auth/passkeys/register-finish", {
    challenge_id: start.challenge_id,
    registration_data: serializeCredential(credential as PublicKeyCredential),
  });
}

export async function assertPasskey(): Promise<void> {
  const start = await postPasskey<PasskeyStartResponse>("/api/auth/passkeys/assert-start");
  const publicKey = decodeRequestOptions(start.request_options);
  const credential = await navigator.credentials.get({ publicKey });
  if (!credential) throw new Error("passkey_assertion_cancelled");
  await postPasskey("/api/auth/passkeys/assert-finish", {
    challenge_id: start.challenge_id,
    assertion_data: serializeCredential(credential as PublicKeyCredential),
  });
}

async function postPasskey<T = unknown>(url: string, body?: unknown): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    const error = data && typeof data === "object" && "error" in data ? String(data.error) : "passkey_failed";
    throw new Error(error);
  }
  return (await response.json()) as T;
}

function decodeCreationOptions(value: unknown): PublicKeyCredentialCreationOptions {
  const publicKey = extractPublicKeyOptions(value);
  publicKey.challenge = base64UrlToArrayBuffer(String(publicKey.challenge ?? ""));
  if (publicKey.user && typeof publicKey.user === "object") {
    const user = publicKey.user as JsonRecord;
    user.id = base64UrlToArrayBuffer(String(user.id ?? ""));
  }
  if (Array.isArray(publicKey.excludeCredentials)) {
    publicKey.excludeCredentials = publicKey.excludeCredentials.map(decodeCredentialDescriptor);
  }
  return publicKey as unknown as PublicKeyCredentialCreationOptions;
}

function decodeRequestOptions(value: unknown): PublicKeyCredentialRequestOptions {
  const publicKey = extractPublicKeyOptions(value);
  publicKey.challenge = base64UrlToArrayBuffer(String(publicKey.challenge ?? ""));
  if (Array.isArray(publicKey.allowCredentials)) {
    publicKey.allowCredentials = publicKey.allowCredentials.map(decodeCredentialDescriptor);
  }
  return publicKey as unknown as PublicKeyCredentialRequestOptions;
}

function decodeCredentialDescriptor(value: unknown): JsonRecord {
  const descriptor = structuredCloneRecord(value);
  descriptor.id = base64UrlToArrayBuffer(String(descriptor.id ?? ""));
  return descriptor;
}

function serializeCredential(credential: PublicKeyCredential): JsonRecord {
  const response = credential.response as AuthenticatorAttestationResponse | AuthenticatorAssertionResponse;
  const serializedResponse: JsonRecord = {
    clientDataJSON: arrayBufferToBase64Url(response.clientDataJSON),
  };

  if ("attestationObject" in response) {
    serializedResponse.attestationObject = arrayBufferToBase64Url(response.attestationObject);
    serializedResponse.transports = response.getTransports();
  }
  if ("authenticatorData" in response) {
    serializedResponse.authenticatorData = arrayBufferToBase64Url(response.authenticatorData);
    serializedResponse.signature = arrayBufferToBase64Url(response.signature);
    serializedResponse.userHandle = response.userHandle ? arrayBufferToBase64Url(response.userHandle) : null;
  }

  return {
    id: credential.id,
    rawId: arrayBufferToBase64Url(credential.rawId),
    type: credential.type,
    response: serializedResponse,
    clientExtensionResults: credential.getClientExtensionResults(),
    authenticatorAttachment: credential.authenticatorAttachment,
  };
}

function structuredCloneRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== "object") {
    throw new Error("invalid_passkey_options");
  }
  return JSON.parse(JSON.stringify(value)) as JsonRecord;
}

function extractPublicKeyOptions(value: unknown): JsonRecord {
  const options = structuredCloneRecord(value);
  const wrapped = options.publicKey ?? options.public_key;
  return wrapped && typeof wrapped === "object" ? structuredCloneRecord(wrapped) : options;
}

function base64UrlToArrayBuffer(value: string): ArrayBuffer {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized.padEnd(normalized.length + ((4 - (normalized.length % 4)) % 4), "=");
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes.buffer;
}

function arrayBufferToBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
