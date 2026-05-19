/**
 * Maps a failed member-summary lookup response to a user-facing message.
 *
 * Implements SPEC-017 §6.3.4 ("Admin app 後端錯誤呈現"): the message is chosen
 * from the HTTP status code *and* the response body's error code, not the
 * status alone.
 */

const MEMBER_LOOKUP_NOT_FOUND_MESSAGE =
  "查無此會員，請確認輸入的會員 ID 是否正確。";
const MEMBER_LOOKUP_UNAVAILABLE_MESSAGE =
  "查詢服務暫時無法使用，請稍後再試。";

/**
 * Error codes that mean "member not found" on the member-summary endpoint.
 * The api-gateway emits `ERROR_CODE_MEMBER_NOT_FOUND` (SPEC-014 §6.1.1); the
 * admin BFF's own mock-fallback and affiliate-scope checks emit
 * `member_not_found`.
 */
const MEMBER_NOT_FOUND_CODES: ReadonlySet<string> = new Set([
  "ERROR_CODE_MEMBER_NOT_FOUND",
  "member_not_found",
]);

/**
 * @param status HTTP status of the failed `/api/members/{id}/summary` response.
 * @param code   Error code read from the response body, or `null` when the
 *               body carried no readable code.
 */
export function memberLookupErrorMessage(
  status: number,
  code: string | null,
): string {
  // A 404 on this endpoint always means not-found; trust a known code, and
  // fall back to the status when the body carried no readable code.
  if (status === 404 && (code === null || MEMBER_NOT_FOUND_CODES.has(code))) {
    return MEMBER_LOOKUP_NOT_FOUND_MESSAGE;
  }
  return MEMBER_LOOKUP_UNAVAILABLE_MESSAGE;
}
