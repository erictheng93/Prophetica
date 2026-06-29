#!/usr/bin/env node
import { execFile } from "node:child_process";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { promisify } from "node:util";
import { pathToFileURL } from "node:url";

import { runLoadTest } from "./matching-engine-hot-market-load.mjs";

const execFileAsync = promisify(execFile);

const DEFAULT_OUTPUT_JSON = "docs/capacity-reports/matching-engine-production-evidence-latest.json";
const DEFAULT_OUTPUT_MD = "docs/capacity-reports/matching-engine-production-evidence-latest.md";
const DEFAULT_NAMESPACE = "prophetica-engine";
const DEFAULT_SELECTOR = "app.kubernetes.io/name=matching-engine,app.kubernetes.io/component=hot-market";

const DEFAULT_PROMETHEUS_QUERIES = {
  orderRouteP99Seconds:
    'histogram_quantile(0.99, sum(rate(proph_engine_http_request_duration_seconds_bucket{route="/api/v1/markets/:market_id/orders"}[5m])) by (le, route))',
  orderRouteP999Seconds:
    'histogram_quantile(0.999, sum(rate(proph_engine_http_request_duration_seconds_bucket{route="/api/v1/markets/:market_id/orders"}[5m])) by (le, route))',
  bookRouteP99Seconds:
    'histogram_quantile(0.99, sum(rate(proph_engine_http_request_duration_seconds_bucket{route="/api/v1/markets/:market_id/book"}[5m])) by (le, route))',
  bookRouteP999Seconds:
    'histogram_quantile(0.999, sum(rate(proph_engine_http_request_duration_seconds_bucket{route="/api/v1/markets/:market_id/book"}[5m])) by (le, route))',
  commandP99Seconds:
    "histogram_quantile(0.99, sum(rate(proph_engine_command_duration_seconds_bucket[5m])) by (le, command))",
  commandQueueDepth: "max(proph_engine_command_queue_depth)",
  kafkaLag: 'max(kafka_consumergroup_lag{topic=~"engine\\\\..*"})',
  kafkaProducerErrors: 'sum(increase(proph_kafka_producer_errors_total{topic=~"engine\\\\..*"}[5m]))',
};

const DEFAULT_THRESHOLDS = {
  orderRouteP99Ms: 30,
  orderRouteP999Ms: 100,
  bookRouteP99Ms: 30,
  bookRouteP999Ms: 100,
  commandP99Ms: 5,
  commandQueueDepth: 512,
  kafkaLag: 0,
  kafkaProducerErrors: 0,
  minReadyPods: 2,
  minDistinctNodes: 2,
};

export function summarizeTopology(kubernetesPods) {
  const pods = Array.isArray(kubernetesPods.items) ? kubernetesPods.items : [];
  const readyPods = pods.filter((pod) => podReady(pod));
  const nodes = new Set(
    readyPods
      .map((pod) => pod?.spec?.nodeName)
      .filter((nodeName) => typeof nodeName === "string" && nodeName.length > 0),
  );

  return {
    podCount: pods.length,
    readyPodCount: readyPods.length,
    distinctReadyNodeCount: nodes.size,
    readyPods: readyPods.map((pod) => ({
      name: pod?.metadata?.name ?? "unknown",
      node: pod?.spec?.nodeName ?? "unknown",
    })),
  };
}

export function evaluateProductionEvidence({ loadReport, topology, prometheus }, thresholds = {}) {
  const limits = { ...DEFAULT_THRESHOLDS, ...thresholds };
  const topologySummary = summarizeTopology(topology);
  const checks = [
    check("load report result", loadReport?.result === "PASS", loadReport?.reason ?? "missing PASS load report"),
    check(
      "ready hot-market pods",
      topologySummary.readyPodCount >= limits.minReadyPods,
      `${topologySummary.readyPodCount} ready pods, required ${limits.minReadyPods}`,
    ),
    check(
      "topology node spread",
      topologySummary.distinctReadyNodeCount >= limits.minDistinctNodes,
      `${topologySummary.distinctReadyNodeCount} ready nodes, required ${limits.minDistinctNodes}`,
    ),
    metricCheck(prometheus, "orderRouteP99Seconds", limits.orderRouteP99Ms / 1000),
    metricCheck(prometheus, "orderRouteP999Seconds", limits.orderRouteP999Ms / 1000),
    metricCheck(prometheus, "bookRouteP99Seconds", limits.bookRouteP99Ms / 1000),
    metricCheck(prometheus, "bookRouteP999Seconds", limits.bookRouteP999Ms / 1000),
    metricCheck(prometheus, "commandP99Seconds", limits.commandP99Ms / 1000),
    metricCheck(prometheus, "commandQueueDepth", limits.commandQueueDepth),
    metricCheck(prometheus, "kafkaLag", limits.kafkaLag),
    metricCheck(prometheus, "kafkaProducerErrors", limits.kafkaProducerErrors),
  ];
  const result = checks.every((item) => item.pass) ? "PASS" : "FAIL";

  return {
    result,
    generatedAt: new Date().toISOString(),
    thresholds: limits,
    topology: topologySummary,
    loadReport,
    prometheus,
    checks,
  };
}

export function renderProductionEvidenceMarkdown(evidence) {
  const rows = evidence.checks
    .map(
      (item) =>
        `| ${item.name} | ${item.pass ? "PASS" : "FAIL"} | ${item.detail.replaceAll("\n", " ")} |`,
    )
    .join("\n");
  const readyPods = evidence.topology.readyPods
    .map((pod) => `| ${pod.name} | ${pod.node} |`)
    .join("\n");

  return `# Matching Engine Production Evidence

| Field | Value |
| --- | --- |
| Result | ${evidence.result} |
| Generated at | ${evidence.generatedAt} |
| Load target | ${evidence.loadReport?.targetUrl ?? "unknown"} |
| Load scenario | ${evidence.loadReport?.scenario ?? "unknown"} |

## Checks

| Check | Result | Detail |
| --- | --- | --- |
${rows}

## Topology

| Metric | Value |
| --- | ---: |
| Pods | ${evidence.topology.podCount} |
| Ready pods | ${evidence.topology.readyPodCount} |
| Distinct ready nodes | ${evidence.topology.distinctReadyNodeCount} |

| Ready pod | Node |
| --- | --- |
${readyPods || "| none | none |"}

## Prometheus Snapshot

\`\`\`json
${JSON.stringify(evidence.prometheus, null, 2)}
\`\`\`
`;
}

async function main() {
  const options = optionsFromEnv();
  const loadReport = await loadReportFromOptions(options);
  const topology = await topologyFromOptions(options);
  const prometheus = await prometheusFromOptions(options);
  const evidence = evaluateProductionEvidence({
    loadReport,
    topology,
    prometheus,
  }, thresholdsFromEnv());

  const jsonPath = resolve(options.outputJson);
  const markdownPath = resolve(options.outputMarkdown);
  await mkdir(dirname(jsonPath), { recursive: true });
  await mkdir(dirname(markdownPath), { recursive: true });
  await writeFile(jsonPath, `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(markdownPath, renderProductionEvidenceMarkdown(evidence));
  console.log(JSON.stringify(evidence, null, 2));
  process.exitCode = evidence.result === "PASS" ? 0 : 1;
}

async function loadReportFromOptions(options) {
  if (options.runLoad) {
    return runLoadTest(options.load);
  }
  if (!options.loadReportJson) {
    throw new Error("MATCHING_ENGINE_EVIDENCE_LOAD_REPORT_JSON is required unless MATCHING_ENGINE_EVIDENCE_RUN_LOAD=1");
  }
  return JSON.parse(await readFile(options.loadReportJson, "utf8"));
}

async function topologyFromOptions(options) {
  if (options.topologyJson) {
    return JSON.parse(await readFile(options.topologyJson, "utf8"));
  }
  const { stdout } = await execFileAsync(options.kubectl, [
    "get",
    "pods",
    "-n",
    options.namespace,
    "-l",
    options.selector,
    "-o",
    "json",
  ]);
  return JSON.parse(stdout);
}

async function prometheusFromOptions(options) {
  if (options.prometheusSnapshotJson) {
    return JSON.parse(await readFile(options.prometheusSnapshotJson, "utf8"));
  }
  if (!options.prometheusUrl) {
    throw new Error("MATCHING_ENGINE_PROMETHEUS_URL is required unless MATCHING_ENGINE_EVIDENCE_PROMETHEUS_JSON is set");
  }

  const entries = await Promise.all(
    Object.entries(options.prometheusQueries).map(async ([name, query]) => [
      name,
      await queryPrometheus(options.prometheusUrl, query),
    ]),
  );
  return Object.fromEntries(entries);
}

async function queryPrometheus(baseUrl, query) {
  const url = new URL("/api/v1/query", trimTrailingSlash(baseUrl));
  url.searchParams.set("query", query);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Prometheus query failed with HTTP ${response.status}: ${query}`);
  }
  const body = await response.json();
  const values = body?.data?.result
    ?.map((item) => Number(item?.value?.[1]))
    ?.filter((value) => Number.isFinite(value));
  if (!values || values.length === 0) {
    return { query, present: false, value: null };
  }
  return { query, present: true, value: Math.max(...values) };
}

function metricCheck(prometheus, name, maxValue) {
  const metric = prometheus?.[name];
  if (!metric?.present || !Number.isFinite(metric.value)) {
    return check(name, false, "missing Prometheus value");
  }
  return check(name, metric.value <= maxValue, `${metric.value} <= ${maxValue}`);
}

function check(name, pass, detail) {
  return { name, pass, detail };
}

function podReady(pod) {
  return pod?.status?.conditions?.some(
    (condition) => condition.type === "Ready" && condition.status === "True",
  ) ?? false;
}

function optionsFromEnv() {
  return {
    runLoad: process.env.MATCHING_ENGINE_EVIDENCE_RUN_LOAD === "1",
    loadReportJson: process.env.MATCHING_ENGINE_EVIDENCE_LOAD_REPORT_JSON,
    topologyJson: process.env.MATCHING_ENGINE_EVIDENCE_TOPOLOGY_JSON,
    prometheusSnapshotJson: process.env.MATCHING_ENGINE_EVIDENCE_PROMETHEUS_JSON,
    prometheusUrl: process.env.MATCHING_ENGINE_PROMETHEUS_URL,
    prometheusQueries: prometheusQueriesFromEnv(),
    kubectl: process.env.KUBECTL ?? "kubectl",
    namespace: process.env.MATCHING_ENGINE_K8S_NAMESPACE ?? DEFAULT_NAMESPACE,
    selector: process.env.MATCHING_ENGINE_K8S_SELECTOR ?? DEFAULT_SELECTOR,
    outputJson: process.env.MATCHING_ENGINE_EVIDENCE_REPORT_JSON ?? DEFAULT_OUTPUT_JSON,
    outputMarkdown: process.env.MATCHING_ENGINE_EVIDENCE_REPORT_MD ?? DEFAULT_OUTPUT_MD,
    load: loadOptionsFromEnv(),
  };
}

function loadOptionsFromEnv() {
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
  };
}

function prometheusQueriesFromEnv() {
  return {
    orderRouteP99Seconds:
      process.env.MATCHING_ENGINE_PROM_QUERY_ORDER_ROUTE_P99 ?? DEFAULT_PROMETHEUS_QUERIES.orderRouteP99Seconds,
    orderRouteP999Seconds:
      process.env.MATCHING_ENGINE_PROM_QUERY_ORDER_ROUTE_P999 ?? DEFAULT_PROMETHEUS_QUERIES.orderRouteP999Seconds,
    bookRouteP99Seconds:
      process.env.MATCHING_ENGINE_PROM_QUERY_BOOK_ROUTE_P99 ?? DEFAULT_PROMETHEUS_QUERIES.bookRouteP99Seconds,
    bookRouteP999Seconds:
      process.env.MATCHING_ENGINE_PROM_QUERY_BOOK_ROUTE_P999 ?? DEFAULT_PROMETHEUS_QUERIES.bookRouteP999Seconds,
    commandP99Seconds:
      process.env.MATCHING_ENGINE_PROM_QUERY_COMMAND_P99 ?? DEFAULT_PROMETHEUS_QUERIES.commandP99Seconds,
    commandQueueDepth:
      process.env.MATCHING_ENGINE_PROM_QUERY_COMMAND_QUEUE_DEPTH ?? DEFAULT_PROMETHEUS_QUERIES.commandQueueDepth,
    kafkaLag:
      process.env.MATCHING_ENGINE_PROM_QUERY_KAFKA_LAG ?? DEFAULT_PROMETHEUS_QUERIES.kafkaLag,
    kafkaProducerErrors:
      process.env.MATCHING_ENGINE_PROM_QUERY_KAFKA_PRODUCER_ERRORS ?? DEFAULT_PROMETHEUS_QUERIES.kafkaProducerErrors,
  };
}

function thresholdsFromEnv() {
  return {
    orderRouteP99Ms: numberFromEnv("MATCHING_ENGINE_EVIDENCE_ORDER_P99_BUDGET_MS", DEFAULT_THRESHOLDS.orderRouteP99Ms),
    orderRouteP999Ms: numberFromEnv("MATCHING_ENGINE_EVIDENCE_ORDER_P999_BUDGET_MS", DEFAULT_THRESHOLDS.orderRouteP999Ms),
    bookRouteP99Ms: numberFromEnv("MATCHING_ENGINE_EVIDENCE_BOOK_P99_BUDGET_MS", DEFAULT_THRESHOLDS.bookRouteP99Ms),
    bookRouteP999Ms: numberFromEnv("MATCHING_ENGINE_EVIDENCE_BOOK_P999_BUDGET_MS", DEFAULT_THRESHOLDS.bookRouteP999Ms),
    commandP99Ms: numberFromEnv("MATCHING_ENGINE_EVIDENCE_COMMAND_P99_BUDGET_MS", DEFAULT_THRESHOLDS.commandP99Ms),
    commandQueueDepth: numberFromEnv("MATCHING_ENGINE_EVIDENCE_MAX_QUEUE_DEPTH", DEFAULT_THRESHOLDS.commandQueueDepth),
    kafkaLag: numberFromEnv("MATCHING_ENGINE_EVIDENCE_MAX_KAFKA_LAG", DEFAULT_THRESHOLDS.kafkaLag),
    kafkaProducerErrors: numberFromEnv("MATCHING_ENGINE_EVIDENCE_MAX_KAFKA_PRODUCER_ERRORS", DEFAULT_THRESHOLDS.kafkaProducerErrors),
    minReadyPods: numberFromEnv("MATCHING_ENGINE_EVIDENCE_MIN_READY_PODS", DEFAULT_THRESHOLDS.minReadyPods),
    minDistinctNodes: numberFromEnv("MATCHING_ENGINE_EVIDENCE_MIN_DISTINCT_NODES", DEFAULT_THRESHOLDS.minDistinctNodes),
  };
}

function numberFromEnv(name, fallback) {
  const value = Number(process.env[name]);
  return Number.isFinite(value) ? value : fallback;
}

function trimTrailingSlash(value) {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
