import assert from "node:assert/strict";
import { test } from "node:test";

import {
  NON_RETRIABLE_CLOSE_CODES,
  ReconnectingWebSocket,
  WS_CONTRACT,
  type WebSocketLike,
  type WsEvent,
} from "../lib/ws-client.js";

class FakeSocket implements WebSocketLike {
  readyState = 0;
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  sent: string[] = [];

  send(data: string): void {
    this.sent.push(data);
  }

  close(code = 1006, reason = ""): void {
    this.readyState = 3;
    this.onclose?.({ code, reason });
  }

  open(): void {
    this.readyState = 1;
    this.onopen?.({});
  }

  deliver(payload: unknown): void {
    this.onmessage?.({ data: JSON.stringify(payload) });
  }
}

interface Harness {
  sockets: FakeSocket[];
  events: WsEvent<unknown>[];
  client: ReconnectingWebSocket;
  flushNext(): void;
  delays: number[];
}

function buildHarness(overrides: Partial<typeof WS_CONTRACT> = {}): Harness {
  const sockets: FakeSocket[] = [];
  const events: WsEvent<unknown>[] = [];
  const delays: number[] = [];
  const reconnectPending: Array<{ fn: () => void; ms: number }> = [];
  const contract = { ...WS_CONTRACT, ...overrides };
  const heartbeatMarkers = new Set<number>([
    contract.heartbeatIntervalMs,
    contract.heartbeatTimeoutMs,
  ]);

  const client = new ReconnectingWebSocket({
    url: "wss://test/ws",
    contract: overrides,
    socketFactory: () => {
      const s = new FakeSocket();
      sockets.push(s);
      return s;
    },
    randomFn: () => 0.5,
    setTimeoutFn: (fn, ms) => {
      if (heartbeatMarkers.has(ms)) {
        // heartbeats are out-of-band; they would otherwise pollute the
        // reconnect-budget assertions, so the harness silently swallows them.
        return -1;
      }
      delays.push(ms);
      reconnectPending.push({ fn, ms });
      return reconnectPending.length - 1;
    },
    clearTimeoutFn: (h) => {
      const idx = h as number;
      if (idx >= 0 && reconnectPending[idx]) {
        reconnectPending[idx] = { fn: () => undefined, ms: -1 };
      }
    },
    onEvent: (e) => events.push(e),
  });

  return {
    sockets,
    events,
    client,
    delays,
    flushNext: () => {
      const next = reconnectPending.shift();
      if (next && next.ms >= 0) next.fn();
    },
  };
}

function lastState(events: WsEvent<unknown>[]): { state: string; attempt: number } | null {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const e = events[i]!;
    if (e.type === "state") return { state: e.state, attempt: e.attempt };
  }
  return null;
}

test("start opens a socket and transitions to open", () => {
  const h = buildHarness();
  h.client.start();
  assert.equal(h.sockets.length, 1);
  h.sockets[0]!.open();

  const s = lastState(h.events);
  assert.equal(s?.state, "open");
  assert.equal(s?.attempt, 0);
});

test("retriable close triggers exponential backoff with mid-jitter delay", () => {
  const h = buildHarness();
  h.client.start();
  h.sockets[0]!.open();
  h.sockets[0]!.close(1006, "abnormal");

  // first reconnect: base 500ms (jitter centered → exactly 500ms with randomFn=0.5)
  assert.equal(h.delays[0], 500);

  h.flushNext();
  assert.equal(h.sockets.length, 2);
  h.sockets[1]!.close(1006);

  // second reconnect: 500 * 2 = 1000ms
  assert.equal(h.delays[1], 1000);
});

test("non-retriable close codes terminate without reconnect", () => {
  for (const code of NON_RETRIABLE_CLOSE_CODES) {
    const h = buildHarness();
    h.client.start();
    h.sockets[0]!.open();
    h.sockets[0]!.close(code, "policy");

    const s = lastState(h.events);
    assert.equal(s?.state, "failed", `code ${code} should fail-stop`);
    assert.equal(h.delays.length, 0, `code ${code} should not schedule reconnect`);
  }
});

test("max attempts exhausted transitions to failed (fail-soft)", () => {
  const h = buildHarness({ maxAttempts: 3, baseDelayMs: 1, capMs: 1, jitter: 0 });
  h.client.start();

  for (let i = 0; i < 3; i += 1) {
    h.sockets[i]!.open();
    h.sockets[i]!.close(1006);
    h.flushNext();
  }
  h.sockets[3]!.open();
  h.sockets[3]!.close(1006);

  const s = lastState(h.events);
  assert.equal(s?.state, "failed");
});

test("restart() resets attempt counter after fail-soft", () => {
  const h = buildHarness({ maxAttempts: 1, baseDelayMs: 1, capMs: 1, jitter: 0 });
  h.client.start();
  h.sockets[0]!.open();
  h.sockets[0]!.close(1006);
  h.flushNext();
  h.sockets[1]!.open();
  h.sockets[1]!.close(1006);

  assert.equal(lastState(h.events)?.state, "failed");

  h.client.restart();
  h.sockets.at(-1)!.open();

  const post = lastState(h.events);
  assert.equal(post?.state, "open");
  assert.equal(post?.attempt, 0);
});

test("delay never exceeds the contract cap regardless of attempt count", () => {
  const h = buildHarness({ baseDelayMs: 1000, factor: 2, capMs: 5000, jitter: 0 });
  h.client.start();

  for (let i = 0; i < 8; i += 1) {
    h.sockets[i]!.open();
    h.sockets[i]!.close(1006);
    h.flushNext();
  }

  for (const d of h.delays) {
    assert.ok(d <= 5000, `delay ${d}ms exceeds cap`);
  }
});

test("incoming message with seq advances lastSeq", () => {
  const h = buildHarness();
  h.client.start();
  h.sockets[0]!.open();
  h.sockets[0]!.deliver({ seq: 1, payload: { x: 1 } });

  assert.equal(h.client.getLastSeq(), 1);
});

test("seq gap emits reconcile signal with the missing range", () => {
  const h = buildHarness();
  h.client.start();
  h.sockets[0]!.open();
  h.sockets[0]!.deliver({ seq: 5, payload: {} });
  h.sockets[0]!.deliver({ seq: 9, payload: {} });

  const reconcile = h.events.find((e) => e.type === "reconcile");
  assert.ok(reconcile, "reconcile signal expected");
  if (reconcile?.type === "reconcile") {
    assert.equal(reconcile.fromSeq, 6);
    assert.equal(reconcile.toSeq, 8);
  }
});

test("messages without seq surface an error event but do not crash the client", () => {
  const h = buildHarness();
  h.client.start();
  h.sockets[0]!.open();
  h.sockets[0]!.deliver({ payload: "missing seq" });

  const err = h.events.find((e) => e.type === "error");
  assert.ok(err);
  assert.equal(lastState(h.events)?.state, "open");
});

test("pong messages are absorbed without surfacing as data", () => {
  const h = buildHarness();
  h.client.start();
  h.sockets[0]!.open();
  h.sockets[0]!.deliver({ type: "pong" });

  const messageEvents = h.events.filter((e) => e.type === "message");
  assert.equal(messageEvents.length, 0);
});

test("stop() prevents further reconnect attempts", () => {
  const h = buildHarness();
  h.client.start();
  h.sockets[0]!.open();
  h.client.stop();

  assert.equal(h.delays.length, 0);
  assert.equal(lastState(h.events)?.state, "idle");
});
