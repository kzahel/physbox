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
  page.setDefaultTimeout(180_000);
  const errors = [];
  const consoleProblems = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error" || message.type() === "warning") {
      consoleProblems.push(`${message.type()}: ${message.text()}`);
    }
  });
  const port = server.address().port;
  const response = await page.goto(`http://127.0.0.1:${port}/showcase-web/`, {
    waitUntil: "networkidle",
  });
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

  await waitForState(
    page,
    (state) =>
      state.ready &&
      state.scene === "water-and-rigid" &&
      state.fixedSteps >= 2 &&
      state.particleCount > 2_000,
  );
  let state = await currentState(page);
  if (
    state.crossOriginIsolated !== false ||
    state.liveBodies !== 8 ||
    state.bodyCapacity !== 256 ||
    state.particleCapacity !== 4096 ||
    !state.waterAvailable
  ) {
    throw new Error(`invalid default water state: ${JSON.stringify(state)}`);
  }
  const physicsOptions = await page
    .locator("#physics-select option")
    .evaluateAll((options) => options.map((option) => option.value));
  if (JSON.stringify(physicsOptions) !== JSON.stringify(["30", "60"])) {
    throw new Error(`unexpected physics options: ${JSON.stringify(physicsOptions)}`);
  }
  if (
    (await page.locator('label[for="body-capacity-select"]').textContent()).trim() !==
      "Max bodies:" ||
    (await page.locator('label[for="water-capacity-select"]').textContent()).trim() !==
      "Max water:" ||
    (await page.locator("footer").count()) !== 0
  ) {
    throw new Error("compact allocation labels or removed footer are incorrect");
  }
  if ((await page.locator(".tool-button").count()) !== 4) {
    throw new Error("showcase does not expose four icon tool buttons");
  }

  const bounds = await canvasBounds(page);
  if (bounds.y > 120 || bounds.width < 900 || bounds.height < 500) {
    throw new Error(
      `showcase canvas is not above-fold and materially visible: ${JSON.stringify(bounds)}`,
    );
  }
  await selectTool(page, "box");
  await clickCanvas(page, 0.5, 0.2);
  await waitForState(page, (candidate) => candidate.liveBodies === 9);

  await selectTool(page, "water");
  await page.locator("#water-capacity-select").selectOption("0");
  await waitForState(page, (candidate) => candidate.configDirty);
  state = await currentState(page);
  if (!state.waterAvailable || state.particleCapacity !== 4096) {
    throw new Error(`water changed before Reset: ${JSON.stringify(state)}`);
  }
  await page.locator("#reset-button").click();
  await waitForState(
    page,
    (candidate) =>
      candidate.particleCapacity === 0 &&
      !candidate.waterAvailable &&
      candidate.tool === "drag" &&
      !candidate.configDirty,
  );

  await page.locator("#scene-select").selectOption("playground");
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "playground" &&
      candidate.liveBodies === 21 &&
      candidate.bodyCapacity === 256 &&
      candidate.particleCapacity === 0 &&
      candidate.fixedSteps >= 2,
  );
  if ((await page.locator("#water-capacity-select").inputValue()) !== "0") {
    throw new Error("dry scene did not select the None water default");
  }
  await page.locator("#water-capacity-select").selectOption("4096");
  await page.locator("#physics-select").selectOption("30");
  await waitForState(page, (candidate) => candidate.configDirty);
  await page.locator("#reset-button").click();
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "playground" &&
      candidate.waterAvailable &&
      candidate.particleCapacity === 4096 &&
      candidate.particleCount === 0 &&
      candidate.physicsHz === 30,
  );
  const waterBefore = (await currentState(page)).particleCount;
  await selectTool(page, "water");
  await dragCanvas(page, [0.62, 0.25], [0.72, 0.34]);
  await waitForState(
    page,
    (candidate, initial) => candidate.particleCount > initial,
    waterBefore,
  );

  const beforePause = (await currentState(page)).fixedSteps;
  await page.locator("#pause-button").click();
  await page.waitForTimeout(180);
  const duringPause = (await currentState(page)).fixedSteps;
  await page.waitForTimeout(180);
  const afterPause = (await currentState(page)).fixedSteps;
  if (afterPause !== duringPause || afterPause < beforePause) {
    throw new Error(`pause did not stop fixed steps: ${beforePause}/${duringPause}/${afterPause}`);
  }
  await page.locator("#pause-button").click();
  await waitForState(
    page,
    (candidate, pausedStep) => candidate.fixedSteps > pausedStep,
    afterPause,
  );

  await page.locator("#scene-select").selectOption("rigid-stress-2k");
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "rigid-stress-2k" &&
      candidate.liveBodies === 2_000 &&
      candidate.bodyCapacity === 2_000 &&
      candidate.particleCapacity === 0 &&
      candidate.fixedSteps >= 2,
  );
  if (
    (await page.locator("#body-capacity-select").inputValue()) !== "2000" ||
    !(await page.locator("#tool-water").isDisabled())
  ) {
    throw new Error("2K stress defaults or Water availability are incorrect");
  }

  const cameraBefore = await currentState(page);
  await selectTool(page, "pan");
  await dragCanvas(page, [0.5, 0.5], [0.62, 0.58]);
  await waitForState(
    page,
    (candidate, initial) =>
      Math.abs(candidate.cameraCenterX - initial.cameraCenterX) > 0.1 ||
      Math.abs(candidate.cameraCenterY - initial.cameraCenterY) > 0.1,
    cameraBefore,
  );
  const cameraAfterPan = await currentState(page);
  const stressBounds = await canvasBounds(page);
  await page.mouse.move(
    stressBounds.x + stressBounds.width * 0.5,
    stressBounds.y + stressBounds.height * 0.5,
  );
  await page.mouse.wheel(0, -240);
  await waitForState(
    page,
    (candidate, initial) => candidate.cameraHalfHeight < initial.cameraHalfHeight,
    cameraAfterPan,
  );
  await page.locator("#reset-button").click();
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "rigid-stress-2k" &&
      Math.abs(candidate.cameraCenterX) < 0.001 &&
      Math.abs(candidate.cameraCenterY - 0.1) < 0.001 &&
      Math.abs(candidate.cameraHalfHeight - 10) < 0.001,
  );

  await page.locator("#scene-select").selectOption("rigid-stress-50k");
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "rigid-stress-50k" &&
      candidate.liveBodies === 50_000 &&
      candidate.bodyCapacity === 50_000 &&
      candidate.fixedSteps >= 2,
  );
  state = await currentState(page);
  if (
    state.fixedStepsThisFrame > 1 ||
    state.physicsHz !== 30 ||
    state.particleCapacity !== 0 ||
    state.waterAvailable
  ) {
    throw new Error(`invalid 50K stress state: ${JSON.stringify(state)}`);
  }
  const status = await page.locator("#status-text").textContent();
  for (const field of ["FPS", "physics", "main", "bodies 50000/50000"]) {
    if (!status.includes(field)) {
      throw new Error(`showcase diagnostic line is missing ${field}: ${status}`);
    }
  }

  await page.screenshot({ path: screenshot, fullPage: true });
  state = await currentState(page);
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

async function currentState(page) {
  return page.evaluate(() => globalThis.__physboxShowcase);
}

async function waitForState(page, predicate, argument = null) {
  await page.waitForFunction(
    ([predicateSource, predicateArgument]) => {
      const state = globalThis.__physboxShowcase;
      return Boolean(
        state &&
          Function(
            "state",
            "argument",
            `return (${predicateSource})(state, argument)`,
          )(state, predicateArgument),
      );
    },
    [predicate.toString(), argument],
  );
}

async function canvasBounds(page) {
  const bounds = await page.locator("#showcase-canvas").boundingBox();
  if (!bounds) throw new Error("showcase canvas disappeared");
  return bounds;
}

async function selectTool(page, tool) {
  await page.locator(`#tool-${tool}`).click();
  await waitForState(page, (state, expected) => state.tool === expected, tool);
}

async function clickCanvas(page, normalizedX, normalizedY) {
  const bounds = await canvasBounds(page);
  await page.mouse.click(
    bounds.x + bounds.width * normalizedX,
    bounds.y + bounds.height * normalizedY,
  );
}

async function dragCanvas(page, start, end) {
  const bounds = await canvasBounds(page);
  await page.mouse.move(
    bounds.x + bounds.width * start[0],
    bounds.y + bounds.height * start[1],
  );
  await page.mouse.down();
  await page.mouse.move(
    bounds.x + bounds.width * end[0],
    bounds.y + bounds.height * end[1],
    { steps: 10 },
  );
  await page.mouse.up();
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
