import init, { start } from "./pkg/physbox_showcase_web.js";
import { webGpuUnavailableMessage } from "./startup-diagnostics.js";

const scenes = {
  playground: {
    title: "Playground",
    description: "Mixed circles and boxes share one GPU-resident rigid world.",
    defaultBodies: 256,
    defaultWater: 0,
  },
  "stack-and-sleep": {
    title: "Stack and sleep",
    description: "A compact stability stack settles, groups, and sleeps entirely on the GPU.",
    defaultBodies: 256,
    defaultWater: 0,
  },
  "water-and-rigid": {
    title: "Water and rigid bodies",
    description:
      "Preview water and rigid shapes exchange motion in one coupled GPU world.",
    defaultBodies: 256,
    defaultWater: 4096,
  },
  "tower-drop-13k": {
    title: "Tower drop · 13K",
    description: "Drop ten masonry blocks into the pool. Reset sets up the same shot.",
    defaultBodies: 64,
    defaultWater: 16384,
    staged: true,
  },
  "tower-drop-32k": {
    title: "Tower drop · 32K",
    description: "The same tower and waterline with a denser pool.",
    defaultBodies: 64,
    defaultWater: 32768,
    staged: true,
  },
  "rigid-stress-1k": {
    title: "Rigid stress 1K",
    description: "Authors exactly 1,000 bodies in independent settling stacks.",
    defaultBodies: 1000,
    defaultWater: 0,
  },
  "rigid-stress-2k": {
    title: "Rigid stress 2K",
    description: "Authors exactly 2,000 bodies in independent settling stacks.",
    defaultBodies: 2000,
    defaultWater: 0,
  },
  "rigid-stress-5k": {
    title: "Rigid stress 5K",
    description: "Authors exactly 5,000 bodies; raise Max bodies for placement headroom.",
    defaultBodies: 5000,
    defaultWater: 0,
  },
  "rigid-stress-10k": {
    title: "Rigid stress 10K",
    description: "Authors exactly 10,000 bodies; use Pan and wheel/trackpad zoom to inspect.",
    defaultBodies: 10000,
    defaultWater: 0,
  },
  "water-stress-4k": {
    title: "Water stress 4K",
    description: "Fills about 80% of the 4K water tier with coupled rigid shapes.",
    defaultBodies: 256,
    defaultWater: 4096,
  },
  "water-stress-8k": {
    title: "Water stress 8K",
    description: "Fills about 80% of the 8K water tier with coupled rigid shapes.",
    defaultBodies: 256,
    defaultWater: 8192,
  },
  "water-stress-16k": {
    title: "Water stress 16K",
    description: "Heavy: fills about 80% of the 16K water tier.",
    defaultBodies: 256,
    defaultWater: 16384,
  },
};

const canvas = document.querySelector("#showcase-canvas");
const sceneSelect = document.querySelector("#scene-select");
const toolButtons = [...document.querySelectorAll(".tool-button")];
const waterToolButton = document.querySelector("#tool-water");
const bodyCapacitySelect = document.querySelector("#body-capacity-select");
const waterCapacitySelect = document.querySelector("#water-capacity-select");
const physicsSelect = document.querySelector("#physics-select");
const sleepVisualsInput = document.querySelector("#sleep-visuals-input");
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
let waitingForDrop = false;
let failed = false;
let configDirty = false;
let pointerActive = false;
let selectedTool = "drag";
let sleepVisualization = true;
let lastPointer = null;
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
  applySceneDefaults(scene);
  updateSceneCopy(scene);
  resizeCanvas();
  app = await start(canvas, scene);
  app.setSleepVisualization(sleepVisualization);
  setInitialPlayback(scene);
  installControls();
  installPointerInput();
  installKeyboardInput();
  updateToolAvailability();
  updateHint();
  new ResizeObserver(resizeCanvas).observe(canvas);
  addEventListener("resize", resizeCanvas);
  requestAnimationFrame(renderFrame);
}

function installControls() {
  sceneSelect.addEventListener("change", () => {
    const scene = sceneSelect.value;
    applySceneDefaults(scene);
    applyConfiguration(scene);
    history.replaceState(null, "", `#${scene}`);
    updateSceneCopy(scene);
  });
  for (const button of toolButtons) {
    button.addEventListener("click", () => setTool(button.dataset.tool));
  }
  for (const control of [
    bodyCapacitySelect,
    waterCapacitySelect,
    physicsSelect,
  ]) {
    control.addEventListener("change", () => {
      configDirty = true;
      configState.textContent = "pending Reset";
      configState.classList.add("pending");
      configState.classList.remove("error");
    });
  }
  pauseButton.addEventListener("click", () => {
    waitingForDrop = false;
    paused = !paused;
    app.setPaused(paused);
    updatePause();
  });
  resetButton.addEventListener("click", () => applyConfiguration(sceneSelect.value));
  sleepVisualsInput.addEventListener("change", () => {
    sleepVisualization = sleepVisualsInput.checked;
    app.setSleepVisualization(sleepVisualization);
  });
}

function installPointerInput() {
  const phases = { pointerdown: 0, pointermove: 1, pointerup: 2, pointercancel: 3 };
  for (const [name, phase] of Object.entries(phases)) {
    canvas.addEventListener(name, (event) => {
      event.preventDefault();
      if (name === "pointerdown") {
        pointerActive = true;
        canvas.focus({ preventScroll: true });
        lastPointer = [xFromEvent(event), yFromEvent(event)];
        canvas.setPointerCapture(event.pointerId);
      }
      const bounds = canvas.getBoundingClientRect();
      const x = (event.clientX - bounds.left) / Math.max(bounds.width, 1);
      const y = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
      if (selectedTool === "drag") {
        app.pointerEvent(phase, x, y);
      } else if (selectedTool === "box" && name === "pointerdown") {
        app.placeBox(x, y);
      } else if (
        selectedTool === "water" &&
        (name === "pointerdown" || (name === "pointermove" && pointerActive))
      ) {
        app.emitWater(x, y);
      } else if (selectedTool === "pan" && name === "pointermove" && pointerActive) {
        const [lastX, lastY] = lastPointer ?? [x, y];
        app.panView(x - lastX, y - lastY);
      }
      lastPointer = [x, y];
      if (name === "pointerup" || name === "pointercancel") {
        pointerActive = false;
        lastPointer = null;
      }
    });
  }
  canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const bounds = canvas.getBoundingClientRect();
      const x = (event.clientX - bounds.left) / Math.max(bounds.width, 1);
      const y = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
      const delta = Math.max(-500, Math.min(500, event.deltaY));
      app.zoomView(x, y, Math.exp(delta * 0.0015));
    },
    { passive: false },
  );
}

function installKeyboardInput() {
  addEventListener("keydown", (event) => {
    if (
      event.defaultPrevented ||
      ["INPUT", "SELECT", "BUTTON", "TEXTAREA"].includes(event.target?.tagName)
    ) {
      return;
    }
    const panByKey = {
      ArrowLeft: [0.08, 0],
      ArrowRight: [-0.08, 0],
      ArrowUp: [0, 0.08],
      ArrowDown: [0, -0.08],
    };
    const delta = panByKey[event.key];
    if (!delta) return;
    event.preventDefault();
    app.panView(delta[0], delta[1]);
  });
}

function applyConfiguration(scene) {
  try {
    app.configureAndReset(
      scene,
      Number.parseInt(bodyCapacitySelect.value, 10),
      Number.parseInt(waterCapacitySelect.value, 10),
      Number.parseInt(physicsSelect.value, 10),
    );
    app.setSleepVisualization(sleepVisualization);
  } catch (error) {
    configDirty = true;
    configState.textContent = error instanceof Error ? error.message : String(error);
    configState.classList.remove("pending");
    configState.classList.add("error");
    return;
  }
  configDirty = false;
  configState.textContent = "";
  configState.classList.remove("pending");
  configState.classList.remove("error");
  setInitialPlayback(scene);
  updateToolAvailability();
  updateHint();
}

function xFromEvent(event) {
  const bounds = canvas.getBoundingClientRect();
  return (event.clientX - bounds.left) / Math.max(bounds.width, 1);
}

function yFromEvent(event) {
  const bounds = canvas.getBoundingClientRect();
  return (event.clientY - bounds.top) / Math.max(bounds.height, 1);
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
      sleepVisualization: app.sleepVisualization,
      tool: selectedTool,
      toolNotice: app.toolNotice ?? null,
      configDirty,
      dragging: app.dragging,
      cameraCenterX: app.cameraCenterX,
      cameraCenterY: app.cameraCenterY,
      cameraHalfHeight: app.cameraHalfHeight,
      crossOriginIsolated: globalThis.crossOriginIsolated,
    };
    requestAnimationFrame(renderFrame);
  } catch (error) {
    fail(error);
  }
}

function sceneFromHash() {
  const candidate = location.hash.slice(1);
  return Object.hasOwn(scenes, candidate) ? candidate : "water-and-rigid";
}

function updateSceneCopy(scene) {
  sceneHeading.textContent = scenes[scene].title;
  sceneDescription.textContent = scenes[scene].description;
}

function applySceneDefaults(scene) {
  if (scenes[scene].staged) physicsSelect.value = "60";
  bodyCapacitySelect.value = String(scenes[scene].defaultBodies);
  waterCapacitySelect.value = String(scenes[scene].defaultWater);
}

function updateToolAvailability() {
  waterToolButton.disabled = !app.waterAvailable;
  if (waterToolButton.disabled && selectedTool === "water") {
    setTool("drag");
  } else {
    updateToolButtons();
  }
}

function setTool(tool) {
  const button = toolButtons.find((candidate) => candidate.dataset.tool === tool);
  if (!button || button.disabled) return;
  selectedTool = tool;
  updateToolButtons();
  updateHint();
}

function updateToolButtons() {
  for (const button of toolButtons) {
    button.setAttribute("aria-pressed", String(button.dataset.tool === selectedTool));
  }
  canvas.dataset.tool = selectedTool;
}

function updateHint() {
  const hints = {
    drag: "Drag any moving body",
    box: "Click to place a standard box",
    water: "Click or drag to add preview water",
    pan: "Drag to pan; wheel or trackpad to zoom",
  };
  sceneHint.textContent = hints[selectedTool];
  canvas.dataset.tool = selectedTool;
}

function setInitialPlayback(scene) {
  waitingForDrop = Boolean(scenes[scene].staged);
  paused = waitingForDrop;
  app.setPaused(paused);
  updatePause();
}

function updatePause() {
  pauseButton.textContent = waitingForDrop ? "Drop tower" : paused ? "Resume" : "Pause";
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
