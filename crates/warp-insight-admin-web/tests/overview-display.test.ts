import { readFileSync } from "node:fs";
import { normalizeOverview } from "../src/api/admin";

// Verifies that a live `/api/v1/admin/agents/overview` response survives the
// frontend normalizer with its status metrics intact. Guards the exact
// regression where normalizeRecentOnlineAgent dropped memoryBytes /
// cpuPercent / adminLatencyMs / metricsHistory (card always rendered "—").
//
// Usage:
//   tsx tests/overview-display.test.ts <overview.json> <agentId> [resources]
//   - resources: pass to also require memoryBytes/cpuPercent to be populated
//     (the daemon measures these on Linux and macOS hosts).
const [overviewPath, agentId, hostFlag] = process.argv.slice(2);
if (!overviewPath || !agentId) {
  console.error(
    "usage: tsx tests/overview-display.test.ts <overview.json> <agentId> [resources]",
  );
  process.exit(2);
}

let payload: unknown;
try {
  payload = JSON.parse(readFileSync(overviewPath, "utf8"));
} catch (err) {
  console.error(`failed to read overview payload ${overviewPath}: ${err}`);
  process.exit(1);
}

const overview = normalizeOverview(payload);
const agent = overview.recentOnlineAgents.find((item) => item.agentId === agentId);
const failures: string[] = [];

if (!agent) {
  failures.push(`agent ${agentId} not found in overview`);
} else {
  const history = agent.metricsHistory ?? [];
  if (history.length < 2) {
    failures.push(`metricsHistory length ${history.length} < 2`);
  }
  if (!history.some((sample) => typeof sample.adminLatencyMs === "number" && sample.adminLatencyMs > 0)) {
    failures.push("no adminLatencyMs sample");
  }
  history.forEach((sample, index) => {
    if (!sample.at) failures.push(`sample ${index} missing at`);
  });
  // The metric keys must survive normalization even when the value is null
  // (unsupported hosts report no memory/CPU). A dropped key means the
  // normalizer strips the field, which is the display bug this test guards.
  if (!("memoryBytes" in agent)) failures.push("memoryBytes dropped by normalizer");
  if (!("cpuPercent" in agent)) failures.push("cpuPercent dropped by normalizer");
  if (!("adminLatencyMs" in agent)) failures.push("adminLatencyMs dropped by normalizer");
  if (hostFlag === "resources") {
    if (typeof agent.memoryBytes !== "number" || agent.memoryBytes <= 0) {
      failures.push("memoryBytes not reported on this host");
    }
    if (typeof agent.cpuPercent !== "number" || agent.cpuPercent < 0) {
      failures.push("cpuPercent not reported on this host");
    }
  }
}

if (failures.length > 0) {
  console.error(`overview display test failed: ${failures.join("; ")}`);
  process.exit(1);
}

const history = agent!.metricsHistory ?? [];
console.log(`overview display ok: ${agentId} history=${history.length} samples`);
