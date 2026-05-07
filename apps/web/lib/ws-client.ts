/*
 * Reconnecting WebSocket client — SPEC-008 §7.1 / §7.3 contract.
 *
 * Contract (must not be relaxed without PR-modifying SPEC-008):
 *   - Exponential backoff with full jitter, base 500ms, factor 2.0,
 *     jitter ±20%, cap 30s, max 30 attempts.
 *   - Heartbeat: client → server `ping` every 15s; 30s no pong → close + reconnect.
 *   - Reconcile: incoming payload carries monotonic `seq`; gap → emit reconcile signal
 *     so the caller can pull a snapshot (SPEC-008 §7.3 step 2).
 *   - Fail-soft: when max attempts exhausted, transition to `failed` state so
 *     UI can show ConnectionStatusBanner + degrade to polling. Client does NOT
 *     auto-resume; caller must explicitly invoke restart() (manual retry button).
 *
 * Anti-patterns explicitly prohibited (SPEC-008 §7.3 + SPEC-007 §6.4.4):
 *   - infinite retry / no upper bound
 *   - resetting attempt counter on transient success without seq progress
 *   - retrying on 4xx-equivalent close codes (1002, 1003, 1007, 1008, 1011)
 */

export interface WsContract {
  baseDelayMs: number;
  factor: number;
  jitter: number;
  capMs: number;
  maxAttempts: number;
  heartbeatIntervalMs: number;
  heartbeatTimeoutMs: number;
  reconcileTailMs: number;
}

export const WS_CONTRACT: WsContract = {
  baseDelayMs: 500,
  factor: 2.0,
  jitter: 0.2,
  capMs: 30_000,
  maxAttempts: 30,
  heartbeatIntervalMs: 15_000,
  heartbeatTimeoutMs: 30_000,
  reconcileTailMs: 60_000,
};

export const NON_RETRIABLE_CLOSE_CODES = new Set([1002, 1003, 1007, 1008, 1011]);

export type WsState = "idle" | "connecting" | "open" | "reconnecting" | "failed";

export interface WsMessage<T = unknown> {
  seq: number;
  payload: T;
}

export interface ReconcileSignal {
  type: "reconcile";
  fromSeq: number;
  toSeq: number;
}

export type WsEvent<T> =
  | { type: "state"; state: WsState; attempt: number }
  | { type: "message"; message: WsMessage<T> }
  | ReconcileSignal
  | { type: "error"; error: Error };

type SocketFactory = (url: string) => WebSocketLike;

export interface WebSocketLike {
  readonly readyState: number;
  send(data: string): void;
  close(code?: number, reason?: string): void;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: string }) => void) | null;
  onclose: ((ev: { code: number; reason: string }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
}

export interface ReconnectingWebSocketOptions {
  url: string;
  socketFactory?: SocketFactory;
  contract?: Partial<WsContract>;
  randomFn?: () => number;
  setTimeoutFn?: (fn: () => void, ms: number) => unknown;
  clearTimeoutFn?: (handle: unknown) => void;
  nowFn?: () => number;
  onEvent: (event: WsEvent<unknown>) => void;
}

export class ReconnectingWebSocket {
  private readonly contract: WsContract;
  private readonly socketFactory: SocketFactory;
  private readonly randomFn: () => number;
  private readonly setTimeoutFn: (fn: () => void, ms: number) => unknown;
  private readonly clearTimeoutFn: (handle: unknown) => void;
  private readonly nowFn: () => number;
  private readonly onEvent: (event: WsEvent<unknown>) => void;
  private readonly url: string;

  private socket: WebSocketLike | null = null;
  private state: WsState = "idle";
  private attempt = 0;
  private lastSeq: number | null = null;
  private reconnectTimer: unknown = null;
  private heartbeatTimer: unknown = null;
  private heartbeatDeadline: unknown = null;
  private stopped = true;

  constructor(options: ReconnectingWebSocketOptions) {
    this.contract = { ...WS_CONTRACT, ...options.contract };
    this.socketFactory =
      options.socketFactory ?? ((url) => new WebSocket(url) as unknown as WebSocketLike);
    this.randomFn = options.randomFn ?? Math.random;
    this.setTimeoutFn = options.setTimeoutFn ?? ((fn, ms) => setTimeout(fn, ms));
    this.clearTimeoutFn =
      options.clearTimeoutFn ?? ((h) => clearTimeout(h as ReturnType<typeof setTimeout>));
    this.nowFn = options.nowFn ?? Date.now;
    this.onEvent = options.onEvent;
    this.url = options.url;
  }

  start(): void {
    if (!this.stopped) return;
    this.stopped = false;
    this.attempt = 0;
    this.openSocket();
  }

  stop(): void {
    this.stopped = true;
    this.clearTimers();
    if (this.socket) {
      try {
        this.socket.close(1000, "client_stop");
      } catch {
        /* socket already closed */
      }
      this.socket = null;
    }
    this.transitionTo("idle");
  }

  /**
   * Manually re-arm after the client has entered `failed` state — e.g. user
   * tapped the "重新嘗試連線" button on ConnectionStatusBanner. Resets the
   * attempt counter; the SPEC-008 §7.3 retry budget contract applies fresh.
   */
  restart(): void {
    this.stopped = true;
    this.clearTimers();
    this.socket = null;
    this.start();
  }

  /**
   * Last sequence number observed; nullable until first inbound message lands.
   * Exposed for callers that want to drive reconcile pulls themselves.
   */
  getLastSeq(): number | null {
    return this.lastSeq;
  }

  private openSocket(): void {
    if (this.stopped) return;
    this.transitionTo(this.attempt === 0 ? "connecting" : "reconnecting");
    let socket: WebSocketLike;
    try {
      socket = this.socketFactory(this.url);
    } catch (error) {
      this.onEvent({ type: "error", error: toError(error) });
      this.scheduleReconnect();
      return;
    }
    this.socket = socket;
    socket.onopen = () => this.handleOpen();
    socket.onmessage = (ev) => this.handleMessage(ev);
    socket.onclose = (ev) => this.handleClose(ev.code, ev.reason);
    socket.onerror = (ev) => this.onEvent({ type: "error", error: toError(ev) });
  }

  private handleOpen(): void {
    // Do NOT reset attempt counter here. Resetting on every open() lets a
    // misbehaving proxy or server churn open→close in a tight loop and bypass
    // the SPEC-008 §7.3 retry budget. We only reset after we observe a
    // verified inbound payload (proof of bidirectional liveness).
    this.transitionTo("open");
    this.armHeartbeat();
  }

  private handleMessage(ev: { data: string }): void {
    this.armHeartbeat();
    let parsed: unknown;
    try {
      parsed = JSON.parse(ev.data);
    } catch (error) {
      this.onEvent({ type: "error", error: toError(error) });
      return;
    }

    if (isPong(parsed)) {
      this.attempt = 0;
      return;
    }

    if (!isWsMessage(parsed)) {
      this.onEvent({
        type: "error",
        error: new Error("ws message missing seq field"),
      });
      return;
    }

    if (this.lastSeq !== null && parsed.seq > this.lastSeq + 1) {
      this.onEvent({
        type: "reconcile",
        fromSeq: this.lastSeq + 1,
        toSeq: parsed.seq - 1,
      });
    }

    if (this.lastSeq === null || parsed.seq > this.lastSeq) {
      this.lastSeq = parsed.seq;
    }
    this.attempt = 0;
    this.onEvent({ type: "message", message: parsed });
  }

  private handleClose(code: number, reason: string): void {
    this.clearTimers();
    if (this.stopped) return;

    if (NON_RETRIABLE_CLOSE_CODES.has(code)) {
      this.onEvent({
        type: "error",
        error: new Error(`ws closed with non-retriable code ${code}: ${reason}`),
      });
      this.stopped = true;
      this.transitionTo("failed");
      return;
    }

    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.attempt >= this.contract.maxAttempts) {
      this.stopped = true;
      this.transitionTo("failed");
      return;
    }
    const delay = this.computeDelay(this.attempt);
    this.attempt += 1;
    this.transitionTo("reconnecting");
    this.reconnectTimer = this.setTimeoutFn(() => {
      this.reconnectTimer = null;
      this.openSocket();
    }, delay);
  }

  private computeDelay(attempt: number): number {
    const exp = this.contract.baseDelayMs * Math.pow(this.contract.factor, attempt);
    const capped = Math.min(exp, this.contract.capMs);
    const jitterRange = capped * this.contract.jitter;
    const offset = (this.randomFn() * 2 - 1) * jitterRange;
    return Math.max(0, Math.round(capped + offset));
  }

  private armHeartbeat(): void {
    this.clearHeartbeat();
    this.heartbeatTimer = this.setTimeoutFn(() => {
      this.sendPing();
    }, this.contract.heartbeatIntervalMs);
  }

  private sendPing(): void {
    if (!this.socket) return;
    try {
      this.socket.send(JSON.stringify({ type: "ping", ts: this.nowFn() }));
    } catch (error) {
      this.onEvent({ type: "error", error: toError(error) });
      return;
    }
    this.heartbeatDeadline = this.setTimeoutFn(() => {
      this.heartbeatDeadline = null;
      try {
        this.socket?.close(4000, "heartbeat_timeout");
      } catch {
        /* socket already closed */
      }
    }, this.contract.heartbeatTimeoutMs);
  }

  private clearHeartbeat(): void {
    if (this.heartbeatTimer) {
      this.clearTimeoutFn(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    if (this.heartbeatDeadline) {
      this.clearTimeoutFn(this.heartbeatDeadline);
      this.heartbeatDeadline = null;
    }
  }

  private clearTimers(): void {
    if (this.reconnectTimer) {
      this.clearTimeoutFn(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.clearHeartbeat();
  }

  private transitionTo(state: WsState): void {
    if (this.state === state) return;
    this.state = state;
    this.onEvent({ type: "state", state, attempt: this.attempt });
  }
}

function isPong(value: unknown): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    (value as { type?: unknown }).type === "pong"
  );
}

function isWsMessage(value: unknown): value is WsMessage {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { seq?: unknown }).seq === "number" &&
    Number.isInteger((value as { seq: number }).seq)
  );
}

function toError(value: unknown): Error {
  if (value instanceof Error) return value;
  if (typeof value === "string") return new Error(value);
  return new Error("unknown ws error");
}
