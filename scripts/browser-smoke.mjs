import { randomUUID } from "node:crypto";
import { rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { chromium } from "@playwright/test";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const screenshot = "/tmp/physbox5-web.png";
const provenancePath = "/tmp/physbox5-web.json";
const scenario = process.env.PHYSBOX5_SCENARIO ?? "bootstrap";
await rm(screenshot, { force: true });
await rm(provenancePath, { force: true });
run(process.execPath, [resolve(root, "scripts/build-web.mjs")]);

const server = await startServer(resolve(root, "target", "web"));
let browser;
try {
  browser = await chromium.launch({
    channel: process.env.PLAYWRIGHT_CHROME_CHANNEL ?? "chrome",
    headless: process.env.HEADED !== "1",
    args: [
      "--enable-unsafe-webgpu",
      ...(process.platform === "linux" ? ["--enable-features=Vulkan", "--use-angle=vulkan"] : []),
      ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
    ],
  });
  const port = server.address().port;
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?scenario=${encodeURIComponent(scenario)}`, { waitUntil: "networkidle" });
  await page.waitForFunction(() => globalThis.__physbox5?.ready, null, { timeout: 30_000 });
  const before = await page.evaluate(() => globalThis.__physbox5);
  if (!before.ok || !before.frame?.scenario?.passed) throw new Error(`shared scenario failed: ${JSON.stringify(before)}`);
  if (before.frame.scenario.name !== scenario) throw new Error(`wrong shared scenario: ${JSON.stringify(before.frame.scenario)}`);
  const bounds = await page.locator("#physbox-canvas").boundingBox();
  if (!bounds || bounds.x !== 0 || bounds.y !== 0 || bounds.width !== 960 || bounds.height !== 600) {
    throw new Error(`canvas does not fill viewport: ${JSON.stringify(bounds)}`);
  }
  await page.mouse.move(480, 300);
  await page.mouse.down();
  await page.mouse.up();
  await page.waitForFunction(
    (count) => globalThis.__physbox5?.frame?.pointerEventsTotal > count,
    before.frame.pointerEventsTotal,
    { timeout: 10_000 },
  );
  const after = await page.evaluate(() => globalThis.__physbox5);
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  await page.waitForTimeout(200);
  await page.screenshot({ path: screenshot });
  const provenance = {
    runId: randomUUID(),
    sourceRevision: git(["rev-parse", "HEAD"]) ?? "unknown",
    sourceDirty: (git(["status", "--porcelain"]) ?? "unknown") !== "",
    host: "web",
    capturePhase: `${scenario}-pointer-transport`,
    adapter: after.diagnostics,
    viewport: after.viewport,
    report: after.frame,
  };
  await writeFile(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  console.log(JSON.stringify({ screenshot, provenancePath, frame: after.frame }, null, 2));
  console.log(`PHYSBOX5_VALIDATION_SUCCESS host=web run_id=${provenance.runId}`);
} finally {
  await browser?.close();
  await new Promise((resolveClose) => server.close(resolveClose));
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function git(args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : undefined;
}
