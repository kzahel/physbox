import assert from "node:assert/strict";
import test from "node:test";

import { summarizeFrameMarkers } from "./summarize-frame-markers.mjs";

test("measures fixed-step rate and slip from compact Android markers", () => {
  const lines = [];
  for (let index = 0; index < 61; index += 1) {
    const timestamp = 1_700_000_000 + index / 60;
    const report = {
      pacing: {
        completedOuterStepsTotal: 100 + index,
        simulatedSecondsTotal: (100 + index) / 60,
        overloadSlippedWallSecondsTotal: 0.25,
      },
      sceneContract: { n: "bootstrap", d: 195 },
    };
    lines.push(
      `${timestamp.toFixed(6)} 1 1 I physbox5: PHYSBOX5_MARK_FRAME_RENDERED ${JSON.stringify(report)}`,
    );
  }
  const summary = summarizeFrameMarkers(lines.join("\n"));
  assert.equal(summary.sampleCount, 61);
  assert.ok(Math.abs(summary.completedStepsPerSecond - 60) < 0.01);
  assert.ok(Math.abs(summary.effectiveSimulationScale - 1) < 0.001);
  assert.equal(summary.overloadSlippedSeconds, 0);
  assert.equal(summary.finalReport.sceneContract.d, 195);
});

test("rejects captures without a useful measurement window", () => {
  assert.throws(() => summarizeFrameMarkers("unrelated log"), /only 0 usable reports/);
});
