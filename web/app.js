import init, { PointerButtonCode, solverLabEnabled, start } from "./pkg/physbox_web.js";

const canvas = document.querySelector("#physbox-canvas");
const fatalError = document.querySelector("#fatal-error");
globalThis.__physbox5 = { ready: false, ok: false };
let renderingPaused = false;
const solverPreferenceKey = "physbox5.solver-lab.v1";
let solverLab = false;
let lastSolverPreference = "";
globalThis.__physbox5FrameControl = Object.freeze({
  pause() { renderingPaused = true; },
  resume() {
    if (!renderingPaused) return;
    renderingPaused = false;
    requestAnimationFrame(renderFrame);
  },
});

async function boot() {
  if (!navigator.gpu) throw new Error("WebGPU is not available in this browser");
  await init();
  solverLab = solverLabEnabled();
  const app = await start(canvas, solverLab ? startupSearch() : location.search);
  const diagnostics = JSON.parse(app.diagnosticsJson());
  globalThis.__physbox5App = app;
  globalThis.__physbox5 = { ready: false, ok: true, diagnostics };
  installResize(app);
  installPointerInput(app);
  requestAnimationFrame(renderFrame);
}

function installResize(app) {
  let queued = false;
  const resize = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      const bounds = canvas.getBoundingClientRect();
      const scale = Math.max(devicePixelRatio || 1, 1);
      const width = Math.max(1, Math.round(bounds.width * scale));
      const height = Math.max(1, Math.round(bounds.height * scale));
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
        app.resize(width, height);
      }
      globalThis.__physbox5.viewport = { width, height };
    });
  };
  new ResizeObserver(resize).observe(canvas);
  addEventListener("resize", resize);
  resize();
}

function installPointerInput(app) {
  const phase = { pointerdown: 0, pointermove: 1, pointerup: 2, pointercancel: 3 };
  for (const name of Object.keys(phase)) {
    canvas.addEventListener(name, (event) => {
      event.preventDefault();
      if (name === "pointerdown") canvas.setPointerCapture(event.pointerId);
      const bounds = canvas.getBoundingClientRect();
      const x = (event.clientX - bounds.left) / Math.max(bounds.width, 1);
      const y = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
      const button = event.pointerType === "touch"
        ? PointerButtonCode.Contact
        : event.button === 2
          ? PointerButtonCode.Secondary
          : event.button === 1
            ? PointerButtonCode.Auxiliary
            : PointerButtonCode.Primary;
      app.pointerEvent(event.pointerId, phase[name], x, y, button);
    });
  }
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const bounds = canvas.getBoundingClientRect();
    const x = (event.clientX - bounds.left) / Math.max(bounds.width, 1);
    const y = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
    app.wheelEvent(x, y, event.deltaY < 0 ? 1 : event.deltaY > 0 ? -1 : 0);
  }, { passive: false });
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
}

function renderFrame(timestamp) {
  if (renderingPaused) return;
  try {
    const report = JSON.parse(globalThis.__physbox5App.render(timestamp));
    syncSolverPreference(report);
    globalThis.__physbox5.frame = report;
    globalThis.__physbox5.ready = report.scenario.complete;
    if (!renderingPaused) requestAnimationFrame(renderFrame);
  } catch (error) {
    fail(error);
  }
}

function startupSearch() {
  const params = new URLSearchParams(location.search);
  if (params.has("solver")) return `?${params}`;
  try {
    const saved = JSON.parse(localStorage.getItem(solverPreferenceKey) ?? "null");
    if (
      saved?.version !== 1
      || !["selected", "branch-robust"].includes(saved.solver)
      || (saved.options !== null && typeof saved.options !== "string")
      || (saved.solver === "selected" && saved.options !== null)
    ) return `?${params}`;
    params.set("solver", saved.solver);
    if (typeof saved.options === "string" && saved.options.length > 0) {
      params.set("solver-options", saved.options);
    }
  } catch (error) {
    console.warn("Ignoring invalid PhysBox solver preference", error);
  }
  return `?${params}`;
}

function syncSolverPreference(report) {
  if (!solverLab) return;
  const contract = report.sceneContract;
  if (!contract || typeof contract.solverProfile !== "string") return;
  const saved = JSON.stringify({
    version: 1,
    solver: contract.solverProfile,
    options: contract.solverProfileOptions ?? null,
  });
  if (saved === lastSolverPreference) return;
  lastSolverPreference = saved;
  try {
    localStorage.setItem(solverPreferenceKey, saved);
  } catch (error) {
    console.warn("Could not persist PhysBox solver preference", error);
  }
  const url = new URL(location.href);
  if (contract.solverProfile === "selected") {
    url.searchParams.delete("solver");
    url.searchParams.delete("solver-options");
  } else {
    url.searchParams.set("solver", contract.solverProfile);
    if (contract.solverProfileOptions) {
      url.searchParams.set("solver-options", contract.solverProfileOptions);
    } else {
      url.searchParams.delete("solver-options");
    }
  }
  history.replaceState(null, "", url);
}

function fail(error) {
  const message = error instanceof Error ? error.message : String(error);
  fatalError.textContent = `PhysBox 5 could not start: ${message}`;
  fatalError.hidden = false;
  globalThis.__physbox5 = { ready: true, ok: false, error: message };
  console.error(error);
}

boot().catch(fail);
