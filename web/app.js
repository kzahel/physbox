import init, { PointerButtonCode, start } from "./pkg/physbox_web.js";

const canvas = document.querySelector("#physbox-canvas");
const fatalError = document.querySelector("#fatal-error");
globalThis.__physbox5 = { ready: false, ok: false };
let renderingPaused = false;
const physicsPreferenceKey = "physbox5.physics.v3";
const priorPhysicsPreferenceKey = "physbox5.physics.v2";
const worldPreferenceKey = "physbox5.world.v1";
const legacySolverPreferenceKey = "physbox5.solver-lab.v1";
const worldCapacityQueryKey = "body-capacity";
const physicsQueryKeys = Object.freeze([
  "solver",
  "sim-hz",
  "substeps",
  "iterations",
  "contact-hz",
  "contact-damping",
  "static-contact-scale",
  "speculative-distance",
  "max-depenetration-velocity",
  "sleeping",
  "solver-options",
]);
let lastPhysicsPreference = "";
let lastWorldPreference = "";
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
  const app = await start(canvas, startupSearch());
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
    syncPhysicsPreference(report);
    syncWorldPreference(report);
    globalThis.__physbox5.frame = report;
    globalThis.__physbox5.ready = report.scenario.complete;
    if (!renderingPaused) requestAnimationFrame(renderFrame);
  } catch (error) {
    fail(error);
  }
}

function startupSearch() {
  const params = new URLSearchParams(location.search);
  try {
    if (!physicsQueryKeys.some((key) => params.has(key))) {
      const saved = loadPhysicsPreference();
      if (saved) writePhysicsQuery(params, saved);
    }
    if (!params.has(worldCapacityQueryKey)) {
      const saved = loadWorldPreference();
      if (saved) params.set(worldCapacityQueryKey, String(saved.bodyCapacity));
    }
  } catch (error) {
    console.warn("Ignoring invalid PhysBox preference", error);
  }
  return `?${params}`;
}

function syncWorldPreference(report) {
  const contract = report.sceneContract;
  const preference = {
    version: 1,
    bodyCapacity: contract?.requestedBodyCapacity,
  };
  if (!validWorldPreference(preference)) return;
  const saved = JSON.stringify(preference);
  if (saved === lastWorldPreference) return;
  lastWorldPreference = saved;
  try {
    localStorage.setItem(worldPreferenceKey, saved);
  } catch (error) {
    console.warn("Could not persist PhysBox world preference", error);
  }
  const url = new URL(location.href);
  if (preference.bodyCapacity === 64) {
    url.searchParams.delete(worldCapacityQueryKey);
  } else {
    url.searchParams.set(worldCapacityQueryKey, String(preference.bodyCapacity));
  }
  history.replaceState(null, "", url);
}

function loadWorldPreference() {
  const saved = JSON.parse(localStorage.getItem(worldPreferenceKey) ?? "null");
  return validWorldPreference(saved) ? saved : null;
}

function validWorldPreference(saved) {
  return saved?.version === 1
    && [64, 256, 1000, 10000].includes(saved.bodyCapacity);
}

function syncPhysicsPreference(report) {
  const contract = report.sceneContract;
  if (!contract || typeof contract.solverProfile !== "string") return;
  const preference = {
    version: 3,
    solver: contract.solverProfile,
    simHz: contract.fixedStepHz,
    substeps: contract.solverSubsteps,
    iterations: contract.solverBiasedIterations,
    contactHz: contract.requestedContactHz,
    contactDamping: contract.contactDampingRatio,
    staticContactScale: contract.requestedStaticContactHz / contract.requestedContactHz,
    speculativeDistance: contract.speculativeDistance,
    maxDepenetrationVelocity: contract.maxDepenetrationVelocity,
    sleeping: contract.requestedSleepingEnabled,
  };
  if (!validPhysicsPreference(preference)) return;
  const saved = JSON.stringify(preference);
  if (saved === lastPhysicsPreference) return;
  lastPhysicsPreference = saved;
  try {
    localStorage.setItem(physicsPreferenceKey, saved);
    localStorage.removeItem(priorPhysicsPreferenceKey);
    localStorage.removeItem(legacySolverPreferenceKey);
  } catch (error) {
    console.warn("Could not persist PhysBox physics preference", error);
  }
  const url = new URL(location.href);
  for (const key of physicsQueryKeys) url.searchParams.delete(key);
  if (isDefaultPhysicsPreference(preference)) {
    history.replaceState(null, "", url);
  } else {
    writePhysicsQuery(url.searchParams, preference);
    history.replaceState(null, "", url);
  }
}

function loadPhysicsPreference() {
  const saved = JSON.parse(localStorage.getItem(physicsPreferenceKey) ?? "null");
  if (validPhysicsPreference(saved)) return saved;

  const prior = JSON.parse(localStorage.getItem(priorPhysicsPreferenceKey) ?? "null");
  if (prior?.version === 2) {
    const migrated = { ...prior, version: 3, sleeping: false };
    if (validPhysicsPreference(migrated)) return migrated;
  }

  const legacy = JSON.parse(localStorage.getItem(legacySolverPreferenceKey) ?? "null");
  if (
    legacy?.version !== 1
    || !["selected", "branch-robust"].includes(legacy.solver)
    || (legacy.options !== null && typeof legacy.options !== "string")
  ) return null;
  const migrated = {
    version: 3,
    solver: legacy.solver === "selected" ? "hard-direct" : "soft-coupled",
    simHz: 60,
    substeps: 4,
    iterations: legacy.solver === "selected" ? 2 : 1,
    contactHz: 30,
    contactDamping: 10,
    staticContactScale: 2,
    speculativeDistance: 0.02,
    maxDepenetrationVelocity: 3,
    sleeping: false,
  };
  if (typeof legacy.options === "string") {
    for (const field of legacy.options.split(",")) {
      const [name, encoded] = field.split("=", 2);
      const value = Number(encoded);
      if (name === "hz") migrated.contactHz = value;
      else if (name === "damping") migrated.contactDamping = value;
      else if (name === "static-scale") migrated.staticContactScale = value;
      else return null;
    }
  }
  return validPhysicsPreference(migrated) ? migrated : null;
}

function validPhysicsPreference(saved) {
  if (
    saved?.version !== 3
    || !["soft-coupled", "hard-direct"].includes(saved.solver)
  ) return false;
  const integerBounds = [
    [saved.simHz, 15, 240],
    [saved.substeps, 1, 8],
    [saved.iterations, 1, 4],
  ];
  const numberBounds = [
    [saved.contactHz, 1, 120],
    [saved.contactDamping, 0.5, 20],
    [saved.staticContactScale, 0.5, 16],
    [saved.speculativeDistance, 0, 1],
    [saved.maxDepenetrationVelocity, 0.1, 100],
  ];
  return typeof saved.sleeping === "boolean"
    && integerBounds.every(([value, minimum, maximum]) =>
    Number.isInteger(value) && value >= minimum && value <= maximum
  ) && numberBounds.every(([value, minimum, maximum]) =>
    Number.isFinite(value) && value >= minimum && value <= maximum
  ) && (saved.solver !== "hard-direct"
    || (saved.simHz === 60 && saved.substeps === 4 && saved.iterations === 2));
}

function writePhysicsQuery(params, preference) {
  params.set("solver", preference.solver);
  params.set("sim-hz", String(preference.simHz));
  params.set("substeps", String(preference.substeps));
  params.set("iterations", String(preference.iterations));
  params.set("contact-hz", String(preference.contactHz));
  params.set("contact-damping", String(preference.contactDamping));
  params.set("static-contact-scale", String(preference.staticContactScale));
  params.set("speculative-distance", String(preference.speculativeDistance));
  params.set("max-depenetration-velocity", String(preference.maxDepenetrationVelocity));
  params.set("sleeping", preference.sleeping ? "on" : "off");
}

function isDefaultPhysicsPreference(preference) {
  return preference.solver === "soft-coupled"
    && preference.simHz === 60
    && preference.substeps === 4
    && preference.iterations === 1
    && preference.contactHz === 30
    && preference.contactDamping === 10
    && preference.staticContactScale === 2
    && preference.speculativeDistance === 0.02
    && preference.maxDepenetrationVelocity === 3
    && preference.sleeping === false;
}

function fail(error) {
  const message = error instanceof Error ? error.message : String(error);
  fatalError.textContent = `PhysBox 5 could not start: ${message}`;
  fatalError.hidden = false;
  globalThis.__physbox5 = { ready: true, ok: false, error: message };
  console.error(error);
}

boot().catch(fail);
