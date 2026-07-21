import { spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const budgetSeconds = Number.parseFloat(process.env.PHYSBOX5_FAST_BUDGET_SECONDS ?? "120");
if (!(budgetSeconds > 0)) throw new Error("PHYSBOX5_FAST_BUDGET_SECONDS must be positive");

const lanes = [
  {
    name: "format",
    command: "cargo",
    args: ["fmt", "--all", "--", "--check"],
  },
  {
    name: "production-rust-and-perf-smoke",
    command: "cargo",
    args: [
      "test",
      "--quiet",
      "--lib",
      "--features",
      "physbox-engine/production-validation",
      "--",
      "--nocapture",
    ],
  },
  {
    name: "tooling-unit",
    command: process.execPath,
    args: [
      "--test",
      "scripts/host-env.test.mjs",
      "android/require-hardware-adapter.test.mjs",
      "android/summarize-frame-markers.test.mjs",
      "android/summarize-surface-latency.test.mjs",
    ],
  },
  {
    name: "platform-boundaries",
    command: process.execPath,
    args: ["scripts/check-platform-boundaries.mjs"],
  },
  {
    name: "browser-webgpu",
    command: process.execPath,
    args: ["scripts/browser-smoke.mjs"],
    env: {
      PHYSBOX5_CAPTURE_SUFFIX: "fast",
      PHYSBOX5_CONCISE: "1",
    },
  },
];

const started = performance.now();
const timings = [];
for (const lane of lanes) {
  const elapsedSeconds = (performance.now() - started) / 1_000;
  const remainingSeconds = budgetSeconds - elapsedSeconds;
  if (remainingSeconds <= 0) fail(`budget exhausted before ${lane.name}`);

  const laneStarted = performance.now();
  console.log(`PHYSBOX5_FAST_LANE_START ${lane.name}`);
  const result = spawnSync(lane.command, lane.args, {
    cwd: root,
    stdio: "inherit",
    timeout: Math.ceil(remainingSeconds * 1_000),
    env: { ...process.env, ...lane.env },
  });
  const laneSeconds = (performance.now() - laneStarted) / 1_000;
  timings.push([lane.name, laneSeconds]);
  if (result.error) fail(`${lane.name}: ${result.error.message}`);
  if (result.status !== 0) fail(`${lane.name}: exited ${result.status ?? "without status"}`);
  console.log(`PHYSBOX5_FAST_LANE_SUCCESS ${lane.name} seconds=${laneSeconds.toFixed(2)}`);
}

const totalSeconds = (performance.now() - started) / 1_000;
if (totalSeconds > budgetSeconds) fail(`total ${totalSeconds.toFixed(2)}s exceeds ${budgetSeconds}s`);
console.log(
  `PHYSBOX5_FAST_SUCCESS total_seconds=${totalSeconds.toFixed(2)} budget_seconds=${budgetSeconds} ${timings
    .map(([name, seconds]) => `${name}=${seconds.toFixed(2)}`)
    .join(" ")}`,
);

function fail(reason) {
  console.error(`PHYSBOX5_FAST_FAILURE ${reason}`);
  process.exit(1);
}
