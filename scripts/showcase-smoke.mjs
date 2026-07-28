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
    viewport: { width: 1180, height: 900 },
    deviceScaleFactor: 1,
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
    state.liveBodies !== 21
  ) {
    throw new Error(`invalid playground state: ${JSON.stringify(state)}`);
  }
  const bounds = await page.locator("#showcase-canvas").boundingBox();
  if (!bounds || bounds.y > 120 || bounds.width < 900 || bounds.height < 500) {
    throw new Error(
      `showcase canvas is not above-fold and materially visible: ${JSON.stringify(bounds)}`,
    );
  }

  await page.locator("#scene-select").selectOption("stack-and-sleep");
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "stack-and-sleep" &&
      globalThis.__physboxShowcase.liveBodies === 15 &&
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
  const point = (normalizedX, normalizedY) => ({
    x: currentBounds.x + currentBounds.width * normalizedX,
    y: currentBounds.y + currentBounds.height * normalizedY,
  });
  await page.mouse.move(point(0.5, 0.25).x, point(0.5, 0.25).y);
  await page.mouse.down();
  await page.waitForFunction(() => globalThis.__physboxShowcase?.dragging, null, {
    timeout: 5_000,
  });
  await page.mouse.move(point(0.66, 0.3).x, point(0.66, 0.3).y, { steps: 8 });
  await page.mouse.up();
  await page.waitForFunction(() => !globalThis.__physboxShowcase?.dragging, null, {
    timeout: 5_000,
  });

  await page.locator("#scene-select").selectOption("water-and-rigid");
  await page.waitForFunction(
    () =>
      globalThis.__physboxShowcase?.scene === "water-and-rigid" &&
      globalThis.__physboxShowcase.particleCount > 2_000 &&
      globalThis.__physboxShowcase.fixedSteps >= 90,
    null,
    { timeout: 30_000 },
  );
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
