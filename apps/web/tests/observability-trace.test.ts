import assert from "node:assert/strict";
import { test } from "node:test";

import {
  formatTraceparent,
  newTraceContext,
  parseTraceparent,
  tracedFetch,
} from "../lib/observability/trace.js";

test("newTraceContext yields W3C-compliant trace_id (16B) and span_id (8B)", () => {
  const ctx = newTraceContext();
  assert.match(ctx.traceId, /^[0-9a-f]{32}$/);
  assert.match(ctx.spanId, /^[0-9a-f]{16}$/);
  assert.equal(ctx.sampled, true);
});

test("newTraceContext inherits parent trace_id and records parent span", () => {
  const root = newTraceContext();
  const child = newTraceContext(root);
  assert.equal(child.traceId, root.traceId);
  assert.equal(child.parentSpanId, root.spanId);
  assert.notEqual(child.spanId, root.spanId);
});

test("formatTraceparent round-trips through parseTraceparent", () => {
  const ctx = newTraceContext();
  const header = formatTraceparent(ctx);
  const parsed = parseTraceparent(header);
  assert.ok(parsed);
  assert.equal(parsed.traceId, ctx.traceId);
  assert.equal(parsed.spanId, ctx.spanId);
  assert.equal(parsed.sampled, true);
});

test("parseTraceparent rejects malformed and zero-id headers", () => {
  assert.equal(parseTraceparent(""), null);
  assert.equal(parseTraceparent("garbage"), null);
  assert.equal(parseTraceparent(`00-${"0".repeat(32)}-${"a".repeat(16)}-01`), null);
  assert.equal(parseTraceparent(`00-${"a".repeat(32)}-${"0".repeat(16)}-01`), null);
  assert.equal(parseTraceparent(`99-${"a".repeat(32)}-${"a".repeat(16)}-01`), null);
});

test("tracedFetch injects traceparent header when caller did not set one", async () => {
  const captures: Headers[] = [];
  const baseFetch = async (_input: string | URL, init?: RequestInit): Promise<Response> => {
    captures.push(new Headers(init?.headers));
    return new Response("{}", { status: 200, headers: { "content-type": "application/json" } });
  };
  await tracedFetch("http://example/test", { method: "GET" }, { baseFetch });
  assert.equal(captures.length, 1);
  const captured = captures[0];
  assert.ok(captured);
  const header = captured.get("traceparent");
  assert.ok(header, "traceparent header should be present");
  assert.ok(parseTraceparent(header), "injected header must be W3C-valid");
});

test("tracedFetch preserves caller-supplied traceparent (does not overwrite)", async () => {
  const fixed = `00-${"a".repeat(32)}-${"b".repeat(16)}-01`;
  const captures: Headers[] = [];
  const baseFetch = async (_input: string | URL, init?: RequestInit): Promise<Response> => {
    captures.push(new Headers(init?.headers));
    return new Response("{}", { status: 200 });
  };
  await tracedFetch(
    "http://example/test",
    { method: "GET", headers: { traceparent: fixed } },
    { baseFetch },
  );
  assert.equal(captures.length, 1);
  const captured = captures[0];
  assert.ok(captured);
  assert.equal(captured.get("traceparent"), fixed);
});
