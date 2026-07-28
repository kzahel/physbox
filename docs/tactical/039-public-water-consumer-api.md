# S39 — Public water consumer API

Status: active 2026-07-28. S38's rigid consumer boundary and packaged Browser
WebGPU smoke pass; public water authoring begins on that proven application
surface.

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
- [ ] Add documented water system and rectangular region definitions.
- [ ] Add a one-time pre-step `World::set_particle_system` operation.
- [ ] Project definitions into the existing coupled scenario internally
      without exposing contract types.
- [ ] Add CPU tests for validation, region ordering, capacity overflow, empty
      authoring, late configuration, and unchanged rigid-only construction.

Gate: a public consumer can describe current preview water without naming
solver or presentation internals.

### S39.2 — Coupled public stepping

- [ ] Construct the existing `GpuParticleWorld` inside public `World` when a
      particle system is configured.
- [ ] Record the unchanged rigid/particle/coupling pass graph from
      `World::step`.
- [ ] Preserve first-party private coupled construction and standard/stabilized
      behavior without duplicate mechanics.
- [ ] Add focused GPU tests for initial count, finite state, rigid response,
      capacity cleanliness, and unchanged rigid-only dispatch behavior.

Gate: the default-feature public world advances coupled water and rigid bodies
through the existing authoritative GPU state.

### S39.3 — Canonical particle observation

- [ ] Add a stable public particle observation record and flags.
- [ ] Project private state with an explicitly encoded compute pass into a
      public buffer.
- [ ] Add layout/version, capacity, radius, buffer-usage, and inactive-slot
      tests.
- [ ] Extend the surface-free external consumer to encode and inspect one
      bounded particle observation without renderer or CPU advancement
      dependency.

Gate: an independent consumer can render or compute from live water state
without learning a private solver layout.

### S39.4 — Web showcase water scene

- [ ] Add Water and rigid bodies using only public definitions.
- [ ] Draw the particle observation in the showcase-owned renderer.
- [ ] Demonstrate visible two-way coupling and reset/pause/resize behavior.
- [ ] Label the scene and copy as preview and avoid unqualified scale or parity
      claims.
- [ ] Extend the packaged Browser WebGPU smoke to select and exercise water.

Gate: the local web showcase is the primary iteration surface for public water
consumption and uses no internal feature or CPU pose/particle loop.

### S39.5 — Cross-consumer and maintained-host qualification

- [ ] Run focused public definition, GPU, observation, external-consumer, and
      showcase tests.
- [ ] Run `npm run test:fast` and production warnings-denied clippy.
- [ ] Run a fresh coupled headless validation with `/tmp` provenance.
- [ ] Run the focused liquid performance smoke and record projection overhead,
      allocation, and dispatch changes.
- [ ] Run `npm run test:web:lab`, ARM64 Android check, and release APK because
      shared coupled construction changes.
- [ ] Compile/test the sibling Quest repository against the final API and
      record its revision.
- [ ] If Quest rendering integration is mechanically obvious under its own
      architecture, add it and perform its required checks; otherwise stop at
      the exact product/presentation decision fork.
- [ ] Update the topic, tactical, and status index with measured evidence and
      remaining preview limitations.

Gate: public water works in two independent-style consumers, maintained
workbench behavior is unchanged, and proportional coupled/platform evidence
passes.

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
