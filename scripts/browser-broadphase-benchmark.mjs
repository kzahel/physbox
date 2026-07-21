import { randomUUID } from "node:crypto";
import { rename, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { closeHttpServer, closeManagedBrowser, launchManagedBrowser } from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const output = process.env.PHYSBOX5_BROADPHASE_BROWSER_OUTPUT
  ?? "/tmp/physbox5-browser-broadphase.json";
const temporary = `${output}.tmp`;
const counts = process.env.PHYSBOX5_BROADPHASE_COUNTS ?? "32,1000,10000";
const patterns = process.env.PHYSBOX5_BROADPHASE_PATTERNS
  ?? "sparse-moving-grid,packed,giant-static,high-speed-crossing";
const seed = Number(process.env.PHYSBOX5_BROADPHASE_SEED ?? "20528");
const warmupSteps = Number(process.env.PHYSBOX5_BROADPHASE_WARMUP ?? "2");
const samples = Number(process.env.PHYSBOX5_BROADPHASE_SAMPLES ?? "5");
await rm(output, { force: true });
await rm(temporary, { force: true });
run(process.execPath, [
  resolve(root, "scripts/build-web.mjs"),
  "--release",
  "--solver-lab",
]);

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
  const gpuValidationErrors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    const text = message.text();
    if (/validation error|wgpu error|webgpu.*error/i.test(text)) gpuValidationErrors.push(text);
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/`, { waitUntil: "networkidle" });
  await page.waitForFunction(() => globalThis.__physbox5?.ready, null, { timeout: 30_000 });
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  const rows = await page.evaluate(
    async ({ counts, patterns, seed, warmupSteps, samples }) => {
      const module = await import("./pkg/physbox_web.js");
      return JSON.parse(
        await module.runBroadphaseBrowserProbe(
          counts,
          patterns,
          seed,
          warmupSteps,
          samples,
        ),
      );
    },
    { counts, patterns, seed, warmupSteps, samples },
  );
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  if (gpuValidationErrors.length) {
    throw new Error(`browser WebGPU validation errors: ${gpuValidationErrors.join("; ")}`);
  }
  const report = {
    schemaVersion: 1,
    artifactKind: "physbox5-browser-broadphase-v1",
    runId: randomUUID(),
    sourceRevision: git(["rev-parse", "HEAD"]) ?? "unknown",
    sourceDirty: (git(["status", "--porcelain"]) ?? "unknown") !== "",
    host: "browser-webgpu",
    browserVersion: await browser.version(),
    browserUserAgent: await page.evaluate(() => navigator.userAgent),
    counts,
    patterns,
    seed,
    warmupSteps,
    samples,
    rows,
    allCompleted: rows.length > 0 && rows.every((row) => row.completed),
    correctnessAuthority: "native-bounded-brute-force-on-identical-wgsl",
  };
  if (!report.allCompleted) throw new Error("browser broadphase matrix did not complete");
  await writeFile(temporary, `${JSON.stringify(report, null, 2)}\n`);
  await rename(temporary, output);
  console.log(JSON.stringify({ output, report }, null, 2));
  console.log(`PHYSBOX5_BROADPHASE_BROWSER_SUCCESS run_id=${report.runId}`);
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
