import { randomUUID } from "node:crypto";
import { mkdir, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import {
  closeHttpServer,
  closeManagedBrowser,
  launchManagedBrowser,
} from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const captureDirectory = resolve(
  process.env.PHYSBOX_SHOWCASE_CAPTURE_DIRECTORY ?? "/tmp",
);
const screenshot = join(captureDirectory, "physbox-showcase.png");
const provenancePath = join(captureDirectory, "physbox-showcase.json");
await mkdir(captureDirectory, { recursive: true });
run(process.execPath, [join(root, "scripts", "build-showcase-web.mjs")]);

const server = await startServer(join(root, "target"), 0, {
  crossOriginIsolation: false,
});
let browserSession;
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
  const page = await browserSession.browser.newPage({
    viewport: { width: 1512, height: 800 },
    deviceScaleFactor: 2,
  });
  const errors = [];
  const consoleProblems = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error" || message.type() === "warning") {
      consoleProblems.push(`${message.type()}: ${message.text()}`);
    }
  });
  const port = server.address().port;
  const response = await page.goto(
    `http://127.0.0.1:${port}/showcase-web/#playground`,
    { waitUntil: "networkidle" },
  );
  if (!response?.ok()) {
    throw new Error(`showcase returned HTTP ${response?.status() ?? "unknown"}`);
  }
  const headers = response.headers();
  if (
    headers["cross-origin-opener-policy"] ||
    headers["cross-origin-embedder-policy"]
  ) {
    throw new Error(`showcase unexpectedly requires isolation: ${JSON.stringify(headers)}`);
  }
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.ready &&
      globalThis.__physboxShowcase.fixedSteps >= 2,
    null,
    { timeout: 30_000 },
  );
  let state = await page.evaluate(() => globalThis.__physboxShowcase);
  if (
    state.scene !== "playground" ||
    state.crossOriginIsolated !== false ||
    state.liveBodies !== 21 ||
    state.bodyCapacity !== 256 ||
    state.waterAvailable !== false
  ) {
    throw new Error(`invalid playground state: ${JSON.stringify(state)}`);
  }
  const bounds = await page.locator("#showcase-canvas").boundingBox();
  if (!bounds || bounds.y > 120 || bounds.width < 900 || bounds.height < 500) {
    throw new Error(
      `showcase canvas is not above-fold and materially visible: ${JSON.stringify(bounds)}`,
    );
  }
  if (!(await page.locator('#tool-select option[value="water"]').isDisabled())) {
    throw new Error("rigid-only playground unexpectedly enables the Water tool");
  }
  const point = (normalizedX, normalizedY) => ({
    x: bounds.x + bounds.width * normalizedX,
    y: bounds.y + bounds.height * normalizedY,
  });
  await page.locator("#tool-select").selectOption("box");
  await page.mouse.click(point(0.50, 0.20).x, point(0.50, 0.20).y);
  await page.waitForFunction(() => globalThis.__physboxShowcase?.liveBodies === 22);

  await page.locator("#body-capacity-select").selectOption("64");
  await page.waitForFunction(() => globalThis.__physboxShowcase?.configDirty);
  state = await page.evaluate(() => globalThis.__physboxShowcase);
  if (
    state.bodyCapacity !== 256 ||
    !(await page.locator("#config-state").textContent()).includes("pending")
  ) {
    throw new Error(`capacity changed before Reset: ${JSON.stringify(state)}`);
  }
  await page.locator("#reset-button").click();
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.bodyCapacity === 64 &&
      globalThis.__physboxShowcase.liveBodies === 21 &&
      !globalThis.__physboxShowcase.configDirty,
  );

  await page.locator("#body-capacity-select").selectOption("256");
  await page.locator("#scene-select").selectOption("stack-and-sleep");
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "stack-and-sleep" &&
      globalThis.__physboxShowcase.liveBodies === 15 &&
      globalThis.__physboxShowcase.bodyCapacity === 256 &&
      globalThis.__physboxShowcase.fixedSteps >= 2,
  );
  const fixedBeforePause = await page.evaluate(
    () => globalThis.__physboxShowcase.fixedSteps,
  );
  await page.locator("#pause-button").click();
  await page.waitForTimeout(150);
  const fixedDuringPause = await page.evaluate(
    () => globalThis.__physboxShowcase.fixedSteps,
  );
  await page.waitForTimeout(150);
  const fixedAfterPause = await page.evaluate(
    () => globalThis.__physboxShowcase.fixedSteps,
  );
  if (fixedAfterPause !== fixedDuringPause || fixedAfterPause < fixedBeforePause) {
    throw new Error(
      `pause did not stop fixed steps: ${fixedBeforePause}/${fixedDuringPause}/${fixedAfterPause}`,
    );
  }
  await page.locator("#pause-button").click();
  await page.waitForFunction(
    (pausedStep) => globalThis.__physboxShowcase.fixedSteps > pausedStep,
    fixedAfterPause,
  );

  await page.locator("#reset-button").click();
  const currentBounds = await page.locator("#showcase-canvas").boundingBox();
  if (!currentBounds) throw new Error("showcase canvas disappeared");
  const currentPoint = (normalizedX, normalizedY) => ({
    x: currentBounds.x + currentBounds.width * normalizedX,
    y: currentBounds.y + currentBounds.height * normalizedY,
  });
  await page.locator("#tool-select").selectOption("drag");
  await page.mouse.move(currentPoint(0.5, 0.25).x, currentPoint(0.5, 0.25).y);
  await page.mouse.down();
  await page.waitForFunction(() => globalThis.__physboxShowcase?.dragging, null, {
    timeout: 5_000,
  });
  await page.mouse.move(currentPoint(0.66, 0.3).x, currentPoint(0.66, 0.3).y, {
    steps: 8,
  });
  await page.mouse.up();
  await page.waitForFunction(() => !globalThis.__physboxShowcase?.dragging, null, {
    timeout: 5_000,
  });

  await page.locator("#scene-select").selectOption("water-and-rigid");
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "water-and-rigid" &&
      globalThis.__physboxShowcase.particleCount > 2_000 &&
      globalThis.__physboxShowcase.fixedSteps >= 2,
    null,
    { timeout: 30_000 },
  );
  const waterBefore = await page.evaluate(
    () => globalThis.__physboxShowcase.particleCount,
  );
  await page.locator("#tool-select").selectOption("water");
  const waterBounds = await page.locator("#showcase-canvas").boundingBox();
  if (!waterBounds) throw new Error("water canvas disappeared");
  await page.mouse.move(
    waterBounds.x + waterBounds.width * 0.70,
    waterBounds.y + waterBounds.height * 0.30,
  );
  await page.mouse.down();
  await page.mouse.move(
    waterBounds.x + waterBounds.width * 0.78,
    waterBounds.y + waterBounds.height * 0.36,
    { steps: 10 },
  );
  await page.mouse.up();
  await page.waitForFunction(
    (initial) => globalThis.__physboxShowcase?.particleCount > initial,
    waterBefore,
    { timeout: 15_000 },
  );

  await page.locator("#water-capacity-select").selectOption("8192");
  await page.locator("#physics-select").selectOption("120");
  await page.waitForFunction(() => globalThis.__physboxShowcase?.configDirty);
  await page.locator("#reset-button").click();
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.particleCapacity === 8192 &&
      globalThis.__physboxShowcase.physicsHz === 120 &&
      globalThis.__physboxShowcase.frames > 40 &&
      globalThis.__physboxShowcase.effectiveSimulationScale < 0.75 &&
      (!globalThis.__physboxShowcase.gpuTimingAvailable ||
        globalThis.__physboxShowcase.gpuFrameMs !== null),
    null,
    { timeout: 30_000 },
  );
  state = await page.evaluate(() => globalThis.__physboxShowcase);
  if (
    state.fixedStepsThisFrame > 1 ||
    state.admittedPhysicsHz > 100 ||
    !state.waterAvailable ||
    (state.gpuTimingAvailable && !Number.isFinite(state.gpuFrameMs)) ||
    (!state.gpuTimingAvailable && state.gpuFrameMs !== null)
  ) {
    throw new Error(`invalid 120 Hz slow-time state: ${JSON.stringify(state)}`);
  }

  await page.locator("#scene-select").selectOption("rigid-stress");
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "rigid-stress" &&
      globalThis.__physboxShowcase.liveBodies === 230,
    null,
    { timeout: 30_000 },
  );
  state = await page.evaluate(() => globalThis.__physboxShowcase);
  if (state.waterAvailable || state.tool !== "drag") {
    throw new Error(`rigid stress tool state is invalid: ${JSON.stringify(state)}`);
  }
  await page.locator("#body-capacity-select").selectOption("64");
  await page.locator("#reset-button").click();
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "rigid-stress" &&
      globalThis.__physboxShowcase.bodyCapacity === 64 &&
      globalThis.__physboxShowcase.liveBodies === 57,
  );
  await page.locator("#tool-select").selectOption("box");
  const stressBounds = await page.locator("#showcase-canvas").boundingBox();
  if (!stressBounds) throw new Error("rigid stress canvas disappeared");
  for (let liveBodies = 58; liveBodies <= 64; liveBodies += 1) {
    await page.mouse.click(
      stressBounds.x + stressBounds.width * (0.25 + (liveBodies - 58) * 0.07),
      stressBounds.y + stressBounds.height * 0.12,
    );
    await page.waitForFunction(
      (expected) => globalThis.__physboxShowcase?.liveBodies === expected,
      liveBodies,
    );
  }
  await page.mouse.click(
    stressBounds.x + stressBounds.width * 0.75,
    stressBounds.y + stressBounds.height * 0.12,
  );
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.liveBodies === 64 &&
      globalThis.__physboxShowcase.toolNotice?.includes("capacity is full"),
  );

  await page.locator("#scene-select").selectOption("water-stress");
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "water-stress" &&
      globalThis.__physboxShowcase.particleCapacity === 8192 &&
      globalThis.__physboxShowcase.particleCount >= 6400,
    null,
    { timeout: 30_000 },
  );
  const status = await page.locator("#status-text").textContent();
  if (
    !status.includes("FPS") ||
    !status.includes("physics") ||
    !status.includes("main") ||
    !status.includes("bodies") ||
    !status.includes("water")
  ) {
    throw new Error(`showcase diagnostic line is incomplete: ${status}`);
  }
  state = await page.evaluate(() => globalThis.__physboxShowcase);
  if (state.fixedSteps < 30 && state.gpuFrameMs !== null) {
    throw new Error(`GPU timing leaked across scene reset: ${JSON.stringify(state)}`);
  }
  await page.screenshot({ path: screenshot, fullPage: true });

  state = await page.evaluate(() => globalThis.__physboxShowcase);
  if (errors.length || consoleProblems.length) {
    throw new Error(
      `browser problems: page=${errors.join("; ")} console=${consoleProblems.join("; ")}`,
    );
  }
  const wasm = await stat(
    join(root, "target", "showcase-web", "pkg", "physbox_showcase_web_bg.wasm"),
  );
  const provenance = {
    runId: randomUUID(),
    host: "browser-webgpu",
    urlPath: "/showcase-web/",
    crossOriginIsolated: state.crossOriginIsolated,
    finalState: state,
    wasmBytes: wasm.size,
    screenshot,
  };
  await writeFile(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  console.log(
    `PHYSBOX_SHOWCASE_SUCCESS run=${provenance.runId} scene=${state.scene} steps=${state.fixedSteps} wasmBytes=${wasm.size}`,
  );
  console.log(`showcase capture: ${screenshot}`);
  console.log(`showcase provenance: ${provenancePath}`);
} finally {
  await closeManagedBrowser(browserSession);
  await closeHttpServer(server);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env: process.env,
  });
  if (result.status !== 0) {
    throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
  }
}
