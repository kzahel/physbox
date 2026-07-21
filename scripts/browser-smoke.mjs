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
const solverScreenshot = join(captureDirectory, `physbox5-web-solver${captureSuffix}.png`);
const grabScreenshot = join(captureDirectory, `physbox5-web-grab${captureSuffix}.png`);
const constructionScreenshot = join(captureDirectory, `physbox5-web-construction${captureSuffix}.png`);
const provenancePath = join(captureDirectory, `physbox5-web${captureSuffix}.json`);
const scenario = process.env.PHYSBOX5_SCENARIO ?? "bootstrap";
const solverLab = process.env.PHYSBOX5_SOLVER_LAB === "1";
const viewportWidth = Number.parseInt(process.env.PHYSBOX5_VIEWPORT_WIDTH ?? "960", 10);
const viewportHeight = Number.parseInt(process.env.PHYSBOX5_VIEWPORT_HEIGHT ?? "600", 10);
if (!(viewportWidth > 0 && viewportHeight > 0)) throw new Error("browser viewport must be positive");
await mkdir(captureDirectory, { recursive: true });
await rm(screenshot, { force: true });
await rm(initialScreenshot, { force: true });
await rm(menuScreenshot, { force: true });
await rm(solverScreenshot, { force: true });
await rm(grabScreenshot, { force: true });
await rm(constructionScreenshot, { force: true });
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
  if (before.frame.fixedStepsTotal < 1) throw new Error(`browser scenario passed without a fixed step: ${JSON.stringify(before.frame.pacing)}`);
  if (before.frame.sceneContract?.solverProfile !== "soft-coupled") {
    throw new Error(`fresh browser did not use soft-coupled: ${JSON.stringify(before.frame.sceneContract)}`);
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
  requireNoGpuValidationErrors(consoleErrors, "browser initial fixed-step presentation");
  await captureCompleteScreenshot(page, initialScreenshot, "browser initial no-input", consoleErrors);
  const bounds = await page.locator("#physbox-canvas").boundingBox();
  if (!bounds || bounds.x !== 0 || bounds.y !== 0 || bounds.width !== viewportWidth || bounds.height !== viewportHeight) {
    throw new Error(`canvas does not fill viewport: ${JSON.stringify(bounds)}`);
  }
  const uiScale = Math.max(1, Math.min(2, Math.min(viewportWidth, viewportHeight) / 600));
  const compactLandscape = viewportHeight < 520 && viewportWidth > viewportHeight;
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
  const solverParameterPlusCenter = (index) => {
    const baseY = compactLandscape ? 176 : 236;
    const column = compactLandscape ? index % 2 : 0;
    const row = compactLandscape ? Math.floor(index / 2) : index;
    const stride = compactLandscape ? 30 : 38;
    const height = compactLandscape ? 28 : 34;
    return [
      (compactLandscape ? 270 + column * 298 : 334) * uiScale,
      (baseY + row * stride + height / 2) * uiScale,
    ];
  };
  const solverSceneCenter = (index) => [
    (105 + index * 174) * uiScale,
    (compactLandscape ? 282 : 491) * uiScale,
  ];
  const solverApplyCenter = [279 * uiScale, (compactLandscape ? 316 : 541) * uiScale];
  await page.mouse.click(62 * uiScale, 50 * uiScale);
  await page.waitForFunction(() => globalThis.__physbox5?.frame?.ui?.menuOpen, null, { timeout: 10_000 });
  await page.evaluate(() => new Promise((resolveFrame) => {
    requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
  }));
  await captureCompleteScreenshot(page, menuScreenshot, "browser menu");
  let activeScenario = scenario;
  if (solverLab) {
    await page.mouse.click(279 * uiScale, 104 * uiScale);
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    await page.mouse.click(...solverProfileCenter(0));
    await page.mouse.click(...solverParameterPlusCenter(0));
    await page.mouse.click(...solverParameterPlusCenter(1));
    await page.mouse.click(...solverParameterPlusCenter(2));
    await page.mouse.click(...solverParameterPlusCenter(5));
    await page.evaluate(() => new Promise((resolveFrame) => {
      requestAnimationFrame(() => requestAnimationFrame(resolveFrame));
    }));
    await captureCompleteScreenshot(page, menuScreenshot, "browser solver menu");
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

    await page.mouse.click(62 * uiScale, 50 * uiScale);
    await page.mouse.click(279 * uiScale, 104 * uiScale);
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

    await page.mouse.click(62 * uiScale, 50 * uiScale);
    await page.mouse.click(279 * uiScale, 104 * uiScale);
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
        if (key !== "scenario") url.searchParams.delete(key);
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
          && frame?.sceneContract?.solverProfileOptions === "hz=30,damping=10,static-scale=2.5";
      },
      { initialScenario: scenario },
      { timeout: 30_000 },
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
    await page.evaluate(() => {
      localStorage.removeItem("physbox5.physics.v2");
      localStorage.setItem("physbox5.solver-lab.v1", JSON.stringify({
        version: 1,
        solver: "branch-robust",
        options: "hz=30,damping=10,static-scale=8",
      }));
      const url = new URL(location.href);
      for (const key of [...url.searchParams.keys()]) {
        if (key !== "scenario") url.searchParams.delete(key);
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
    const scenarioColumn = compactLandscape ? scenarioIndex % 3 : 0;
    const scenarioRow = compactLandscape ? Math.floor(scenarioIndex / 3) : scenarioIndex;
    await page.mouse.click(
      (118 + scenarioColumn * 198) * uiScale,
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
  } else if (launchOnlyScenarios.has(scenario)) {
    await page.mouse.click(62 * uiScale, 50 * uiScale);
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
  if (process.env.PHYSBOX5_CONSTRUCTION_CAPTURE === "1") {
    if (scenario !== "bootstrap") {
      throw new Error("PHYSBOX5_CONSTRUCTION_CAPTURE currently requires bootstrap");
    }
    const beforeConstruction = await page.evaluate(() => globalThis.__physbox5.frame);
    await page.mouse.click(229 * uiScale, 50 * uiScale);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "circle",
      null,
      { timeout: 10_000 },
    );
    await page.mouse.click(viewportWidth * 0.75, viewportHeight * 0.3);
    await page.waitForFunction(
      ({ executed }) => {
        const frame = globalThis.__physbox5?.frame;
        return frame?.bodyCommands?.executedConstructionIntentsTotal > executed
          && frame?.constructionDiagnostic?.state?.attemptedIntents >= 1;
      },
      { executed: beforeConstruction.bodyCommands.executedConstructionIntentsTotal },
      { timeout: 10_000 },
    );
    capturedConstruction = await page.evaluate(() => globalThis.__physbox5.frame);
    if (
      capturedConstruction.constructionDiagnostic.source !== "async-gpu"
      || capturedConstruction.constructionDiagnostic.state.lastResult !== "placed"
      || capturedConstruction.constructionDiagnostic.state.liveBodies !== 3
      || capturedConstruction.constructionDiagnostic.ageFrames === null
      || capturedConstruction.constructionDiagnostic.blockingReadbackForAdvancement
      || capturedConstruction.ui.diagnosticSource !== "async-gpu"
    ) {
      throw new Error(`browser construction diagnostic is invalid: ${JSON.stringify(capturedConstruction)}`);
    }
    await page.screenshot({ path: constructionScreenshot });
    await page.mouse.click(153 * uiScale, 50 * uiScale);
    await page.waitForFunction(
      () => globalThis.__physbox5?.frame?.ui?.currentTool === "grab",
      null,
      { timeout: 10_000 },
    );
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
  if (after.frame.camera.center[0] === 0 && after.frame.camera.center[1] === 0) {
    throw new Error(`right-button drag did not pan shared camera: ${JSON.stringify(after.frame.camera)}`);
  }
  if (!(after.frame.camera.orthographicHalfHeight < 1)) {
    throw new Error(`wheel did not zoom shared camera: ${JSON.stringify(after.frame.camera)}`);
  }
  if (errors.length) throw new Error(`browser errors: ${errors.join("; ")}`);
  requireNoGpuValidationErrors(consoleErrors, "browser final presentation");
  await page.waitForFunction(
    (name) => globalThis.__physbox5?.frame?.scenario?.name === name
      && globalThis.__physbox5?.frame?.scenario?.passed,
    activeScenario,
    { timeout: solverLab ? 30_000 : 10_000 },
  );
  await page.waitForTimeout(200);
  await captureCompleteScreenshot(page, screenshot, "browser final", consoleErrors);
  const final = await page.evaluate(() => globalThis.__physbox5);
  await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
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
    solverScreenshot: solverLab ? solverScreenshot : null,
    grabScreenshot: capturedGrab ? grabScreenshot : null,
    grabFrame: capturedGrab,
    constructionScreenshot: capturedConstruction ? constructionScreenshot : null,
    constructionFrame: capturedConstruction,
    report: final.frame,
  };
  await writeFile(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  console.log(JSON.stringify({ initialScreenshot, screenshot, menuScreenshot, grabScreenshot: capturedGrab ? grabScreenshot : null, constructionScreenshot: capturedConstruction ? constructionScreenshot : null, provenancePath, frame: final.frame }, null, 2));
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

async function requirePhysicsPreference(page, expected, expectClean = false) {
  await page.waitForFunction(
    ({ expectedPhysics, clean }) => {
      const saved = JSON.parse(localStorage.getItem("physbox5.physics.v2") ?? "null");
      const url = new URL(location.href);
      const stored = saved?.version === 2
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
          && !url.searchParams.has("solver-options");
      return stored && addressed;
    },
    { expectedPhysics: expected, clean: expectClean },
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
