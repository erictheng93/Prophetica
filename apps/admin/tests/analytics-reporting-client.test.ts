import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import {
  createJob,
  exportJob,
  fetchSchedules,
  fetchTemplates,
} from "../lib/analytics-reporting.js";

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

test("fetchTemplates calls the analytics templates BFF route", async () => {
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  globalThis.fetch = async (input, init) => {
    calls.push([input, init]);
    return jsonResponse([]);
  };

  await fetchTemplates();

  assert.equal(String(calls[0]?.[0]), "/api/analytics/templates");
  assert.equal(calls[0]?.[1]?.cache, "no-store");
});

test("createJob posts JSON to the analytics jobs BFF route", async () => {
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  globalThis.fetch = async (input, init) => {
    calls.push([input, init]);
    return jsonResponse({ id: "rpt-job-000001" });
  };

  await createJob({
    templateCode: "ops-kpi-daily",
    params: {
      startAt: "2026-05-10T00:00:00+08:00",
      endAt: "2026-05-10T23:59:59+08:00",
      timezone: "Asia/Taipei",
      sortBy: "report_date",
      sortDirection: "asc",
      columns: ["report_date"],
    },
  });

  assert.equal(String(calls[0]?.[0]), "/api/analytics/jobs");
  assert.equal(calls[0]?.[1]?.method, "POST");
  assert.equal((calls[0]?.[1]?.headers as Record<string, string>)["content-type"], "application/json");
  assert.match(String(calls[0]?.[1]?.body), /ops-kpi-daily/);
});

test("exportJob returns the raw response without JSON parsing", async () => {
  globalThis.fetch = async () => new Response("csv-bytes", { status: 200 });

  const response = await exportJob("rpt-job-000001", "csv");

  assert.equal(await response.text(), "csv-bytes");
});

test("fetchSchedules calls the analytics schedules BFF route", async () => {
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  globalThis.fetch = async (input, init) => {
    calls.push([input, init]);
    return jsonResponse([]);
  };

  await fetchSchedules();

  assert.equal(String(calls[0]?.[0]), "/api/analytics/schedules");
});

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}
