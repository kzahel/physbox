import { randomUUID } from "node:crypto";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { PNG } from "pngjs";
import { closeHttpServer, closeManagedBrowser, launchManagedBrowser } from "./browser-process.mjs";
import { startServer } from "./server.mjs";

const root = resolve(import.meta.dirname, "..");
const roles = [2, 4, 8, 12, 14, 16];
const arguments_ = process.argv.slice(2);
if (arguments_.length !== 4 || arguments_[0] !== "--native-evidence" || arguments_[2] !== "--output") {
  throw new Error("usage: node scripts/qualify-awake-finalist.mjs --native-evidence PATH --output PATH");
}
const nativeEvidencePath = resolve(arguments_[1]);
const outputPath = resolve(arguments_[3]);
const outputDirectory = dirname(outputPath);
const nativeEvidence = JSON.parse(await readFile(nativeEvidencePath, "utf8"));
const revision = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]);
if (!revision || dirty === undefined || dirty !== "") throw new Error("browser qualification requires a clean source worktree");
if (
  nativeEvidence.passed !== true
  || nativeEvidence.sourceDirty !== false
  || nativeEvidence.sourceRevision !== revision
  || nativeEvidence.physicsConfiguration?.substeps !== 8
  || nativeEvidence.physicsConfiguration?.sleepingEnabled !== false
) throw new Error("native evidence does not qualify this clean source revision and finalist configuration");
const expectedRows = new Map(nativeEvidence.rows.map((row) => [row.rectangleCount, row]));
if (roles.some((count) => !expectedRows.has(count))) throw new Error("native evidence is missing a stack role");

await mkdir(outputDirectory, { recursive: true });
await rm(outputPath, { force: true });
run(process.execPath, [resolve(root, "scripts/build-web.mjs"), "--release", "--solver-lab"]);

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
  const port = server.address().port;
  const rows = [];
  for (const count of roles) {
    const scenario = `solver-lab-stack-${count}`;
    const screenshot = join(outputDirectory, `web-${scenario}.png`);
    await rm(screenshot, { force: true });
    const page = await browserSession.browser.newPage({ viewport: { width: 960, height: 600 } });
    const pageErrors = [];
    const consoleErrors = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error" || message.type() === "warning") {
        consoleErrors.push(`${message.type()}: ${message.text()}`);
      }
    });
    const query = new URLSearchParams({
      scenario,
      solver: "soft-coupled",
      "sim-hz": "60",
      substeps: "8",
      iterations: "1",
      "contact-hz": "30",
      "contact-damping": "10",
      "static-contact-scale": "2",
      "speculative-distance": "0.02",
      "max-depenetration-velocity": "3",
      sleeping: "off",
    });
    await page.goto(`http://127.0.0.1:${port}/?${query}`, { waitUntil: "networkidle" });
    await page.waitForFunction(
      (expectedScenario) => {
        const state = globalThis.__physbox5;
        return state?.ready
          && state.frame?.scenario?.name === expectedScenario
          && state.frame?.scenario?.complete === true;
      },
      scenario,
      { timeout: 45_000 },
    );
    await page.evaluate(() => globalThis.__physbox5FrameControl.pause());
    const state = await page.evaluate(() => globalThis.__physbox5);
    const contract = state.frame?.sceneContract;
    const expected = expectedRows.get(count);
    if (
      state.ok !== true
      || state.frame?.scenario?.passed !== true
      || contract?.scenario !== scenario
      || contract?.scenarioHash !== expected.sourceScenarioHash
      || contract?.physicsConfigurationHash !== nativeEvidence.physicsConfigurationHash
      || contract?.solverProfile !== "soft-coupled"
      || contract?.fixedStepHz !== 60
      || contract?.solverSubsteps !== 8
      || contract?.solverBiasedIterations !== 1
      || contract?.requestedContactHz !== 30
      || contract?.contactDampingRatio !== 10
      || contract?.requestedStaticContactHz !== 60
      || contract?.requestedSleepingEnabled !== false
      || contract?.effectiveSleepingEnabled !== false
      || contract?.capacityOverflowed !== false
    ) throw new Error(`browser ${scenario} contract mismatch: ${JSON.stringify(state)}`);
    const validationErrors = consoleErrors.filter((message) =>
      /invalid (?:shader|compute|render|command)|while (?:calling|encoding).*create(?:shader|compute|render)|queue\].submit/i.test(message)
    );
    if (pageErrors.length || validationErrors.length) {
      throw new Error(`browser ${scenario} errors: ${[...pageErrors, ...validationErrors].join("; ")}`);
    }
    await captureCompleteScreenshot(page, screenshot);
    rows.push({
      role: `stack-${count}`,
      rectangleCount: count,
      blocking: expected.blocking,
      nativeQualityPassed: expected.qualityPassed,
      screenshot,
      adapter: state.diagnostics,
      report: state.frame,
    });
    console.log(`BROWSER_FINALIST ${scenario} PASS scenario-hash=${contract.scenarioHash}`);
    await page.close();
  }
  const evidence = {
    version: 1,
    runId: randomUUID(),
    sourceRevision: revision,
    sourceDirty: false,
    host: "web",
    capturePhase: "s25-finalist-six-role-complete",
    nativeEvidence: nativeEvidencePath,
    physicsConfigurationHash: nativeEvidence.physicsConfigurationHash,
    physicsConfiguration: nativeEvidence.physicsConfiguration,
    rows,
    passed: true,
  };
  await writeFreshJson(outputPath, evidence);
  console.log(`PHYSBOX5_VALIDATION_SUCCESS host=web evidence=s25-awake-finalist output=${outputPath}`);
} finally {
  if (browserSession) await closeManagedBrowser(browserSession);
  await closeHttpServer(server);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${basename(command)} exited ${result.status ?? "without status"}`);
}

function git(args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : undefined;
}

async function captureCompleteScreenshot(page, path) {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    await page.screenshot({ path });
    const image = PNG.sync.read(await readFile(path));
    let black = 0;
    let foreground = 0;
    const pixels = image.width * image.height;
    for (let offset = 0; offset < image.data.length; offset += 4) {
      const red = image.data[offset];
      const green = image.data[offset + 1];
      const blue = image.data[offset + 2];
      black += Number(red <= 3 && green <= 3 && blue <= 3);
      foreground += Number(Math.max(red, green, blue) >= 48);
    }
    if (black <= pixels * 0.01 && foreground >= pixels * 0.05) return;
    await page.waitForTimeout(100);
  }
  throw new Error(`browser screenshot remained incomplete: ${path}`);
}

async function writeFreshJson(path, value) {
  const temporary = `${path}.tmp-${process.pid}`;
  await rm(temporary, { force: true });
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`);
  await rename(temporary, path);
}
