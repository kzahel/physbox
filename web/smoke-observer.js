export function installSmokeObserver(app, frameControl) {
  globalThis.__physbox5 = {
    ready: false,
    ok: true,
    diagnostics: JSON.parse(app.diagnosticsJson()),
  };
  globalThis.__physbox5FrameControl = Object.freeze(frameControl);

  return Object.freeze({
    capture() {
      const frame = JSON.parse(app.observerSnapshot());
      globalThis.__physbox5.frame = frame;
      globalThis.__physbox5.ready = frame.scenario.complete;
    },
    resize(width, height) {
      globalThis.__physbox5.viewport = { width, height };
    },
    fail(message) {
      globalThis.__physbox5 = { ready: true, ok: false, error: message };
    },
  });
}
