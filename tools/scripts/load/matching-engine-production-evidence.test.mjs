import assert from "node:assert/strict";
import test from "node:test";

import {
  evaluateProductionEvidence,
  renderProductionEvidenceMarkdown,
  summarizeTopology,
} from "./matching-engine-production-evidence.mjs";

test("summarizeTopology counts ready pods and node spread", () => {
  const summary = summarizeTopology({
    items: [
      pod("engine-a", "node-a", true),
      pod("engine-b", "node-b", true),
      pod("engine-c", "node-b", false),
    ],
  });

  assert.equal(summary.podCount, 3);
  assert.equal(summary.readyPodCount, 2);
  assert.equal(summary.distinctReadyNodeCount, 2);
});

test("evaluateProductionEvidence passes only with load, topology, and metrics inside thresholds", () => {
  const evidence = evaluateProductionEvidence({
    loadReport: loadReport("PASS"),
    topology: {
      items: [pod("engine-a", "node-a", true), pod("engine-b", "node-b", true)],
    },
    prometheus: passingPrometheus(),
  });

  assert.equal(evidence.result, "PASS");
  assert(evidence.checks.every((check) => check.pass));
  assert.match(renderProductionEvidenceMarkdown(evidence), /Matching Engine Production Evidence/);
});

test("evaluateProductionEvidence fails when hot pods are not spread across nodes", () => {
  const evidence = evaluateProductionEvidence({
    loadReport: loadReport("PASS"),
    topology: {
      items: [pod("engine-a", "node-a", true), pod("engine-b", "node-a", true)],
    },
    prometheus: passingPrometheus(),
  });

  assert.equal(evidence.result, "FAIL");
  assert.equal(evidence.checks.find((check) => check.name === "topology node spread").pass, false);
});

test("evaluateProductionEvidence fails closed when a required Prometheus value is missing", () => {
  const prometheus = passingPrometheus();
  prometheus.kafkaLag = { present: false, value: null };

  const evidence = evaluateProductionEvidence({
    loadReport: loadReport("PASS"),
    topology: {
      items: [pod("engine-a", "node-a", true), pod("engine-b", "node-b", true)],
    },
    prometheus,
  });

  assert.equal(evidence.result, "FAIL");
  assert.equal(evidence.checks.find((check) => check.name === "kafkaLag").detail, "missing Prometheus value");
});

test("evaluateProductionEvidence fails when order P999 is missing or over budget", () => {
  const missing = passingPrometheus();
  delete missing.orderRouteP999Seconds;

  const missingEvidence = evaluateProductionEvidence({
    loadReport: loadReport("PASS"),
    topology: {
      items: [pod("engine-a", "node-a", true), pod("engine-b", "node-b", true)],
    },
    prometheus: missing,
  });

  assert.equal(missingEvidence.result, "FAIL");
  assert.equal(
    missingEvidence.checks.find((check) => check.name === "orderRouteP999Seconds").detail,
    "missing Prometheus value",
  );

  const overBudget = passingPrometheus();
  overBudget.orderRouteP999Seconds = present(0.15);

  const overBudgetEvidence = evaluateProductionEvidence({
    loadReport: loadReport("PASS"),
    topology: {
      items: [pod("engine-a", "node-a", true), pod("engine-b", "node-b", true)],
    },
    prometheus: overBudget,
  });

  assert.equal(overBudgetEvidence.result, "FAIL");
  assert.equal(overBudgetEvidence.checks.find((check) => check.name === "orderRouteP999Seconds").pass, false);
});

test("evaluateProductionEvidence fails when the load report fails", () => {
  const evidence = evaluateProductionEvidence({
    loadReport: loadReport("FAIL"),
    topology: {
      items: [pod("engine-a", "node-a", true), pod("engine-b", "node-b", true)],
    },
    prometheus: passingPrometheus(),
  });

  assert.equal(evidence.result, "FAIL");
  assert.equal(evidence.checks.find((check) => check.name === "load report result").pass, false);
});

function pod(name, node, ready) {
  return {
    metadata: { name },
    spec: { nodeName: node },
    status: {
      conditions: [
        {
          type: "Ready",
          status: ready ? "True" : "False",
        },
      ],
    },
  };
}

function loadReport(result) {
  return {
    result,
    reason: result === "PASS" ? "within thresholds" : "order_p99_ms=100.00",
    targetUrl: "https://matching-engine.example",
    scenario: "hot-market skew",
  };
}

function passingPrometheus() {
  return {
    orderRouteP99Seconds: present(0.02),
    orderRouteP999Seconds: present(0.08),
    bookRouteP99Seconds: present(0.01),
    bookRouteP999Seconds: present(0.04),
    commandP99Seconds: present(0.003),
    commandQueueDepth: present(10),
    kafkaLag: present(0),
    kafkaProducerErrors: present(0),
  };
}

function present(value) {
  return { present: true, value };
}
