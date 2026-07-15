# Box2D rigid authority

Status: deterministic authority and synchronized behavioral playback
implemented, 2026-07-14.

Governing direction:
[layered rigid oracle model](physbox5-unified-gpu-physics.md#layered-rigid-oracle-model).
Execution tracker:
[S1.3 Box2D authority and microscope lanes](../tactical/001-rigid-solver-laboratory.md#s13--box2d-authority-and-microscope-lanes).

## Decision

The rigid behavior authority is the clean, unmodified Box2D v3 checkout at
revision `3a4f0da8374af61293a03021c9a0b3ebcfe67948`. A native test-tool
executable calls Box2D's public C API directly and runs only through the shared
process-isolated oracle protocol. The product engine, platform applications,
and WebGPU path do not link the executable or Box2D.

The build script rejects a missing, dirty, or differently pinned checkout
before compiling. It configures an out-of-source static Release library with
samples, benchmarks, documentation, profiling, and Box2D unit tests disabled;
validation enabled; SIMD disabled; and floating-point contraction disabled by
the upstream build. The result provenance records the observed Rust, CMake,
and C compiler versions as well as these options.

The deterministic world is single-threaded (`workerCount = 1`, no task
callbacks), with sleep and continuous collision enabled and contact softening
disabled. The request supplies fixed `dt`, gravity, substep count, and
restitution threshold. Box2D owns its world, collision generation, warm state,
and iteration policy. Candidate-only bias, mass, impulse, and iteration fields
remain in the hashed source projection but are explicitly recorded as
source-only and are not translated into approximate Box2D settings.

## Shared projection

Both lanes consume `OracleRequestV1`; neither owns fixtures or another wire
format.

- The `world` lane projects neutral static and dynamic circles/rectangles,
  transforms, velocities, exact mass/inertia, material, initial awake state,
  gravity changes, point impulses, and a single ordered grab begin/target/end
  sequence. It executes exactly the requested fixed-step count and returns
  canonical body state plus public final manifolds.
- The `narrowphase` lane calls `b2CollideCircles`,
  `b2CollidePolygonAndCircle`, or `b2CollidePolygons` directly for every body
  pair and returns canonical manifolds without stepping a world.
- Authored manifolds and the contact cache remain candidate inputs. The
  authority deliberately regenerates its own contacts and solver state from
  the same neutral bodies rather than accepting those candidate values.

Unsupported semantics are rejected as structured `UnsupportedFeature` data;
they are never approximated. Current explicit rejections include inconsistent
static inertia, dynamic locked rotation in the world lane, missing action
bodies, actions outside the requested range, and any action in the direct
narrowphase lane. Unknown shapes or actions cannot deserialize as schema
version 1.

S4.3 maps the neutral grab only through public Box2D APIs. A kinematic target
body drives a motor joint whose body-B local frame is the authored world anchor
projected with `b2Body_GetLocalPoint`; target actions use
`b2Body_SetTargetTransform`, and end destroys the joint and target body. The
mapping uses `linearHertz = sqrt(springAcceleration) / tau`, damping ratio
`dampingRate / (2 * sqrt(springAcceleration))`, and maximum spring force
`mass * maxAcceleration`. The product linear/angular safety bounds are
explicitly source-only in Box2D provenance and remain GPU invariant gates; they
are not silently presented as mapped authority behavior. This freezes no
PhysBox product default or acceptance tolerance and does not change Box2D
authority precedence, world settings, source pin, or process isolation.
Missing/fixed bodies, nested grabs, target/end without an active grab,
unordered/out-of-range actions, and narrowphase actions are rejected rather
than approximated.

## Freshness and acceptance

The shared runner independently verifies the clean source pin before each
process launch. Every run receives a newly created directory and an atomically
written canonical request. The adapter creates one result staging file, syncs
it, and emits the exact success sentinel only after completion. The runner
accepts and atomically promotes that file only after schema, run id,
source/projection hashes, source identity, sentinel, and structured-failure
checks pass. Interrupted, stale, wrong-pin, wrong-hash, and unsupported results
cannot appear as accepted artifacts.

Direct public manifold comparisons agree within `2e-5` with the deliberately
scoped scalar generator for circle/floor, rectangle/floor, and axis-aligned
box-on-box geometry and topology. A deterministic 180-step circle replay
settles at `y = 0.49992886`, zero vertical velocity, and asleep on the current
authority build; an independent subprocess replay produces identical final
body and manifold bytes.

## Separate lanes

The validation build favors reproducibility and diagnostics. It is not the
optimized CPU performance comparator; S1.5 builds and labels that lane
separately. No patched microscope checkout is currently needed because scalar
pass traces and public Box2D body/manifold output localize the supported
fixtures. If a future failure cannot be localized, the microscope must use a
separate worktree, record this base revision plus its patch digest, and remain
diagnostic rather than acceptance truth.

## Validation

The authority crate tests direct manifold geometry, deterministic world replay,
structured rejection, clean-pin preflight, actual subprocess isolation,
sentinel/freshness promotion, and rejected-artifact preservation. The platform
boundary gate also rejects any dependency on the native oracle from an app or
shared product crate.

The synchronized playback command runs the shared circle/floor, P5.1
rectangle/floor projection, general rectangle/floor, and sliding-friction
scenarios through scalar and authority
lanes at completed steps 1, 15, 30, 60, 90, 120, and 180. It creates one fresh
directory containing a canonical versioned manifest, the 28 complete oracle
process directories, and a self-contained side-by-side SVG. The manifest
reports objective deltas but applies no unreviewed behavior tolerance. Dynamic
sleep differences remain visible because the scalar executable specification
does not implement a sleep policy.

```sh
cargo build -p physbox-box2d-oracle
cargo run -p physbox-lab --bin physbox-synchronized-playback -- \
  --oracle target/debug/physbox-box2d-oracle \
  --box2d-source reference/box2d \
  --output-root /tmp/physbox5-playback \
  --run-id RUN_ID
```

The command refuses to reuse a run directory and emits
`PHYSBOX5_VALIDATION_SUCCESS` only after syncing the manifest and SVG. A dirty
PhysBox harness is allowed for diagnosis only when its patch digest is
recorded; the S1.6 review package is regenerated from a clean committed tree.

## Reconsideration triggers

Changing the source revision, authority precedence, deterministic world
settings, projection meaning, or accepted public-manifold tolerance requires
human review backed by a measured migration. Add a projection feature only
when it has one exact public Box2D mapping. Add a microscope only after the
existing scalar and public output demonstrably fail to identify the first
divergence.
