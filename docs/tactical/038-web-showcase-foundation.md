# S38 — Web showcase foundation

Status: active 2026-07-28.

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
- [ ] Add the showcase application crate outside maintained production
      default members.
- [ ] Add dependency negatives proving it imports only public
      default-feature `physbox-sim` among PhysBox crates.
- [ ] Explicitly scope existing browser leaf checks to the existing
      workbench adapter while retaining their negative fixtures.

Gate: the dependency graph and platform checks distinguish an independent
consumer application from an `EngineHost` browser adapter.

### S38.2 — Caller-owned browser and renderer

- [ ] Request the browser WebGPU adapter/device and configure a canvas surface.
- [ ] Create and submit the caller-owned encoder each animation frame.
- [ ] Render circles and rectangles from `GpuBodyObservation` plus
      consumer-owned immutable shape metadata.
- [ ] Handle device-pixel-ratio resize and clear startup failures.
- [ ] Keep render and step ordering GPU-to-GPU with no current-frame map/wait.

Gate: a local browser shows a stepped rigid scene entirely through the public
observation contract.

### S38.3 — Showcase scenes and controls

- [ ] Add Playground and Stack and sleep scenes through public definitions.
- [ ] Add an accessible HTML selector, reset and pause controls, concise scene
      description, and manipulation hint.
- [ ] Map pointer gestures to public point-query/constraint operations and
      reconcile their terminal results asynchronously.
- [ ] Preserve a stable query/hash scene URL without importing launch policy.
- [ ] Confirm scene switching reconstructs consumer state without leaking
      animation loops, surfaces, or input handlers.

Gate: scene selection, reset, pause, resize, drag/release, and deep-link reload
work in a real browser.

### S38.4 — Local packaging and static-host readiness

- [ ] Add one repository build command and one plain local static-server
      command.
- [ ] Use only relative packaged assets and verify an arbitrary nested base
      path.
- [ ] Verify the server sends no COOP/COEP requirement and the app reports
      `crossOriginIsolated === false`.
- [ ] Add a Browser WebGPU smoke against the exact packaged output.
- [ ] Document local review commands and the deliberate Pages hold.

Gate: a clean local build serves from its output directory and passes the
packaged browser smoke without isolation headers.

### S38.5 — Qualification and closure

- [ ] Run focused showcase, dependency-negative, and browser tests.
- [ ] Run `npm run test:fast`.
- [ ] Run production warnings-denied clippy.
- [ ] Record build size, browser run identity, and material limitations.
- [ ] Update the topic, this tactical, and the status index with final
      evidence.

Gate: the standalone rigid showcase is reviewable locally without a new
mechanical implementation, publication action, or regression in maintained
hosts.

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
