import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { chromium } from "playwright";

const HOST = "127.0.0.1";
const PORT = 4175;
const URL = `http://${HOST}:${PORT}`;
const PHYSBOX3 = path.resolve(process.cwd(), "../physbox3");

function parseArguments(argv) {
  let output = path.join("/tmp", `physbox5-physbox3-web-${Date.now()}`);
  for (const argument of argv) {
    if (argument === "--") continue;
    if (argument.startsWith("--output=")) {
      output = path.resolve(argument.slice("--output=".length));
      continue;
    }
    throw new Error(`unknown argument ${argument}`);
  }
  return { output };
}

function git(repository, args) {
  const result = spawnSync("git", ["-C", repository, ...args], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || `git ${args.join(" ")} failed`);
  }
  return result.stdout.trim();
}

async function waitForServer(server) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (server.exitCode !== null) {
      throw new Error(`PhysBox 3 Vite server exited with ${server.exitCode}`);
    }
    try {
      const response = await fetch(URL);
      if (response.ok) return;
    } catch {
      // Vite has not started listening yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("timed out waiting for the PhysBox 3 Vite server");
}

async function stopServer(server) {
  if (server.exitCode !== null) return;
  server.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => server.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 5_000)),
  ]);
  if (server.exitCode === null) server.kill("SIGKILL");
}

async function canvasBounds(page) {
  const bounds = await page.locator("#canvas-container").boundingBox();
  if (!bounds) throw new Error("PhysBox 3 canvas container has no bounds");
  return bounds;
}

function worldToScreen(bounds, x, y) {
  const zoom = 30;
  return {
    x: bounds.x + bounds.width / 2 + x * zoom,
    y: bounds.y + bounds.height / 2 - y * zoom,
  };
}

async function selectTool(page, name) {
  await page.getByRole("button", { name, exact: true }).click();
}

async function drawPlatform(page, bounds, start, end) {
  const from = worldToScreen(bounds, start.x, start.y);
  const to = worldToScreen(bounds, end.x, end.y);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 8 });
  await page.mouse.up();
}

async function buildContainer(page, bounds, centerX) {
  await selectTool(page, "Platform");
  await drawPlatform(page, bounds, { x: centerX - 3, y: 0.15 }, { x: centerX - 3, y: 5 });
  await drawPlatform(page, bounds, { x: centerX + 3, y: 0.15 }, { x: centerX + 3, y: 5 });
}

async function pour(page, bounds, points, repetitions = 4) {
  await selectTool(page, "Fluid");
  for (let repetition = 0; repetition < repetitions; repetition += 1) {
    for (const point of points) {
      const screen = worldToScreen(bounds, point.x, point.y);
      await page.mouse.click(screen.x, screen.y);
    }
    await page.waitForTimeout(80);
  }
}

async function placeBody(page, bounds, tool, point) {
  await selectTool(page, tool);
  const screen = worldToScreen(bounds, point.x, point.y);
  await page.mouse.click(screen.x, screen.y);
}

async function resetAndClear(page) {
  await page.locator("#s-reset").click();
  await page.waitForTimeout(150);
  await page.locator("#s-clear").click();
  await page.waitForTimeout(100);
}

function parseLeadingInteger(text) {
  const match = text.match(/(\d+)/);
  return match ? Number.parseInt(match[1], 10) : null;
}

async function capture(page, output, id, description) {
  await page.waitForTimeout(400);
  const screenshot = path.join(output, `${id}.png`);
  await page.screenshot({ path: screenshot, fullPage: true });
  const fluidText = await page.locator("#d-fluid").innerText();
  const countsText = await page.locator("#d-counts").innerText();
  const fpsText = await page.locator("#d-fps").innerText();
  return {
    id,
    description,
    screenshot: path.basename(screenshot),
    screenshotSha256: createHash("sha256").update(await readFile(screenshot)).digest("hex"),
    particleCount: parseLeadingInteger(fluidText.replace(/^Fluid:\s*/, "")),
    fluidText,
    countsText,
    fpsText,
  };
}

async function main() {
  const { output } = parseArguments(process.argv.slice(2));
  await mkdir(output, { recursive: false });

  const physbox3Revision = git(PHYSBOX3, ["rev-parse", "HEAD"]);
  const physbox3Status = git(PHYSBOX3, ["status", "--porcelain=v1", "--untracked-files=all"]);
  const box2dRevision = git(path.join(PHYSBOX3, "vendor/box2d3-wasm/box2d"), [
    "rev-parse",
    "HEAD",
  ]);
  const bridgeRevision = git(path.join(PHYSBOX3, "vendor/box2d3-wasm"), ["rev-parse", "HEAD"]);
  if (physbox3Status !== "") {
    throw new Error(`PhysBox 3 calibration source is dirty:\n${physbox3Status}`);
  }

  const server = spawn(
    path.join(PHYSBOX3, "node_modules/.bin/vite"),
    ["--host", HOST, "--port", String(PORT), "--strictPort"],
    {
      cwd: PHYSBOX3,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let serverLog = "";
  server.stdout.on("data", (chunk) => {
    serverLog += chunk.toString();
  });
  server.stderr.on("data", (chunk) => {
    serverLog += chunk.toString();
  });

  let browser;
  try {
    await waitForServer(server);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    const browserErrors = [];
    page.on("console", (message) => {
      if (message.type() === "error") browserErrors.push(message.text());
    });
    page.on("pageerror", (error) => browserErrors.push(error.message));
    await page.goto(URL, { waitUntil: "networkidle" });
    await page.locator("#canvas").waitFor({ state: "visible" });
    await page.locator("#s-debug-toggle").click();
    const bounds = await canvasBounds(page);
    const scenes = [];

    await resetAndClear(page);
    await buildContainer(page, bounds, -8);
    await pour(
      page,
      bounds,
      [
        { x: -9, y: 4.1 },
        { x: -8, y: 4.3 },
        { x: -7, y: 4.1 },
      ],
      5,
    );
    await page.waitForTimeout(2_000);
    scenes.push(
      await capture(
        page,
        output,
        "contained-water",
        "Fluid settling in a UI-authored fixed platform container.",
      ),
    );

    await resetAndClear(page);
    await buildContainer(page, bounds, 0);
    await pour(
      page,
      bounds,
      [
        { x: -1.2, y: 3.5 },
        { x: -0.2, y: 3.8 },
        { x: 0.8, y: 3.5 },
      ],
      5,
    );
    await page.waitForTimeout(1_200);
    await placeBody(page, bounds, "Box", { x: 0.5, y: 4.8 });
    await page.waitForTimeout(2_000);
    scenes.push(
      await capture(
        page,
        output,
        "displacement",
        "A UI-authored dynamic box entered and displaced settled fluid.",
      ),
    );

    await resetAndClear(page);
    await buildContainer(page, bounds, 8);
    await pour(
      page,
      bounds,
      [
        { x: 6.8, y: 3.3 },
        { x: 7.8, y: 3.6 },
        { x: 8.8, y: 3.3 },
      ],
      5,
    );
    await placeBody(page, bounds, "Ball", { x: 8.8, y: 5.5 });
    await page.waitForTimeout(2_500);
    scenes.push(
      await capture(
        page,
        output,
        "dynamic-body-interaction",
        "A UI-authored dynamic ball fell through and exchanged motion with fluid.",
      ),
    );

    const report = {
      schemaVersion: 1,
      runId: `physbox3-web-${Date.now()}`,
      generatedAt: new Date().toISOString(),
      authority: "subjective-calibration-only",
      source: {
        physbox3Revision,
        physbox3Dirty: false,
        bridgeRevision,
        box2dRevision,
      },
      host: {
        platform: process.platform,
        architecture: process.arch,
        browser: await browser.version(),
        viewport: { width: 1280, height: 800 },
        url: URL,
      },
      interaction: {
        projection: "raw PhysBox 3 engine-rendered toolbar and canvas pointer events",
        camera: { x: 0, y: 0, pixelsPerMeter: 30 },
        fixedWaitsMs: {
          containedWater: 2000,
          displacementSettle: 1200,
          displacementResponse: 2000,
          dynamicBodyResponse: 2500,
        },
      },
      browserErrors,
      scenes,
    };
    if (browserErrors.length > 0) {
      throw new Error(`PhysBox 3 browser emitted errors:\n${browserErrors.join("\n")}`);
    }
    const reportPath = path.join(output, "report.json");
    await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
    console.log(`PHYSBOX5_PHYSBOX3_WEB_CALIBRATION_SUCCESS ${reportPath}`);
  } finally {
    await browser?.close();
    await stopServer(server);
    await writeFile(path.join(output, "vite.log"), serverLog, "utf8");
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
