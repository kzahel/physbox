import { randomUUID } from "node:crypto";
import { rename, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { closeHttpServer, closeManagedBrowser, launchManagedBrowser } from "./browser-process.mjs";
import { startServer } from "./server.mjs";
import { sourceState } from "./source-state.mjs";

const root = resolve(import.meta.dirname, "..");
const output = process.env.PHYSBOX5_LIQUID_SCALE_OUTPUT
  ?? "/tmp/physbox5-browser-liquid-scale.json";
const temporary = `${output}.tmp`;
const solidTier = process.env.PHYSBOX5_LIQUID_SOLIDS ?? "20";
const particleCount = positiveInteger("PHYSBOX5_LIQUID_PARTICLES", "4096");
const warmupSteps = positiveInteger("PHYSBOX5_LIQUID_WARMUP", "16");
const samples = positiveInteger("PHYSBOX5_LIQUID_SAMPLES", "30");
if (!["water-only", "20", "100"].includes(solidTier)) {
  throw new Error("PHYSBOX5_LIQUID_SOLIDS must be water-only, 20, or 100");
}

await rm(output, { force: true });
await rm(temporary, { force: true });
run(process.execPath, [
  resolve(root, "scripts/build-web.mjs"),
  "--release",
  "--solver-lab",
  "--no-box2d-comparator",
]);

const { sourceRevision, sourceDirty, sourcePatchDigest } = await sourceState(root);
const rustcVersion = commandText("rustc", ["--version"]);
if (!rustcVersion) throw new Error("browser liquid benchmark requires rustc provenance");
const runId = randomUUID();

const server = await startServer(resolve(root, "target", "web"));
let browserSession;
try {
  browserSession = await launchManagedBrowser({
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: process.env.HEADED !== "1",
    args: [
      "--enable-unsafe-webgpu",
      ...(process.platform === "linux" ? ["--enable-features=Vulkan", "--use-angle=vulkan"] : []),
      ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
    ],
  });
  const { browser } = browserSession;
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}/?smoke-observer=1`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(() => globalThis.__physbox5?.ready, null, { timeout: 30_000 });
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  const browserVersion = await browser.version();
  const browserHost = await page.evaluate(() => navigator.userAgent);
  const artifact = await page.evaluate(
    async (parameters) => {
      const module = await import("./pkg/physbox_web.js");
      return JSON.parse(await module.runLiquidScaleBenchmark(
        parameters.solidTier,
        parameters.particleCount,
        parameters.warmupSteps,
        parameters.samples,
        parameters.runId,
        parameters.sourceRevision,
        parameters.sourceDirty,
        parameters.sourcePatchDigest,
        parameters.browserVersion,
        parameters.browserHost,
        parameters.toolchain,
      ));
    },
    {
      solidTier,
      particleCount,
      warmupSteps,
      samples,
      runId,
      sourceRevision,
      sourceDirty,
      sourcePatchDigest,
      browserVersion,
      browserHost,
      toolchain: `${rustcVersion}; wasm-bindgen-cli 0.2.125`,
    },
  );
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  await writeFile(temporary, `${JSON.stringify(artifact, null, 2)}\n`);
  await rename(temporary, output);
  console.log(JSON.stringify({ output, artifact }, null, 2));
  console.log(
    `PHYSBOX5_LIQUID_SCALE_SUCCESS host=browser scenario=${artifact.scenarioId} output=${output}`,
  );
} finally {
  await closeManagedBrowser(browserSession);
  await closeHttpServer(server);
}

function positiveInteger(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${name} must be positive`);
  return value;
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function commandText(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : undefined;
}
