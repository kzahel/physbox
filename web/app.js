import init, { box2dComparatorEnabled, PointerButtonCode, start } from "./pkg/physbox_web.js";

const canvas = document.querySelector("#physbox-canvas");
const fatalError = document.querySelector("#fatal-error");
let app;
let observer;
let renderingPaused = false;

async function boot() {
  if (!navigator.gpu) throw new Error("WebGPU is not available in this browser");
  await init();
  let comparatorRuntime;
  if (box2dComparatorEnabled()) {
    const { loadBox2dComparator } = await import("./box2d-comparator/loader.js");
    comparatorRuntime = await loadBox2dComparator();
  }
  app = await start(canvas, location.search, comparatorRuntime);
  if (app.observerEnabled()) {
    const { installSmokeObserver } = await import("./smoke-observer.js");
    observer = installSmokeObserver(app, {
      pause() { renderingPaused = true; },
      resume() {
        if (!renderingPaused) return;
        renderingPaused = false;
        requestAnimationFrame(renderFrame);
      },
    });
  }
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
      observer?.resize(width, height);
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
    app.render(timestamp);
    observer?.capture();
    if (!renderingPaused) requestAnimationFrame(renderFrame);
  } catch (error) {
    fail(error);
  }
}

function fail(error) {
  const message = error instanceof Error ? error.message : String(error);
  fatalError.textContent = `PhysBox 5 could not start: ${message}`;
  fatalError.hidden = false;
  observer?.fail(message);
  console.error(error);
}

boot().catch(fail);
