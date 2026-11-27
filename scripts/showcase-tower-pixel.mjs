// node scripts/showcase-tower-pixel.mjs BUILD_DIR OUTPUT_DIR [13k|32k] [time|film|profile]
import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { PNG } from "pngjs";
import { chromium } from "playwright";
import { startServer } from "./server.mjs";

import { installTowerGpuProfile } from "./tower-gpu-profile.mjs";

const [build, output, tier = "13k", mode = "time"] = process.argv.slice(2);
if (
  !build ||
  !output ||
  !["13k", "32k"].includes(tier) ||
  !["time", "film", "profile"].includes(mode)
) {
  throw Error("Expected BUILD_DIR OUTPUT_DIR [13k|32k] [time|film|profile]");
}
const adb = (...args) =>
  execFileSync("adb", ["-d", ...args], { encoding: "utf8" }).trim();
const revision = execFileSync("git", ["rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
const dirty = Boolean(
  execFileSync("git", ["status", "--porcelain"], { encoding: "utf8" }).trim(),
);
const hash = createHash("sha256")
  .update(readFileSync(join(build, "pkg/physbox_showcase_web_bg.wasm")))
  .digest("hex");
const scene = "tower-drop-" + tier;
const runId = randomUUID();
mkdirSync(output, { recursive: true });
const prefix = resolve(output, scene + "-" + mode + "-" + runId);
const server = await startServer(build, 0, { crossOriginIsolation: false });
const port = server.address().port;
const url = "http://127.0.0.1:" + port + "/?run=" + runId + "#" + scene;
let forward, reverse, page;
try {
  adb("reverse", "--no-rebind", "tcp:" + port, "tcp:" + port);
  reverse = true;
  forward = adb("forward", "tcp:0", "localabstract:chrome_devtools_remote");
  adb(
    "shell",
    "am",
    "start",
    "-W",
    "-a",
    "android.intent.action.VIEW",
    "-d",
    url,
    "com.android.chrome",
  );
  const browser = await chromium.connectOverCDP("http://127.0.0.1:" + forward);
  for (let i = 0; i < 30 && !page; i++) {
    page = browser
      .contexts()[0]
      .pages()
      .find((p) => p.url().startsWith("http://127.0.0.1:" + port + "/"));
    if (!page) await new Promise((r) => setTimeout(r, 150));
  }
  if (!page) throw Error("Could not find the new phone tab");
  page.on("pageerror", (e) => console.error(e.message));
  await page.waitForFunction(
    (scene) =>
      window.__physboxShowcase?.ready &&
      window.__physboxShowcase.scene === scene,
    scene,
    { timeout: 45000 },
  );
  if (mode === "profile") await page.evaluate(installTowerGpuProfile);
  const result = await page.evaluate(async (mode) => {
    const state = () => window.__physboxShowcase;
    const canvas = document.querySelector("canvas");
    const button = document.querySelector("#pause-button");
    if (button.textContent !== "Drop tower") throw Error("Tower not staged");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter || adapter.info.isFallbackAdapter)
      throw Error("Hardware adapter required");
    const wake = await navigator.wakeLock?.request("screen").catch(() => null);
    let recorder, stopped, stream;
    const chunks = [];
    if (mode === "film") {
      stream = canvas.captureStream(30);
      recorder = new MediaRecorder(stream, {
        mimeType: "video/webm;codecs=vp8",
        videoBitsPerSecond: 4000000,
      });
      recorder.ondataavailable = (e) => chunks.push(e.data);
      stopped = new Promise((r) => (recorder.onstop = r));
      recorder.start();
    }
    await new Promise((r) => setTimeout(r, 700));
    const initial = state();
    const samples = [];
    const endStep = mode === "film" ? 480 : 240;
    await new Promise((done, reject) => {
      const timer = setTimeout(
        () => reject(Error("Drop did not reach final step")),
        90000,
      );
      function sample(t) {
        if (document.visibilityState !== "visible" || state().error) {
          clearTimeout(timer);
          reject(Error(state().error ?? "Phone tab hidden"));
          return;
        }
        const s = state();
        samples.push({
          t,
          frame: s.frames,
          step: s.fixedSteps - initial.fixedSteps,
          live: s.particleCount,
          dropped: s.droppedParticles,
          gpuPhysicsMs: s.gpuPhysicsMs,
          gpuRenderMs: s.gpuRenderMs,
        });
        if (samples.length === 1) button.click();
        if (s.fixedSteps - initial.fixedSteps >= endStep) {
          clearTimeout(timer);
          done();
        } else requestAnimationFrame(sample);
      }
      requestAnimationFrame(sample);
    });
    let video;
    if (recorder) {
      recorder.stop();
      await stopped;
      stream.getTracks().forEach((t) => t.stop());
      const bytes = new Uint8Array(await new Blob(chunks).arrayBuffer());
      let chars = "";
      for (let i = 0; i < bytes.length; i += 8192)
        chars += String.fromCharCode(...bytes.subarray(i, i + 8192));
      video = btoa(chars);
    }
    const phases = {};
    for (const [name, lo, hi] of [
      ["release", 0, 30],
      ["impact", 30, 80],
      ["rebound", 80, 150],
      ["settling", 150, 240],
    ]) {
      const a = samples.findIndex((s) => s.step >= lo);
      const b = samples.findIndex((s) => s.step >= hi);
      const first = samples[a],
        last = samples[b];
      const seconds = (last.t - first.t) / 1000;
      const intervals = samples
        .slice(a + 1, b + 1)
        .map((s, i) => s.t - samples[a + i].t)
        .sort((x, y) => x - y);
      phases[name] = {
        lo,
        hi,
        fps: (last.frame - first.frame) / seconds,
        stepsHz: (last.step - first.step) / seconds,
        p95Ms: intervals[Math.ceil(intervals.length * 0.95) - 1],
      };
    }
    if (wake) await wake.release();
    return {
      initial,
      final: state(),
      canvas: [canvas.width, canvas.height],
      timestampQuery: adapter.features.has("timestamp-query"),
      adapter: [adapter.info.vendor, adapter.info.architecture],
      phases,
      samples,
      video,
    };
  }, mode);

  if (mode === "profile") {
    result.gpuProfile = await page.evaluate(async () => {
      const {frames, errors, pending} = window.towerGpuProfile;
      await Promise.all(pending);
      return {frames, errors};
    });
  }
  if (result.video) {
    writeFileSync(prefix + ".webm", Buffer.from(result.video, "base64"));
    delete result.video;
  }
  if (mode === "film")
    result.interaction = await verifyReplayAndDrag(page, result);
  writeFileSync(
    prefix + ".json",
    JSON.stringify(
      {
        runId,
        revision,
        dirty,
        wasmSha256: hash,
        host: adb("shell", "getprop", "ro.product.model"),
        browser: browser.version(),
        scene,
        mode,
        ...result,
      },
      null,
      2,
    ),
  );
  if (mode === "profile") {
    const { frames, errors } = result.gpuProfile;
    for (let step = 80; step < 150; step += 5) {
      if (!frames.some(f => f.step === step && f.passes.some(p => p.label === "particle_correction_gather"))) {
        throw Error("Missing GPU profile for step " + step + " in " + prefix);
      }
    }
    if (errors.length) throw Error(errors.join("\n"));
  }
  const expectedCount = tier === "32k" ? 32_640 : 13_122;
  if (
    result.initial.particleCount !== expectedCount ||
    result.initial.liveBodies !== 13 ||
    result.initial.bodyCapacity !== 64 ||
    result.initial.physicsHz !== 60
  ) {
    throw Error("Unexpected tower scene parameters in " + prefix);
  }
  const counts = result.samples.map((s) => s.live);
  if (
    counts.some((n) => n !== result.initial.particleCount) ||
    result.samples.some((s) => s.dropped)
  ) {
    throw Error(
      "Water population changed during the shot: " +
        Math.min(...counts) +
        ".." +
        Math.max(...counts) +
        " (" +
        prefix +
        ")",
    );
  }
  console.log(
    "PHYSBOX5_VALIDATION_SUCCESS",
    prefix,
    result.initial.particleCount,
    JSON.stringify(result.phases),
  );
} finally {
  if (page) await page.close().catch(() => {});
  if (forward) adb("forward", "--remove", "tcp:" + forward);
  if (reverse) adb("reverse", "--remove", "tcp:" + port);
  server.closeAllConnections();
  await new Promise((r) => server.close(r));
}
// CDP belongs to the phone browser. Leave its other tabs open.
process.exit(0);

async function verifyReplayAndDrag(page, result) {
  await page.locator("#pause-button").evaluate((b) => b.click());
  const canvas = page.locator("canvas");
  const pixels = PNG.sync.read(await canvas.screenshot());
  // Locate the interior of a warm-colored block in the rendered scene.
  const warm = (x, y) => {
    const i = 4 * (pixels.width * y + x);
    const [r, g, b] = pixels.data.subarray(i, i + 3);
    return r > 160 && r > g + 16 && r > b + 12;
  };
  let point;
  for (let y = 8; y < pixels.height - 8 && !point; y += 3) {
    for (let x = 8; x < pixels.width - 8; x += 3) {
      if (
        warm(x, y) &&
        warm(x - 6, y) &&
        warm(x + 6, y) &&
        warm(x, y - 6) &&
        warm(x, y + 6)
      ) {
        point = [x / pixels.width, y / pixels.height];
        break;
      }
    }
  }
  if (!point) throw Error("No colored block could be picked");
  const bounds = await canvas.boundingBox();
  const x = bounds.x + bounds.width * point[0],
    y = bounds.y + bounds.height * point[1];
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.locator("#pause-button").evaluate((b) => b.click());
  const start = await page.evaluate(() => window.__physboxShowcase.fixedSteps);
  await page.mouse.move(x + bounds.width * 0.1, y - bounds.height * 0.1, {
    steps: 5,
  });
  await page.waitForFunction(
    (start) =>
      window.__physboxShowcase.fixedSteps >= start + 15 &&
      window.__physboxShowcase.dragging,
    start,
    { timeout: 5000 },
  );
  await page.mouse.up();
  await page.waitForFunction(() => !window.__physboxShowcase.dragging);
  await page.locator("#reset-button").evaluate((b) => b.click());
  await page.waitForFunction(
    (count) =>
      window.__physboxShowcase.frames >= 8 &&
      window.__physboxShowcase.fixedSteps <= 1 &&
      window.__physboxShowcase.liveBodies === 13 &&
      window.__physboxShowcase.particleCount === count,
    result.initial.particleCount,
  );
  await page
    .getByRole("button", { name: "Drop tower" })
    .evaluate((b) => b.click());
  await page.waitForFunction(() => window.__physboxShowcase.fixedSteps >= 8);
  return { pick: point, heldSteps: 15, released: true, replayed: true };
}
