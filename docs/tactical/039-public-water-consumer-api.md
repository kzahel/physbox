# S39 — Public water consumer API

Status: complete 2026-07-28. Public preview-water authoring and canonical GPU
observation work in the standalone Web Showcase and sibling Quest consumer;
maintained browser, headless, performance, and Android qualification pass.

Governing architecture:
[web showcase and public water consumption](../topics/web-showcase-and-public-water-consumption.md).
Library boundary:
[reusable GPU physics library boundary](../topics/reusable-gpu-library-boundary.md).
Coupled mechanics:
[coupled liquid/rigid constraint surface](../topics/coupled-liquid-rigid-constraint-surface.md).
Active stabilization work:
[S35 fluid/solid contact stabilization](035-fluid-solid-contact-stabilization.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Expose the current preview-water system through a small physics-only
`physbox-sim` API, prove its canonical GPU observation from an independent
consumer, add a Water and rigid bodies scene to the web showcase, and compile
the sibling Quest consumer against the same public surface.

The implementation reuses the existing GPU particle/coupling mechanics and
current standard default. It does not promote S31/S35 quality, change
equations, or define a final material-tuning API.

## Exclusions

- Do not change water parameters, grid dimensions, pass ordering, substeps,
  pressure or coupling iterations, standard/stabilized default, capacity
  frontier, or rigid solver behavior.
- Do not expose private particle buffers, grid/density/correction state,
  indirect draws, dispatch topology, validation diagnostics, engine resources,
  or `physbox-contracts`.
- Do not add a renderer, smooth surface, refraction, multi-material system,
  public solver tuning, runtime emit/erase, particle queries, or particle
  snapshots.
- Do not add Pages deployment or public performance/correctness claims.
- Do not make this repository depend on the sibling Quest checkout.

## Ownership and API constraints

- A `ParticleSystemDef` is configured exactly once before the first world
  step. The initial surface supports one semantic water material and bounded
  rectangular initial regions.
- Invalid capacity, radius, spacing, bounds, initial velocity, empty region
  sets, material references, and capacity overflow fail before GPU allocation.
- `World::step` records the existing coupled pass graph in the caller's
  encoder. Caller-owned submission and zero-or-one interactive pacing remain
  unchanged.
- `GpuParticleObservation` is a versioned render-neutral projection with
  `STORAGE | COPY_SRC` usage. Its records expose only stable particle
  position, velocity, identity/material, slot, and live state.
- Creating or encoding observation does not map, poll, submit, or block.
- The Web Showcase and Quest own all particle rendering and platform mapping.
- Water remains explicitly labeled preview until the existing manual
  checkpoints are resolved.

## Ordered delivery slices

### S39.1 — Public definitions and validation

- [x] Promote this tactical to active after S38's dependency boundary passes.
- [x] Add documented water system and rectangular region definitions.
- [x] Add a one-time pre-step `World::set_particle_system` operation.
- [x] Project definitions into the existing coupled scenario internally
      without exposing contract types.
- [x] Add CPU tests for validation, region ordering, capacity overflow, empty
      authoring, late configuration, and unchanged rigid-only construction.

Gate: a public consumer can describe current preview water without naming
solver or presentation internals.

### S39.2 — Coupled public stepping

- [x] Construct the existing `GpuParticleWorld` inside public `World` when a
      particle system is configured.
- [x] Record the unchanged rigid/particle/coupling pass graph from
      `World::step`.
- [x] Preserve first-party private coupled construction and standard/stabilized
      behavior without duplicate mechanics.
- [x] Add focused GPU tests for initial count, finite state, rigid response,
      capacity cleanliness, and unchanged rigid-only dispatch behavior.

Gate: the default-feature public world advances coupled water and rigid bodies
through the existing authoritative GPU state.

### S39.3 — Canonical particle observation

- [x] Add a stable public particle observation record and flags.
- [x] Project private state with an explicitly encoded compute pass into a
      public buffer.
- [x] Add layout/version, capacity, radius, buffer-usage, and inactive-slot
      tests.
- [x] Extend the surface-free external consumer to encode and inspect one
      bounded particle observation without renderer or CPU advancement
      dependency.

Gate: an independent consumer can render or compute from live water state
without learning a private solver layout.

Public API checkpoint, 2026-07-28:

- `ParticleSystemDef::water` and `ParticleRegionDef::rectangle` validate one
  bounded pre-step preview-water system, the current `0.05 m` particle radius,
  diameter-or-greater spacing, finite initial velocity, supported authored
  world bounds, nonempty placement, and exact capacity before coupled GPU
  allocation.
- `World::set_particle_system` is one-time and pre-step. `World::step`
  internally projects it into the existing coupled scenario and constructs the
  same `GpuParticleWorld`, standard/stabilized policy, rigid storage, and
  particle pass graph used by the first-party engine.
- `GpuParticleObservationRecord` is a version-1 48-byte projection containing
  position, velocity, generation, public live flag, slot, and material ID.
  The observation's `STORAGE | COPY_SRC` buffer is explicitly encoded by the
  caller and exposes no grid, density, pressure, correction, coupling, or
  indirect-draw state.
- The surface-free public GPU integration passes 3/3, including 24 live water
  records and inactive capacity slots. The external-style consumer emits
  `PHYSBOX_EXTERNAL_CONSUMER_SUCCESS ... water_particles=24`. All-feature
  engine compilation, all-feature warnings-denied simulation clippy, and
  platform dependency negatives pass.

### S39.4 — Web showcase water scene

- [x] Add Water and rigid bodies using only public definitions.
- [x] Draw the particle observation in the showcase-owned renderer.
- [x] Demonstrate visible two-way coupling and reset/pause/resize behavior.
- [x] Label the scene and copy as preview and avoid unqualified scale or parity
      claims.
- [x] Extend the packaged Browser WebGPU smoke to select and exercise water.

Gate: the local web showcase is the primary iteration surface for public water
consumption and uses no internal feature or CPU pose/particle loop.

Showcase checkpoint, 2026-07-28:

- Water and rigid bodies authors 2,650 particles, four dynamic shapes, the
  basin, and an angled fixed obstacle solely through public `World`,
  `ParticleSystemDef`, body/shape definitions, and the two canonical GPU
  observations.
- The app-owned particle renderer reads only the versioned observation,
  renders water before consumer-owned rigid geometry, and keeps mechanics,
  both observation projections, and both draw pipelines in one caller-owned
  encoder/submission.
- Final packaged Browser WebGPU run
  `1629d29e-58a3-4471-aded-859826dfb8ce` passes the existing rigid
  selector/pause/reset/drag gates, then advances the water scene for 99 fixed
  frames and 100 fixed steps. It reports 2,650 particles, clean console/GPU
  validation, `crossOriginIsolated=false`, and a 3,324,794-byte debug WASM
  module. The inspected coupled capture is `/tmp/physbox-showcase.png`;
  provenance is `/tmp/physbox-showcase.json`.
- Focused WASM warnings-denied app clippy and platform dependency negatives
  pass. The dependency's existing WASM-wide `Arc<Buffer>` clippy warnings are
  outside this slice; `--no-deps` isolates the new app while the governed
  production clippy lane remains the shared final gate.

### S39.5 — Cross-consumer and maintained-host qualification

- [x] Run focused public definition, GPU, observation, external-consumer, and
      showcase tests.
- [x] Run `npm run test:fast` and production warnings-denied clippy.
- [x] Run a fresh coupled headless validation with `/tmp` provenance.
- [x] Run the focused liquid performance smoke and record projection overhead,
      allocation, and dispatch changes.
- [x] Run `npm run test:web:lab`, ARM64 Android check, and release APK because
      shared coupled construction changes.
- [x] Compile/test the sibling Quest repository against the final API and
      record its revision.
- [x] If Quest rendering integration is mechanically obvious under its own
      architecture, add it and perform its required checks; otherwise stop at
      the exact product/presentation decision fork.
- [x] Update the topic, tactical, and status index with measured evidence and
      remaining preview limitations.

Gate: public water works in two independent-style consumers, maintained
workbench behavior is unchanged, and proportional coupled/platform evidence
passes.

Qualification and measured evidence, 2026-07-28:

- Focused public authoring/observation integration passes 3/3, including
  finite coupled state, 24 live water records, inactive capacity slots, rigid
  response, and the unchanged rigid-only path. The independent surface-free
  client reports
  `PHYSBOX_EXTERNAL_CONSUMER_SUCCESS ... water_particles=24`; the final
  showcase smoke is recorded under S39.4.
- At clean mechanics revision `5718782`, `npm run test:fast` passed in
  28.24 seconds, including maintained Browser WebGPU run
  `cb63eaba-032c-4888-829c-b40589e9379b`; production warnings-denied clippy
  and `npm run test:web:lab` run
  `7f7c88a6-35db-4ea8-bdaa-2c28f344036f` also passed.
- Clean headless run `1785225210096091372-1633817` advanced the maintained
  `liquid-sandbox` for 600 explicit steps with 3,952 finite particles in an
  8,192-slot capacity, 92 particle dispatches, 1,053,828 particle bytes,
  two-way rigid coupling, and no overflow, escape, kill, or drop. Its capture
  and provenance are
  `/tmp/physbox5-s39-liquid-600-20260728.{png,json}`.
- Browser performance run
  `s35-coupling-browser-5e412d5e-1b03-42bb-a3a6-73371e0f77c4` retained eight
  samples per quality. Standard measured 0.750 ms CPU encode, 2.061 ms liquid,
  and 2.556 ms total GPU P50; Stabilized measured 0.700 ms, 2.037 ms, and
  2.508 ms respectively. Both retained the existing 92 particle dispatches
  and capacity-clean work. The canonical observation is opt-in: it adds one
  `ceil(capacity / 64)` compute projection and 48 bytes per capacity slot only
  when created and encoded. That is 196,608 bytes and 64 workgroups for the
  showcase's 4,096 capacity, or 1,572,864 bytes and 512 workgroups at the
  supported 32,768 frontier; no first-party allocation, dispatch, tuning, or
  default changed.
- The ARM64 Android check, debug APK, and 8,576,621-byte unsigned release APK
  passed. Final clean harness revision `aa1f542` then passed the
  hardware-backed Vulkan AVD `liquid-sandbox` lane as run
  `1785225656-1641436`: 2,560 × 1,600 nonblack capture, 3,952 particles,
  8,192 capacity, valid two-way coupling, no overflow, and confirmed emulator
  shutdown. Artifacts are `/tmp/physbox5-android-avd.png{,.json}`.
- Sibling Quest revision `521d617` consumes the same public API in an opt-in
  `water-bucket-v0` scene while retaining `rigid-bucket-v0` as default. Its
  20 host tests, host/ARM64 warnings-denied clippy, release APK, and physical
  Quest 3 capture pass. The headset rendered 1,088 particles from a 2,048-slot
  canonical observation as Quest-owned stereo geometry, produced distinct
  1,680 × 1,760 eye images, and completed application/display cleanup.
  Artifacts are under
  `/tmp/physbox-quest-water-bucket-v0-20260728`.

No stop condition fired. The integration did not require a shared renderer,
private layout, CPU particle mirror, parameter/default change, or Quest
product-default decision. Preview quality, runtime particle lifecycle,
Box2D comparison, repository publication, and Pages deployment remain outside
this completed workstream.

## Stop conditions

Stop for human review if public construction cannot reuse the existing coupled
world, observation needs a private layout, a qualified gate changes, current
water parameters/defaults need modification, projection cost is material at
the supported frontier, or Quest integration requires a product-specific
visual/input decision not already owned by its application.

## Completion condition

S39 is complete when default-feature `physbox-sim` exposes validated pre-step
preview-water authoring and a canonical GPU observation; the local Web
Showcase demonstrates two-way water/rigid behavior solely through that public
surface; first-party results and defaults remain unchanged; proportional
browser/headless/performance/Android gates pass; and the sibling Quest
consumer compiles against the final API with any presentation fork recorded
honestly.
