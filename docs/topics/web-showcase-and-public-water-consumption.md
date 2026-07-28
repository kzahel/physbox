# Web showcase and public water consumption

Status: decided 2026-07-28; implementation tracked by S38 and S39.

This topic defines the first-party PhysBox Web Showcase, its relationship to
the existing workbench and Quest consumer, and the minimum public particle
surface needed to demonstrate coupled water through `physbox-sim`.

Related records:

- [Reusable GPU physics library boundary](reusable-gpu-library-boundary.md)
- [Presentation and modular workbench architecture](presentation-and-workbench-architecture.md)
- [Coupled liquid/rigid constraint surface](coupled-liquid-rigid-constraint-surface.md)
- [Performance and scale contract](performance-and-scale.md)
- [Simulation time and overload](simulation-time-and-overload.md)
- [Platform architecture](../platform-architecture.md)

## Decision

PhysBox has three intentionally different first-party applications:

- The PhysBox 5 workbench is the native-first internal product and validation
  client. It may use the private engine bridge, shared canvas UI, launch
  catalog, reports, and Box2D review surface.
- The Quest application is an independent external-style consumer. It depends
  on public `physbox-sim`, owns OpenXR lifecycle and presentation, and proves
  that the library boundary works outside this repository.
- PhysBox Web Showcase is a new first-party external-style browser consumer.
  It depends on public `physbox-sim`, owns its browser lifecycle, curated
  scenes, HTML controls, camera, input mapping, and renderer, and demonstrates
  the library without exposing the internal workbench.

The showcase is an application, not another platform adapter for
`EngineHost`. Therefore the existing rule that `apps/web` is a fullscreen
canvas leaf remains unchanged. Ordinary HTML controls and scenario names are
allowed in the showcase because they are consumer-owned application policy,
not a duplicated browser implementation of the workbench.

The first version is a standalone local web build. It must remain compatible
with static subpath hosting and use only relative assets, but it does not add
or enable GitHub Pages, a Pages workflow, a custom domain, or any production
deployment. Publication waits for the repository-visibility review requested
by the owner.

## Product and naming boundary

Public copy uses the product name **PhysBox** and the descriptive label
**PhysBox Web Showcase**. The repository, package, and compatibility names may
remain `physbox5`; renaming them is independent work with substantially wider
cost.

The first screen should explain what the visitor is seeing, then get out of
the way. A compact HTML scene selector, reset/pause controls, short per-scene
description, and direct manipulation hints are appropriate. Debug reports,
solver tuning, validation controls, native-menu duplication, and internal
milestone vocabulary are not.

## Showcase ownership

The showcase owns:

- its curated scene catalog and stable URL spelling;
- its DOM, copy, layout, theme, and accessible controls;
- browser surface creation and resize handling;
- world-space camera framing and raw pointer-to-world projection;
- mapping pointer gestures to public neutral interaction primitives;
- its renderer and all visual metadata, colors, geometry projection, and draw
  passes; and
- local build and smoke-test entry points.

The showcase may depend on WGPU, Web APIs, and `physbox-sim`. It may not depend
on `physbox-engine`, `physbox-contracts`, `physbox-launch`, `physbox-shell`,
`physbox-ui`, the Box2D comparator, an existing app crate, or private
`engine-internal` features.

No renderer is added to `physbox-sim`. Bodies are drawn from
`GpuBodyObservation`; water is drawn from the public particle observation
defined below. Presentation metadata remains consumer-owned.

## Initial showcase content

The first useful catalog is deliberately small:

1. **Playground** — a mixed rigid scene with pointer dragging.
2. **Stack and sleep** — a visible stability and sleeping demonstration.
3. **Water and rigid bodies** — preview water interacting with rigid shapes.

Every scene is authored in the showcase through public definitions. The water
scene must be labeled a preview while S31 and S35 remain at their recorded
human-review checkpoints. The showcase must not convert a pleasing visual into
an unqualified correctness, parity, or performance claim.

## Public preview-water surface

`physbox-sim` exposes the minimum physics-only particle contract required by a
consumer:

- one pre-step `ParticleSystemDef` with a bounded capacity;
- one selected version-1 water material using the current qualified preview
  parameters;
- one or more validated rectangular `ParticleRegionDef` values with initial
  velocity;
- `World::set_particle_system` before the first step; and
- a reusable, explicitly encoded `GpuParticleObservation`.

The initial public material is semantic water, not a bag of solver
coefficients. Pressure strengths, support kernel, grid dimensions, substeps,
coupling schedule, and internal pass topology remain implementation details.
The definition may expose particle radius and region spacing only where a
consumer needs them for authored scale; unsafe or unsupported combinations
must be rejected before GPU allocation.

The particle observation is a versioned render-neutral projection indexed by
particle slot. Each record exposes position, linear velocity, generation,
material identifier, slot, and a public live flag. The observation owns a
consumer-readable `STORAGE | COPY_SRC` buffer and records an explicit compute
projection into the caller's encoder. It exposes no private grid, density,
pressure, correction, body-contribution, dispatch, indirect-draw, or solver
storage.

The caller continues to own encoders, submission, ordering, targets, and
rendering. No CPU result is required to advance or render the next interactive
frame.

Runtime emission, erasure, particle selection, multiple materials, material
tuning, smooth-surface meshing, and particle snapshots are not part of this
first API. They require their own consumer need and interaction/lifecycle
contract.

## Pacing, headers, and comparison policy

The showcase uses the same zero-or-one fixed-step interactive pacing contract
as every other host. It does not repay wall-time debt, step from a render
callback more than once, or read GPU results to admit the next frame.

The first version does not require `SharedArrayBuffer`, cross-origin
isolation, WebAssembly threads, or WebAssembly SIMD. It must start from a
plain local static server without COOP/COEP response headers. This is both the
simplest publication shape and an honest proof that PhysBox itself does not
depend on those features.

Box2D is omitted from the initial showcase. A later public comparison must be
a separately governed benchmark/review application. It must give each engine
its best supported browser configuration, isolate runs to prevent contention,
state SIMD/thread/SAB and browser-header conditions, distinguish throughput
from visual review, and follow the
[performance and scale contract](performance-and-scale.md). A same-canvas
race or an unqualified frame-rate counter is not acceptable evidence.

## Static-hosting readiness

Before publication, the built artifact must:

- work below an arbitrary URL subpath without absolute asset URLs;
- contain no runtime dependency on repository source files or sibling
  checkouts;
- provide a useful startup failure message when WebGPU is unavailable;
- use no secrets or runtime backend;
- load without cross-origin-isolation headers; and
- pass a local browser smoke from the exact packaged directory.

The future Pages workflow, permissions, public repository state, canonical
URL, analytics, and custom-domain choice remain outside S38/S39. They require
explicit owner review.

## Divergence policy

Some application divergence is intentional: the workbench, Quest, and web
showcase have different presentation and input needs. Mechanical divergence
is not.

The following are shared through `physbox-sim`: definitions, mechanics,
configuration validation, fixed-step admission, interaction primitives,
observations, and public defaults. The following may differ by app: scene
catalog, renderer, camera, controls, copy, layout, and platform lifecycle.

If implementing a showcase scene requires copying physics equations, private
WGSL, private buffer layouts, or workbench-only semantic policy, stop and
repair the public library boundary rather than accepting that divergence.

## Validation

S38 and S39 must prove:

- dependency negatives for the showcase and public simulation crate;
- a default-feature public consumer build with no private bridge;
- local packaged startup in a real Browser WebGPU lane;
- scene switching, reset, pause, resize, and pointer interaction;
- GPU-only body and particle observation through caller-owned encoders;
- plain-server operation without cross-origin isolation;
- unchanged maintained workbench behavior and physics defaults;
- a fresh coupled headless result for public-water changes;
- warnings-denied production clippy and the ordinary maintained gate; and
- compilation of the sibling Quest consumer against the final water API.

Any material change to solver equations, water parameters, coupled scheduling,
product defaults, or existing first-party scene outcomes is an architecture or
product decision and stops unattended implementation for human review.

## Alternatives rejected

- **Put the showcase controls into the current browser workbench.** This would
  weaken the leaf-adapter rule and make a public product page depend on
  internal engine/UI ownership.
- **Publish the current fullscreen workbench as the showcase.** It exposes
  validation-oriented complexity and does not prove the external library
  boundary.
- **Extract a shared renderer.** S36 already established that renderer policy
  is application-owned. The two renderers can share public observation
  contracts without sharing implementation.
- **Expose the private particle render view.** It would freeze solver layouts
  and make an internal bridge the consumer API.
- **Lead with Box2D performance.** The necessary browser isolation, optimized
  foreign build, methodology, and claims deserve a separate measured
  workstream.

## Reconsideration triggers

Revisit this decision if:

- two independent consumers need the same render-neutral derived particle
  field not expressible by the observation record;
- static subpath hosting cannot support the packaged WebGPU application;
- the public particle projection has material measured cost at the supported
  capacity frontier;
- Quest requires runtime particle lifecycle or a different stable observation
  field;
- public review requires a qualified Box2D comparison; or
- the repository/package naming becomes a genuine discoverability blocker
  after the showcase has a reviewed public URL.
