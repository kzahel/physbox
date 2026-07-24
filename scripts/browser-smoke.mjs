import { randomUUID } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { PNG } from "pngjs";
import { closeHttpServer, closeManagedBrowser, launchManagedBrowser } from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const captureSuffix = process.env.PHYSBOX5_CAPTURE_SUFFIX
  ? `-${process.env.PHYSBOX5_CAPTURE_SUFFIX.replaceAll(/[^a-zA-Z0-9_-]/g, "")}`
  : "";
const captureDirectory = resolve(process.env.PHYSBOX5_CAPTURE_DIRECTORY ?? "/tmp");
const screenshot = join(captureDirectory, `physbox5-web${captureSuffix}.png`);
const initialScreenshot = join(captureDirectory, `physbox5-web-initial${captureSuffix}.png`);
const menuScreenshot = join(captureDirectory, `physbox5-web-menu${captureSuffix}.png`);
const scenesMenuScreenshot = join(captureDirectory, `physbox5-web-scenes-menu${captureSuffix}.png`);
const worldMenuScreenshot = join(captureDirectory, `physbox5-web-world-menu${captureSuffix}.png`);
const solverMenuScreenshot = join(captureDirectory, `physbox5-web-solver-menu${captureSuffix}.png`);
const sleepScreenshot = join(captureDirectory, `physbox5-web-sleep${captureSuffix}.png`);
const renderScaleScreenshot = join(captureDirectory, `physbox5-web-render-scale-50${captureSuffix}.png`);
const box2dScreenshot = join(captureDirectory, `physbox5-web-box2d${captureSuffix}.png`);
const solverScreenshot = join(captureDirectory, `physbox5-web-solver${captureSuffix}.png`);
const grabScreenshot = join(captureDirectory, `physbox5-web-grab${captureSuffix}.png`);
const constructionScreenshot = join(captureDirectory, `physbox5-web-construction${captureSuffix}.png`);
const liquidInteractionScreenshot = join(captureDirectory, `physbox5-web-liquid-tools${captureSuffix}.png`);
const provenancePath = join(captureDirectory, `physbox5-web${captureSuffix}.json`);
const scenario = process.env.PHYSBOX5_SCENARIO ?? "bootstrap";
const requestedBodyCapacity = Number.parseInt(process.env.PHYSBOX5_BODY_CAPACITY ?? "64", 10);
if (![64, 256, 1000, 10000].includes(requestedBodyCapacity)) {
  throw new Error("PHYSBOX5_BODY_CAPACITY must be 64, 256, 1000, or 10000");
}
const solverLab = process.env.PHYSBOX5_SOLVER_LAB === "1";
const conciseOutput = process.env.PHYSBOX5_CONCISE === "1";
const viewportWidth = Number.parseInt(process.env.PHYSBOX5_VIEWPORT_WIDTH ?? "960", 10);
const viewportHeight = Number.parseInt(process.env.PHYSBOX5_VIEWPORT_HEIGHT ?? "600", 10);
const deviceScaleFactor = Number.parseFloat(process.env.PHYSBOX5_DEVICE_SCALE_FACTOR ?? "1");
if (!(viewportWidth > 0 && viewportHeight > 0 && deviceScaleFactor > 0)) {
  throw new Error("browser viewport and device scale factor must be positive");
}
await mkdir(captureDirectory, { recursive: true });
await rm(screenshot, { force: true });
await rm(initialScreenshot, { force: true });
await rm(menuScreenshot, { force: true });
await rm(scenesMenuScreenshot, { force: true });
await rm(worldMenuScreenshot, { force: true });
await rm(solverMenuScreenshot, { force: true });
await rm(sleepScreenshot, { force: true });
await rm(renderScaleScreenshot, { force: true });
await rm(box2dScreenshot, { force: true });
await rm(solverScreenshot, { force: true });
await rm(grabScreenshot, { force: true });
await rm(constructionScreenshot, { force: true });
await rm(liquidInteractionScreenshot, { force: true });
await rm(provenancePath, { force: true });
run(process.execPath, [
  resolve(root, "scripts/build-web.mjs"),
  ...(solverLab ? ["--solver-lab"] : []),
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
  const port = server.address().port;
  const page = await browser.newPage({
    viewport: { width: viewportWidth, height: viewportHeight },
    deviceScaleFactor,
  });
  const errors = [];
  const consoleErrors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error" || message.type() === "warning") {
      consoleErrors.push(`${message.type()}: ${message.text()}`);
    }
  });
  const startupParams = new URLSearchParams({ scenario });
  startupParams.set("smoke-observer", "1");
  if (requestedBodyCapacity !== 64) {
    startupParams.set("body-capacity", String(requestedBodyCapacity));
  }
  await page.goto(`http://127.0.0.1:${port}/?${startupParams}`, { waitUntil: "networkidle" });
  try {
    await page.waitForFunction(() => globalThis.__physbox5?.ready, null, { timeout: 30_000 });
  } catch (error) {
    const fatal = await page.locator("#fatal-error").textContent();
    throw new Error(
      `browser startup did not become ready: ${error.message}; fatal=${fatal}; page=${errors.join("; ")}; console=${consoleErrors.join("; ")}`,
    );
  }
  const before = await page.evaluate(() => globalThis.__physbox5);
  if (!before.ok || !before.frame?.scenario?.passed) throw new Error(`shared scenario failed: ${JSON.stringify(before)}`);
  if (before.frame.scenario.name !== scenario) throw new Error(`wrong shared scenario: ${JSON.stringify(before.frame.scenario)}`);
  if (before.frame.fixedStepsTotal < 1) throw new Error(`browser scenario passed without a fixed step: ${JSON.stringify(before.frame.pacing)}`);
  if (before.frame.sceneContract?.solverProfile !== "soft-coupled") {
    throw new Error(`fresh browser did not use soft-coupled: ${JSON.stringify(before.frame.sceneContract)}`);
  }
  if (before.frame.ui?.version !== 12 || before.frame.ui?.sleepVisualsEnabled !== false) {
    throw new Error(`browser did not expose UI contract 12 defaults: ${JSON.stringify(before.frame.ui)}`);
  }
  if (
    before.frame.renderScale?.requestedPercent !== 100
    || before.frame.renderScale?.path !== "direct"
    || before.frame.renderScale?.intermediateColorBytes !== 0
    || JSON.stringify(before.frame.renderScale?.presentationSize) !== JSON.stringify(before.frame.targetSize)
    || JSON.stringify(before.frame.renderScale?.worldRenderSize) !== JSON.stringify(before.frame.targetSize)
  ) {
    throw new Error(`browser did not start on the native render path: ${JSON.stringify(before.frame.renderScale)}`);
  }
  if (
    before.frame.ui?.selectedEngine !== "physbox"
    || before.frame.ui?.comparatorAvailable !== true
    || before.frame.ui?.box2dRevision !== "3a4f0da8374af61293a03021c9a0b3ebcfe67948"
    || before.frame.ui?.box2dFailure !== null
  ) {
    throw new Error(`browser comparator did not start in the default PhysBox view: ${JSON.stringify(before.frame.ui)}`);
  }
  if (
    before.frame.sceneContract?.physicsConfigurationVersion !== 3
    || before.frame.sceneContract?.sleepPolicy !== "experimental-s23-v1"
    || before.frame.sceneContract?.sleepSpeedThreshold !== 0.02
    || before.frame.sceneContract?.sleepGrouping !== "exact"
    || before.frame.sceneContract?.box2dSleepSpeedThreshold !== 0.02
    || before.frame.sceneContract?.requestedSleepingEnabled !== true
    || before.frame.sceneContract?.effectiveSleepingEnabled !== true
    || before.frame.sceneContract?.awakeCompactionEnabled !== true
  ) {
    throw new Error(`browser sleep checkpoint contract is incomplete: ${JSON.stringify(before.frame.sceneContract)}`);
  }
  if (before.frame.sceneContract?.requestedBodyCapacity !== requestedBodyCapacity) {
    throw new Error(`browser did not use requested world preset ${requestedBodyCapacity}: ${JSON.stringify(before.frame.sceneContract)}`);
  }
  await requirePhysicsPreference(page, {
    solver: "soft-coupled",
    simHz: 60,
    substeps: 4,
    iterations: 1,
    contactHz: 30,
    contactDamping: 10,
    staticContactScale: 2,
    speculativeDistance: 0.02,
    maxDepenetrationVelocity: 3,
  }, true);
  await requirePresentationPreference(page, false);
  requireNoGpuValidationErrors(consoleErrors, "browser initial fixed-step presentation");
  await captureCompleteScreenshot(page, initialScreenshot, "browser initial no-input", consoleErrors);
  const bounds = await page.locator("#physbox-canvas").boundingBox();
  if (!bounds || bounds.x !== 0 || bounds.y !== 0 || bounds.width !== viewportWidth || bounds.height !== viewportHeight) {
    throw new Error(`canvas does not fill viewport: ${JSON.stringify(bounds)}`);
  }
  const pixelsPerPoint = before.frame.ui.pixelsPerPoint;
  const toolbarScale = before.frame.ui.toolbarScale / pixelsPerPoint;
  const uiScale = before.frame.ui.menuScale / pixelsPerPoint;
  if (
    !Number.isFinite(pixelsPerPoint)
    || pixelsPerPoint <= 0
    || !Number.isFinite(toolbarScale)
    || toolbarScale <= 0
    || !Number.isFinite(uiScale)
    || uiScale <= 0
  ) {
    throw new Error(`browser UI density contract is invalid: ${JSON.stringify(before.frame.ui)}`);
  }
  const compactLandscape = viewportHeight < 520 && viewportWidth > viewportHeight;
  const phonePortrait = viewportWidth < 520 && viewportHeight >= viewportWidth;
  const logicalWidth = viewportWidth / toolbarScale;
  const toolStride = logicalWidth < 460
    ? Math.max(1, (logicalWidth - 78) / 6)
    : 58;
  const toolWidth = logicalWidth < 460 ? Math.max(1, toolStride - 6) : 52;
  const toolCenter = (index) => [
    (70 + index * toolStride + toolWidth * 0.5) * toolbarScale,
    50 * toolbarScale,
  ];
  const menuButtonCenter = [38 * toolbarScale, 50 * toolbarScale];
  const comparatorLiveBodiesBefore = before.frame.ui.box2dLiveBodies;
  await page.mouse.click(...menuButtonCenter);
  await page.mouse.click(300 * uiScale, 104 * uiScale);
  await page.mouse.click(291 * uiScale, 135 * uiScale);
  await page.waitForFunction(
    () => {
      const ui = globalThis.__physbox5?.frame?.ui;
      return ui?.selectedEngine === "box2d"
        && ui?.box2dAligned === true
        && ui?.box2dFailure === null;
    },
    null,
    { timeout: 10_000 },
  );
  await captureCompleteScreenshot(page, box2dScreenshot, "browser Box2D view", consoleErrors);
  if (scenario === "bootstrap") {
    const constructionBeforeComparator = await page.evaluate(
      () => globalThis.__physbox5.frame.bodyCommands.executedConstructionIntentsTotal,
    );
    await page.mouse.click(...toolCenter(1));
    await page.mouse.click(viewportWidth * 0.80, viewportHeight * 0.20);
    try {
      await page.waitForFunction(
        ({ constructionBefore, liveBodiesBefore }) => {
          const frame = globalThis.__physbox5?.frame;
          return frame?.bodyCommands?.executedConstructionIntentsTotal > constructionBefore
            && frame?.ui?.box2dPendingActions === 0
            && frame?.ui?.box2dLiveBodies > liveBodiesBefore
            && frame?.ui?.box2dIdentityDisagreements === 0
            && frame?.ui?.box2dFailure === null;
        },
        {
          constructionBefore: constructionBeforeComparator,
          liveBodiesBefore: comparatorLiveBodiesBefore,
        },
        { timeout: 10_000 },
      );
    } catch (error) {
      const frame = await page.evaluate(() => globalThis.__physbox5?.frame);
      const fatal = await page.locator("#fatal-error").textContent();
      throw new Error(
        `browser comparator action did not reconcile: ${error.message}; frame=${JSON.stringify(frame)}; fatal=${fatal}; page=${errors.join("; ")}; console=${consoleErrors.join("; ")}`,
      );
    }
  }
  const box2dFrame = await page.evaluate(() => globalThis.__physbox5.frame);
  await page.mouse.click(...toolCenter(0));
  await page.mouse.click(...menuButtonCenter);
  await page.mouse.click(300 * uiScale, 104 * uiScale);
  await page.mouse.click(169 * uiScale, 135 * uiScale);
  await page.waitForFunction(
    () => globalThis.__physbox5?.frame?.ui?.selectedEngine === "physbox"
      && globalThis.__physbox5?.frame?.ui?.box2dFailure === null,
    null,
    { timeout: 10_000 },
  );
  if (scenario === "bootstrap") {
    const activations = await page.evaluate(
      () => globalThis.__physbox5.frame.ui.scenarioActivationsTotal,
    );
    await page.mouse.click(...menuButtonCenter);
    await page.mouse.click(76 * uiScale, 104 * uiScale);
    await page.mouse.click(192 * uiScale, 143 * uiScale);
    await page.waitForFunction(
      (previousActivations) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.scenario === "bootstrap"
          && frame?.ui?.scenarioActivationsTotal > previousActivations
          && frame?.ui?.selectedEngine === "physbox"
          && frame?.ui?.box2dAligned === true
          && frame?.ui?.box2dFailure === null
          && !frame?.ui?.menuOpen;
      },
      activations,
      { timeout: 10_000 },
    );
  }
  const solverProfileCenter = (index) => {
    const columns = compactLandscape ? 2 : 1;
    const column = index % columns;
    const row = Math.floor(index / columns);
    const width = columns === 1 ? 336 : 286;
    const startY = compactLandscape ? 144 : 154;
    const stride = compactLandscape ? 30 : 38;
    const height = compactLandscape ? 28 : 34;
    return [
      (24 + column * 298 + width / 2) * uiScale,
      (startY + row * stride + height / 2) * uiScale,
    ];
  };
  const solverParameterPosition = (index, normalized) => {
    const baseY = compactLandscape ? 176 : 236;
    const column = compactLandscape ? index % 2 : 0;
    const row = compactLandscape ? Math.floor(index / 2) : index;
    const stride = compactLandscape ? 30 : phonePortrait ? 58 : 38;
    const height = compactLandscape ? 28 : phonePortrait ? 52 : 34;
    const menuLogicalWidth = viewportWidth / uiScale;
    const x = compactLandscape
      ? 160 + column * 298
      : phonePortrait
        ? Math.min(235, Math.max(166, menuLogicalWidth * 0.45))
        : 166;
    const width = compactLandscape ? 128 : phonePortrait ? menuLogicalWidth - x - 24 : 194;
    return [
      (x + width * normalized) * uiScale,
      (baseY + row * stride + height / 2) * uiScale,
    ];
  };
  const solverSceneCenter = (index) => [
    (105 + index * 174) * uiScale,
    (compactLandscape ? 282 : phonePortrait ? 620 : 491) * uiScale,
  ];
  const solverApplyCenter = [
    279 * uiScale,
    (compactLandscape ? 316 : phonePortrait ? 679 : 541) * uiScale,
  ];
  const worldSleepingCenter = [
    (compactLandscape ? 496 : 192) * uiScale,
    (compactLandscape ? 171 : 331) * uiScale,
  ];
  const worldCapacity64Center = [192 * uiScale, 171 * uiScale];
  const worldSleepThresholdCenter = [
    (compactLandscape ? 496 : 192) * uiScale,
    (compactLandscape ? 217 : 377) * uiScale,
  ];
  const worldSleepGroupingCenter = [
    (compactLandscape ? 496 : 192) * uiScale,
    (compactLandscape ? 263 : 423) * uiScale,
  ];
  const worldSleepVisualsCenter = [
    (compactLandscape ? 496 : 192) * uiScale,
    (compactLandscape ? 309 : 469) * uiScale,
  ];
  const worldRenderScaleCenter = [
    192 * uiScale,
    (compactLandscape ? 331 : 553) * uiScale,
  ];
  const clickWorldControl = async (center) => {
    await page.mouse.click(...menuButtonCenter);
    await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
    await page.mouse.click(300 * uiScale, 104 * uiScale);
    await page.mouse.click(...center);
  };
  const initialPhysicsHash = before.frame.sceneContract.physicsConfigurationHash;
  const initialCamera = before.frame.camera;
  await clickWorldControl(worldRenderScaleCenter);
  await page.waitForFunction(
    () => {
      const report = globalThis.__physbox5?.frame?.renderScale;
      return report?.requestedPercent === 75
        && report.path === "linear-composite"
        && report.intermediateColorBytes > 0
        && report.worldRenderSize[0] === Math.ceil(report.presentationSize[0] * 0.75)
        && report.worldRenderSize[1] === Math.ceil(report.presentationSize[1] * 0.75);
    },
    null,
    { timeout: 30_000 },
  );
  await requirePresentationPreference(page, false, 75);
  await page.reload({ waitUntil: "networkidle" });
  await page.waitForFunction(
    () => globalThis.__physbox5?.ready
      && globalThis.__physbox5?.frame?.renderScale?.requestedPercent === 75
      && globalThis.__physbox5?.frame?.renderScale?.path === "linear-composite",
    null,
    { timeout: 30_000 },
  );
  const resizedViewport = { width: viewportWidth + 37, height: viewportHeight + 19 };
  await page.setViewportSize(resizedViewport);
  await page.waitForFunction(
    ({ originalWidth, originalHeight }) => {
      const frame = globalThis.__physbox5?.frame;
      const report = frame?.renderScale;
      return report?.requestedPercent === 75
        && report.presentationSize[0] !== originalWidth
        && report.presentationSize[1] !== originalHeight
        && report.worldRenderSize[0] === Math.ceil(report.presentationSize[0] * 0.75)
        && report.worldRenderSize[1] === Math.ceil(report.presentationSize[1] * 0.75)
        && JSON.stringify(frame.targetSize) === JSON.stringify(report.presentationSize);
    },
    {
      originalWidth: before.frame.targetSize[0],
      originalHeight: before.frame.targetSize[1],
    },
    { timeout: 30_000 },
  );
  await page.setViewportSize({ width: viewportWidth, height: viewportHeight });
  await page.waitForFunction(
    (nativeSize) =>
      JSON.stringify(globalThis.__physbox5?.frame?.renderScale?.presentationSize)
        === JSON.stringify(nativeSize),
    before.frame.targetSize,
    { timeout: 30_000 },
  );
  await clickWorldControl(worldRenderScaleCenter);
  await page.waitForFunction(
    () => {
      const report = globalThis.__physbox5?.frame?.renderScale;
      return report?.requestedPercent === 50
        && report.path === "linear-composite"
        && report.worldRenderSize[0] === Math.ceil(report.presentationSize[0] * 0.5)
        && report.worldRenderSize[1] === Math.ceil(report.presentationSize[1] * 0.5);
    },
    null,
    { timeout: 30_000 },
  );
  await requirePresentationPreference(page, false, 50);
  await captureCompleteScreenshot(page, renderScaleScreenshot, "browser 50-percent render scale", consoleErrors);
  const scaledFrame = await page.evaluate(() => globalThis.__physbox5.frame);
  if (
    scaledFrame.sceneContract.physicsConfigurationHash !== initialPhysicsHash
    || JSON.stringify(scaledFrame.camera) !== JSON.stringify(initialCamera)
  ) {
    throw new Error(`render scale changed physics or camera: ${JSON.stringify(scaledFrame)}`);
  }
  await clickWorldControl(worldRenderScaleCenter);
  await page.waitForFunction(
    () => {
      const report = globalThis.__physbox5?.frame?.renderScale;
      return report?.requestedPercent === 100
        && report.path === "direct"
        && report.intermediateColorBytes === 0;
    },
    null,
    { timeout: 30_000 },
  );
  await requirePresentationPreference(page, false, 100);
  await page.mouse.click(...menuButtonCenter);
  await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
  await page.evaluate(() => new Promise((resolveFrame) => {
    requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
  }));
  await captureCompleteScreenshot(page, menuScreenshot, "browser menu");
  await captureCompleteScreenshot(page, scenesMenuScreenshot, "browser scenes menu");
  let activeScenario = scenario;
  if (!solverLab && scenario === "bootstrap" && requestedBodyCapacity === 64) {
    const restarts = before.frame.ui.scenarioRestartsTotal;
    await page.mouse.click(300 * uiScale, 104 * uiScale);
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    await captureCompleteScreenshot(page, menuScreenshot, "browser world-capacity menu");
    await captureCompleteScreenshot(page, worldMenuScreenshot, "browser world-capacity menu");
    await page.mouse.click(192 * uiScale, 209 * uiScale);
    await page.waitForFunction(
      ({ previousRestarts }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.requestedBodyCapacity === 256
          && frame?.sceneContract?.bodyCapacity === 256
          && frame?.ui?.scenarioRestartsTotal > previousRestarts
          && !frame?.ui?.menuOpen;
      },
      { previousRestarts: restarts },
      { timeout: 30_000 },
    );
    await requireWorldPreference(page, 256);
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForFunction(
      () => globalThis.__physbox5?.ready
        && globalThis.__physbox5?.frame?.sceneContract?.requestedBodyCapacity === 256
        && globalThis.__physbox5?.frame?.sceneContract?.bodyCapacity === 256,
      null,
      { timeout: 30_000 },
    );
    await requireWorldPreference(page, 256);
    await page.mouse.click(...menuButtonCenter);
    await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
    await page.mouse.click(300 * uiScale, 104 * uiScale);
    await page.mouse.click(...worldSleepingCenter);
    try {
      await page.waitForFunction(
        () => {
          const frame = globalThis.__physbox5?.frame;
          return frame?.sceneContract?.requestedSleepingEnabled === false
            && frame?.sceneContract?.effectiveSleepingEnabled === false
            && !frame?.ui?.menuOpen;
        },
        null,
        { timeout: 30_000 },
      );
    } catch (error) {
      const state = await page.evaluate(() => globalThis.__physbox5);
      throw new Error(`sleep opt-out did not apply: ${error.message}; state=${JSON.stringify(state)}`);
    }
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 60,
      substeps: 4,
      iterations: 1,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
      sleeping: false,
      sleepGrouping: "exact",
    });
    await clickWorldControl(worldSleepingCenter);
    await page.waitForFunction(
      () => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.requestedSleepingEnabled === true
          && frame?.sceneContract?.effectiveSleepingEnabled === true
          && !frame?.ui?.menuOpen;
      },
      null,
      { timeout: 30_000 },
    );
    await clickWorldControl(worldSleepThresholdCenter);
    try {
      await page.waitForFunction(
        () => globalThis.__physbox5?.frame?.sceneContract?.sleepSpeedThreshold === 0.05
          && globalThis.__physbox5?.frame?.sceneContract?.box2dSleepSpeedThreshold === 0.05
          && !globalThis.__physbox5?.frame?.ui?.menuOpen,
        null,
        { timeout: 30_000 },
      );
    } catch (error) {
      const state = await page.evaluate(() => globalThis.__physbox5);
      throw new Error(`shared sleep threshold did not apply: ${error.message}; state=${JSON.stringify(state)}`);
    }
    await clickWorldControl(worldSleepGroupingCenter);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.sceneContract?.sleepGrouping === "local-4"
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      null,
      { timeout: 30_000 },
    );
    await clickWorldControl(worldCapacity64Center);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.sceneContract?.requestedBodyCapacity === 64
        && globalThis.__physbox5?.frame?.sceneContract?.bodyCapacity === 64
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      null,
      { timeout: 30_000 },
    );
    await clickWorldControl(worldSleepGroupingCenter);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.sceneContract?.sleepGrouping === "local-8"
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      null,
      { timeout: 30_000 },
    );
    await clickWorldControl(worldSleepGroupingCenter);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.sceneContract?.sleepGrouping === "exact"
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      null,
      { timeout: 30_000 },
    );
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 60,
      substeps: 4,
      iterations: 1,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
      sleeping: true,
      sleepThreshold: 0.05,
      sleepGrouping: "exact",
    });
    await page.reload({ waitUntil: "networkidle" });
    try {
      await page.waitForFunction(
        () => globalThis.__physbox5?.ready
          && globalThis.__physbox5?.ok
          && globalThis.__physbox5?.frame?.sceneContract?.requestedSleepingEnabled === true
          && globalThis.__physbox5?.frame?.sceneContract?.effectiveSleepingEnabled === true
          && globalThis.__physbox5?.frame?.sceneContract?.sleepSpeedThreshold === 0.05
          && globalThis.__physbox5?.frame?.sceneContract?.sleepGrouping === "exact",
        null,
        { timeout: 30_000 },
      );
    } catch (error) {
      const state = await page.evaluate(() => globalThis.__physbox5);
      throw new Error(`persisted exact sleep configuration failed: ${error.message}; state=${JSON.stringify(state)}`);
    }
    await page.waitForFunction(
      () => {
        const diagnostics = globalThis.__physbox5?.frame?.sleepCycleDiagnostic;
        const physbox = diagnostics?.state;
        const box2d = diagnostics?.comparisonState;
        return diagnostics?.source === "asynchronous-gpu"
          && physbox?.version === 1
          && physbox.mixedAwakeAsleepContactCuts !== null
          && physbox.dynamicBodies === physbox.awakeBodies + physbox.sleepingBodies
          && box2d?.version === 1
          && box2d.mixedAwakeAsleepContactCuts === null;
      },
      null,
      { timeout: 30_000 },
    );
    await page.mouse.click(...menuButtonCenter);
    await page.mouse.click(300 * uiScale, 104 * uiScale);
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    try {
    await captureCompleteScreenshot(page, menuScreenshot, "browser sleeping request on menu");
    await captureCompleteScreenshot(page, worldMenuScreenshot, "browser sleeping request on menu");
    } catch (error) {
      const state = await page.evaluate(() => globalThis.__physbox5);
      throw new Error(`${error.message}; state=${JSON.stringify(state)}`);
    }
    await page.mouse.click(...worldSleepVisualsCenter);
    await page.waitForFunction(
      () => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.ui?.sleepVisualsEnabled === true
          && frame?.sceneContract?.requestedSleepingEnabled === true
          && frame?.sceneContract?.effectiveSleepingEnabled === true
          && !frame?.ui?.menuOpen;
      },
      null,
      { timeout: 30_000 },
    );
    await requirePresentationPreference(page, true);
    await page.waitForTimeout(750);
    await captureCompleteScreenshot(page, sleepScreenshot, "browser sleep-state visualization");
    for (const threshold of [0.1, 0.01, 0.02]) {
      await clickWorldControl(worldSleepThresholdCenter);
      await page.waitForFunction(
        ({ expectedThreshold }) =>
          globalThis.__physbox5?.frame?.sceneContract?.sleepSpeedThreshold === expectedThreshold
          && !globalThis.__physbox5?.frame?.ui?.menuOpen,
        { expectedThreshold: threshold },
        { timeout: 30_000 },
      );
    }
    await clickWorldControl(worldSleepGroupingCenter);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.sceneContract?.sleepGrouping === "local-4"
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      null,
      { timeout: 30_000 },
    );
    await page.mouse.click(...menuButtonCenter);
    await page.mouse.click(300 * uiScale, 104 * uiScale);
    await page.mouse.click(...worldSleepingCenter);
    await page.waitForFunction(
      () => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.requestedSleepingEnabled === false
          && frame?.sceneContract?.effectiveSleepingEnabled === false
          && frame?.ui?.sleepVisualsEnabled === true
          && !frame?.ui?.menuOpen;
      },
      null,
      { timeout: 30_000 },
    );
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 60,
      substeps: 4,
      iterations: 1,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
      sleeping: false,
      sleepGrouping: "local-4",
    });
    await page.mouse.click(...menuButtonCenter);
    await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
    await page.mouse.click(300 * uiScale, 104 * uiScale);
    await page.mouse.click(...worldSleepVisualsCenter);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.sleepVisualsEnabled === false
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      null,
      { timeout: 10_000 },
    );
    await requirePresentationPreference(page, false);
    await page.mouse.click(...menuButtonCenter);
    await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
    await page.mouse.click(76 * uiScale, 104 * uiScale);
  }
  if (solverLab) {
    await page.mouse.click(188 * uiScale, 104 * uiScale);
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    await page.mouse.click(...solverProfileCenter(0));
    await page.mouse.click(...solverParameterPosition(0, (75 - 15) / (240 - 15)));
    await page.mouse.click(...solverParameterPosition(1, (5 - 1) / (8 - 1)));
    await page.mouse.click(...solverParameterPosition(2, (2 - 1) / (4 - 1)));
    await page.mouse.click(...solverParameterPosition(5, (2.5 - 0.5) / (16 - 0.5)));
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    await captureCompleteScreenshot(page, menuScreenshot, "browser solver menu");
    await captureCompleteScreenshot(page, solverMenuScreenshot, "browser solver menu");
    await page.mouse.click(...solverSceneCenter(1));
    activeScenario = "solver-lab-stack-16";
    try {
      await page.waitForFunction(
        ({ scenarioName }) => {
          const frame = globalThis.__physbox5?.frame;
          return frame?.sceneContract?.scenario === scenarioName && !frame?.ui?.menuOpen;
        },
        { scenarioName: activeScenario },
        { timeout: 10_000 },
      );
    } catch (error) {
      const state = await page.evaluate(() => globalThis.__physbox5);
      throw new Error(`configured solver scene did not activate: ${JSON.stringify(state)}; console=${consoleErrors.join("; ")}`, { cause: error });
    }
    const appliedPhysics = await page.evaluate(() => globalThis.__physbox5?.frame?.sceneContract);
    if (
      appliedPhysics?.solverProfile !== "soft-coupled"
      || appliedPhysics?.solverProfileOptions !== "hz=30,damping=10,static-scale=2.5"
      || appliedPhysics?.fixedStepHz !== 75
      || appliedPhysics?.solverSubsteps !== 5
      || appliedPhysics?.solverBiasedIterations !== 2
      || appliedPhysics?.generatedContactCoefficientPolicy !== "authority-tunable-soft"
    ) throw new Error(`wrong applied physics configuration: ${JSON.stringify(appliedPhysics)}`);
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 75,
      substeps: 5,
      iterations: 2,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2.5,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
    });

    await page.mouse.click(...menuButtonCenter);
    await page.mouse.click(188 * uiScale, 104 * uiScale);
    await page.mouse.click(...solverProfileCenter(1));
    await page.mouse.click(...solverApplyCenter);
    await page.waitForFunction(
      ({ scenarioName }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.scenario === scenarioName
          && frame?.sceneContract?.solverProfile === "hard-direct"
          && !frame?.ui?.menuOpen;
      },
      { scenarioName: activeScenario },
      { timeout: 10_000 },
    );
    await requirePhysicsPreference(page, {
      solver: "hard-direct",
      simHz: 60,
      substeps: 4,
      iterations: 2,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
    });

    await page.mouse.click(...menuButtonCenter);
    await page.mouse.click(188 * uiScale, 104 * uiScale);
    await page.mouse.click(...solverProfileCenter(0));
    await page.mouse.click(...solverApplyCenter);
    await page.waitForFunction(
      ({ scenarioName }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.scenario === scenarioName
          && frame?.sceneContract?.solverProfile === "soft-coupled"
          && !frame?.ui?.menuOpen;
      },
      { scenarioName: activeScenario },
      { timeout: 10_000 },
    );
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 75,
      substeps: 5,
      iterations: 2,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2.5,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
    });
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    await captureCompleteScreenshot(page, solverScreenshot, "browser solver candidate");

    await page.evaluate(() => {
      const url = new URL(location.href);
      for (const key of [...url.searchParams.keys()]) {
        if (key !== "scenario" && key !== "smoke-observer") url.searchParams.delete(key);
      }
      history.replaceState(null, "", url);
    });
    await page.reload({ waitUntil: "networkidle" });
    try {
      await page.waitForFunction(
        ({ initialScenario }) => {
          const frame = globalThis.__physbox5?.frame;
          return globalThis.__physbox5?.ready
            && frame?.sceneContract?.scenario === initialScenario
            && frame?.sceneContract?.solverProfile === "soft-coupled"
            && frame?.sceneContract?.solverProfileOptions === "hz=30,damping=10,static-scale=2.5";
        },
        { initialScenario: scenario },
        { timeout: 30_000 },
      );
    } catch (error) {
      const state = await page.evaluate(() => globalThis.__physbox5);
      const fatal = await page.locator("#fatal-error").textContent();
      const documentState = await page.evaluate(() => ({
        href: location.href,
        readyState: document.readyState,
        fatalHidden: document.querySelector("#fatal-error")?.hidden,
        resources: performance.getEntriesByType("resource").map((entry) => ({
          name: entry.name,
          duration: entry.duration,
          transferSize: entry.transferSize,
        })),
      }));
      throw new Error(
        `persisted solver reload did not become ready: ${error.message}; state=${JSON.stringify(state)}; fatal=${fatal}; document=${JSON.stringify(documentState)}; page=${errors.join("; ")}; console=${consoleErrors.join("; ")}`,
      );
    }
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 75,
      substeps: 5,
      iterations: 2,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 2.5,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
    });
    await page.evaluate(() => {
      localStorage.removeItem("physbox5.physics.v5");
      localStorage.removeItem("physbox5.physics.v4");
      localStorage.removeItem("physbox5.physics.v3");
      localStorage.removeItem("physbox5.physics.v2");
      localStorage.setItem("physbox5.solver-lab.v1", JSON.stringify({
        version: 1,
        solver: "branch-robust",
        options: "hz=30,damping=10,static-scale=8",
      }));
      const url = new URL(location.href);
      for (const key of [...url.searchParams.keys()]) {
        if (key !== "scenario" && key !== "smoke-observer") url.searchParams.delete(key);
      }
      history.replaceState(null, "", url);
    });
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForFunction(
      ({ initialScenario }) => {
        const frame = globalThis.__physbox5?.frame;
        return globalThis.__physbox5?.ready
          && frame?.sceneContract?.scenario === initialScenario
          && frame?.sceneContract?.solverProfile === "soft-coupled"
          && frame?.sceneContract?.solverProfileOptions === "hz=30,damping=10,static-scale=8";
      },
      { initialScenario: scenario },
      { timeout: 30_000 },
    );
    await requirePhysicsPreference(page, {
      solver: "soft-coupled",
      simHz: 60,
      substeps: 4,
      iterations: 1,
      contactHz: 30,
      contactDamping: 10,
      staticContactScale: 8,
      speculativeDistance: 0.02,
      maxDepenetrationVelocity: 3,
    });
    activeScenario = scenario;
  }
  const menuScenarios = [
    "bootstrap",
    "rigid-floor",
    "rigid-stack",
    "liquid-sandbox",
    "rectangle-stack",
    "circle-bucket-100",
    "rectangle-bucket-100",
    "body-free-flight",
    "reset-grab",
    "solver-jacobi",
    "solver-colored",
    "solver-manifold-i2",
  ];
  const launchOnlyScenarios = new Set([
    "solver-parallel-coloring",
    "settling-hard-8",
    "settling-candidate-8",
    "settling-hard-16",
    "settling-identity-16",
    "settling-softness-16",
    "settling-candidate-16",
  ]);
  const scenarioIndex = menuScenarios.indexOf(scenario);
  if (solverLab) {
    // The solver flow above already exercised scene activation, closed the menu, and reloaded.
  } else if (scenarioIndex >= 0) {
    const compactLandscape = viewportHeight < 520 && viewportWidth > viewportHeight;
    const logicalWidth = viewportWidth / uiScale;
    const scenarioColumns = compactLandscape ? 3 : logicalWidth >= 632 ? 2 : 1;
    const scenarioColumn = scenarioIndex % scenarioColumns;
    const scenarioRow = Math.floor(scenarioIndex / scenarioColumns);
    const scenarioCenterX = scenarioColumns === 3
      ? 118 + scenarioColumn * 198
      : scenarioColumns === 2
        ? 167 + scenarioColumn * 298
        : 192;
    await page.mouse.click(76 * uiScale, 104 * uiScale);
    await page.mouse.click(
      scenarioCenterX * uiScale,
      (143 + scenarioRow * 38) * uiScale,
    );
    await page.waitForFunction(
      ({ name, activations, restarts }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.sceneContract?.scenario === name
          && (
            frame?.ui?.scenarioActivationsTotal > activations
            || frame?.ui?.scenarioRestartsTotal > restarts
          )
          && !frame?.ui?.menuOpen;
      },
      {
        name: scenario,
        activations: before.frame.ui.scenarioActivationsTotal,
        restarts: before.frame.ui.scenarioRestartsTotal,
      },
      { timeout: 10_000 },
    );
  } else if (launchOnlyScenarios.has(scenario)) {
    await page.mouse.click(...menuButtonCenter);
    await page.waitForFunction(
      (name) => globalThis.__physbox5?.frame?.sceneContract?.scenario === name
        && !globalThis.__physbox5?.frame?.ui?.menuOpen,
      scenario,
      { timeout: 10_000 },
    );
  } else {
    throw new Error(`scenario is absent from shared launch and menu contracts: ${scenario}`);
  }
  let capturedGrab = null;
  let capturedConstruction = null;
  let capturedLiquidInteraction = null;
  if (process.env.PHYSBOX5_CONSTRUCTION_CAPTURE === "1") {
    if (scenario !== "bootstrap") {
      throw new Error("PHYSBOX5_CONSTRUCTION_CAPTURE currently requires bootstrap");
    }
    const beforeConstruction = await page.evaluate(() => globalThis.__physbox5.frame);
    await page.mouse.click(...toolCenter(1));
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "circle",
      null,
      { timeout: 10_000 },
    );
    await page.mouse.move(viewportWidth * 0.68, viewportHeight * 0.35);
    await page.mouse.down();
    await page.waitForFunction(
      ({ executed }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.bodyCommands?.executedConstructionIntentsTotal > executed;
      },
      { executed: beforeConstruction.bodyCommands.executedConstructionIntentsTotal },
      { timeout: 10_000 },
    );
    await page.mouse.move(viewportWidth * 0.82, viewportHeight * 0.35, { steps: 4 });
    await page.waitForFunction(
      ({ executed }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.bodyCommands?.executedConstructionIntentsTotal >= executed + 2
          && frame?.constructionDiagnostic?.state?.attemptedIntents >= 2;
      },
      { executed: beforeConstruction.bodyCommands.executedConstructionIntentsTotal },
      { timeout: 10_000 },
    );
    await page.mouse.up();
    capturedConstruction = await page.evaluate(() => globalThis.__physbox5.frame);
    if (
      capturedConstruction.constructionDiagnostic.source !== "async-gpu"
      || capturedConstruction.constructionDiagnostic.state.lastResult !== "placed"
      || capturedConstruction.constructionDiagnostic.state.liveBodies !== 4
      || capturedConstruction.constructionDiagnostic.ageFrames === null
      || capturedConstruction.constructionDiagnostic.blockingReadbackForAdvancement
      || capturedConstruction.ui.diagnosticSource !== "async-gpu"
    ) {
      throw new Error(`browser construction diagnostic is invalid: ${JSON.stringify(capturedConstruction)}`);
    }
    await page.screenshot({ path: constructionScreenshot });
    await page.mouse.click(...toolCenter(0));
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "grab",
      null,
      { timeout: 10_000 },
    );
  }
  if (scenario === "liquid-sandbox") {
    const beforeLiquid = await page.evaluate(() => globalThis.__physbox5.frame);
    const target = [viewportWidth * 0.68, viewportHeight * 0.34];

    await page.mouse.click(...toolCenter(4));
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "water",
      null,
      { timeout: 10_000 },
    );
    await page.mouse.click(...target);
    await page.waitForFunction(
      (executed) => globalThis.__physbox5?.frame?.bodyCommands
        ?.executedParticleCommandsTotal > executed,
      beforeLiquid.bodyCommands.executedParticleCommandsTotal,
      { timeout: 10_000 },
    );

    const afterWater = await page.evaluate(() => globalThis.__physbox5.frame);
    await page.mouse.click(...toolCenter(3));
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "erase",
      null,
      { timeout: 10_000 },
    );
    await page.mouse.move(...target);
    await page.mouse.down();
    await page.mouse.move(viewportWidth * 0.74, viewportHeight * 0.32, { steps: 4 });
    await page.waitForFunction(
      (executed) => globalThis.__physbox5?.frame?.bodyCommands
        ?.executedParticleCommandsTotal >= executed + 2,
      afterWater.bodyCommands.executedParticleCommandsTotal,
      { timeout: 10_000 },
    );
    await page.mouse.up();

    const beforeSurface = await page.evaluate(() => globalThis.__physbox5.frame);
    await page.mouse.click(...toolCenter(5));
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "surface",
      null,
      { timeout: 10_000 },
    );
    await page.mouse.move(viewportWidth * 0.56, viewportHeight * 0.48);
    await page.mouse.down();
    await page.mouse.move(viewportWidth * 0.80, viewportHeight * 0.40, { steps: 4 });
    await captureCompleteScreenshot(
      page,
      liquidInteractionScreenshot,
      "browser liquid Surface preview",
      consoleErrors,
    );
    await page.mouse.up();
    await page.waitForFunction(
      (executed) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.bodyCommands?.executedConstructionIntentsTotal > executed
          && frame?.constructionDiagnostic?.state?.lastResult === "placed"
          && frame?.constructionDiagnostic?.blockingReadbackForAdvancement === false;
      },
      beforeSurface.bodyCommands.executedConstructionIntentsTotal,
      { timeout: 10_000 },
    );
    capturedLiquidInteraction = await page.evaluate(() => globalThis.__physbox5.frame);
    if (
      capturedLiquidInteraction.bodyCommands.executedParticleCommandsTotal
        < beforeLiquid.bodyCommands.executedParticleCommandsTotal + 3
      || capturedLiquidInteraction.sceneContract.particleCount !== 3952
      || capturedLiquidInteraction.sceneContract.particleCapacity !== 8192
      || capturedLiquidInteraction.bodyCommands.overflowed
      || capturedLiquidInteraction.sceneContract.particleReadbackForFrameAdvancement
      || capturedLiquidInteraction.ui.box2dIdentityDisagreements !== 0
      || capturedLiquidInteraction.ui.box2dFailure !== null
    ) {
      throw new Error(`browser liquid tools violated their bounded contract: ${JSON.stringify(capturedLiquidInteraction)}`);
    }
  }
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
  if (
    after.frame.camera.center[0] === before.frame.camera.center[0]
    && after.frame.camera.center[1] === before.frame.camera.center[1]
  ) {
    throw new Error(`right-button drag did not pan shared camera: ${JSON.stringify(after.frame.camera)}`);
  }
  if (!(after.frame.camera.orthographicHalfHeight < before.frame.camera.orthographicHalfHeight)) {
    throw new Error(`wheel did not zoom shared camera: ${JSON.stringify(after.frame.camera)}`);
  }
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  requireNoGpuValidationErrors(consoleErrors, "browser final presentation");
  await page.waitForFunction(
    (name) => globalThis.__physbox5?.frame?.scenario?.name === name
      && globalThis.__physbox5?.frame?.scenario?.passed,
    activeScenario,
    { timeout: solverLab || scenario === "liquid-sandbox" ? 30_000 : 10_000 },
  );
  await page.waitForTimeout(200);
  await captureCompleteScreenshot(page, screenshot, "browser final", consoleErrors);
  const final = await page.evaluate(() => globalThis.__physbox5);
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
  await requireOpaqueProductionPage(browser, `http://127.0.0.1:${port}/?scenario=bootstrap`);
  await requireComparatorStartupFailure(browser, `http://127.0.0.1:${port}/?scenario=bootstrap`);
  const provenance = {
    runId: randomUUID(),
    sourceRevision: git(["rev-parse", "HEAD"]) ?? "unknown",
    sourceDirty: (git(["status", "--porcelain"]) ?? "unknown") !== "",
    host: "web",
    capturePhase: `${activeScenario}-pointer-transport`,
    adapter: final.diagnostics,
    viewport: final.viewport,
    initialScreenshot,
    menuScreenshot,
    scenesMenuScreenshot,
    worldMenuScreenshot,
    solverMenuScreenshot: solverLab ? solverMenuScreenshot : null,
    sleepScreenshot,
    renderScaleScreenshot,
    box2dScreenshot,
    box2dFrame,
    solverScreenshot: solverLab ? solverScreenshot : null,
    grabScreenshot: capturedGrab ? grabScreenshot : null,
    grabFrame: capturedGrab,
    constructionScreenshot: capturedConstruction ? constructionScreenshot : null,
    constructionFrame: capturedConstruction,
    liquidInteractionScreenshot: capturedLiquidInteraction ? liquidInteractionScreenshot : null,
    liquidInteractionFrame: capturedLiquidInteraction,
    report: final.frame,
  };
  await writeFile(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  const output = conciseOutput
    ? {
      provenancePath,
      scenario: final.frame.scenario,
      physics: {
        profile: final.frame.sceneContract?.solverProfile,
        configurationHash: final.frame.sceneContract?.physicsConfigurationHash,
        fixedStepHz: final.frame.sceneContract?.fixedStepHz,
        substeps: final.frame.sceneContract?.solverSubsteps,
        iterations: final.frame.sceneContract?.solverBiasedIterations,
      },
      world: {
        requestedBodyCapacity: final.frame.sceneContract?.requestedBodyCapacity,
        bodyCapacity: final.frame.sceneContract?.bodyCapacity,
      },
      pacing: {
        completedSteps: final.frame.pacing?.completedOuterStepsTotal,
        effectiveScale: final.frame.pacing?.effectiveSimulationScale,
        slippedSeconds: final.frame.pacing?.overloadSlippedWallSecondsTotal,
      },
    }
    : {
      initialScreenshot,
      screenshot,
      menuScreenshot,
      scenesMenuScreenshot,
      worldMenuScreenshot,
      solverMenuScreenshot: solverLab ? solverMenuScreenshot : null,
      sleepScreenshot,
      renderScaleScreenshot,
      box2dScreenshot,
      grabScreenshot: capturedGrab ? grabScreenshot : null,
      constructionScreenshot: capturedConstruction ? constructionScreenshot : null,
      liquidInteractionScreenshot: capturedLiquidInteraction ? liquidInteractionScreenshot : null,
      provenancePath,
      frame: final.frame,
    };
  console.log(JSON.stringify(output, null, 2));
  console.log(`PHYSBOX5_VALIDATION_SUCCESS host=web run_id=${provenance.runId}`);
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

async function captureCompleteScreenshot(page, path, label, consoleErrors = []) {
  let blackPixels = 0;
  let foregroundPixels = 0;
  let pixelCount = 0;
  for (let attempt = 0; attempt < 5; attempt += 1) {
    await page.screenshot({ path });
    const pixels = PNG.sync.read(await readFile(path));
    blackPixels = 0;
    foregroundPixels = 0;
    pixelCount = pixels.width * pixels.height;
    for (let offset = 0; offset < pixels.data.length; offset += 4) {
      if (pixels.data[offset] <= 3 && pixels.data[offset + 1] <= 3 && pixels.data[offset + 2] <= 3) {
        blackPixels += 1;
      }
      if (Math.max(pixels.data[offset], pixels.data[offset + 1], pixels.data[offset + 2]) >= 48) {
        foregroundPixels += 1;
      }
    }
    if (blackPixels <= pixelCount * 0.01 && foregroundPixels >= pixelCount * 0.05) return;
    await page.waitForTimeout(100);
  }
  throw new Error(
    `${label} is incomplete after five captures: black=${blackPixels}/${pixelCount} foreground=${foregroundPixels}/${pixelCount}; console=${consoleErrors.join("; ")}`,
  );
}

function requireNoGpuValidationErrors(messages, label) {
  const failures = messages.filter((message) =>
    /invalid (?:shader|compute|render|command)|while (?:calling|encoding).*create(?:shader|compute|render)|must only be called from uniform control flow|queue\].submit/i.test(message)
  );
  if (failures.length) throw new Error(`${label} reported WebGPU validation errors: ${failures.join("; ")}`);
}

async function requireOpaqueProductionPage(browser, url) {
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  try {
    await page.goto(url, { waitUntil: "networkidle" });
    await page.waitForTimeout(500);
    const state = await page.evaluate(() => ({
      observerState: typeof globalThis.__physbox5,
      frameControl: typeof globalThis.__physbox5FrameControl,
      fatalHidden: document.querySelector("#fatal-error")?.hidden,
    }));
    if (
      state.observerState !== "undefined"
      || state.frameControl !== "undefined"
      || state.fatalHidden !== true
    ) {
      throw new Error(`ordinary browser page exposed smoke state: ${JSON.stringify(state)}`);
    }
    const pixels = PNG.sync.read(await page.screenshot());
    let foregroundPixels = 0;
    for (let offset = 0; offset < pixels.data.length; offset += 4) {
      if (Math.max(pixels.data[offset], pixels.data[offset + 1], pixels.data[offset + 2]) >= 48) {
        foregroundPixels += 1;
      }
    }
    if (foregroundPixels < pixels.width * pixels.height * 0.05) {
      throw new Error("ordinary browser page did not render the shared frame");
    }
  } finally {
    await page.close();
  }
}

async function requireComparatorStartupFailure(browser, url) {
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  try {
    await page.route("**/box2d-comparator/box2d-comparator.mjs", async (route) => {
      await route.fulfill({
        contentType: "text/javascript",
        body: "export default async function createMismatchedComparator() { return {}; }\n",
      });
    });
    await page.goto(url, { waitUntil: "networkidle" });
    await page.waitForFunction(
      () => document.querySelector("#fatal-error")?.hidden === false,
      null,
      { timeout: 10_000 },
    );
    const message = await page.locator("#fatal-error").textContent();
    if (!message?.includes("Box2D comparator runtime is missing _malloc")) {
      throw new Error(`mismatched comparator did not fail explicitly: ${message}`);
    }
  } finally {
    await page.close();
  }
}

async function requirePhysicsPreference(page, expected, expectClean = false) {
  await page.waitForFunction(
    ({ expectedPhysics, clean }) => {
      const saved = JSON.parse(localStorage.getItem("physbox5.physics.v5") ?? "null");
      const url = new URL(location.href);
      const expectedSleeping = expectedPhysics.sleeping ?? true;
      const expectedSleepThreshold = expectedPhysics.sleepThreshold ?? 0.02;
      const expectedSleepGrouping = expectedPhysics.sleepGrouping ?? "exact";
      const stored = saved?.version === 5
        && saved.sleeping === expectedSleeping
        && saved.sleepThreshold === expectedSleepThreshold
        && saved.sleepGrouping === expectedSleepGrouping
        && Object.entries(expectedPhysics).every(([key, value]) => saved[key] === value);
      const keys = [
        "solver",
        "sim-hz",
        "substeps",
        "iterations",
        "contact-hz",
        "contact-damping",
        "static-contact-scale",
        "speculative-distance",
        "max-depenetration-velocity",
        "sleeping",
        "sleep-threshold",
        "sleep-grouping",
        "solver-options",
      ];
      const addressed = clean
        ? keys.every((key) => !url.searchParams.has(key))
        : url.searchParams.get("solver") === expectedPhysics.solver
          && url.searchParams.get("sim-hz") === String(expectedPhysics.simHz)
          && url.searchParams.get("substeps") === String(expectedPhysics.substeps)
          && url.searchParams.get("iterations") === String(expectedPhysics.iterations)
          && url.searchParams.get("contact-hz") === String(expectedPhysics.contactHz)
          && url.searchParams.get("contact-damping") === String(expectedPhysics.contactDamping)
          && url.searchParams.get("static-contact-scale") === String(expectedPhysics.staticContactScale)
          && url.searchParams.get("speculative-distance") === String(expectedPhysics.speculativeDistance)
          && url.searchParams.get("max-depenetration-velocity") === String(expectedPhysics.maxDepenetrationVelocity)
          && url.searchParams.get("sleeping") === (expectedSleeping ? "on" : "off")
          && url.searchParams.get("sleep-threshold") === String(expectedSleepThreshold)
          && url.searchParams.get("sleep-grouping") === expectedSleepGrouping
          && !url.searchParams.has("solver-options");
      return stored && addressed;
    },
    { expectedPhysics: expected, clean: expectClean },
    { timeout: 10_000 },
  );
}

async function requirePresentationPreference(page, sleepVisualsEnabled, renderScale = 100) {
  await page.waitForFunction(
    ({ expectedSleepVisuals, expectedRenderScale }) => {
      const saved = JSON.parse(localStorage.getItem("physbox5.presentation.v2") ?? "null");
      const url = new URL(location.href);
      return saved?.version === 2
        && saved.sleepVisualsEnabled === expectedSleepVisuals
        && saved.renderScale === String(expectedRenderScale)
        && (expectedSleepVisuals
          ? url.searchParams.get("sleep-visuals") === "on"
          : !url.searchParams.has("sleep-visuals"))
        && (expectedRenderScale === 100
          ? !url.searchParams.has("render-scale")
          : url.searchParams.get("render-scale") === String(expectedRenderScale));
    },
    { expectedSleepVisuals: sleepVisualsEnabled, expectedRenderScale: renderScale },
    { timeout: 10_000 },
  );
}

async function requireWorldPreference(page, bodyCapacity) {
  await page.waitForFunction(
    ({ expectedCapacity }) => {
      const saved = JSON.parse(localStorage.getItem("physbox5.world.v1") ?? "null");
      const url = new URL(location.href);
      return saved?.version === 1
        && saved.bodyCapacity === expectedCapacity
        && (expectedCapacity === 64
          ? !url.searchParams.has("body-capacity")
          : url.searchParams.get("body-capacity") === String(expectedCapacity));
    },
    { expectedCapacity: bodyCapacity },
    { timeout: 10_000 },
  );
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
