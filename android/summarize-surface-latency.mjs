import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function summarizeSurfaceLatency(source) {
  const lines = source.trim().split(/\r?\n/);
  const refreshPeriodNanoseconds = Number(lines.shift());
  if (!(refreshPeriodNanoseconds > 0)) {
    throw new Error("SurfaceFlinger latency output omitted its refresh period");
  }
  const actualPresentNanoseconds = lines
    .map((line) => line.trim().split(/\s+/).map(Number))
    .filter((row) => row.length === 3 && row.every(Number.isFinite) && row[1] > 0)
    .map((row) => row[1]);
  const intervalsMilliseconds = actualPresentNanoseconds
    .slice(1)
    .map((timestamp, index) => (timestamp - actualPresentNanoseconds[index]) / 1e6)
    .filter((interval) => interval > 0 && interval < 1_000);
  if (intervalsMilliseconds.length < 20) {
    throw new Error(
      `SurfaceFlinger latency output has only ${intervalsMilliseconds.length} usable intervals`,
    );
  }
  const sorted = [...intervalsMilliseconds].sort((left, right) => left - right);
  const percentile = (fraction) => sorted[Math.ceil(fraction * sorted.length) - 1];
  const meanMilliseconds = sorted.reduce((sum, value) => sum + value, 0) / sorted.length;
  const refreshPeriodMilliseconds = refreshPeriodNanoseconds / 1e6;
  return {
    sampleCount: sorted.length,
    refreshPeriodMilliseconds,
    meanMilliseconds,
    p50Milliseconds: percentile(0.50),
    p95Milliseconds: percentile(0.95),
    p99Milliseconds: percentile(0.99),
    maxMilliseconds: sorted.at(-1),
    meanFramesPerSecond: 1_000 / meanMilliseconds,
    intervalsOverOneAndAHalfRefreshes: sorted.filter(
      (interval) => interval > refreshPeriodMilliseconds * 1.5,
    ).length,
  };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const source = readFileSync(0, "utf8");
    process.stdout.write(`${JSON.stringify(summarizeSurfaceLatency(source), null, 2)}\n`);
  } catch (error) {
    console.error(`surface latency summary error: ${error.message}`);
    process.exitCode = 1;
  }
}
