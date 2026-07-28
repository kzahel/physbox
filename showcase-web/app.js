import init, { start } from "./pkg/physbox_showcase_web.js";
import { webGpuUnavailableMessage } from "./startup-diagnostics.js";

const scenes = {
  playground: {
    title: "Playground",
    description: "Mixed circles and boxes share one GPU-resident rigid world.",
    water: false,
  },
  "stack-and-sleep": {
    title: "Stack and sleep",
    description: "A compact stability stack settles, groups, and sleeps entirely on the GPU.",
    water: false,
  },
  "water-and-rigid": {
    title: "Water and rigid bodies",
    description:
      "Preview water and rigid shapes exchange motion in one coupled GPU world.",
    water: true,
  },
  "rigid-stress": {
    title: "Rigid stress",
    description: "Fills about 90% of the selected body tier and leaves placement headroom.",
    water: false,
  },
  "water-stress": {
    title: "Water stress",
    description: "Fills about 80% of the selected water tier with coupled rigid shapes.",
    water: true,
  },
};

const canvas = document.querySelector("#showcase-canvas");
const sceneSelect = document.querySelector("#scene-select");
const toolSelect = document.querySelector("#tool-select");
const bodyCapacitySelect = document.querySelector("#body-capacity-select");
const waterCapacitySelect = document.querySelector("#water-capacity-select");
const physicsSelect = document.querySelector("#physics-select");
const sceneHeading = document.querySelector("#scene-heading");
const sceneDescription = document.querySelector("#scene-description");
const sceneHint = document.querySelector("#scene-hint");
const pauseButton = document.querySelector("#pause-button");
const resetButton = document.querySelector("#reset-button");
const configState = document.querySelector("#config-state");
const statusText = document.querySelector("#status-text");
const fatalError = document.querySelector("#fatal-error");

let app;
let paused = false;
let failed = false;
let configDirty = false;
let pointerActive = false;
let frameTimes = [];
let mainThreadMs = 0;

async function boot() {
  if (!navigator.gpu) {
    throw new Error(
      webGpuUnavailableMessage({
        isSecureContext: globalThis.isSecureContext,
        protocol: location.protocol,
        host: location.host,
      }),
    );
  }
  await init();
  const scene = sceneFromHash();
  sceneSelect.value = scene;
  updateSceneCopy(scene);
  resizeCanvas();
  app = await start(canvas, scene);
  installControls();
  installPointerInput();
  updateToolAvailability();
  updateHint();
  new ResizeObserver(resizeCanvas).observe(canvas);
  addEventListener("resize", resizeCanvas);
  requestAnimationFrame(renderFrame);
}

function installControls() {
  sceneSelect.addEventListener("change", () => {
    const scene = sceneSelect.value;
    applyConfiguration(scene);
    history.replaceState(null, "", `#${scene}`);
    updateSceneCopy(scene);
  });
  toolSelect.addEventListener("change", updateHint);
  for (const control of [
    bodyCapacitySelect,
    waterCapacitySelect,
    physicsSelect,
  ]) {
    control.addEventListener("change", () => {
      configDirty = true;
      configState.textContent = "pending Reset";
      configState.classList.add("pending");
    });
  }
  pauseButton.addEventListener("click", () => {
    paused = !paused;
    app.setPaused(paused);
    updatePause();
  });
  resetButton.addEventListener("click", () => applyConfiguration(sceneSelect.value));
}

function installPointerInput() {
  const phases = { pointerdown: 0, pointermove: 1, pointerup: 2, pointercancel: 3 };
  for (const [name, phase] of Object.entries(phases)) {
    canvas.addEventListener(name, (event) => {
      event.preventDefault();
      if (name === "pointerdown") {
        pointerActive = true;
        canvas.setPointerCapture(event.pointerId);
      }
      const bounds = canvas.getBoundingClientRect();
      const x = (event.clientX - bounds.left) / Math.max(bounds.width, 1);
      const y = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
      if (toolSelect.value === "drag") {
        app.pointerEvent(phase, x, y);
      } else if (toolSelect.value === "box" && name === "pointerdown") {
        app.placeBox(x, y);
      } else if (
        toolSelect.value === "water" &&
        (name === "pointerdown" || (name === "pointermove" && pointerActive))
      ) {
        app.emitWater(x, y);
      }
      if (name === "pointerup" || name === "pointercancel") pointerActive = false;
    });
  }
}

function applyConfiguration(scene) {
  app.configureAndReset(
    scene,
    Number.parseInt(bodyCapacitySelect.value, 10),
    Number.parseInt(waterCapacitySelect.value, 10),
    Number.parseInt(physicsSelect.value, 10),
  );
  configDirty = false;
  configState.textContent = "";
  configState.classList.remove("pending");
  paused = false;
  updatePause();
  updateToolAvailability();
  updateHint();
}

function resizeCanvas() {
  if (failed) return;
  const bounds = canvas.getBoundingClientRect();
  const scale = Math.max(devicePixelRatio || 1, 1);
  const width = Math.max(1, Math.round(bounds.width * scale));
  const height = Math.max(1, Math.round(bounds.height * scale));
  if (canvas.width === width && canvas.height === height) return;
  canvas.width = width;
  canvas.height = height;
  app?.resize(width, height);
}

function renderFrame(timestamp) {
  if (failed) return;
  try {
    const before = performance.now();
    app.render(timestamp);
    const elapsed = performance.now() - before;
    mainThreadMs = mainThreadMs === 0 ? elapsed : mainThreadMs * 0.9 + elapsed * 0.1;
    frameTimes.push(timestamp);
    while (frameTimes.length > 1 && timestamp - frameTimes[0] > 1_000) {
      frameTimes.shift();
    }
    const fps =
      frameTimes.length > 1
        ? ((frameTimes.length - 1) * 1_000) /
          Math.max(frameTimes.at(-1) - frameTimes[0], 1)
        : 0;
    const water = app.particleCapacity
      ? ` | water ${app.particleCount}/${app.particleCapacity}` +
        (app.droppedParticles ? ` (${app.droppedParticles} dropped)` : "")
      : "";
    const gpuFrameMs = app.gpuFrameMs;
    const gpu =
      app.gpuTimingAvailable && Number.isFinite(gpuFrameMs)
        ? `GPU ${gpuFrameMs.toFixed(2)} ms (physics ${app.gpuPhysicsMs.toFixed(2)}, render ${app.gpuRenderMs.toFixed(2)})`
        : "GPU n/a";
    const notice = app.toolNotice ? ` | ${app.toolNotice}` : "";
    statusText.textContent =
      `FPS ${fps.toFixed(0)} | physics ${app.admittedPhysicsHz.toFixed(1)}/${app.physicsHz} Hz` +
      ` | scale ${app.effectiveSimulationScale.toFixed(2)}x` +
      ` | main ${mainThreadMs.toFixed(2)} ms | ${gpu}` +
      ` | bodies ${app.liveBodies}/${app.bodyCapacity}${water}${notice}`;
    globalThis.__physboxShowcase = {
      ready: app.frameCount > 1,
      scene: app.scene,
      frames: Number(app.frameCount),
      fixedSteps: Number(app.fixedSteps),
      fixedStepsThisFrame: app.fixedStepsThisFrame,
      liveBodies: app.liveBodies,
      bodyCapacity: app.bodyCapacity,
      particleCount: app.particleCount,
      particleCapacity: app.particleCapacity,
      droppedParticles: app.droppedParticles,
      physicsHz: app.physicsHz,
      admittedPhysicsHz: app.admittedPhysicsHz,
      effectiveSimulationScale: app.effectiveSimulationScale,
      displayFps: fps,
      mainThreadMs,
      gpuFrameMs: Number.isFinite(gpuFrameMs) ? gpuFrameMs : null,
      gpuPhysicsMs: app.gpuPhysicsMs ?? null,
      gpuRenderMs: app.gpuRenderMs ?? null,
      gpuTimingAvailable: app.gpuTimingAvailable,
      gpuTimingError: app.gpuTimingError ?? null,
      waterAvailable: app.waterAvailable,
      tool: toolSelect.value,
      toolNotice: app.toolNotice ?? null,
      configDirty,
      dragging: app.dragging,
      crossOriginIsolated: globalThis.crossOriginIsolated,
    };
    requestAnimationFrame(renderFrame);
  } catch (error) {
    fail(error);
  }
}

function sceneFromHash() {
  const candidate = location.hash.slice(1);
  return Object.hasOwn(scenes, candidate) ? candidate : "playground";
}

function updateSceneCopy(scene) {
  sceneHeading.textContent = scenes[scene].title;
  sceneDescription.textContent = scenes[scene].description;
}

function updateToolAvailability() {
  const waterOption = toolSelect.querySelector('option[value="water"]');
  waterOption.disabled = !scenes[sceneSelect.value].water;
  if (waterOption.disabled && toolSelect.value === "water") {
    toolSelect.value = "drag";
  }
  canvas.dataset.tool = toolSelect.value;
}

function updateHint() {
  const hints = {
    drag: "Drag any moving body",
    box: "Click to place a standard box",
    water: "Click or drag to add preview water",
  };
  sceneHint.textContent = hints[toolSelect.value];
  canvas.dataset.tool = toolSelect.value;
}

function updatePause() {
  pauseButton.textContent = paused ? "Resume" : "Pause";
  pauseButton.setAttribute("aria-pressed", String(paused));
}

function fail(error) {
  failed = true;
  const message = error instanceof Error ? error.message : String(error);
  fatalError.textContent = `PhysBox could not start: ${message}`;
  fatalError.hidden = false;
  statusText.textContent =
    !globalThis.isSecureContext && !navigator.gpu ? "Secure HTTPS required" : "Startup failed";
  globalThis.__physboxShowcase = { ready: false, error: message };
  console.error(error);
}

boot().catch(fail);
