# S34 — Render resolution scaling

Status: active, 2026-07-24.

Governing topic:
[render resolution scaling](../topics/render-resolution-scaling.md).
Presentation architecture:
[presentation and modular workbench architecture](../topics/presentation-and-workbench-architecture.md).
Platform boundary:
[platform architecture](../platform-architecture.md).
Performance contract:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Deliver one manual 100/75/50 percent render-scale setting through the shared
engine-rendered menu and launch contract. Reduced presets render the world at
the bounded internal extent, linearly composite it to the native target, and
draw native-resolution UI without changing physics, input, camera, scenario,
or comparison semantics. Qualify the same behavior on desktop, browser,
headless, Android, and comparator feature-on/off builds.

## Exclusions

- Do not add automatic or adaptive resolution.
- Do not change surface presentation cadence, fixed-step rate, catch-up,
  interpolation, solver work, world capacity, or physics defaults.
- Do not put render-scale meaning, defaults, persistence, or query mapping in
  browser JavaScript, Winit, Android, or desktop app code.
- Do not add host-local UI or durable native preference stores.
- Do not change world style, smooth water, antialiasing families, shaders
  beyond the bounded composite, or the existing S33 subjective disposition.
- Do not claim a universal performance gain from one adapter.

## Ownership constraints

- `physbox-launch` owns the preset type, external name/default, parsing,
  browser query projection, and preference migration.
- `physbox-engine` owns the menu action, effective state, world target,
  composition, reports, and invariant tests.
- `physbox-ui` remains unaware of rendering or product settings.
- Platform adapters forward only native extent, display metrics, lifecycle,
  and raw input.
- The headless path uses the same composition behavior and retains final
  capture dimensions.
- PhysBox and Box2D share one selected render scale and compositor.

## Ordered delivery slices

### S34.1 — Contract and decision record

- [x] Record the durable render-scale decision, alternatives, reporting, and
  reconsideration triggers.
- [x] Create this successor tactical without expanding S33.
- [x] Add both documents to their indexes.

Gate: the topic and tactical define native presentation versus scaled world
extent, manual presets, ownership, evidence requirements, and completion.

### S34.2 — Shared setting and menu

- [ ] Add a bounded 100/75/50 preset owned by `physbox-launch`.
- [ ] Add shared CLI, web-query, and Android-JSON parsing with 100 percent as
  the default.
- [ ] Project the setting into engine presentation configuration exactly once.
- [ ] Add one engine-rendered menu control visible in PhysBox and Box2D.
- [ ] Apply changes without scenario/world rebuild, physics hash change,
  camera reset, or tool reset.
- [ ] Migrate browser presentation persistence and URL synchronization in
  shared Rust.

Gate: focused launch, preference, engine UI, feature-on, and feature-off tests
prove the complete setting contract and browser JavaScript remains semantic
free.

### S34.3 — Shared composition path

- [ ] Separate native presentation and world-render targets inside the one
  frame boundary.
- [ ] Retain direct world/UI rendering and zero intermediate allocation at
  100 percent.
- [ ] Allocate a bounded sampleable color target and encode one linear
  fullscreen composite at 75/50 percent.
- [ ] Render the canvas UI at native extent after composition.
- [ ] Recreate only size-dependent presentation resources on resize or scale
  change.
- [ ] Exercise the same path in surface and offscreen renderers.
- [ ] Preserve selected scale through native surface recreation.

Gate: target, resize, direct/composite, camera/input, offscreen, and GPU
validation tests pass with no change to scenario or physics identity.

### S34.4 — Reports and timing

- [ ] Report native and world extents, requested/effective scale, path, and
  intermediate bytes.
- [ ] Add a distinct asynchronous GPU composite bucket without blocking frame
  advancement.
- [ ] Display the applied scale and target in the existing bounded HUD.
- [ ] Update semantic/compact report tests and relevant browser/Android
  consumers.

Gate: supported and unsupported timestamp adapters report honestly; async
readback remains bounded and no report value is required for the next frame.

### S34.5 — Cross-host and performance qualification

- [ ] Run focused package and engine tests while iterating.
- [ ] Run `npm run test:fast`.
- [ ] Run production clippy with warnings denied.
- [ ] Produce fresh native-size headless 100/50 percent captures and reports.
- [ ] Run Browser WebGPU menu/persistence/resize coverage plus paired
  high-pixel-count 100/50 percent evidence.
- [ ] Prove comparator default feature on and explicit feature off.
- [ ] Run the ARM64 Android check and build a release APK.
- [ ] Exercise the setting and pixels on one physical Android device.
- [ ] Measure paired 100/50 percent compute, render, composite, UI, total GPU,
  full-frame, and pacing on the 4K Chromebook when available; otherwise record
  the exact representative-hardware gap without inventing a claim.

Gate: required commands exit successfully, artifacts are fresh and
capacity-clean, every host exposes the same setting, 100 percent retains the
current path, and performance conclusions match the measured bottleneck.

## Completion condition

S34 is complete when the fresh default remains native resolution; 75 and 50
percent use one bounded shared scaled-world/native-UI composition path;
launch, menu, browser persistence, reports, timing, resize, lifecycle,
offscreen, comparator, and cross-host gates pass; physics/input/scenario
identity remains unchanged; and paired high-pixel-count evidence states the
measured benefit or limitation without extrapolation.
