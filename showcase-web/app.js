import init, { start } from "./pkg/physbox_showcase_web.js";

const scenes = {
  playground: {
    title: "Playground",
    description: "Mixed circles and boxes share one GPU-resident rigid world.",
  },
  "stack-and-sleep": {
    title: "Stack and sleep",
    description: "A compact stability stack settles, groups, and sleeps entirely on the GPU.",
  },
  "water-and-rigid": {
    title: "Water and rigid bodies",
    description:
      "Preview water and rigid shapes exchange motion in one coupled GPU world.",
  },
};

const canvas = document.querySelector("#showcase-canvas");
const sceneSelect = document.querySelector("#scene-select");
const sceneHeading = document.querySelector("#scene-heading");
const sceneDescription = document.querySelector("#scene-description");
const pauseButton = document.querySelector("#pause-button");
const resetButton = document.querySelector("#reset-button");
const statusText = document.querySelector("#status-text");
const fatalError = document.querySelector("#fatal-error");

let app;
let paused = false;
let failed = false;

async function boot() {
  if (!navigator.gpu) throw new Error("WebGPU is not available in this browser");
  await init();
  const scene = sceneFromHash();
  sceneSelect.value = scene;
  updateSceneCopy(scene);
  resizeCanvas();
  app = await start(canvas, scene);
  installControls();
  installPointerInput();
  new ResizeObserver(resizeCanvas).observe(canvas);
  addEventListener("resize", resizeCanvas);
  requestAnimationFrame(renderFrame);
}

function installControls() {
  sceneSelect.addEventListener("change", () => {
    const scene = sceneSelect.value;
    app.selectScene(scene);
    history.replaceState(null, "", `#${scene}`);
    updateSceneCopy(scene);
    paused = false;
    updatePause();
  });
  pauseButton.addEventListener("click", () => {
    paused = !paused;
    app.setPaused(paused);
    updatePause();
  });
  resetButton.addEventListener("click", () => app.reset());
}

function installPointerInput() {
  const phases = { pointerdown: 0, pointermove: 1, pointerup: 2, pointercancel: 3 };
  for (const [name, phase] of Object.entries(phases)) {
    canvas.addEventListener(name, (event) => {
      event.preventDefault();
      if (name === "pointerdown") canvas.setPointerCapture(event.pointerId);
      const bounds = canvas.getBoundingClientRect();
      app.pointerEvent(
        phase,
        (event.clientX - bounds.left) / Math.max(bounds.width, 1),
        (event.clientY - bounds.top) / Math.max(bounds.height, 1),
      );
    });
  }
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
    app.render(timestamp);
    const particles = app.particleCount
      ? `${app.particleCount} water particles · `
      : "";
    statusText.textContent =
      `${app.liveBodies} bodies · ${particles}${app.fixedSteps} fixed steps`;
    globalThis.__physboxShowcase = {
      ready: app.frameCount > 1,
      scene: app.scene,
      frames: Number(app.frameCount),
      fixedSteps: Number(app.fixedSteps),
      fixedStepsThisFrame: app.fixedStepsThisFrame,
      liveBodies: app.liveBodies,
      particleCount: app.particleCount,
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

function updatePause() {
  pauseButton.textContent = paused ? "Resume" : "Pause";
  pauseButton.setAttribute("aria-pressed", String(paused));
}

function fail(error) {
  failed = true;
  const message = error instanceof Error ? error.message : String(error);
  fatalError.textContent = `PhysBox could not start: ${message}`;
  fatalError.hidden = false;
  statusText.textContent = "WebGPU unavailable";
  globalThis.__physboxShowcase = { ready: false, error: message };
  console.error(error);
}

boot().catch(fail);
