# Reusable GPU physics library boundary

Status: implemented and corrected to a physics-only consumer contract on
2026-07-24. [S36](../tactical/036-reusable-gpu-library-extraction.md) records
the extraction and correction; [S37](../tactical/037-deferred-rigid-interaction-api.md)
records the implemented deferred rigid-interaction extension. This topic owns
the durable boundary between the GPU-resident mechanical world and
applications that consume it. It does not authorize publishing a crate or
changing a physics default.

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

PhysBox 5 exposes one unpublished workspace library for unversioned local-path
consumption:

- `physbox-sim` owns the authoritative GPU mechanical world, supported
  simulation configuration, explicit fixed-step encoding, stable logical
  handles, bounded diagnostics, and versioned physics-state observation.

There is no reusable PhysBox renderer. The former `physbox-render` extraction
was a boundary error and has been removed. The first-party 2D workbench
renderer is private to `physbox-engine`; external applications own their
geometry, materials, cameras, render passes, targets, and presentation.
`physbox-sim` exports no product renderer, rendering surface, camera, target,
draw command, shader, or private solver-buffer view.

`physbox-engine` remains the first-party application host. It owns launch
projection, authored scenarios, workbench tools, camera and input policy, UI,
reports, validation workflow, its 2D renderer, and the optional Box2D shadow.

```text
                         physbox-engine
                     first-party sandbox
                         /          \
                        v            v
                 physbox-sim     physbox-ui
                        ^
                        |
              consumer physics adapter
                        |
        OpenXR / renderer / surface / app policy
```

The first independent consumer is the separate `physbox-quest` repository. It
uses a filesystem path dependency on the current PhysBox 5 checkout. PhysBox 5
CI remains self-contained and never requires that sibling repository.

## Box2D-like consumer experience

The supported API should feel familiar to users of a Box2D-style Rust crate:

```rust
let mut world = World::new(&device, WorldDef::default())?;

let body = world.create_body(BodyDef::dynamic())?;
world.create_shape(body, ShapeDef::circle(0.1).with_density(1.0))?;

world.step(
    &device,
    &queue,
    &mut encoder,
    StepDef::fixed(1.0 / 60.0),
)?;
```

The resemblance covers a world object, definition values, opaque
generational handles, explicit create/destroy operations, supported global
settings, and explicit stepping. Definitions describe mechanics rather than
application scenes. They contain no camera, color, model, menu, launch preset,
validation rule, or backend-selection state.

The supported configuration includes Box2D-like physical controls where
PhysBox implements the same concept, plus promoted PhysBox quality controls:

- fixed outer-step rate;
- solver substeps and biased iterations;
- contact frequency, damping, speculative distance, and maximum
  depenetration velocity;
- sleeping enablement, threshold, and grouping quality;
- body capacity; and
- the selected rigid or coupled solver profile and promoted coupling quality.

Laboratory candidates, individual compute-pass switches, validation
instrumentation, and private storage choices remain internal. A Box2D setting
is not added merely for name symmetry when PhysBox has no corresponding
behavior.

PhysBox remains GPU resident. Unlike a CPU Box2D implementation, it does not
promise synchronous getters for current transforms, contacts, or particles.
That difference belongs in the consumer's backend adapter rather than being
hidden by a per-frame CPU mirror.

## Deferred queries, constraints, and lifecycle results

Decided 2026-07-24: reusable rigid interaction is expressed as ordinary
physics primitives rather than first-party tool intents. The public surface
adds:

- a bounded point query that reserves and returns a generational
  `BodySelectionId`;
- point-constraint creation from a deferred selection, with a synchronously
  reserved generational `PointConstraintId`;
- point-constraint target updates and destruction;
- selected-body destruction; and
- bounded asynchronous terminal command results and lifecycle counters.

This is the GPU-deferred counterpart of the common Box2D composition:
query shapes at a point, test eligibility, create/update/destroy a temporary
constraint, and destroy an identified body. It deliberately does not export
`BeginGrab`, `EraseAt`, pointer, controller, hand, Quest, OpenXR, tool, or
comparator vocabulary. A consumer adapter may compose those application
intents:

```text
BeginGrab -> point query -> create point constraint
Update    -> set point-constraint target
End       -> destroy point constraint
EraseAt   -> point query -> destroy selected body
```

Deferred handles are immediately usable by later commands queued for the same
outer step. Point-constraint creation reserves and returns its handle before
the query executes. A later query miss produces a terminal result and releases
the reservation. Target and destroy commands queued while creation is pending
execute predictably in command order: they either apply to the newly created
constraint or terminate as cancelled when its dependency misses or fails.

Each accepted state-mutating command has exactly one terminal result:
`succeeded`, `invalid`, `missed`, `cancelled`, `superseded`, or `failed`.
Capacity rejection is synchronous and returns no `CommandId`. Target updates
may either consume bounded command capacity individually or be coalesced;
when coalesced, every accepted superseded command still receives its terminal
result. Result delivery may apply bounded backpressure to further command
admission, but internal lifecycle reconciliation is non-lossy and cannot
depend on the application polling quickly enough.

Point selection is explicitly deterministic for the first public version:

- only live dynamic bodies qualify;
- sleeping dynamic bodies qualify and are awakened by a successful
  constraint;
- the point must be contained by the authored shape;
- overlapping hits choose the nearest body origin; and
- equal-distance hits choose the lowest generational `BodyId`.

Constraint tuning is mechanics-facing: frequency in hertz, damping ratio,
maximum acceleration, maximum linear speed, and maximum angular speed.
PhysBox must not label acceleration limiting as force limiting unless it
implements the required mass scaling. Defaults preserve the current
first-party interaction mechanics.

`World::step` only records GPU work. It never maps, waits, or submits.
Applications explicitly request and submit bounded result copies, then poll
them asynchronously. Commands cancelled before `step` receive deterministic
cancelled results and release their reservations. Once commands have been
recorded into a caller-owned encoder they cannot be revoked; their result batch
must be reconciled before admitting more commands. Dropping or replacing a
world releases all remaining reservations without inventing observable
results after the consumer has discarded the world.

Public counters distinguish command and result capacity, queued and in-flight
commands, pending and live constraints, pending body destructions, high-water
marks, and synchronous rejections. They describe boundedness and lifecycle,
not sandbox diagnostics or private compute-pass state.

## GPU ownership and stepping

The application owns:

- adapter and device selection;
- the `wgpu::Device` and `wgpu::Queue`;
- command-encoder creation, ordering, and queue submission;
- windows, surfaces, swapchains, OpenXR images, and device-loss recovery;
- all render resources and passes; and
- platform-specific unsafe graphics interop.

`physbox-sim` constructs resources on a caller-provided device. `World::step`
records one explicit outer fixed step into a caller-provided encoder and may
use the queue only for bounded uploads associated with explicit authored
operations. It never creates a graphics target and never privately submits the
caller's frame.

An interactive consumer admits zero or one outer step per presented frame.
Excess wall time slips permanently; solver substeps are internal numerical
work, not catch-up.

## Physics observation, not rendering interoperability

Applications often need live physics state for rendering or game logic without
reading every body through the CPU. The public same-device seam is therefore a
versioned physics observation:

- `GpuBodyObservation` owns a canonical output buffer separate from private
  solver storage;
- the caller explicitly records its projection with `encode`;
- records contain only pose, velocity, generation, slot, and public
  live/sleeping/fixed flags;
- the output buffer supports storage-buffer consumption and bounded copy-out;
  and
- the caller owns ordering, submission, and any interpretation after the
  observation has been recorded.

The observation contains no shape geometry, color, draw ordering, indirect
draw arguments, camera, world-to-clip matrix, target format, material,
lighting, or render pipeline. Consumers already own or project the shapes they
authored and associate them with logical handles. A Quest renderer may turn a
circle into a stereo sphere and a rectangle into a cuboid; a desktop game may
choose sprites; neither choice is part of physics.

The canonical record is an interoperability contract, not permission to bind
private mechanical buffers. `RenderView` and `ParticleRenderView` are not in
the public crate surface. The `engine-internal` feature and hidden module are
unstable first-party migration plumbing and are forbidden to external
consumers.

Explicit asynchronous snapshots remain available for tools, save flows, and
bounded validation. Interactive advancement and ordinary consumer rendering
must not depend on per-frame snapshot completion.

## Simulation ownership

`physbox-sim` owns:

- rigid and particle mechanical state;
- body and shape lifecycle command encoding;
- broadphase, narrowphase, contact persistence, solving, sleeping, waking,
  and interaction mechanics;
- liquid mechanics and fluid/solid coupling;
- private GPU layouts and mechanical compute pipelines;
- semantic simulation configuration and validation;
- the fixed-step clock policy;
- stable generational handles; and
- bounded mechanical capacity, timing, and asynchronous-result diagnostics.

It does not own:

- a renderer, surface, texture target, render pass, camera, visual theme, or
  application model;
- menus, tools, authored scene catalogs, or scenario names;
- launch spelling, query strings, persistence, or product presets;
- Box2D, a comparator, or a generic multi-backend trait;
- platform lifecycle or native graphics interop; or
- oracle fixtures, acceptance verdicts, benchmark orchestration, and product
  reports.

The current public authoring surface covers rigid circles and rectangles.
Particle mechanics are present in the first-party implementation, but public
particle authoring and canonical particle observation require a later
consumer-driven slice. Validation `ScenarioV1` values remain engine/tooling
inputs and are not public world definitions.

## Consumer-owned backend wrapper

PhysBox does not define a lowest-common-denominator `PhysicsEngine` trait.
Each application owns the abstraction for the behavior it actually needs.
For example, an application may implement:

```text
consumer PhysicsEngine
        |
        +-- PhysBoxBackend -> physbox-sim
        |
        +-- Box2dBackend  -> independently imported Box2D
```

The wrapper should use consumer-owned entity IDs, world/shape descriptions,
actions, observations, events, and capability flags. Backend-native handles
stay inside each adapter. Shared rigid operations can remain intentionally
similar while capabilities report meaningful differences such as GPU
observation, water, or synchronous CPU inspection.

This keeps the public PhysBox API naturally close enough to Box2D to wrap
without pretending the implementations have identical execution models.
In particular:

- do not force PhysBox into synchronous per-body reads because Box2D has them;
- do not claim Box2D supports PhysBox water;
- do not expose a Box2D-only setting as a PhysBox setting;
- do not make either backend's native handle the application entity ID; and
- do not let backend selection silently change authored scene meaning.

An application that wants comparison imports and pins Box2D independently.
The first-party PhysBox comparator remains private application code and is
never re-exported by `physbox-sim`. Performance measurements instantiate and
step only the measured backend.

## Compatibility and publication

`physbox-sim` remains workspace version `0.1.0` with `publish = false`. A local
path dependency observes the current checkout, including uncommitted files.
Reproducible evidence records the exact PhysBox revision and dirty state.

Before publication, a separate decision must cover naming, semantic-version
policy, MSRV, deprecation, documentation, license/package contents, features,
WGPU compatibility, `cargo package`, and release provenance. S36 proves a
usable repository boundary, not an ecosystem stability promise.

## Validation

The boundary is accepted only when:

- `physbox-sim` has no application, engine, launch, UI, comparator, shell, or
  rendering dependency;
- public rustdoc contains no product renderer or render-view type;
- deferred query/constraint/destruction commands chain within one step,
  produce one guaranteed terminal result per accepted mutation, and reconcile
  body and handle lifecycle without synchronous GPU waits;
- point destruction eventually corrects live-body accounting, invalidates
  stale public handles, and permits deterministic slot reuse;
- the external proof depends only on `physbox-sim`, creates and steps a rigid
  world without a surface, performs query/constraint/target/release/destruction
  through the public API, and completes explicitly requested observation and
  result copies;
- the separate Quest adapter compiles through the same public local-path API;
- first-party simulation results, dispatch graphs, reports, pacing, and
  pixels remain unchanged by ownership movement; and
- native, Browser WebGPU, headless, comparator feature-on/off, performance,
  ARM64/APK, and physical Android lanes pass in proportion to affected code.

Exact correction evidence is retained in S36.

## Alternatives considered

### Expose `EngineHost` or `SurfaceRenderer`

Rejected. Both are first-party application composition and presentation
boundaries containing sandbox policy.

### Publish an optional 2D renderer crate

Rejected after the initial extraction. A 2D renderer still defines shapes,
visual styles, target formats, and camera conventions that do not belong to a
physics library and do not help the stereo Quest use case. Keeping it adjacent
made private GPU layout leakage appear to be a supported consumer contract.

### Expose private solver buffers as a render view

Rejected. It couples consumers to storage layouts, indirect-draw conventions,
and presentation metadata that physics must remain free to change. A
dedicated canonical physics observation is the supported GPU-to-GPU seam.

### Add a CPU product backend or PhysBox-owned backend trait

Rejected. A CPU authority contradicts the one-GPU-world direction, while the
useful wrapper boundary depends on each application and its capability needs.

### Publish before the independent consumer proof

Rejected. Local path consumption by a separate product is the API-design test;
publication would freeze names and compatibility too early.

## Reconsideration triggers

Revisit API details if:

- a real consumer cannot render or operate from canonical physics
  observations without private layout access or per-frame CPU readback;
- a useful Box2D-style interaction cannot be composed from neutral point
  queries and temporary constraints without exposing application tool policy;
- bounded command-result backpressure prevents an interactive consumer from
  maintaining its intended fixed-step cadence;
- a required physical state cannot be represented in a bounded,
  versioned same-device observation;
- stable logical handles require an unbounded CPU mechanical mirror;
- caller-recorded work cannot preserve ordering across maintained hosts;
- ownership movement changes results, dispatch count, or mobile cost
  materially; or
- WGPU version coupling makes the intended local-path integration unusable.

These triggers do not authorize exposing the first-party renderer, surface,
workbench, comparator, or private solver storage as the library API.
