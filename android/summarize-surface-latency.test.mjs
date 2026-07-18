import assert from "node:assert/strict";
import test from "node:test";

import { summarizeSurfaceLatency } from "./summarize-surface-latency.mjs";

test("summarizes actual presentation intervals with nearest-rank percentiles", () => {
  let timestamp = 1_000_000_000;
  const intervals = [
    ...Array(20).fill(16_666_667),
    33_333_334,
  ];
  const rows = [`16666667`, `${timestamp - 1_000}\t${timestamp}\t${timestamp + 1_000}`];
  for (const interval of intervals) {
    timestamp += interval;
    rows.push(`${timestamp - 1_000}\t${timestamp}\t${timestamp + 1_000}`);
  }
  const summary = summarizeSurfaceLatency(rows.join("\n"));
  assert.equal(summary.sampleCount, 21);
  assert.equal(summary.p50Milliseconds, 16.666667);
  assert.equal(summary.p95Milliseconds, 16.666667);
  assert.equal(summary.p99Milliseconds, 33.333334);
  assert.equal(summary.intervalsOverOneAndAHalfRefreshes, 1);
  assert.ok(summary.meanFramesPerSecond > 57 && summary.meanFramesPerSecond < 58);
});

test("rejects truncated or malformed captures", () => {
  assert.throws(() => summarizeSurfaceLatency(""), /refresh period/);
  assert.throws(
    () => summarizeSurfaceLatency("16666667\n1 2 3\n2 3 4\n"),
    /only 1 usable intervals/,
  );
});
