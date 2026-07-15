import { randomUUID } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { chromium } from "@playwright/test";
import { PNG } from "pngjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const captureSuffix = process.env.PHYSBOX5_CAPTURE_SUFFIX
  ? `-${process.env.PHYSBOX5_CAPTURE_SUFFIX.replaceAll(/[^a-zA-Z0-9_-]/g, "")}`
  : "";
const captureDirectory = resolve(process.env.PHYSBOX5_CAPTURE_DIRECTORY ?? "/tmp");
const screenshot = join(captureDirectory, `physbox5-web${captureSuffix}.png`);
const menuScreenshot = join(captureDirectory, `physbox5-web-menu${captureSuffix}.png`);
const grabScreenshot = join(captureDirectory, `physbox5-web-grab${captureSuffix}.png`);
const provenancePath = join(captureDirectory, `physbox5-web${captureSuffix}.json`);
const scenario = process.env.PHYSBOX5_SCENARIO ?? "bootstrap";
const viewportWidth = Number.parseInt(process.env.PHYSBOX5_VIEWPORT_WIDTH ?? "960", 10);
const viewportHeight = Number.parseInt(process.env.PHYSBOX5_VIEWPORT_HEIGHT ?? "600", 10);
if (!(viewportWidth > 0 && viewportHeight > 0)) throw new Error("browser viewport must be positive");
await mkdir(captureDirectory, { recursive: true });
await rm(screenshot, { force: true });
await rm(menuScreenshot, { force: true });
await rm(grabScreenshot, { force: true });
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
  const page = await browser.newPage({ viewport: { width: viewportWidth, height: viewportHeight } });
  const errors = [];
  const consoleErrors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error" || message.type() === "warning") {
      consoleErrors.push(`${message.type()}: ${message.text()}`);
    }
  });
  await page.goto(`http://127.0.0.1:${port}/?scenario=${encodeURIComponent(scenario)}`, { waitUntil: "networkidle" });
  await page.waitForFunction(() => globalThis.__physbox5?.ready, null, { timeout: 30_000 });
  const before = await page.evaluate(() => globalThis.__physbox5);
  if (!before.ok || !before.frame?.scenario?.passed) throw new Error(`shared scenario failed: ${JSON.stringify(before)}`);
  if (before.frame.scenario.name !== scenario) throw new Error(`wrong shared scenario: ${JSON.stringify(before.frame.scenario)}`);
  const bounds = await page.locator("#physbox-canvas").boundingBox();
  if (!bounds || bounds.x !== 0 || bounds.y !== 0 || bounds.width !== viewportWidth || bounds.height !== viewportHeight) {
    throw new Error(`canvas does not fill viewport: ${JSON.stringify(bounds)}`);
  }
  const uiScale = Math.max(1, Math.min(2, Math.min(viewportWidth, viewportHeight) / 600));
  await page.mouse.click(62 * uiScale, 50 * uiScale);
  await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
  await page.screenshot({ path: menuScreenshot });
  const scenarioIndex = [
    "bootstrap",
    "rigid-floor",
    "rigid-stack",
    "body-free-flight",
    "reset-grab",
    "solver-jacobi",
    "solver-colored",
  ].indexOf(scenario);
  if (scenarioIndex < 0) throw new Error(`scenario is absent from shared menu: ${scenario}`);
  const compactLandscape = viewportHeight < 520 && viewportWidth > viewportHeight;
  const scenarioColumn = compactLandscape ? scenarioIndex % 2 : 0;
  const scenarioRow = compactLandscape ? Math.floor(scenarioIndex / 2) : scenarioIndex;
  await page.mouse.click(
    (192 + scenarioColumn * 238) * uiScale,
    (143 + scenarioRow * 38) * uiScale,
  );
  await page.waitForFunction(
    ({ name, activations }) => {
      const frame = globalThis.__physbox5?.frame;
      return frame?.sceneContract?.scenario === name
        && frame?.ui?.scenarioActivationsTotal > activations
        && !frame?.ui?.menuOpen;
    },
    { name: scenario, activations: before.frame.ui.scenarioActivationsTotal },
    { timeout: 10_000 },
  );
  let capturedGrab = null;
  if (process.env.PHYSBOX5_GRAB_CAPTURE === "1") {
    if (scenario !== "rigid-floor") {
      throw new Error("PHYSBOX5_GRAB_CAPTURE currently requires rigid-floor");
    }
    const beforeGrab = await page.evaluate(() => globalThis.__physbox5.frame);
    await page.mouse.move(viewportWidth * 0.5, viewportHeight * 0.175);
    await page.mouse.down();
    await page.mouse.move(viewportWidth * 0.75, viewportHeight * 0.35, { steps: 4 });
    await page.waitForFunction(
      ({ queued, executed }) => {
        const commands = globalThis.__physbox5?.frame?.bodyCommands;
        return commands?.grabPointerActive
          && commands?.queuedCommandsTotal > queued
          && commands?.executedCommandsTotal > executed;
      },
      {
        queued: beforeGrab.bodyCommands.queuedCommandsTotal,
        executed: beforeGrab.bodyCommands.executedCommandsTotal,
      },
      { timeout: 10_000 },
    );
    await page.screenshot({ path: grabScreenshot });
    capturedGrab = await page.evaluate(() => globalThis.__physbox5.frame);
    if (capturedGrab.bodyCommands.gpuSelectionReadback) {
      throw new Error("interactive grab reported a forbidden GPU selection readback");
    }
    if (
      capturedGrab.bodyCommands.grabTetherPrepareDispatchesThisFrame !== 1
      || capturedGrab.bodyCommands.grabTetherDrawCallsThisFrame !== 1
    ) {
      throw new Error(`interactive grab omitted bounded tether work: ${JSON.stringify(capturedGrab.bodyCommands)}`);
    }
    requireGrabEndpoint(
      PNG.sync.read(await readFile(grabScreenshot)),
      Math.round(viewportWidth * 0.75),
      Math.round(viewportHeight * 0.35),
    );
    await page.mouse.up();
    await page.waitForFunction(
      () => !globalThis.__physbox5?.frame?.bodyCommands?.grabPointerActive,
      null,
      { timeout: 10_000 },
    );
  }
  await page.mouse.move(viewportWidth * 0.5, viewportHeight * 0.5);
  await page.mouse.down();
  await page.mouse.up();
  await page.waitForFunction(
    (count) => globalThis.__physbox5?.frame?.pointerEventsTotal > count,
    before.frame.pointerEventsTotal,
    { timeout: 10_000 },
  );
  await page.mouse.down({ button: "right" });
  await page.mouse.move(viewportWidth * 0.6, viewportHeight * 0.4);
  await page.mouse.up({ button: "right" });
  await page.mouse.wheel(0, -100);
  await page.waitForFunction(
    ({ pan, zoom }) => {
      const camera = globalThis.__physbox5?.frame?.camera;
      return camera?.panChangesTotal > pan && camera?.zoomChangesTotal > zoom;
    },
    {
      pan: before.frame.camera.panChangesTotal,
      zoom: before.frame.camera.zoomChangesTotal,
    },
    { timeout: 10_000 },
  );
  const after = await page.evaluate(() => globalThis.__physbox5);
  if (after.frame.fixedStepsThisFrame > 1) {
    throw new Error(`interactive input admitted catch-up steps: ${JSON.stringify(after.frame.pacing)}`);
  }
  if (after.frame.camera.center[0] === 0 && after.frame.camera.center[1] === 0) {
    throw new Error(`right-button drag did not pan shared camera: ${JSON.stringify(after.frame.camera)}`);
  }
  if (!(after.frame.camera.orthographicHalfHeight < 1)) {
    throw new Error(`wheel did not zoom shared camera: ${JSON.stringify(after.frame.camera)}`);
  }
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  await page.waitForFunction(
    (name) => globalThis.__physbox5?.frame?.scenario?.name === name
      && globalThis.__physbox5?.frame?.scenario?.passed,
    scenario,
    { timeout: 10_000 },
  );
  await page.waitForTimeout(200);
  await page.screenshot({ path: screenshot });
  const final = await page.evaluate(() => globalThis.__physbox5);
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  const pixels = PNG.sync.read(await readFile(screenshot));
  let blackPixels = 0;
  for (let offset = 0; offset < pixels.data.length; offset += 4) {
    if (pixels.data[offset] <= 3 && pixels.data[offset + 1] <= 3 && pixels.data[offset + 2] <= 3) {
      blackPixels += 1;
    }
  }
  if (blackPixels > pixels.width * pixels.height * 0.01) {
    throw new Error(
      `browser screenshot contains ${blackPixels} unexpected black pixels; console=${consoleErrors.join("; ")}`,
    );
  }
  const provenance = {
    runId: randomUUID(),
    sourceRevision: git(["rev-parse", "HEAD"]) ?? "unknown",
    sourceDirty: (git(["status", "--porcelain"]) ?? "unknown") !== "",
    host: "web",
    capturePhase: `${scenario}-pointer-transport`,
    adapter: final.diagnostics,
    viewport: final.viewport,
    menuScreenshot,
    grabScreenshot: capturedGrab ? grabScreenshot : null,
    grabFrame: capturedGrab,
    report: final.frame,
  };
  await writeFile(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  console.log(JSON.stringify({ screenshot, menuScreenshot, grabScreenshot: capturedGrab ? grabScreenshot : null, provenancePath, frame: final.frame }, null, 2));
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

function requireGrabEndpoint(image, targetX, targetY) {
  let pixels = 0;
  for (let y = targetY - 16; y <= targetY + 16; y += 1) {
    for (let x = targetX - 16; x <= targetX + 16; x += 1) {
      const offset = (y * image.width + x) * 4;
      const [red, green, blue] = image.data.subarray(offset, offset + 3);
      pixels += Number(red > 120 && green > 180 && green > blue + 5 && green > red + 20);
    }
  }
  if (pixels < 20) throw new Error(`grab endpoint pixel gate failed: endpoint_pixels=${pixels}`);
}
