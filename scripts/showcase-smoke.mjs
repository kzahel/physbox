import { randomUUID } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import {
  closeHttpServer,
  closeManagedBrowser,
  launchManagedBrowser,
} from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const serverRoot = resolve(
  process.env.PHYSBOX_SHOWCASE_SERVER_ROOT ?? join(root, "target"),
);
const urlPath = process.env.PHYSBOX_SHOWCASE_URL_PATH ?? "/showcase-web/";
const externalBaseUrl = process.env.PHYSBOX_SHOWCASE_BASE_URL;
const captureDirectory = resolve(
  process.env.PHYSBOX_SHOWCASE_CAPTURE_DIRECTORY ?? "/tmp",
);
const screenshot = join(captureDirectory, "physbox-showcase.png");
const provenancePath = join(captureDirectory, "physbox-showcase.json");
await mkdir(captureDirectory, { recursive: true });
if (process.env.PHYSBOX_SHOWCASE_SKIP_BUILD !== "1") {
  run(process.execPath, [join(root, "scripts", "build-showcase-web.mjs")]);
}

const server = externalBaseUrl
  ? null
  : await startServer(serverRoot, 0, {
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
  await page.addInitScript(installBodyObservation);
  const errors = [];
  const consoleProblems = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error" || message.type() === "warning") {
      consoleProblems.push(`${message.type()}: ${message.text()}`);
    }
  });
  const pageUrl =
    externalBaseUrl ?? `http://127.0.0.1:${server.address().port}${urlPath}`;
  let wasmResponse;
  page.on("response", (response) => {
    if (new URL(response.url()).pathname.endsWith("/pkg/physbox_showcase_web_bg.wasm")) {
      wasmResponse = response;
    }
  });
  const response = await page.goto(pageUrl, {
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
    !state.waterAvailable ||
    !state.sleepVisualization ||
    !(await page.locator("#sleep-visuals-input").isChecked())
  ) {
    throw new Error(`invalid default water state: ${JSON.stringify(state)}`);
  }
  const viewport = page.viewportSize();
  for (const size of [viewport, { width: 1280, height: 740 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(size);
    const canvas = await canvasBounds(page);
    if (canvas.width !== size.width || Math.abs(canvas.y + canvas.height - size.height) > 1) {
      throw new Error(`showcase canvas does not fill ${size.width}×${size.height}: ${JSON.stringify(canvas)}`);
    }
  }
  await page.setViewportSize(viewport);
  await page.getByRole("button", { name: "Fullscreen", exact: true }).click();
  await page.waitForFunction(() => document.fullscreenElement === document.body);
  await page.getByRole("button", { name: "Exit fullscreen" }).click();
  await page.waitForFunction(() => document.fullscreenElement === null);
  const sceneOptions = await page
    .locator("#scene-select option")
    .evaluateAll((options) => options.map((option) => option.value));
  for (const scene of [
    "rigid-stress-1k",
    "rigid-stress-2k",
    "rigid-stress-5k",
    "rigid-stress-10k",
    "rigid-rain-1k",
    "rigid-avalanche-2k",
  ]) {
    if (!sceneOptions.includes(scene)) {
      throw new Error(`showcase scene catalog is missing ${scene}`);
    }
  }
  if (sceneOptions.includes("rigid-stress-20k") || sceneOptions.includes("rigid-stress-50k")) {
    throw new Error(`showcase retains rejected rigid stress tiers: ${sceneOptions}`);
  }
  const physicsOptions = await page
    .locator("#physics-select option")
    .evaluateAll((options) => options.map((option) => option.value));
  if (JSON.stringify(physicsOptions) !== JSON.stringify(["30", "60"])) {
    throw new Error(`unexpected physics options: ${JSON.stringify(physicsOptions)}`);
  }
  const bodyCapacityOptions = await page
    .locator("#body-capacity-select option")
    .evaluateAll((options) => options.map((option) => option.value));
  if (
    JSON.stringify(bodyCapacityOptions) !==
    JSON.stringify(["64", "256", "1000", "2000", "5000", "10000"])
  ) {
    throw new Error(
      `unexpected body capacity options: ${JSON.stringify(bodyCapacityOptions)}`,
    );
  }
  const waterCapacityOptions = await page
    .locator("#water-capacity-select option")
    .evaluateAll((options) => options.map((option) => option.value));
  if (
    JSON.stringify(waterCapacityOptions) !==
    JSON.stringify(["0", "4096", "8192", "16384", "32768"])
  ) {
    throw new Error(
      `unexpected water capacity options: ${JSON.stringify(waterCapacityOptions)}`,
    );
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
  await clickCanvas(page, 0.67, 0.3);
  await waitForState(
    page,
    (candidate, initial) => candidate.particleCount >= initial + 51,
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

  await page.locator("#sleep-visuals-input").uncheck();
  await waitForState(page, (candidate) => !candidate.sleepVisualization);
  await page.locator("#scene-select").selectOption("rigid-stress-1k");
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "rigid-stress-1k" &&
      candidate.liveBodies === 1_000 &&
      candidate.bodyCapacity === 1_000 &&
      candidate.particleCapacity === 0 &&
      !candidate.sleepVisualization &&
      candidate.fixedSteps >= 2,
  );
  if (
    (await page.locator("#body-capacity-select").inputValue()) !== "1000" ||
    (await page.locator("#sleep-visuals-input").isChecked())
  ) {
    throw new Error("1K stress defaults or Sleep tint persistence are incorrect");
  }
  await page.locator("#sleep-visuals-input").check();
  await waitForState(page, (candidate) => candidate.sleepVisualization);

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
    (candidate, home) =>
      candidate.scene === "rigid-stress-2k" &&
      candidate.cameraCenterX === home.cameraCenterX &&
      candidate.cameraCenterY === home.cameraCenterY &&
      candidate.cameraHalfHeight === home.cameraHalfHeight,
    cameraBefore,
  );

  const cameraBeforeArrow = await currentState(page);
  await page.locator("#showcase-canvas").focus();
  await page.keyboard.press("ArrowRight");
  await waitForState(
    page,
    (candidate, initial) => candidate.cameraCenterX > initial.cameraCenterX + 0.1,
    cameraBeforeArrow,
  );
  const cameraAfterArrow = await currentState(page);
  await page.locator("#pause-button").focus();
  await page.keyboard.press("ArrowLeft");
  await page.waitForTimeout(50);
  const cameraAfterButtonArrow = await currentState(page);
  if (
    cameraAfterButtonArrow.cameraCenterX !== cameraAfterArrow.cameraCenterX ||
    cameraAfterButtonArrow.cameraCenterY !== cameraAfterArrow.cameraCenterY
  ) {
    throw new Error("arrow navigation intercepted a focused button");
  }

  await page.locator("#scene-select").selectOption("rigid-stress-10k");
  await waitForState(
    page,
    (candidate) =>
      candidate.scene === "rigid-stress-10k" &&
      candidate.liveBodies === 10_000 &&
      candidate.bodyCapacity === 10_000 &&
      candidate.particleCapacity === 0 &&
      candidate.fixedSteps >= 2,
  );
  state = await currentState(page);
  if (
    state.fixedStepsThisFrame > 1 ||
    state.physicsHz !== 30 ||
    state.waterAvailable
  ) {
    throw new Error(`invalid 10K stress state: ${JSON.stringify(state)}`);
  }

  for (const [scene, count, description] of [
    ["rigid-stress-5k", 5_000, /land in waves/],
    ["rigid-rain-1k", 1_000, /rains onto five ramps/],
    ["rigid-avalanche-2k", 2_000, /hits the ramps/],
  ]) {
    await page.locator("#scene-select").selectOption(scene);
    await waitForState(
      page,
      (s, expected) => s.scene === expected.scene &&
        s.liveBodies === expected.count && s.bodyCapacity === expected.count &&
        s.particleCapacity === 0 && s.fixedSteps >= 6,
      { scene, count },
    );
    if (!description.test(await page.locator("#scene-description").textContent())) {
      throw new Error(`missing scene explanation for ${scene}`);
    }
  }

  await page.locator("#physics-select").selectOption("60");
  const motionDemos = [
    {
      scene: "domino-run", bodies: 47, particles: 0, steps: 600,
      check: (bodies) => bodies.length === 40 && bodies.every(({ angle }) => angle < -0.5),
    },
    {
      scene: "marble-pegboard", bodies: 153, particles: 0, steps: 300,
      check: (bodies) => {
        const collected = bodies.filter(({ y }) => y < -2.8);
        const bins = new Set(collected.map(({ x }) => Math.floor(x / 1.24)));
        return bodies.length === 64 && collected.length >= 56 && bins.size >= 6;
      },
    },
    {
      scene: "water-surge", bodies: 6, particles: 3414, steps: 125,
      check: (bodies) => bodies.length === 3 && bodies[0].x > 2 &&
        bodies[0].y > bodies[2].y + 0.75 && bodies[2].y < -3.3,
    },
    {
      scene: "seesaw-launch", bodies: 11, particles: 0, steps: 190,
      check: (bodies) => {
        const marble = bodies[2];
        return bodies.length === 3 && marble.x > 3 && marble.x < 6.45 &&
          marble.y > -3.2 && marble.y < -2.25;
      },
    },
    {
      scene: "breakwater", bodies: 10, particles: 3106, steps: 150,
      check: (bodies) => {
        if (bodies.length !== 4) return false;
        const [openNear, openFar, protectedNear, protectedFar] = bodies;
        const openTravel = (openNear.x + 4.6) + (openFar.x + 2.7);
        const protectedTravel = (protectedNear.x - 9.6) + (protectedFar.x - 11.5);
        return openTravel > protectedTravel + 2;
      },
    },
    {
      scene: "bounce-trio", bodies: 6, particles: 0, steps: 118,
      check: (bodies) => bodies.length === 3 && bodies[1].y > bodies[0].y + 0.75 &&
        bodies[2].y > bodies[1].y + 1.25,
    },
  ];
  for (const { scene, bodies, particles, steps, check } of motionDemos) {
    await page.locator("#scene-select").selectOption(scene);
    await waitForState(page, (s, expected) => s.scene === expected.scene &&
      s.liveBodies === expected.bodies && s.particleCount === expected.particles &&
      s.droppedParticles === 0 && s.fixedSteps >= expected.steps, { scene, bodies, particles, steps });
    if (!check(await page.evaluate(() => globalThis.__testObserveBodies()))) {
      throw new Error(`${scene} did not complete its expected motion`);
    }
    console.log(`verified motion in ${scene}`);
  }
  await verifyBrickRemoval(page);

  for (const [scene, count] of [["tower-drop-13k", 13_122], ["tower-drop-32k", 32_640]]) {
    await page.locator("#scene-select").selectOption(scene);
    await waitForState(page, (s, expected) => s.scene === expected.scene &&
      s.particleCount === expected.count && s.liveBodies === 13 &&
      s.physicsHz === 60 && s.fixedSteps >= 12, { scene, count });
    if (!(await page.locator("#scene-description").textContent()).includes(count.toLocaleString("en-US"))) {
      throw new Error(`missing particle count in explanation for ${scene}`);
    }
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    const beforeReset = await currentState(page);
    await page.locator("#reset-button").click();
    await waitForState(page, (s, frames) => s.fixedSteps >= 4 && s.frames < frames, beforeReset.frames);
    if ((await page.locator("#pause-button").textContent()) !== "Pause") {
      throw new Error(`reset did not restart ${scene}`);
    }
  }
  // The remaining stress checks exercise the manually selected 30 Hz configuration.
  await page.locator("#physics-select").selectOption("30");
  for (const [scene, capacity] of [
    ["water-stress-4k", 4_096],
    ["water-stress-8k", 8_192],
    ["water-stress-16k", 16_384],
  ]) {
    console.log(`checking ${scene}`);
    await page.locator("#scene-select").selectOption(scene);
    await waitForState(
      page,
      (candidate, expected) =>
        candidate.scene === expected.scene &&
        candidate.bodyCapacity === 256 &&
        candidate.liveBodies === 11 &&
        candidate.particleCapacity === expected.capacity &&
        candidate.particleCount >= Math.floor(expected.capacity * 0.79) &&
        candidate.fixedSteps >= 2,
      { scene, capacity },
    );
    console.log(`advanced ${scene}`);
    if ((await page.locator("#water-capacity-select").inputValue()) !== String(capacity)) {
      throw new Error(`${scene} did not select its exact Max water tier`);
    }
  }

  state = await currentState(page);
  if (
    state.fixedStepsThisFrame > 1 ||
    state.physicsHz !== 30 ||
    state.scene !== "water-stress-16k" ||
    state.particleCapacity !== 16_384 ||
    !state.waterAvailable
  ) {
    throw new Error(`invalid 16K water stress state: ${JSON.stringify(state)}`);
  }
  const status = await page.locator("#status-text").textContent();
  for (const field of ["FPS", "physics", "main", "bodies 11/256", "water"]) {
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
  if (!wasmResponse?.ok()) throw new Error("Missing showcase WASM response");
  const wasmBytes = (await wasmResponse.body()).byteLength;
  const provenance = {
    runId: randomUUID(),
    host: "browser-webgpu",
    url: pageUrl,
    urlPath: new URL(pageUrl).pathname,
    crossOriginIsolated: state.crossOriginIsolated,
    finalState: state,
    wasmBytes,
    screenshot,
  };
  await writeFile(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`);
  console.log(
    `PHYSBOX_SHOWCASE_SUCCESS run=${provenance.runId} scene=${state.scene} steps=${state.fixedSteps} wasmBytes=${wasmBytes}`,
  );
  console.log(`showcase capture: ${screenshot}`);
  console.log(`showcase provenance: ${provenancePath}`);
} finally {
  await closeManagedBrowser(browserSession);
  if (server) {
    await closeHttpServer(server);
  }
}

async function currentState(page) {
  return page.evaluate(() => globalThis.__physboxShowcase);
}

async function verifyBrickRemoval(page) {
  await page.locator("#scene-select").selectOption("pull-a-brick");
  await waitForState(page, s => s.scene === "pull-a-brick" && s.liveBodies === 33 && s.fixedSteps >= 150);
  await selectTool(page, "drag");
  const settled = await page.evaluate(() => globalThis.__testObserveBodies());
  const brick = settled[3];
  const bounds = await canvasBounds(page);
  const aspect = bounds.width / bounds.height;
  const halfHeight = Math.max(5, 8 / aspect);
  const halfWidth = halfHeight * aspect;
  const point = (x, y) => [
    bounds.x + bounds.width * (0.5 + x / (2 * halfWidth)),
    bounds.y + bounds.height * (0.5 - y / (2 * halfHeight)),
  ];
  await page.mouse.move(...point(brick.x, brick.y));
  await page.mouse.down();
  await waitForState(page, s => s.dragging);
  await page.mouse.move(...point(-4.6, brick.y), { steps: 18 });
  const held = await currentState(page);
  await waitForState(page, (s, start) => s.fixedSteps >= start + 65, held.fixedSteps);
  await page.mouse.up();
  await waitForState(page, (s, start) => !s.dragging && s.fixedSteps >= start + 220, held.fixedSteps);
  const after = await page.evaluate(() => globalThis.__testObserveBodies());
  const startingTop = Math.max(...settled.map(body => body.y));
  const endingTop = Math.max(...after.map(body => body.y));
  if (settled.length !== 30 || startingTop < 1 || after.length !== 30 ||
      after[3].x > brick.x - 1.5 || endingTop > startingTop - 0.5) {
    throw new Error(`brick removal did not topple the tower: top ${startingTop} -> ${endingTop}`);
  }
  console.log("verified drag and collapse in pull-a-brick");
}

function installBodyObservation() {
  if (!globalThis.GPUDevice) return;
  const createBuffer = GPUDevice.prototype.createBuffer;
  let observation;
  GPUDevice.prototype.createBuffer = function (descriptor) {
    const buffer = createBuffer.call(this, descriptor);
    if (descriptor.label === "physbox_sim_public_body_observation") {
      observation = { device: this, buffer };
    }
    return buffer;
  };
  globalThis.__testObserveBodies = async () => {
    const { device, buffer } = observation;
    const readback = device.createBuffer({
      size: buffer.size,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    try {
      const encoder = device.createCommandEncoder();
      encoder.copyBufferToBuffer(buffer, 0, readback, 0, buffer.size);
      device.queue.submit([encoder.finish()]);
      await readback.mapAsync(GPUMapMode.READ);
      const bytes = readback.getMappedRange();
      const floats = new Float32Array(bytes);
      const words = new Uint32Array(bytes);
      const bodies = [];
      // Public GpuBodyObservationRecord has 12 words, with live/fixed flags in word nine.
      for (let i = 0; i < words.length; i += 12) {
        const flags = words[i + 9];
        if ((flags & 1) && !(flags & 4)) {
          bodies.push({ x: floats[i], y: floats[i + 1], angle: floats[i + 2] });
        }
      }
      return bodies;
    } finally {
      readback.destroy();
    }
  };
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
