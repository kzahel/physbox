# S38 — Web showcase foundation

Status: complete 2026-07-28. The rigid showcase builds as a separate public
`physbox-sim` consumer, serves locally without cross-origin isolation, and
passes its packaged Browser WebGPU interaction smoke. GitHub Pages remains
deliberately unconfigured.

Governing architecture:
[web showcase and public water consumption](../topics/web-showcase-and-public-water-consumption.md).
Library boundary:
[reusable GPU physics library boundary](../topics/reusable-gpu-library-boundary.md).
Pacing:
[simulation time and overload](../topics/simulation-time-and-overload.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Deliver a locally packaged, standalone PhysBox Web Showcase that is a genuine
public `physbox-sim` consumer. It owns a compact HTML scene selector and
presentation renderer, demonstrates rigid mechanics through canonical GPU
observation, supports direct manipulation, and remains ready for a later
static-subpath deployment without configuring or deploying GitHub Pages.

## Exclusions

- Do not add a Pages workflow, enable Pages, publish an artifact, push a
  deployment branch, configure a domain, or change repository visibility.
- Do not add Box2D, a comparator, benchmark claims, WebAssembly threads,
  `SharedArrayBuffer`, cross-origin isolation, or SIMD-specific code.
- Do not depend on first-party engine, launch, shell, UI, contracts,
  comparator, app, or private simulation features.
- Do not add a renderer, scene catalog, camera, DOM, or browser lifecycle to
  `physbox-sim`.
- Do not change physics equations, configuration defaults, workbench
  behavior, or the fullscreen leaf-adapter rule for `apps/web`.
- Do not add water before S39 supplies a supported public particle boundary.

## Ownership constraints

- The showcase crate is an application and may own DOM controls and semantic
  scene names.
- Mechanics, handles, fixed-step admission, interactions, and observations
  come only from public default-feature `physbox-sim`.
- The application owns the WGPU surface, target configuration, command
  encoder, submission, renderer, view transform, and visual metadata.
- Interactive time executes zero or one fixed outer step per presented frame
  and permanently slips excess whole-step wall time.
- Rendering consumes `GpuBodyObservation` on the same device without
  per-frame CPU pose readback.

## Ordered delivery slices

### S38.1 — Architecture and package boundary

- [x] Add the governing topic and this tactical in creation order.
- [x] Add the showcase application crate outside maintained production
      default members.
- [x] Add dependency negatives proving it imports only public
      default-feature `physbox-sim` among PhysBox crates.
- [x] Explicitly scope existing browser leaf checks to the existing
      workbench adapter while retaining their negative fixtures.

Gate: the dependency graph and platform checks distinguish an independent
consumer application from an `EngineHost` browser adapter.

### S38.2 — Caller-owned browser and renderer

- [x] Request the browser WebGPU adapter/device and configure a canvas surface.
- [x] Create and submit the caller-owned encoder each animation frame.
- [x] Render circles and rectangles from `GpuBodyObservation` plus
      consumer-owned immutable shape metadata.
- [x] Handle device-pixel-ratio resize and clear startup failures.
- [x] Keep render and step ordering GPU-to-GPU with no current-frame map/wait.

Gate: a local browser shows a stepped rigid scene entirely through the public
observation contract.

### S38.3 — Showcase scenes and controls

- [x] Add Playground and Stack and sleep scenes through public definitions.
- [x] Add an accessible HTML selector, reset and pause controls, concise scene
      description, and manipulation hint.
- [x] Map pointer gestures to public point-query/constraint operations and
      reconcile their terminal results asynchronously.
- [x] Preserve a stable query/hash scene URL without importing launch policy.
- [x] Confirm scene switching reconstructs consumer state without leaking
      animation loops, surfaces, or input handlers.

Gate: scene selection, reset, pause, resize, drag/release, and deep-link reload
work in a real browser.

### S38.4 — Local packaging and static-host readiness

- [x] Add one repository build command and one plain local static-server
      command.
- [x] Use only relative packaged assets and verify an arbitrary nested base
      path.
- [x] Verify the server sends no COOP/COEP requirement and the app reports
      `crossOriginIsolated === false`.
- [x] Add a Browser WebGPU smoke against the exact packaged output.
- [x] Document local review commands and the deliberate Pages hold.

Gate: a clean local build serves from its output directory and passes the
packaged browser smoke without isolation headers.

### S38.5 — Qualification and closure

- [x] Run focused showcase, dependency-negative, and browser tests.
- [x] Run `npm run test:fast`.
- [x] Run production warnings-denied clippy plus focused warnings-denied
      clippy for the new non-default app.
- [x] Record build size, browser run identity, and material limitations.
- [x] Update the topic, this tactical, and the status index with final
      evidence.

Gate: the standalone rigid showcase is reviewable locally without a new
mechanical implementation, publication action, or regression in maintained
hosts.

Implementation and qualification, 2026-07-28:

- `physbox-showcase-web` is a non-default workspace application depending on
  public default-feature `physbox-sim` as its only PhysBox crate. The boundary
  checker explicitly treats it as a consumer application while preserving the
  existing `apps/web` leaf rules and rejects private bridge or application
  dependencies.
- Its renderer projects the public 48-byte `GpuBodyObservation` records and
  consumer-owned shape/color metadata directly into an app-owned render pass.
  Simulation, observation, rendering, and presentation share the caller-owned
  encoder/submission; only bounded deferred pointer-command results map
  asynchronously.
- The package includes Playground and Stack and sleep, hash deep links,
  accessible selector/pause/reset controls, device-pixel-ratio resize, and
  deferred point-query/constraint dragging. It uses relative assets and the
  focused smoke serves it at `/showcase-web/`, proving nested-base behavior.
- `cargo test -p physbox-showcase-web`, the platform-boundary negatives, the
  debug WASM build, and focused warnings-denied clippy pass.
- `npm run test:fast` passes in 23.12 seconds, including maintained Browser
  WebGPU run `585b6aef-5060-4312-9bdf-0e5701c06d5a`; production
  warnings-denied clippy also passes.
- Browser WebGPU run `73c329cc-e975-4a12-a458-898d6c744f3b` passes scene
  switching, pause/resume, reset, drag/release, visible canvas, and clean
  browser-console gates. It reports `crossOriginIsolated=false`, sends no
  COOP/COEP headers, reaches 20 fixed steps in the final scene, and packages a
  3,232,288-byte debug WASM module. The inspected capture is
  `/tmp/physbox-showcase.png`; provenance is
  `/tmp/physbox-showcase.json`.
- This first slice remains rigid-only. Water is the next S39 slice; Box2D,
  performance claims, Pages configuration, repository visibility, and
  production deployment remain held.

## Stop conditions

Stop for human review if implementation requires a shared renderer, private
engine bridge, relaxed platform boundary, physics/default change, background
service, non-static host, cross-origin isolation, or a new product-facing scene
or brand decision beyond the catalog and naming already recorded.

## Completion condition

S38 is complete when a clean checkout can build and serve a standalone,
static-host-ready PhysBox Web Showcase locally; its two rigid scenes,
controls, GPU observation renderer, and direct manipulation pass a real
Browser WebGPU smoke; dependency and pacing boundaries are enforced; and no
Pages or production deployment has been configured.
