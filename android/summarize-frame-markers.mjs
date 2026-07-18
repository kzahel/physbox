import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function summarizeFrameMarkers(source) {
  const samples = source
    .split(/\r?\n/)
    .map((line) => {
      const match = line.match(
        /^\s*(\d+(?:\.\d+)?)\s+.*PHYSBOX5_MARK_FRAME_RENDERED\s+(\{.*\})$/,
      );
      if (!match) return null;
      return { timestampSeconds: Number(match[1]), report: JSON.parse(match[2]) };
    })
    .filter(Boolean);
  if (samples.length < 20) {
    throw new Error(`frame marker capture has only ${samples.length} usable reports`);
  }
  const first = samples[0];
  const last = samples.at(-1);
  const wallSeconds = last.timestampSeconds - first.timestampSeconds;
  const completedSteps =
    last.report.pacing.completedOuterStepsTotal -
    first.report.pacing.completedOuterStepsTotal;
  const simulatedSeconds =
    last.report.pacing.simulatedSecondsTotal - first.report.pacing.simulatedSecondsTotal;
  const overloadSlippedSeconds =
    last.report.pacing.overloadSlippedWallSecondsTotal -
    first.report.pacing.overloadSlippedWallSecondsTotal;
  if (!(wallSeconds > 0) || completedSteps < 0 || simulatedSeconds < 0) {
    throw new Error("frame marker capture is not monotonic");
  }
  return {
    sampleCount: samples.length,
    wallSeconds,
    completedSteps,
    completedStepsPerSecond: completedSteps / wallSeconds,
    simulatedSeconds,
    effectiveSimulationScale: simulatedSeconds / wallSeconds,
    overloadSlippedSeconds,
    finalReport: last.report,
  };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const source = readFileSync(0, "utf8");
    process.stdout.write(`${JSON.stringify(summarizeFrameMarkers(source), null, 2)}\n`);
  } catch (error) {
    console.error(`frame marker summary error: ${error.message}`);
    process.exitCode = 1;
  }
}
