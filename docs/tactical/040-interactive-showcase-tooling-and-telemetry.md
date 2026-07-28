# S40 — Interactive showcase tooling and telemetry

Status: active 2026-07-28. Human direction authorizes implementation through
one locally reviewable Browser WebGPU checkpoint; publication and the
lower-value presentation experiments listed below remain held.

Governing architecture:
[web showcase and public water consumption](../topics/web-showcase-and-public-water-consumption.md).
Library boundary:
[reusable GPU physics library boundary](../topics/reusable-gpu-library-boundary.md).
Pacing:
[simulation time and overload](../topics/simulation-time-and-overload.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Turn the compact PhysBox Web Showcase into a useful interactive capability
demo: add consumer-owned Drag/Box/Water tools, reset-applied capacity and
fixed-rate controls, bounded rigid and water stress scenes, and honest live
cadence/occupancy/timing diagnostics. Expose only the minimum neutral runtime
water operation and asynchronous count observation needed by independent
consumers.

Stop after the packaged localhost/Tailscale review surface is qualified and
return the next high-value product choices to a human checkpoint.

## Exclusions

- Do not deploy GitHub Pages, publish the repository, change the production
  domain, or push.
- Do not add Box2D comparison or make performance, parity, capacity, or water
  quality claims.
- Do not change solver equations, particle parameters, pass ordering,
  substeps, coupling policy, sleeping behavior, physics defaults, or existing
  first-party scene outcomes.
- Do not add particle erasure/selection, multiple materials, tool-size or
  material editors, smooth water rendering, persistence, or a CPU particle
  mirror.
- Do not add render interpolation, a half-RAF/render-rate limiter, an
  independent render clock, or multi-step catch-up.
- Do not put showcase controls into the fullscreen workbench or move
  application presentation into `physbox-sim`.

## Ownership constraints

- `physbox-sim` owns validation and deferred recording of one semantic
  preview-water circle emission plus its stable compact asynchronous count
  result. It knows no HTML tool, scene, camera, or pointer-gesture concept.
- The showcase owns tool selection, click/drag mapping, throttling policy,
  scene catalog, stress layouts, capacity/rate presets, pending-reset
  presentation, renderer metadata, and live diagnostic presentation.
- `World::step` remains caller-encoded and zero-or-one admitted. Runtime
  commands are consumed at the next admitted step and never cause an extra
  step.
- Particle capacity, rigid capacity, and fixed rate are construction-time
  settings. A showcase change is applied only by an explicit Reset or scene
  change.
- Full particle state remains in canonical GPU observations. Compact counts
  use delayed asynchronous readback and cannot gate simulation or rendering.
- Timestamp markers, if supported, are sparse and application-owned. A
  missing feature is reported as unavailable rather than substituted.

## Ordered delivery slices

### S40.1 — Contract and plan

- [x] Record the runtime-emission, control, stress-scene, telemetry, pacing,
      and deferred-rendering decisions in the governing topic.
- [x] Define this tactical's exclusions, ownership, gates, stop conditions,
      and mandatory human checkpoint.
- [x] Add S40 to the tactical status index before implementation.

Gate: the implementation boundary is explicit and does not reopen solver,
publication, comparison, or render-cadence architecture.

### S40.2 — Neutral runtime water surface

- [ ] Add a validated public semantic-water circle emission definition.
- [ ] Queue at most the library's bounded number of commands for the next
      admitted step and report queue-full/unsupported-world errors without
      mutating GPU state.
- [ ] Reuse the existing internal GPU emission path and preserve pre-step
      region authoring and rigid-only stepping.
- [ ] Add a stable compact particle-count snapshot request with explicit
      encode, asynchronous map, and nonblocking completion.
- [ ] Add CPU and focused GPU tests for invalid definitions, queue bounds,
      successful emission, capacity drops, count consistency, and reset.
- [ ] Extend the public external-consumer proof without exposing private
      particle layouts or requiring CPU results for advancement.

Gate: a public consumer can add bounded preview water at runtime and observe
eventual occupancy/capacity drops through a neutral, asynchronous API.

### S40.3 — Configurable showcase world and scenes

- [ ] Add pending body-capacity, water-capacity, and 30/60/120/240 Hz controls
      that apply on Reset or scene selection.
- [ ] Rebuild renderer allocations and the simulation clock consistently with
      the selected world settings.
- [ ] Add Rigid stress near 90 percent of selected body capacity with
      placement headroom.
- [ ] Add Water stress near 80 percent of selected water capacity with
      emission headroom.
- [ ] Keep 10K bodies and 32K particles explicit heavy options and preserve
      compact first-screen layout at laptop dimensions.

Gate: all five scenes construct solely through the public simulation API at
each supported bounded setting, and pending settings never partially mutate a
live world.

### S40.4 — Placement tools

- [ ] Add compact Drag/Box/Water HTML selection.
- [ ] Keep Drag on the current public deferred query/constraint path.
- [ ] Place one standard dynamic box on click through public body and shape
      construction, updating only consumer-owned renderer metadata.
- [ ] Paint bounded preview water on click/drag through the public emission
      operation with at most one coalesced emission per admitted fixed step.
- [ ] Disable Water for rigid-only scenes and return selection to Drag when
      changing to one.
- [ ] Surface full body/water capacity without crashing, blocking, or silently
      switching mechanics.

Gate: packaged Browser WebGPU automation and human interaction can place,
drag, and paint as allowed by the current scene and capacity.

### S40.5 — Live diagnostic telemetry

- [ ] Display rolling `requestAnimationFrame` FPS, configured/admitted physics
      Hz, effective simulation scale, and main-thread frame work.
- [ ] Display authoritative body occupancy and eventual particle
      occupancy/capacity from stable public counters.
- [ ] Add sparse optional caller-owned GPU frame/physics timestamps with
      delayed nonblocking readback, or display `GPU n/a` when unsupported.
- [ ] Label live numbers as diagnostics, keep them out of retained benchmark
      evidence, and verify that a requested rate above display cadence shows
      slowdown rather than catch-up.

Gate: the diagnostic line is compact, honest about unavailable/stale data,
and never becomes an admission or rendering dependency.

### S40.6 — Qualification and human checkpoint

- [ ] Run focused public API, showcase, dependency-negative, and packaged
      Browser WebGPU tests.
- [ ] Run `npm run test:fast` and production warnings-denied clippy.
- [ ] Run one fresh coupled headless smoke because public coupled command
      routing changes.
- [ ] Run proportional maintained browser/ARM64 checks if shared coupled
      construction requires them; record why physical-device behavior is or
      is not affected.
- [ ] Keep the local server reachable through the already reviewed secure
      Tailscale URL and produce a fresh laptop-size capture.
- [ ] Record measured evidence, limitations, and exact deferred choices in
      this tactical and the governing topic.
- [ ] Stop for human review of the working demo and choose the next
      high-value slice.

Gate: the exact packaged local artifact is reviewable with clean WebGPU
operation, all authorized behavior is qualified, and no publication action
has occurred.

## Stop conditions

Stop before continuing if:

- runtime emission requires exposing a private particle/material/counter
  layout or changing coupled mechanics;
- compact counters require blocking readback or a CPU state mirror;
- supported stress capacity cannot be authored without changing the public
  capacity contract or existing solver limits;
- HTML control semantics must move into shared simulation code;
- useful timing requires a private solver profiler or unsupported mandatory
  WGPU feature;
- an existing first-party outcome/default or a qualified gate changes; or
- the next useful step is publication, comparison, interpolation, render-rate
  limiting, new physics tuning, or another product choice outside this scope.

## Human checkpoint questions

After qualification, review:

1. Are the compact controls and five-scene catalog the right public demo
   surface?
2. Are the Box/Water tool sizes and stress densities useful without exposing
   advanced tuning?
3. Do the live diagnostics explain high-refresh and overload behavior clearly?
4. Is a render-rate cap or interpolation worth a separate frame-contract
   workstream?
5. Should the next investment be visual polish, additional curated scenes,
   Quest runtime water interaction, static publication readiness, or a
   separately qualified Box2D comparison?

## Completion condition

S40 is complete only after the neutral public runtime-water/count contract,
configurable showcase scenes/tools, and honest live diagnostics pass their
focused and proportional gates, the exact packaged artifact is locally
reviewable, evidence and limitations are recorded, and a human evaluates the
checkpoint. Until that review, S40 remains active even when implementation
gates pass.
