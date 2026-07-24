import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { open, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import {
  closeHttpServer,
  closeManagedBrowser,
  launchManagedBrowser,
} from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const options = parseArguments(process.argv.slice(2));
run(process.execPath, [resolve(root, "scripts/build-web.mjs")]);

const sourceRevision = git(["rev-parse", "HEAD"]);
const sourceStatus = git(["status", "--porcelain=v1", "--untracked-files=all"]);
const sourceDirty = sourceStatus.length !== 0;
const server = await startServer(resolve(root, "target", "web"));
let browserSession;
const started = performance.now();
try {
  browserSession = await launchManagedBrowser({
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: process.env.HEADED !== "1",
    args: [
      "--enable-unsafe-webgpu",
      ...(process.platform === "linux"
        ? ["--enable-features=Vulkan", "--use-angle=vulkan"]
        : []),
      ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
    ],
  });
  const port = server.address().port;
  await collectQuality(browserSession.browser, port, "standard", {
    ...options,
    samples: 1,
  });
  const trialRows = [];
  const trialOrder = [];
  for (let trialIndex = 0; trialIndex < options.trials; trialIndex += 1) {
    const qualities =
      trialIndex % 2 === 0
        ? ["standard", "stabilized"]
        : ["stabilized", "standard"];
    for (const quality of qualities) {
      trialOrder.push(quality);
      trialRows.push({
        trialIndex,
        measurementOrder: trialRows.length,
        ...(await collectQuality(
          browserSession.browser,
          port,
          quality,
          options,
        )),
      });
    }
  }
  const rows = ["standard", "stabilized"].map((quality) =>
    aggregateQuality(quality, trialRows),
  );
  const artifact = {
    version: 1,
    artifactKind: "physbox5-s35-browser-coupling-performance",
    runId: options.runId,
    sourceRevision,
    sourceDirty,
    host: "browser-webgpu",
    samplePolicy: {
      warmupSamples: options.warmupSamples,
      retainedSamplesPerTrial: options.samples,
      trialsPerQuality: options.trials,
      engineSampleIntervalSteps: rows[0].sceneContract.sampleIntervalSteps,
      percentileMethod: "nearest-rank",
      freshWorldPerQuality: true,
      untimedSystemWarmup: "one Standard world",
      trialOrder,
    },
    elapsedSeconds: (performance.now() - started) / 1000,
    rows,
    comparison: compare(rows),
  };
  await createFreshJson(options.output, artifact);
  console.log(
    `PHYSBOX5_VALIDATION_SUCCESS host=browser-webgpu scenario=liquid-sandbox run_id=${options.runId} samples=${options.samples} output=${options.output}`,
  );
} finally {
  await closeManagedBrowser(browserSession);
  await closeHttpServer(server);
}

async function collectQuality(browser, port, quality, options) {
  const context = await browser.newContext({
    viewport: { width: options.width, height: options.height },
  });
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  const query = new URLSearchParams({
    scenario: "liquid-sandbox",
    "coupling-quality": quality,
    "smoke-observer": "1",
  });
  await page.goto(`http://127.0.0.1:${port}/?${query}`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(
    (expected) => {
      const state = globalThis.__physbox5;
      return state?.ok
        && state.frame?.sceneContract?.scenario === "liquid-sandbox"
        && state.frame?.sceneContract?.fluidSolidCouplingQuality === expected
        && state.frame?.frameTiming?.timestampQueriesSupported === true
        && state.frame?.fixedStepsTotal >= 30;
    },
    quality,
    { timeout: 30_000 },
  );
  const initial = await page.evaluate(() => globalThis.__physbox5);
  if (initial.ok !== true || pageErrors.length !== 0) {
    throw new Error(
      `${quality} browser startup failed: state=${JSON.stringify(initial)} errors=${pageErrors.join("; ")}`,
    );
  }

  const raw = [];
  let lastFrameIndex = null;
  const required = options.warmupSamples + options.samples;
  while (raw.length < required) {
    await page.waitForFunction(
      (last) => {
        const timing = globalThis.__physbox5?.frame?.frameTiming;
        return timing?.sampleFrameIndex != null
          && timing.sampleFrameIndex !== last
          && timing.sampleFixedSteps === 1
          && timing.error == null;
      },
      lastFrameIndex,
      { timeout: 30_000 },
    );
    const frame = await page.evaluate(() => globalThis.__physbox5.frame);
    lastFrameIndex = frame.frameTiming.sampleFrameIndex;
    raw.push({
      sampleFrameIndex: lastFrameIndex,
      sampleFixedSteps: frame.frameTiming.sampleFixedSteps,
      ageFrames: frame.frameTiming.ageFrames,
      cpuEncodeMs: frame.frameTiming.cpuEncodeMs,
      rigidComputeMs: frame.frameTiming.rigidComputeMs,
      liquidComputeMs: frame.frameTiming.liquidComputeMs,
      bodyRenderMs: frame.frameTiming.bodyRenderMs,
      particleRenderMs: frame.frameTiming.particleRenderMs,
      compositeRenderMs: frame.frameTiming.compositeRenderMs,
      uiRenderMs: frame.frameTiming.uiRenderMs,
      totalGpuMs: frame.frameTiming.totalGpuMs,
      presentedFrameHz: frame.pacing.presentedFrameHz,
      effectiveSimulationScale: frame.pacing.effectiveSimulationScale,
      overloadSlippedWallSecondsTotal:
        frame.pacing.overloadSlippedWallSecondsTotal,
    });
  }
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  const finalFrame = await page.evaluate(() => globalThis.__physbox5.frame);
  await context.close();

  const samples = raw.slice(options.warmupSamples);
  for (const sample of samples) {
    for (const key of [
      "cpuEncodeMs",
      "rigidComputeMs",
      "liquidComputeMs",
      "totalGpuMs",
      "presentedFrameHz",
      "effectiveSimulationScale",
    ]) {
      if (!Number.isFinite(sample[key])) {
        throw new Error(`${quality} sample has invalid ${key}: ${JSON.stringify(sample)}`);
      }
    }
  }
  return {
    quality,
    adapter: initial.diagnostics,
    sceneContract: {
      bodyCount: finalFrame.sceneContract.bodyCount,
      particleCount: finalFrame.sceneContract.particleCount,
      solverDispatchesPerStep: finalFrame.sceneContract.solverDispatchesPerStep,
      solverCapacityBytes: finalFrame.sceneContract.solverCapacityBytes,
      particleDispatchesPerStep:
        finalFrame.sceneContract.particleDispatchesPerStep,
      sampleIntervalSteps: finalFrame.frameTiming.sampleIntervalSteps,
      capacityOverflowed: finalFrame.sceneContract.capacityOverflowed,
    },
    summary: summarize(samples),
    samples,
  };
}

function summarize(samples) {
  return Object.fromEntries(
    [
      "cpuEncodeMs",
      "rigidComputeMs",
      "liquidComputeMs",
      "bodyRenderMs",
      "particleRenderMs",
      "compositeRenderMs",
      "uiRenderMs",
      "totalGpuMs",
      "presentedFrameHz",
      "effectiveSimulationScale",
    ].map((key) => [key, statistics(samples.map((sample) => sample[key]))]),
  );
}

function aggregateQuality(quality, trialRows) {
  const trials = trialRows.filter((row) => row.quality === quality);
  const sceneContracts = new Set(
    trials.map((row) => JSON.stringify(row.sceneContract)),
  );
  if (trials.length === 0 || sceneContracts.size !== 1) {
    throw new Error(
      `${quality} trials changed or omitted their scene contract: ${[...sceneContracts].join("; ")}`,
    );
  }
  const samples = trials.flatMap((trial) =>
    trial.samples.map((sample) => ({
      trialIndex: trial.trialIndex,
      measurementOrder: trial.measurementOrder,
      ...sample,
    })),
  );
  return {
    quality,
    adapter: trials[0].adapter,
    sceneContract: trials[0].sceneContract,
    summary: summarize(samples),
    trialSummaries: trials.map((trial) => ({
      trialIndex: trial.trialIndex,
      measurementOrder: trial.measurementOrder,
      summary: trial.summary,
    })),
    samples,
  };
}

function statistics(values) {
  const sorted = values.toSorted((left, right) => left - right);
  return {
    minimum: sorted[0],
    p50: percentile(sorted, 0.5),
    p95: percentile(sorted, 0.95),
    p99: percentile(sorted, 0.99),
    maximum: sorted.at(-1),
    mean: sorted.reduce((sum, value) => sum + value, 0) / sorted.length,
  };
}

function percentile(sorted, fraction) {
  return sorted[Math.max(0, Math.ceil(fraction * sorted.length) - 1)];
}

function compare(rows) {
  const standard = rows.find((row) => row.quality === "standard");
  const stabilized = rows.find((row) => row.quality === "stabilized");
  return Object.fromEntries(
    [
      "cpuEncodeMs",
      "rigidComputeMs",
      "liquidComputeMs",
      "totalGpuMs",
      "presentedFrameHz",
      "effectiveSimulationScale",
    ].map((key) => {
      const before = standard.summary[key].p50;
      const after = stabilized.summary[key].p50;
      return [
        key,
        {
          standardP50: before,
          stabilizedP50: after,
          delta: after - before,
          deltaFraction: before === 0 ? null : (after - before) / before,
        },
      ];
    }),
  );
}

function parseArguments(arguments_) {
  const parsed = {
    runId: `s35-coupling-browser-${randomUUID()}`,
    output: null,
    samples: 24,
    warmupSamples: 4,
    trials: 2,
    width: 960,
    height: 600,
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    const value = () => {
      index += 1;
      if (index >= arguments_.length) {
        throw new Error(`${argument} requires a value`);
      }
      return arguments_[index];
    };
    if (argument === "--run-id") parsed.runId = value();
    else if (argument === "--output") parsed.output = resolve(value());
    else if (argument === "--samples") parsed.samples = Number.parseInt(value(), 10);
    else if (argument === "--trials") parsed.trials = Number.parseInt(value(), 10);
    else if (argument === "--warmup-samples") {
      parsed.warmupSamples = Number.parseInt(value(), 10);
    } else if (argument === "--width") parsed.width = Number.parseInt(value(), 10);
    else if (argument === "--height") parsed.height = Number.parseInt(value(), 10);
    else throw new Error(`unknown argument ${argument}`);
  }
  if (!parsed.output) throw new Error("--output is required");
  for (const key of ["samples", "warmupSamples", "trials", "width", "height"]) {
    if (!Number.isSafeInteger(parsed[key]) || parsed[key] <= 0) {
      throw new Error(`${key} must be a positive integer`);
    }
  }
  return parsed;
}

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${arguments_.join(" ")} failed with ${result.status}`);
  }
}

function git(arguments_) {
  const result = spawnSync("git", arguments_, {
    cwd: root,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`git ${arguments_.join(" ")} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

async function createFreshJson(path, value) {
  const handle = await open(path, "wx");
  try {
    await writeFile(handle, `${JSON.stringify(value, null, 2)}\n`);
    await handle.sync();
  } finally {
    await handle.close();
  }
}
