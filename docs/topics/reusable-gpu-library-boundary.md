# Reusable GPU simulation and rendering library boundary

Status: decided direction, 2026-07-24; extraction and external-consumer proof
are tracked in
[S36](../tactical/036-reusable-gpu-library-extraction.md). This topic owns the
durable boundary between the GPU-resident mechanical world, optional reusable
world rendering, and application hosts. It does not authorize publishing a
crate or changing a physics default.

Authoritative engine direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Application composition:
[presentation and modular workbench architecture](presentation-and-workbench-architecture.md).
Platform ownership:
[platform architecture](../platform-architecture.md).
Interactive pacing:
[simulation time and overload](simulation-time-and-overload.md).
Configuration ownership:
[physics configuration and solver profiles](physics-configuration-and-solver-profiles.md).
Performance obligations:
[performance and scale](performance-and-scale.md).

## Decision

PhysBox 5 will expose two workspace libraries for unversioned local-path
consumption before considering publication:

- `physbox-sim` owns the authoritative GPU mechanical world, supported
  simulation configuration, fixed-step admission, command encoding, stable
  logical handles, bounded diagnostics, and asynchronous observations.
- `physbox-render` owns optional body, particle, and bounded physics-debug
  rendering into a caller-provided target from a read-only same-device
  simulation render view.

`physbox-engine` remains the first-party application composition host. It
depends on both libraries and continues to own launch projection, authored
scenarios, workbench tools, camera and input policy, UI, reports, validation
workflow, and the optional Box2D comparison shadow.

The intended dependency direction is:

```text
                         physbox-engine
                    first-party sandbox host
                   /          |           \
                  v           v            v
         physbox-sim   physbox-render   physbox-ui
                  ^           ^
                  |           |
                  +-----+-----+
                        |
             external application host
       OpenXR / window / surface / app policy
```

The first external consumer is the separate `physbox-quest` repository. It
uses filesystem path dependencies against the current PhysBox 5 checkout.
PhysBox 5 CI must remain self-contained and must not require that sibling
repository to exist.

## Consumer experience

The supported API should feel familiar to users of a Box2D-style Rust crate:

```rust
let mut world = World::new(&device, WorldDef::default())?;

let body = world.create_body(BodyDef::dynamic())?;
world.create_shape(body, ShapeDef::circle(0.1).with_density(1.0))?;

world.step(
    &device,
    &queue,
    &mut encoder,
    StepDef::fixed(Duration::from_secs_f64(1.0 / 60.0)),
)?;
```

That resemblance covers world construction, definition objects, opaque
handles, explicit mutation, supported global controls, and explicit stepping.
It does not imply synchronous access to live transforms or contacts. PhysBox
mechanics remain GPU resident, so state observation is asynchronous and cannot
be required to advance the next interactive frame.

The public surface is organized around:

- `WorldDef`, `WorldCapacity`, and `QualityDef`;
- `World`, `BodyId`, and `ShapeId`;
- `BodyDef`, `ShapeDef`, material properties, circle and rectangle geometry;
- queued create, change, and destroy operations;
- `StepDef` and the fixed-step pacing contract;
- supported global toggles such as sleeping and their documented PhysBox
  quality controls;
- bounded counters and diagnostics that do not expose private pipeline layout;
  and
- explicitly requested asynchronous snapshots or query results.

Definitions describe neutral mechanics. They do not contain scenario names,
menu labels, launch presets, browser settings, validation completion rules,
camera state, colors, or Box2D comparison choices.

## GPU ownership and frame contract

The application owns:

- adapter and device selection;
- the `wgpu::Device` and `wgpu::Queue`;
- command-encoder creation and queue submission;
- surfaces, swapchains, OpenXR images, texture targets, and presentation;
- device-loss recovery and platform-specific interop; and
- composition order around simulation, world rendering, UI, and application
  rendering.

`physbox-sim` receives a compatible device while constructing a world and
records simulation work into a caller-provided command encoder. It may use the
queue for bounded uploads required by explicit CPU-authored operations, but it
does not submit the encoder or assume ownership of the queue. One interactive
admission records zero or one outer fixed step. Solver substeps remain internal
work inside that outer step and never repay slipped wall time.

`physbox-render` receives a target format at renderer construction and a
caller-provided `wgpu::TextureView`, extent, world-to-clip transform, load
operation, and command encoder when drawing. It does not create a surface or
own a presentation loop.

Every simulation/render interaction is same-device. A simulation world exposes
only a read-only `RenderView` whose lifetime prevents use after the originating
world is mutably advanced. The view may reveal typed buffer bindings, counts,
and render metadata needed by `physbox-render`; it must not expose writable
mechanical resources, private compute pipeline objects, or CPU mirrors of live
state.

The first-party `SurfaceRenderer` and `OffscreenRenderer` remain host adapters
that create targets and compose `physbox-sim`, `physbox-render`, and
`physbox-ui`. Their constructors are not the reusable simulation boundary.

## Simulation ownership

`physbox-sim` owns:

- rigid and particle mechanical state;
- body and shape lifecycle command encoding;
- broadphase, narrowphase, contact persistence, solver, sleeping, waking, and
  interaction mechanics;
- liquid mechanics and fluid/solid coupling once their existing implementation
  is migrated;
- GPU resource layouts and mechanical compute pipelines;
- semantic simulation configuration and validation;
- fixed-step clock behavior shared by interactive consumers;
- stable generational logical handles; and
- bounded mechanical capacity, timing, and asynchronous-result diagnostics.

It does not own:

- menus, tools, camera gestures, authored scenes, or scenario catalogs;
- launch argument names, query strings, persistence, or named application
  presets;
- a Box2D runtime, comparator state, or generic multi-backend interface;
- surface formats, backgrounds, colors, UI, or application render passes;
- platform lifecycle or unsafe native graphics interop; or
- oracle fixtures, acceptance verdicts, benchmark orchestration, or product
  reports.

Validation scenarios remain engine- or tooling-owned neutral projections.
They may construct a `World` through its supported API, but `ScenarioV1` and
`ValidationScenario` are not public world definitions.

## Configuration support boundary

Public controls are divided into three stability classes:

1. **Supported:** ordinary physical and quality controls that applications may
   store and expose, including fixed rate, substeps, iterations, supported
   contact response, sleeping, capacity, and explicitly promoted PhysBox
   quality choices.
2. **Experimental:** opt-in controls that have a documented public behavior
   but no compatibility promise. They require an explicit feature and cannot
   alter ordinary defaults.
3. **Internal:** solver-laboratory candidates, diagnostic pass switches,
   scenario-specific overrides, pipeline-layout choices, and validation-only
   instrumentation. These stay crate-private.

`physbox-sim` owns semantic validation and canonical defaults for supported
configuration. `physbox-launch` owns external spelling, parsing, migration,
and projection into that configuration. `physbox-engine` owns menu labels and
interaction. This prevents a reusable library from depending on application
launch policy while retaining one mechanical meaning on every host.

## Mutation and observation

CPU-authored mutations may reserve and return a logical generational handle
synchronously. Their mechanical effect is queued and becomes visible at the
next explicitly documented command-flush or step boundary. Invalid references,
capacity exhaustion, and rejected operations produce explicit results or
bounded asynchronous error records; they never silently disappear.

PhysBox does not add a per-body CPU transform mirror to imitate a synchronous
CPU engine. Live transforms, velocities, contacts, and particle data use one
of:

- GPU-to-GPU consumption through an approved read-only view;
- an explicitly requested bounded asynchronous snapshot;
- compact diagnostics arriving asynchronously; or
- a bounded headless flush after the final requested step.

Interactive point selection and grabbing remain GPU-resolved mechanics. An
application may express a neutral world-space grab intent, but it does not need
to read all body transforms back to the CPU to choose a body.

## Rendering ownership

`physbox-render` is optional. It owns:

- body, particle, and bounded debug render pipelines;
- presentation-only styles and render metadata interpretation;
- world-to-clip/view uniforms;
- encoding into a caller-provided color target; and
- bounded render reports.

It does not own simulation mutation or application semantics. It must be
possible to use `physbox-sim` without `physbox-render`, and an external
application may implement its own renderer against the supported render view.

Backgrounds and final composition are caller decisions. The first-party engine
may retain its current background pass and PhysBox visual theme. A Quest host
may instead draw physics into a transparent offscreen texture and place that
texture on a world-space plane without changing simulation ownership.

## Box2D and consumer-owned backend abstraction

Neither reusable crate re-exports the first-party Box2D comparator or defines a
generic `PhysicsBackend` trait for applications.

An application that wants comparison owns its abstraction and independently
imports Box2D. For example, `physbox-quest` may implement both
`PhysBoxBackend` and `Box2dBackend` behind its own capability-aware interface.
That arrangement:

- tests PhysBox through its real consumer API;
- prevents sandbox comparator policy and packaging from entering the library;
- lets a consumer choose which common operations it supports;
- keeps performance runs single-backend and authoritative; and
- exercises API alignment without claiming identical GPU/CPU observation
  behavior.

The existing default-enabled first-party comparator remains an
engine-controlled non-authoritative shadow under its current feature and
validation contract.

## Migration order

The extraction proceeds as a sequence of behavior-preserving seams:

1. Establish `physbox-sim` with the fixed-step clock, supported semantic
   configuration, public definitions, handles, and dependency negatives.
2. Separate mechanical compute resources from render pipelines inside the
   existing engine, removing target-format and camera dependencies from the
   mechanical world.
3. Move one complete rigid-world construction and stepping path into
   `physbox-sim`; retain temporary engine re-exports where necessary.
4. Move remaining particle and coupling mechanics without changing pass order,
   defaults, buffer layouts, or performance claims.
5. Establish `physbox-render` against the explicit read-only render view and
   migrate the first-party renderer.
6. Add a repository-local external-consumer example that depends only on the
   public crates and uses a caller-owned device, encoder, and target.
7. Let `physbox-quest` replace the local proof through path dependencies and
   qualify the same contract on a physical headset.

An empty placeholder crate does not satisfy a migration slice. Each new crate
must own real behavior before it becomes a declared consumer dependency.

## Compatibility and publication

Both crates remain workspace version `0.1.0` with `publish = false`. Local path
consumers observe the current checkout, including uncommitted changes; they are
responsible for recording an exact PhysBox 5 source revision and dirty state in
reproducible evidence.

Before publication, a separate human decision must cover:

- crate and package names;
- semantic-version and minimum-supported-Rust policy;
- API stability and deprecation;
- documentation and complete examples;
- license and package contents;
- feature/default policy;
- WGPU version interoperability;
- `cargo package` validation; and
- release provenance.

S36 proves a usable repository boundary, not a stable ecosystem promise.

## Validation

The extraction must prove:

- `physbox-sim` has no dependency on engine, launch, UI, comparator, shell, or
  app crates;
- `physbox-render` depends only on `physbox-sim`, WGPU, and small
  presentation/data utilities;
- a minimal external-style consumer creates and steps a rigid world without a
  surface or synchronous readback;
- the same consumer renders into a caller-owned texture;
- first-party scenario contracts, reports, configuration hashes, dispatch
  counts, and ordinary pixels remain unchanged except where ownership-only
  report provenance is deliberately updated;
- `npm run test:fast`, production clippy, fresh headless evidence, Browser
  WebGPU, comparator feature-on/off, direct comparator tests, ARM64 Android
  compilation, and one physical Android validation pass as required by the
  affected boundaries; and
- performance probes show no unexplained material regression from the
  ownership migration.

## Alternatives considered

### Make `EngineHost` public

Rejected. It is an application composition boundary containing tools, UI,
camera, scenario, report, comparison, and presentation policy. Publishing it
would expose the first-party sandbox rather than a mechanical world.

### Export `SurfaceRenderer`

Rejected as the core API. It owns device and surface lifecycle and therefore
cannot compose naturally with OpenXR or another application that already owns
its graphics context.

### Combine simulation and rendering in one crate

Rejected. Headless consumers and custom renderers should not require
presentation pipelines or a target format. The explicit same-device render
view is the required interoperability seam.

### Introduce a CPU product backend

Rejected. It contradicts the authoritative one-GPU-world direction and would
hide rather than solve GPU ownership and observation constraints.

### Put the generic backend trait in PhysBox

Rejected. The useful common surface depends on each application. Keeping it in
the consumer exposes whether PhysBox's actual API is usable without turning a
Box2D comparison convenience into library policy.

### Publish before the Quest proof

Rejected. A real independent application using local path dependencies is the
API-design test. Publication would prematurely freeze names and compatibility.

## Reconsideration triggers

Revisit this direction if:

- WGPU cannot safely express a useful read-only same-device render view without
  leaking private mechanical layouts;
- caller-recorded simulation and render work cannot preserve portable
  ordering across maintained hosts;
- stable logical handles require an unbounded CPU mirror;
- an important external consumer cannot operate with asynchronous observation;
- moving ownership changes dispatch count, simulation results, or mobile cost
  materially; or
- WGPU version coupling makes separate simulation/render crates unusable for
  the intended consumers.

These triggers may revise API details. They do not authorize synchronous
per-frame readback, host-owned mechanics, automatic CPU fallback, or exposing
the first-party workbench as the simulation library.
