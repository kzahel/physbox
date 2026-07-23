import { randomUUID } from "node:crypto";
import { rename, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { closeHttpServer, closeManagedBrowser, launchManagedBrowser } from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const output = process.env.PHYSBOX5_BODY_BENCHMARK_OUTPUT ?? "/tmp/physbox5-browser-bodies.json";
const temporary = `${output}.tmp`;
const counts = process.env.PHYSBOX5_BODY_COUNTS ?? "1000,10000,100000";
const seed = Number(process.env.PHYSBOX5_BODY_SEED ?? "24301");
const width = Number(process.env.PHYSBOX5_BODY_WIDTH ?? "960");
const height = Number(process.env.PHYSBOX5_BODY_HEIGHT ?? "600");
const warmupSteps = Number(process.env.PHYSBOX5_BODY_WARMUP ?? "3");
const samples = Number(process.env.PHYSBOX5_BODY_SAMPLES ?? "20");
await rm(output, { force: true });
await rm(temporary, { force: true });
run(process.execPath, [resolve(root, "scripts/build-web.mjs"), "--release"]);

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
  const page = await browser.newPage({ viewport: { width, height } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}/?smoke-observer=1`, {
    waitUntil: "networkidle",
  });
  await page.waitForFunction(() => globalThis.__physbox5?.ready, null, { timeout: 30_000 });
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  const lanes = await page.evaluate(
    async ({ counts, seed, width, height, warmupSteps, samples }) => {
      const module = await import("./pkg/physbox_web.js");
      return JSON.parse(
        await module.runBodyBenchmark(counts, seed, width, height, warmupSteps, samples),
      );
    },
    { counts, seed, width, height, warmupSteps, samples },
  );
  const browserAdapterInfo = await page.evaluate(async () => {
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) return null;
    const info = adapter.info;
    return {
      vendor: info.vendor,
      architecture: info.architecture,
      device: info.device,
      description: info.description,
    };
  });
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  const report = {
    schemaVersion: 1,
    runId: randomUUID(),
    sourceRevision: git(["rev-parse", "HEAD"]) ?? "unknown",
    sourceDirty: (git(["status", "--porcelain"]) ?? "unknown") !== "",
    host: "browser-webgpu",
    capturePhase: "p5.1-body-free-flight-scale",
    browserVersion: await browser.version(),
    browserUserAgent: await page.evaluate(() => navigator.userAgent),
    browserAdapterInfo,
    seed,
    viewport: [width, height],
    warmupSteps,
    samples,
    lanes,
  };
  await writeFile(temporary, `${JSON.stringify(report, null, 2)}\n`);
  await rename(temporary, output);
  console.log(JSON.stringify({ output, report }, null, 2));
  console.log(`PHYSBOX5_BODY_BENCHMARK_SUCCESS host=browser run_id=${report.runId}`);
} finally {
  await closeManagedBrowser(browserSession);
  await closeHttpServer(server);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function git(args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : undefined;
}
