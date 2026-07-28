# S40 — Interactive showcase tooling and telemetry

Status: active 2026-07-28. Implementation and automated qualification are
complete; the exact local Browser WebGPU artifact is awaiting the mandatory
human checkpoint. Publication and the lower-value presentation experiments
listed below remain held.

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

- [x] Add a validated public semantic-water circle emission definition.
- [x] Queue at most the library's bounded number of commands for the next
      admitted step and report queue-full/unsupported-world errors without
      mutating GPU state.
- [x] Reuse the existing internal GPU emission path and preserve pre-step
      region authoring and rigid-only stepping.
- [x] Add a stable compact particle-count snapshot request with explicit
      encode, asynchronous map, and nonblocking completion.
- [x] Add CPU and focused GPU tests for invalid definitions, queue bounds,
      successful emission, capacity drops, count consistency, and reset.
- [x] Extend the public external-consumer proof without exposing private
      particle layouts or requiring CPU results for advancement.

Gate: a public consumer can add bounded preview water at runtime and observe
eventual occupancy/capacity drops through a neutral, asynchronous API.

Library checkpoint, 2026-07-28:

- `ParticleEmissionDef::water_circle` defaults to the current `0.1 m`
  particle-diameter spacing and validates finite center/radius/spacing/
  velocity, supported world bounds, and `1..=4,096` requested particles.
- `World::queue_particle_emission` admits at most 64 commands for the next
  ordinary step, rejects rigid-only worlds and overflow before recording, and
  projects to the existing internal single-material GPU command path. No
  solver parameter, pass order, or default changed.
- `ParticleCountSnapshotRequest` explicitly copies five stable words and maps
  asynchronously. Its version-1 result reports capacity, live, cumulative
  spawned/erased/killed, and dropped particles; it carries no particle pose or
  private counter layout.
- The focused public GPU test proves exact successful emission followed by
  capacity saturation/drop accounting. All 45 library unit tests and 3 public
  GPU integrations pass. The independent surface-free consumer reports
  `PHYSBOX_EXTERNAL_CONSUMER_SUCCESS ... water_particles=37` after observing
  24 initial and 13 runtime-emitted particles.

### S40.3 — Configurable showcase world and scenes

- [x] Add pending body-capacity, water-capacity, and 30/60/120/240 Hz controls
      that apply on Reset or scene selection.
- [x] Rebuild renderer allocations and the simulation clock consistently with
      the selected world settings.
- [x] Add Rigid stress near 90 percent of selected body capacity with
      placement headroom.
- [x] Add Water stress near 80 percent of selected water capacity with
      emission headroom.
- [x] Keep 10K bodies and 16K particles explicit heavy options and preserve
      compact first-screen layout at laptop dimensions.

Gate: all five scenes construct solely through the public simulation API at
each supported bounded setting, and pending settings never partially mutate a
live world.

### S40.4 — Placement tools

- [x] Add compact Drag/Box/Water HTML selection.
- [x] Keep Drag on the current public deferred query/constraint path.
- [x] Place one standard dynamic box on click through public body and shape
      construction, updating only consumer-owned renderer metadata.
- [x] Paint bounded preview water on click/drag through the public emission
      operation with at most one coalesced emission per admitted fixed step.
- [x] Disable Water for rigid-only scenes and return selection to Drag when
      changing to one.
- [x] Surface full body/water capacity without crashing, blocking, or silently
      switching mechanics.

Gate: packaged Browser WebGPU automation and human interaction can place,
drag, and paint as allowed by the current scene and capacity.

### S40.5 — Live diagnostic telemetry

- [x] Display rolling `requestAnimationFrame` FPS, configured/admitted physics
      Hz, effective simulation scale, and main-thread frame work.
- [x] Display authoritative body occupancy and eventual particle
      occupancy/capacity from stable public counters.
- [x] Add sparse optional caller-owned GPU frame/physics timestamps with
      delayed nonblocking readback, or display `GPU n/a` when unsupported.
- [x] Label live numbers as diagnostics, keep them out of retained benchmark
      evidence, and verify that a requested rate above display cadence shows
      slowdown rather than catch-up.

Gate: the diagnostic line is compact, honest about unavailable/stale data,
and never becomes an admission or rendering dependency.

Showcase implementation checkpoint, 2026-07-28:

- One reset path constructs the selected 64/256/1K/10K body tier, 4K/8K/16K
  water tier, and 30/60/120/240 Hz clock/configuration together. Control
  changes remain visibly pending until Reset; scene selection applies them.
- Rigid stress authors 90 percent of each tier. A separate Browser WebGPU
  frontier check constructed and advanced 9,000 live bodies in the 10,000-slot
  tier at a requested 30 Hz. Water stress authors at least 79 percent of each
  offered tier; the 16K row constructed 13,104 particles and advanced.
- The library still supports 32,768 particle slots, but a cold coupled 32K
  stress selection did not return its first page state within a bounded
  90-second Browser WebGPU check on this host. It was removed from the
  favorable public selector instead of being presented as a usable demo tier.
- Packaged run `d60a3c09-07fd-4759-b2fa-d73c55c7d600` passes Box placement,
  Drag creation/release, Water drag emission, pending/reset semantics, both
  stress scenes, exact 64-body capacity signaling, 8,192-capacity/6,497-live
  water, and zero page/console validation errors at 1,512 × 800 with DPR 2.
- The same smoke requests 120 Hz, proves zero-or-one steps and an admitted rate
  below 100 Hz/effective scale below `0.75`, and accepts either finite sparse
  timestamp-query results or the explicit unavailable path. Timing samples are
  triple-buffered every 30 admitted steps and invalidated on every scene
  rebuild so old scene numbers are never relabeled as current.

### S40.6 — Qualification and human checkpoint

- [x] Run focused public API, showcase, dependency-negative, and packaged
      Browser WebGPU tests.
- [x] Run `npm run test:fast` and production warnings-denied clippy.
- [x] Run one fresh coupled headless smoke because public coupled command
      routing changes.
- [x] Run proportional maintained browser/ARM64 checks if shared coupled
      construction requires them; record why physical-device behavior is or
      is not affected.
- [x] Keep the local server reachable through the already reviewed secure
      Tailscale URL and produce a fresh laptop-size capture.
- [x] Record measured evidence, limitations, and exact deferred choices in
      this tactical and the governing topic.
- [ ] Stop for human review of the working demo and choose the next
      high-value slice.

Gate: the exact packaged local artifact is reviewable with clean WebGPU
operation, all authorized behavior is qualified, and no publication action
has occurred.

Automated qualification checkpoint, 2026-07-28:

- Final packaged run `d60a3c09-07fd-4759-b2fa-d73c55c7d600` passed
  `npm run test:showcase` at 1,512 × 800 CSS pixels and DPR 2. Its final Water
  stress state had 11/64 live bodies, 6,497/8,192 live particles, no drops,
  120 Hz configured versus about 40 Hz admitted, and no page or console
  errors. The fresh capture is `/tmp/physbox-showcase.png`; the smoke also
  exercises a finite timestamp-query sample before its final scene reset.
- `npm run test:fast` passed in 28.24 seconds, including the maintained real
  Browser WebGPU run
  `45a4749a-5213-4834-82b8-25398b8e2753`. Production warnings-denied clippy
  passed. The showcase dependency-negative/platform-boundary check and its
  focused startup-diagnosis/public API tests remained green.
- Clean headless run `1785232576637621795-1682862` at source
  `0b17111a4aefbe1d998e01dd3e84b62b02ec1a41` completed 600 explicit
  liquid-sandbox steps with 3,952/8,192 GPU-resident particles and emitted
  `PHYSBOX5_VALIDATION_SUCCESS`. Its artifacts are
  `/tmp/physbox5-s40-liquid-600-20260728.{png,json}`.
- `cargo ndk -t arm64-v8a --platform 28 check -p physbox-android --lib`
  passed. The sibling Quest consumer's 20 tests passed against the final local
  public library, including its public water scene. No solver, first-party
  host, launch, renderer, or Android/Quest adapter behavior changed, so this
  consumer-owned web interaction slice does not justify another APK or
  physical-device run before the human product checkpoint.
- The local server still listens on `0.0.0.0:8080`; Tailscale Serve returns
  the packaged artifact at
  `https://zblinux.tail71bc5c.ts.net:8444/` as a secure tailnet-only origin.
  No push, public repository change, Pages configuration, or deployment
  occurred.
- The deliberately deferred choices remain half-RAF rendering, render
  interpolation, publication, Box2D comparison, tuning/tool-size controls,
  further visual treatment, and a Quest-owned runtime Water interaction
  surface. The 32K water frontier also remains outside the favorable selector
  pending optimization or purpose-built evidence.

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
