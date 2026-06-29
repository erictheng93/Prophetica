#!/usr/bin/env node
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";

const DEFAULT_TARGET_URL = "http://127.0.0.1:8088";
const DEFAULT_HOT_MARKET_ID = "mkt-hot-prod-evidence";

export function percentile(values, quantile) {
  if (values.length === 0) {
    return 0;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((quantile / 100) * sorted.length) - 1),
  );
  return sorted[index];
}

export function summarizeOperation(samples) {
  const latencies = samples.map((sample) => sample.durationMs);
  const errors = samples.filter((sample) => sample.status < 200 || sample.status >= 300).length;
  return {
    count: samples.length,
    errorRate: samples.length === 0 ? 0 : errors / samples.length,
    p50Ms: percentile(latencies, 50),
    p95Ms: percentile(latencies, 95),
    p99Ms: percentile(latencies, 99),
    p999Ms: percentile(latencies, 99.9),
  };
}

export function chooseMarket(sequence, hotMarketId, coldMarketCount, hotMarketRatio) {
  if (coldMarketCount <= 0 || sequence % 100 < hotMarketRatio * 100) {
    return hotMarketId;
  }
  return `mkt-cold-${sequence % coldMarketCount}`;
}

export function renderMarkdownReport(report) {
  const order = report.operations.place_order;
  const read = report.operations.get_book;
  return `# Matching Engine Hot-Market Capacity Evidence

| Field | Value |
| --- | --- |
| Target level | ${report.targetLevel} |
| Scenario | ${report.scenario} |
| Result | ${report.result} |
| Test date | ${report.testDate} |
| Target URL | ${report.targetUrl} |
| Hot market | ${report.workload.hotMarketId} |
| Test duration | ${report.workload.durationSeconds}s |

## Workload

| Input | Value |
| --- | ---: |
| Order TPS target | ${report.workload.orderTps} |
| Read QPS target | ${report.workload.readQps} |
| Cold markets | ${report.workload.coldMarketCount} |
| Hot-market ratio | ${report.workload.hotMarketRatio} |

## Latency And Errors

| Route / operation | Count | P50 | P95 | P99 | P999 | Error rate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| POST /api/v1/markets/:market_id/orders | ${order.count} | ${order.p50Ms.toFixed(2)}ms | ${order.p95Ms.toFixed(2)}ms | ${order.p99Ms.toFixed(2)}ms | ${order.p999Ms.toFixed(2)}ms | ${(order.errorRate * 100).toFixed(3)}% |
| GET /api/v1/markets/:market_id/book | ${read.count} | ${read.p50Ms.toFixed(2)}ms | ${read.p95Ms.toFixed(2)}ms | ${read.p99Ms.toFixed(2)}ms | ${read.p999Ms.toFixed(2)}ms | ${(read.errorRate * 100).toFixed(3)}% |

## Decision

Decision: ${report.result}

Reason:

\`\`\`text
${report.reason}
\`\`\`
`;
}

export async function runLoadTest(options) {
  const targetUrl = trimTrailingSlash(options.targetUrl ?? DEFAULT_TARGET_URL);
  const hotMarketId = options.hotMarketId ?? DEFAULT_HOT_MARKET_ID;
  const durationSeconds = positiveNumber(options.durationSeconds, 60);
  const orderTps = positiveNumber(options.orderTps, 10);
  const readQps = positiveNumber(options.readQps, 20);
  const coldMarketCount = Math.max(0, Number(options.coldMarketCount ?? 9));
  const hotMarketRatio = clamp(Number(options.hotMarketRatio ?? 0.8), 0, 1);
  const userIdBase = Math.max(1, Number(options.userIdBase ?? 100_000));
  const startedAt = new Date();
  const deadline = performance.now() + durationSeconds * 1000;
  const samples = {
    place_order: [],
    get_book: [],
  };

  let sequence = 0;
  while (performance.now() < deadline) {
    const loopStarted = performance.now();
    const orderCount = Math.max(1, Math.round(orderTps));
    const readCount = Math.max(1, Math.round(readQps));

    await Promise.all([
      ...Array.from({ length: orderCount }, async () => {
        sequence += 1;
        const marketId = chooseMarket(sequence, hotMarketId, coldMarketCount, hotMarketRatio);
        samples.place_order.push(
          await placeOrder(targetUrl, marketId, userIdBase + (sequence % 10_000), sequence),
        );
      }),
      ...Array.from({ length: readCount }, async () => {
        sequence += 1;
        const marketId = chooseMarket(sequence, hotMarketId, coldMarketCount, hotMarketRatio);
        samples.get_book.push(await getBook(targetUrl, marketId));
      }),
    ]);

    const elapsed = performance.now() - loopStarted;
    if (elapsed < 1000) {
      await sleep(1000 - elapsed);
    }
  }

  const report = {
    targetLevel: options.targetLevel ?? "L1",
    scenario: options.scenario ?? "hot-market skew",
    result: "FAIL",
    testDate: startedAt.toISOString(),
    targetUrl,
    workload: {
      durationSeconds,
      orderTps,
      readQps,
      hotMarketId,
      coldMarketCount,
      hotMarketRatio,
    },
    operations: {
      place_order: summarizeOperation(samples.place_order),
      get_book: summarizeOperation(samples.get_book),
    },
    reason: "",
  };

  const orderP99BudgetMs = Number(options.orderP99BudgetMs ?? 30);
  const readP99BudgetMs = Number(options.readP99BudgetMs ?? 30);
  const maxErrorRate = Number(options.maxErrorRate ?? 0.001);
  const order = report.operations.place_order;
  const read = report.operations.get_book;
  const orderPass = order.p99Ms <= orderP99BudgetMs && order.errorRate < maxErrorRate;
  const readPass = read.p99Ms <= readP99BudgetMs && read.errorRate < maxErrorRate;
  report.result = orderPass && readPass ? "PASS" : "FAIL";
  report.reason = `order_p99_ms=${order.p99Ms.toFixed(2)}, read_p99_ms=${read.p99Ms.toFixed(2)}, order_error_rate=${(order.errorRate * 100).toFixed(3)}%, read_error_rate=${(read.errorRate * 100).toFixed(3)}%, thresholds: order_p99<=${orderP99BudgetMs}ms, read_p99<=${readP99BudgetMs}ms, error_rate<${(maxErrorRate * 100).toFixed(3)}%`;

  return report;
}

async function placeOrder(targetUrl, marketId, userId, sequence) {
  const side = sequence % 2 === 0 ? "sell" : "buy";
  const price = side === "sell" ? 55 : 56;
  const started = performance.now();
  let status = 0;
  try {
    const response = await fetch(`${targetUrl}/api/v1/markets/${marketId}/orders`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-user-id": String(userId),
        "x-client-order-id": `load-${sequence}`,
      },
      body: JSON.stringify({
        side,
        outcome: "YES",
        type: "LIMIT",
        price_pts: price,
        quantity: 1,
        time_in_force: "GTC",
      }),
    });
    status = response.status;
    await response.arrayBuffer();
  } catch {
    status = 0;
  }
  return { status, durationMs: performance.now() - started };
}

async function getBook(targetUrl, marketId) {
  const started = performance.now();
  let status = 0;
  try {
    const response = await fetch(`${targetUrl}/api/v1/markets/${marketId}/book?depth=10`);
    status = response.status;
    await response.arrayBuffer();
  } catch {
    status = 0;
  }
  return { status, durationMs: performance.now() - started };
}

async function main() {
  const options = optionsFromEnv();
  const report = await runLoadTest(options);
  const jsonPath = resolve(options.reportJson);
  const markdownPath = resolve(options.reportMarkdown);
  await mkdir(dirname(jsonPath), { recursive: true });
  await mkdir(dirname(markdownPath), { recursive: true });
  await writeFile(jsonPath, `${JSON.stringify(report, null, 2)}\n`);
  await writeFile(markdownPath, renderMarkdownReport(report));
  console.log(JSON.stringify(report, null, 2));
  process.exitCode = report.result === "PASS" ? 0 : 1;
}

function optionsFromEnv() {
  return {
    targetUrl: process.env.MATCHING_ENGINE_TARGET_URL,
    hotMarketId: process.env.MATCHING_ENGINE_HOT_MARKET_ID,
    durationSeconds: process.env.MATCHING_ENGINE_LOAD_DURATION_SECONDS,
    orderTps: process.env.MATCHING_ENGINE_LOAD_ORDER_TPS,
    readQps: process.env.MATCHING_ENGINE_LOAD_READ_QPS,
    coldMarketCount: process.env.MATCHING_ENGINE_LOAD_COLD_MARKETS,
    hotMarketRatio: process.env.MATCHING_ENGINE_LOAD_HOT_RATIO,
    targetLevel: process.env.MATCHING_ENGINE_LOAD_TARGET_LEVEL,
    scenario: process.env.MATCHING_ENGINE_LOAD_SCENARIO,
    orderP99BudgetMs: process.env.MATCHING_ENGINE_LOAD_ORDER_P99_BUDGET_MS,
    readP99BudgetMs: process.env.MATCHING_ENGINE_LOAD_READ_P99_BUDGET_MS,
    maxErrorRate: process.env.MATCHING_ENGINE_LOAD_MAX_ERROR_RATE,
    reportJson:
      process.env.MATCHING_ENGINE_LOAD_REPORT_JSON ??
      "docs/capacity-reports/matching-engine-hot-market-latest.json",
    reportMarkdown:
      process.env.MATCHING_ENGINE_LOAD_REPORT_MD ??
      "docs/capacity-reports/matching-engine-hot-market-latest.md",
  };
}

function positiveNumber(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function clamp(value, min, max) {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, value));
}

function trimTrailingSlash(value) {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function sleep(ms) {
  return new Promise((resolveSleep) => {
    setTimeout(resolveSleep, ms);
  });
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
