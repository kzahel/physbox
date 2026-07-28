# Web showcase and public water consumption

Status: decided and locally qualified through S40's revised automated gates on
2026-07-28. Icon tools, configuration-owned water, 30/60 Hz choices,
2K-through-50K rigid stress variants, and camera navigation are implemented;
S40 awaits review of the revised artifact. GitHub Pages configuration,
repository publication, and Box2D comparison remain deliberately held for
separate review.

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
the way. A compact HTML scene selector, reset/pause controls, direct
manipulation tools, bounded capacity and fixed-rate choices, short per-scene
description, and a terse live diagnostic line are appropriate. Debug reports,
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
- zero or more validated rectangular `ParticleRegionDef` values with initial
  velocity, where zero creates an initially empty system for runtime emission;
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

S40 adds one bounded runtime operation: enqueue a semantic preview-water
circle emission for the next admitted step. The command validates its finite
center, radius, spacing, velocity, supported world bounds, and maximum
requested particle count before queueing. It is accepted only by a world with
a configured particle system and uses the same bounded deferred GPU command
path as the first-party engine. Queue capacity, exact slot allocation, and the
private material identifier remain implementation details.

The library also exposes a compact asynchronous particle-count snapshot for
diagnostics. It reports stable capacity, current live count, cumulative
spawned count, and cumulative capacity/command drops. The caller explicitly
encodes a copy, submits it, and polls a nonblocking request; simulation and
rendering never depend on the CPU result.

Runtime erasure, particle selection, multiple materials, material tuning,
smooth-surface meshing, and full particle snapshots remain outside the public
surface. They require a separate consumer need and interaction/lifecycle
contract.

Human-review refinement, 2026-07-28: an empty initial region list is now valid
because runtime emission is a public operation. The system still allocates and
validates one bounded semantic-water capacity before the first step, starts
with zero live particles, and exposes the same GPU observation/count
contracts. This avoids hidden seed particles when a consumer enables Water in
an otherwise dry scene. The public rigid capacity enum additionally gains
2K/5K/20K/50K opt-in tiers for the explicit showcase stress variants; the
64-slot default and every existing tier remain unchanged.

## Interactive showcase controls

The S40 showcase toolbar remains deliberately compact:

- **Scene** selects the curated catalog.
- **Tool** is a group of individually labeled icon buttons for body Drag, Box,
  Water, and camera Pan; it is not a dropdown.
- **Max bodies** selects the public world-capacity tier.
- **Max water** selects `None` or a bounded particle capacity.
- **Physics** selects only 30 or 60 Hz.
- **Pause** and **Reset** control the current world.

Capacity and physics-rate changes are pending configuration. They take effect
only when Reset is pressed or a new scene is selected because they determine
GPU allocation and the simulation clock. The UI must make pending state
visible and must not silently recreate the world while the visitor is
interacting.

Water availability follows the applied `Max water` configuration, not the
scene name. Dry scenes select `None` by default; choosing a capacity and
pressing Reset creates an initially empty semantic-water system so the Water
tool can emit into that scene. Water-authored scenes select a bounded water
default, and Water and rigid bodies is the initial showcase scene.

Drag keeps the existing public point-query/constraint path. Box places one
standard dynamic box through public body/shape construction. Water click or
drag submits throttled neutral emission commands; raw pointer frequency may
not become an unbounded GPU command stream. Tool size, body material editing,
water erasure, and persisted configurations are deferred.

Two consumer-owned scenes extend the catalog:

- **Rigid stress 2K/5K/10K/20K/50K** author the exact named live-body count
  into a matching default capacity. A larger `Max bodies` tier may be selected
  before Reset to restore placement headroom.
- **Water stress** fills approximately 80 percent of the selected particle
  capacity, while reserving slots for the water tool.

The 10,000-through-50,000-body and 16,384-particle showcase tiers are explicit
heavy options, not startup defaults. The library's 32,768-particle capacity
frontier is not a showcase option: a bounded cold Browser WebGPU check did not
return its first 32K coupled stress frame within 90 seconds on the development
host, while the 16K tier did advance. That frontier remains available to
purpose-built evidence tools and may be reconsidered after optimization.
Stress scenes demonstrate bounded scale and overload behavior; they are not
benchmark or quality claims.

The showcase owns camera navigation. Pan changes only the consumer camera, and
wheel/trackpad zoom preserves the world point under the pointer. Scene
selection and Reset restore the authored home frame. Navigation never changes
physics state or requires a GPU readback.

## Live diagnostics

The showcase may display rolling display FPS, admitted physics rate versus
the configured fixed rate, effective simulation scale, main-thread frame
work, optional GPU frame/physics duration, and current body/water occupancy.
These are sparse, rolling, environment-dependent diagnostics for the person
using the page. They are not retained benchmark samples and must not be
presented as comparative evidence.

GPU durations are optional. When the adapter supports portable timestamp
queries, the application may record sparse caller-owned markers and read them
back asynchronously with delayed buffers. It must display unavailable when
the feature is absent and may not block an interactive frame. It does not
expose private solver-pass timing through `physbox-sim`.

Display cadence and fixed-step cadence remain separate. The showcase exposes
only 30 and 60 Hz because every outer step already contains four solver
substeps. Higher valid library rates remain available to purpose-built
consumers, but presenting 120/240 Hz beside ordinary RAF implied
480/960 internal substeps per simulated second and mostly demonstrated
slow-time rather than a useful demo-quality choice.

An optional render-rate cap and render interpolation are deferred until the
S40 human checkpoint. A cap alone would intentionally lower presentation
cadence; interpolation would require a defined pair of GPU observations and
latency/identity semantics. Neither is smuggled into this tooling slice.

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

S40 additionally proves:

- public runtime emission validation, bounded queueing, capacity-drop
  reporting, and unchanged rigid-only and pre-step water behavior;
- reset-applied capacity/rate configuration and correct Water-tool
  availability;
- box placement, throttled water painting, stress scenes, pause/reset, and
  scene switching in a packaged Browser WebGPU run;
- compact occupancy and cadence diagnostics without synchronous GPU/CPU
  dependency;
- graceful `n/a` behavior when timestamp queries are unavailable; and
- unchanged maintained-host mechanics and defaults.

S40 stops at a human checkpoint after those gates. Publication, Box2D,
half-RAF rendering, interpolation, new material/size controls, and promotion
of any performance or water-quality claim require a new decision.

Qualification completed on 2026-07-28 without changing those mechanics or
defaults:

- the packaged Browser WebGPU showcase passed at `/showcase-web/` with
  `crossOriginIsolated=false`, all three scenes, public pointer interaction,
  reset/pause/resize, 2,650 water particles, and no private PhysBox dependency;
- the public GPU integration and surface-free external consumer passed with
  finite live/inactive particle records, while the maintained
  `npm run test:fast`, production warnings-denied clippy, and
  `npm run test:web:lab` lanes remained green;
- a clean 600-step headless liquid run retained 3,952 finite particles in an
  8,192-slot capacity with 92 particle dispatches, no overflow, and unchanged
  first-party coupled scene work;
- the retained Browser WebGPU performance sample measured the unchanged
  first-party Standard path at 2.556 ms total GPU P50 and Stabilized at
  2.508 ms. Public observation is opt-in and structurally adds one compute
  projection plus 48 bytes per capacity slot only when a consumer creates and
  encodes it;
- the ARM64 Android check and release APK passed, followed by a hardware-backed
  Vulkan AVD liquid capture with fresh provenance and clean shutdown; and
- sibling Quest revision `521d617` rendered 1,088 publicly authored water
  particles from the same canonical observation in distinct stereo views on a
  physical Quest 3. Its existing rigid scene remains the default and Box2D
  rejects the water scene explicitly.

The showcase remains a favorable capability demonstration, not a quality,
parity, or performance promotion. Water keeps its preview label while S31 and
S35 remain at their recorded review checkpoints.

S40 automated qualification completed on 2026-07-28 without changing solver
mechanics, fixed-step defaults, or first-party host behavior:

- packaged Browser WebGPU run
  `d60a3c09-07fd-4759-b2fa-d73c55c7d600` passed the five-scene/tool/config
  interaction script at 1,512 × 800 and DPR 2, ending with 6,497 live
  particles in the selected 8,192-slot Water stress tier;
- the same script proved explicit capacity signaling, pending/reset
  configuration, zero-or-one step overload at a requested 120 Hz, delayed
  timestamp-query results when supported, and clean timing invalidation on a
  scene rebuild;
- `npm run test:fast`, production warnings-denied clippy, a clean 600-step
  coupled headless run, the ARM64 Android check, and all 20 sibling Quest
  consumer tests passed at source
  `0b17111a4aefbe1d998e01dd3e84b62b02ec1a41`;
- the favorable selector stops at 16K particles because a cold 32K coupled
  stress frame did not return within 90 seconds on the development host; and
- the exact artifact remains reviewable at the secure tailnet-only
  `https://zblinux.tail71bc5c.ts.net:8444/` URL. No publication or deployment
  action occurred.

Human review now owns the next choice: keep the compact surface as-is, improve
its presentation or curated scenes, expose runtime Water interaction in the
Quest-owned adapter, open a separate render-cadence/interpolation workstream,
prepare static publication, or plan an independently qualified Box2D
comparison.

Human review selected the S40.7 revisions rather than one of those deferred
workstreams. Revised packaged Browser WebGPU run
`d62d7972-9972-4dd4-a3b7-167c215b7d66` proves:

- the page starts on Water and rigid bodies with `Max water: 4K`;
- dry scenes select `None`, while choosing a water capacity and pressing Reset
  creates a genuinely empty GPU particle system that accepts runtime Water;
- four icon buttons select body Drag, Box, Water, or camera Pan, and
  pointer-anchored wheel/trackpad zoom plus Reset restore the expected
  consumer camera projection;
- the only presented fixed rates are 30 and 60 Hz;
- exact Rigid stress 2K/5K/10K/20K/50K variants use direct opt-in public
  capacities without changing the workbench's four cross-host product
  presets; and
- the 50K variant constructed 50,000/50,000 live bodies and advanced 12
  zero-or-one outer steps. Its captured overload is diagnostic evidence, not
  a rate, quality, or comparative performance claim.

The maintained fast lane, production clippy, fresh 600-step coupled headless
run, ARM64 Android check, and all 20 Quest consumer tests pass. The compact
laptop capture contains no footer copy and remains reviewable at
`https://zblinux.tail71bc5c.ts.net:8444/`. S40 remains active until a human
evaluates this revised artifact.

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
- live showcase use demonstrates that render-rate limiting or interpolation
  materially improves review on high-refresh displays;
- public review requires a qualified Box2D comparison; or
- the repository/package naming becomes a genuine discoverability blocker
  after the showcase has a reviewed public URL.
