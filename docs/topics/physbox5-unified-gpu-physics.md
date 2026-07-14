# PhysBox 5: One GPU-Resident 2D Physics World

Status: active direction, 2026-07-14. The lean PhysBox 5 bootstrap is live at
`~/code/physbox5`, fresh-history commit
`3de0b12001b2246e3966f7d1264225dd4da903f4`, and
<https://physbox5.kzahel.com>. PhysBox 4 remains the preserved snapshot of the
split CPU-rigid/GPU-fluid investigation.

This is the authoritative technical direction for PhysBox 5. `docs/vision.md`
is its short entry-point summary; when the two differ, update this topic and
then bring the summary back into agreement.

## Decision in principle

PhysBox 5 should pursue one small, purpose-built, GPU-resident 2D physics world
for rigid bodies and fluid. It should feel like a deliberately simplified
Box2D with a native fluid material, not like a general 3D engine constrained to
a plane and not like two simulators connected by delayed messages.

The required implementation must run through the same Rust, `wgpu`, and WGSL
engine on native desktop, browser WebGPU through `wasm32-unknown-unknown`,
Android, and headless validation. Native-only float atomics, subgroups, or
larger limits may accelerate the same algorithms, but the portable WebGPU path
defines the product contract.

The first simulation slice is a rigid-only foundation:

- fixed and dynamic circles;
- fixed and dynamic oriented rectangles;
- gravity, direct placement, erase, and grab;
- circle/circle, circle/rectangle, and rectangle/rectangle contact;
- friction, restitution, stacking, sleeping, and waking;
- direct rendering from GPU-resident body state;
- side-by-side validation against a test-only Box2D oracle.

Liquid begins only after the GPU rigid solver reproduces a deliberate suite of
basic 2D behaviors convincingly. Particle coupling will then use
Box2D/LiquidFun and PhysBox 3 as its behavioral oracle. Body joints, capsules,
convex polygons, chains, ragdolls, machines, soft bodies, terrain, CCD, and the
broader PhysBox tool set likewise follow only after the minimal rigid world is
trustworthy.

## Why this is a new engine direction

PhysBox 3 established the desired subjective baseline. Its particle counts
were lower, but rigid bodies and LiquidFun particles participated in one
synchronous mechanical world. Contact used the body's current velocity and
effective mass, and equal-and-opposite response happened at the contact step.
The result felt consistent, natural, and smooth.

PhysBox 4 deliberately tested a different architecture:

```text
CPU Rapier rigid world
    -> per-step collider snapshots uploaded to the GPU
    -> GPU PBF/XPBD particles project against those snapshots
    -> compact reaction packets are read back asynchronously
    -> packets affect a later CPU rigid state
```

The implementation fixed rendered-frame impulse bursts, tagged packets by
source step, bounded queues, measured packet age, added passivity and damping
experiments, and improved stationary-boundary treatment. Those changes made
the failure understandable and reduced specific artifacts, but they did not
restore a single mechanical state. At normal load, reaction packets still
arrive about three fixed steps late. They are derived from particle positional
projection rather than the exact momentum exchange of a coupled contact, and
the delayed CPU application can inject energy into translation or rotation.
Light and near-neutrally buoyant bodies also raise the unresolved risk of
added-mass instability in explicit partitioned fluid/structure coupling even
if transport delay is reduced.

This is no longer primarily a tuning problem. Passivity guards, predictors,
added-mass approximations, local damping, sleeping classifications, and
settled-boundary policies can each mitigate a failure mode, but together they
remain a synchronization protocol between separate worlds. Continuing to add
compensators risks making the sandbox numerically acceptable while leaving it
subjectively unpleasant and difficult to reason about.

The conclusion is not that GPU fluid was a mistake. The conclusion is that
interactive dynamic bodies cannot remain authoritative on the CPU if natural
two-way fluid interaction is a defining product behavior.

See:

- [Asynchronous fluid-to-rigid feedback](https://github.com/kzahel/physbox4/blob/main/docs/topics/asynchronous-fluid-rigid-feedback.md)
  for the measured delay loop, trampoline reproduction, passivity results, and
  unresolved light-body and torque behavior;
- [Settled dynamic fluid boundaries](https://github.com/kzahel/physbox4/blob/main/docs/tactical/018-settled-dynamic-fluid-boundaries.md)
  for the separate moving-versus-stationary boundary representation gap;
- [PhysBox 3 and LiquidFun water reference](https://github.com/kzahel/physbox4/blob/main/docs/topics/physbox3-liquidfun-water.md) for
  the synchronous behavioral reference;
- [Float atomics and WebGPU portability](https://github.com/kzahel/physbox4/blob/main/docs/topics/float-atomics.md) for required and
  optional GPU capability tiers.

## Product vision

PhysBox 5 is a touch-first 2D construction and material sandbox in which
circles, rectangles, and fluid visibly belong to the same world. A body falling
into water, floating, rotating under an off-center load, settling against the
floor, being grabbed, and being struck by a wave should form one continuous
interaction rather than a sequence of approximate cross-simulator effects.

Consistency and feel take priority over headline particle count. GPU residency
exists to make one rich world possible at useful scale, not to justify a
larger but less convincing particle number. A lower-count coherent scene is a
better product than a high-count scene whose objects trampoline, lag, pin, or
require scene-specific damping.

The initial rigid experience should remain playable throughout development:

- place circles and rectangles;
- grab, drop, stack, and throw bodies;
- build a small stack, container, or ramp;
- observe falling, sliding, rolling, edge impacts, friction, restitution, and
  sleep/wake transitions;
- reset and replay the same scene on desktop and in the browser.

After those interactions match the intended Box2D-like baseline, water returns
as the first additional material. The later experience adds pouring, buoyancy,
displacement, off-center fluid torque, and wave impacts without introducing a
second mechanical world.

## One authoritative mechanical state

The GPU owns live rigid-body simulation state from the first milestone and
later owns particle state in the same world:

```text
CPU-authored commands and neutral input intents
    -> GPU command buffer
    -> shared fixed-step compute graph
       -> predict bodies and, later, particles
       -> build spatial/contact data
       -> solve body/body constraints
       -> later solve fluid and particle/body constraints in the same graph
       -> apply equal-and-opposite corrections
       -> reconstruct body and, later, particle velocities
       -> update sleeping/wake state
    -> render directly from GPU buffers
```

The CPU continues to own authored scene records, tools, UI, save/load policy,
platform lifecycle, and small commands. It may receive delayed compact
diagnostics, selection results, events, and explicit save snapshots. None of
those readbacks may be required to advance the next simulation step.

"GPU-resident" therefore does not mean placing the entire application in a
shader. It means that every value participating in mechanical feedback is
current and available to the same GPU solve graph.

One authoritative state does not require one constraint formulation for every
material. A Box2D-like velocity-impulse rigid solver may later coexist with
XPBD/PBF fluid constraints if both operate on current GPU state, exchange
equal-and-opposite response inside the same fixed step, and share an explicit
ordering and accumulation contract. Do not select a weaker rigid formulation
only because liquid may use position constraints later.

## Minimal 2D body model

A dynamic body initially needs only:

```text
stable body id
shape kind and dimensions
position: vec2<f32>
angle: f32
linear velocity: vec2<f32>
angular velocity: f32
inverse mass: f32
inverse moment of inertia: f32
material and collision flags
sleep state
```

True 2D avoids much of a general 3D engine:

- no quaternions;
- no 3D inertia tensor or world-space tensor rotation;
- no gyroscopic rigid-body terms;
- scalar angular velocity and torque;
- one friction tangent rather than a tangent plane;
- small circle/rectangle contact manifolds;
- no swing, twist, cone, or generic six-degree-of-freedom joint machinery.

The first version should permit one analytic shape per body. Compound bodies
and arbitrary convex geometry are deferred until the core representation and
contact solve are proven.

## Later fluid-coupling contract

This contract is not part of the first rigid milestone. It records the
requirement that motivates building the rigid solver first, so liquid cannot be
added later through another delayed CPU/GPU bridge.

For a particle contacting a body along normal `n`, with `r` from the body's
center of mass to the contact point, the effective inverse mass includes both
translation and rotation:

```text
w = particle_inverse_mass
  + body_inverse_mass
  + body_inverse_inertia * cross(r, n)^2
```

One contact correction or impulse must affect both participants during the
same fixed step:

```text
particle delta       += lambda * particle_inverse_mass * n
body position delta  -= lambda * body_inverse_mass * n
body angle delta     -= lambda * body_inverse_inertia * cross(r, n)
```

The exact signs and formulation depend on the selected constraint convention,
but the invariant is durable: fluid and rigid response are one constraint, not
an estimated reaction exported to another solver.

Fluid boundary density, iterative pressure correction, final non-penetration,
surface-relative velocity, friction/drag, and velocity reconstruction must be
designed together. It is insufficient to make only the last collider
projection two-way while earlier fluid iterations treat the body as an
unchanging wall.

## Initial rigid GPU pipeline hypothesis

The complete pipeline should favor explicit contracts and validation before an
immediately optimal broadphase:

1. Apply bounded GPU body commands.
2. Integrate gravity and predict body position and angle.
3. Generate canonical broadphase circle/rectangle candidate pairs.
4. Produce narrowphase manifolds with stable feature identifiers.
5. Merge current and previous contacts and transfer warm-start impulses.
6. Prepare normal, friction, restitution, and effective-mass constraints.
7. Solve constraints through a portable conflict or reduction strategy.
8. Integrate or reconstruct final body poses and velocities.
9. Apply damping and sleep/wake policy.
10. Render directly from the resulting GPU state.

The leading solver hypothesis is a Box2D Soft-Step/TGS-like velocity-impulse
solver: several substeps, contact generation once per fixed step, local contact
anchors retained across the substeps, separation updated from current poses,
accumulated projected impulses, and a biased solve followed by an unbiased
relaxation solve. Rigid XPBD remains a measured alternative, not the default
selected merely because fluid may later use XPBD/PBF constraints.

### Solver laboratory before the product pipeline

Solver selection must happen before broadphase, narrowphase, tools, or product
rendering can disguise its behavior. A small engine-owned laboratory should
feed authored contact manifolds directly into:

- a scalar Rust executable specification;
- portable contact-centric Jacobi with per-body gather;
- graph-colored or disjoint-batch Soft-TGS;
- optionally a rigid XPBD or split position/velocity alternative.

The laboratory is validation code, not a second product engine. It should cover
one contact, symmetric and off-center impact, two-body chains, two-, five-, and
ten-body stacks, pyramids, thin-edge landings, friction, restitution, extreme
mass ratios, and fixed-step/substep sweeps. Every variant consumes the same
precomputed manifolds and emits the same versioned result schema.

Compare variants using:

- bounded penetration and stack-height error;
- resting linear/angular velocity and jitter RMS;
- artificial kinetic and total-energy change;
- friction, restitution, and sleep-transition behavior;
- convergence versus substep or iteration count;
- robustness to mass ratio and contact ordering;
- GPU time, dispatch count, scratch memory, and capacity pressure;
- repeatability and behavior across portable WebGPU adapters;
- suitability for later particle/body contribution gathering.

The solver foundation is chosen after this objective matrix and one
side-by-side behavior review. That is a deliberate human decision gate. Later
milestones may tune accepted parameters inside checked numerical bands, but
must not silently switch solver families.

The chosen design should optimize first for convincing stacks, edge contacts,
sliding, rolling, bounce, and sleep at the expected sandbox body count. It is
acceptable for a GPU solver to lose to Box2D at tiny body counts. GPU residency
is initially required for coherent future coupling, not to claim a speedup for
ten bodies. Fluid-specific constraints and coupled iterations must not
complicate the solver laboratory.

## Portable and accelerated accumulation

Standard browser WGSL guarantees integer atomics, not `atomic<f32>`. The
primary portable experiment should therefore be contact-centric Jacobi with a
body gather:

1. One invocation owns a contact manifold, reads both bodies from the previous
   iteration, computes one shared normal/friction impulse, updates its cached
   impulse, and writes equal-and-opposite contribution records for bodies A and
   B.
2. A body-to-contact-edge adjacency for the current active topology is built on
   the GPU with count, prefix scan, and scatter.
3. One body-owning invocation or workgroup gathers its contiguous `f32`
   contributions and writes the next velocity buffer.
4. Body velocities ping-pong between iterations or substeps.

Unlike a pseudo-Jacobi method in which each body independently solves its side
of a contact, this retains one impulse per contact and preserves
equal-and-opposite response by construction. Mass splitting should be measured
for parallel-stack convergence rather than assumed. The contact-edge layout
also creates a plausible later path for many particle constraints targeting one
rigid body.

The principal alternative is GPU graph coloring or bounded disjoint batches.
It permits Gauss-Seidel-style immediate writes and may reproduce Box2D stacks
with fewer solver iterations, but it must be evaluated with the cost of every
color dispatch and with a fully GPU-driven coloring path. A CPU readback of
color count, convergence, or conflict state may not be required to advance the
step. A fixed bounded color schedule is acceptable only with explicit overflow
and fallback failure, never silently dropped constraints.

Other portable fallbacks that may be measured include segmented `f32`
reduction and bounded fixed-point integer atomic accumulation with documented
precision, range, saturation, and overflow diagnostics.

Supported native Vulkan or Metal adapters may select
`SHADER_FLOAT32_ATOMIC` for an otherwise equivalent accumulation stage. This
is a capability-selected shared-engine optimization, not a separate native
physics implementation. Both paths must consume the same contacts, produce the
same logical per-body corrections, and remain within documented scenario
tolerances.

The portable path may be slower without being lower quality. Sorted or
segmented `f32` reduction can be more repeatable than unordered float atomics.
No material, shape, tool, or scene may require the native path.

The first rigid solver may avoid atomic accumulation altogether through
constraint coloring or disjoint batches. The reduction strategies become more
important when liquid introduces many particle contacts targeting one body.

## Pair, manifold, and contact persistence contract

Broadphase, narrowphase, contact persistence, and solving must be separate
contracts so each can be compared against a simpler oracle. The initial
contract is:

1. Broadphase emits canonical `(min_body_handle, max_body_handle)` pairs.
2. Narrowphase emits a canonical normal and at most two points per manifold.
3. Each manifold point carries stable geometric feature identifiers, local
   anchors on both bodies, original separation, and material data.
4. A persistent contact key contains both body handles, including slot
   generations, plus the feature identifiers.
5. Current and previous keys are lexicographically radix-sorted and merged.
6. Matching points inherit accumulated normal and tangent impulses; unmatched
   points begin cold and expired points are removed.
7. Prepared constraints retain the local anchors needed to update separation
   without regenerating the manifold during each solver substep.

Use tuples of `u32` fields rather than requiring shader 64-bit integers.
Initially prefer sorted merge over a concurrent GPU hash table. It is easier to
inspect, is more repeatable, and shares scan/radix infrastructure with the
broadphase. Slot generation is required so a newly created body cannot inherit
the warm-start state of an erased body that previously occupied the same slot.

Circle/circle and circle/rectangle contact are analytic. Rectangle/rectangle
uses SAT to select a reference and incident edge, clips the incident segment,
and produces stable vertex/edge feature pairs. Test feature stability around
nearly equal separating axes and use an explicit bias to prevent reference-face
flip-flop. Speculative separation and slop are shared solver parameters rather
than hidden narrowphase constants.

Restitution should be a distinct velocity stage rather than folded into
penetration stabilization. Friction clamps the accumulated tangent impulse
against the current accumulated normal impulse. Manifold points should be
solved together by one contact invocation initially so they share body reads
and a consistent normal/friction convention.

## Broadphase development contract

Do not port Box2D's dynamic AABB tree merely because Box2D is the behavior
oracle. It is a semantic reference for complete and stable pair lifecycle, not
the presumptive GPU algorithm.

Develop the broadphase in three steps:

1. Use all-pairs AABB testing for small solver and oracle fixtures. It is the
   trusted baseline and remains the brute-force validation oracle.
2. Bake off full-sort 1D sweep-and-prune against a multi-cell uniform/hash grid
   on the same scenario corpus.
3. Introduce an LBVH only if object-size variation, sparse large worlds, or
   measured grid/SAP behavior justifies its additional complexity.

The grid candidate should count AABB/cell overlaps, prefix scan, scatter cell
records, radix-sort by cell, enumerate canonical pairs, and sort/unique the
result. Very large fixed shapes such as the floor should live in a small global
static list tested separately against dynamic bodies so they do not determine
grid cell size or appear in many cells.

All candidates produce the same sorted canonical pair contract. Randomized and
adversarial tests compare complete pair sets against brute force: false
positives cost time but are allowed, while a false negative is always a
correctness failure. Record candidate amplification, duplicate count, maximum
cell occupancy or traversal depth, capacity high-water, and every overflow.
No capacity failure may discard a pair silently.

## What to learn from existing engines

### Box2D rigid oracle

The first milestone should run the GPU solver beside an unmodified, test-only
Box2D world. One engine-owned scenario definition supplies the same authored
bodies, initial states, gravity, fixed step, and neutral action sequence to
both. Box2D is an oracle and comparison harness, not a product backend, browser
fallback, or second scene format.

The initial source reference is the Box2D checkout already used by PhysBox 3,
commit `3a4f0da8374af61293a03021c9a0b3ebcfe67948` (currently
`v3.1.1-18-g3a4f0da`). Freeze the exact revision, build options, world settings,
worker count, and scenario projection. The deterministic oracle lane should use
a single-threaded/unthreaded configuration; a separate PhysBox 3 product lane
may preserve the original worker and LiquidFun settings. An oracle result is
not reproducible if it records only "Box2D" without this configuration.

Use Box2D as the primary reference for rigid behavior:

- circle and polygon manifold generation;
- contact-point effective mass;
- warm starting;
- restitution and Coulomb friction;
- sleeping and waking;
- mouse/grab constraints;
- revolute and distance joint semantics;
- the subjective behavior expected from a playful 2D sandbox.

PhysBox 5 is not required to preserve the Box2D API, serialization, internal
handles, or exact solver. "Simplified Box2D" describes the feature model and
quality target. Exact per-step equality is also not expected: the GPU solver
may use different constraint ordering and floating-point reduction. The oracle
should compare trajectories and durable behavior within scenario-specific
tolerances while retaining side-by-side visual playback for failures.

The first oracle fixture set should cover:

- one circle and one rectangle falling onto a fixed floor;
- a rectangle landing flat, on a corner, and on a thin edge;
- circle/rectangle and rectangle/rectangle impact from both sides;
- symmetric and off-center collision with visible rotation;
- two-, five-, and ten-body stacks settling under gravity;
- a pyramid or alternating stack that exposes manifold instability;
- sliding to rest under friction;
- rolling and restitution at several material values;
- a body waking another sleeping body;
- grab, move, release, and resulting linear/angular velocity;
- fixed-step and solver-iteration sweeps that reveal instability rather than
  hiding it behind one tuned default.

Each scenario should record body position, angle, linear/angular velocity,
penetration, contact count, sleep transition, and total kinetic/potential
energy summaries. Acceptance should emphasize bounded penetration, stable
resting contact, plausible trajectories, and absence of artificial energy
growth rather than matching Box2D bits.

### Layered rigid oracle model

Full-world trajectory comparison is necessary but too coarse to explain most
failures. Validation should form a hierarchy:

| Layer | Oracle | Required result |
| --- | --- | --- |
| Shader math and integration | Scalar Rust executable specification | Exact or tightly bounded `f32` results |
| Narrowphase | Public Box2D collision functions | Same manifold topology and bounded normal, point, separation, and feature results |
| Broadphase | Brute-force AABB enumeration | No missing canonical pairs |
| Contact solve | Analytic invariants plus scalar solver | Correct impulse signs, limits, and equal-and-opposite response |
| Full rigid scenario | Pinned unmodified Box2D world | Scenario-specific trajectory and stability bands |
| Product behavior | PhysBox 3 traces and synchronized playback | Intended neutral interaction and subjective feel |
| Cross-host behavior | Same PhysBox GPU scenario on each host | Same structural outcome and justified numerical bands |

The scalar Rust solver mirrors the selected GPU equations and buffer ordering.
It is an executable specification and diagnostic tool, not a CPU product
fallback. Individual compute passes should be runnable against captured input
buffers and compared with that specification before a full-world mismatch is
investigated.

Tests should include physical and metamorphic invariants independent of Box2D:

- translation and rotation equivariance;
- body-label swap symmetry under the canonical normal convention;
- no normal impulse for separating contacts outside the speculative margin;
- nonnegative accumulated normal impulse;
- `abs(tangent_impulse) <= friction * normal_impulse`;
- linear and angular momentum conservation for isolated frictionless systems;
- bounded energy change under the configured restitution;
- no NaN, infinity, unreported saturation, or unreported overflow;
- decreasing error rather than divergence as substeps increase;
- no sleeping drift and no wake without a defined cause.

Randomized failures should be deterministically reduced to smaller bodies,
contacts, action sequences, and fixture windows. The minimized reproducer joins
the permanent corpus. Checked numerical baselines are reviewed artifacts; a
failing tolerance is not widened or regenerated automatically.

### Box2D authority and microscope builds

Keep two distinct Box2D lanes:

- The authoritative lane links an unmodified pinned Box2D library. Its public
  state, events, manifolds, and debug draw data own pass/fail comparisons.
- A separately patched microscope build records internal stages for diagnosis.
  Its patch digest is provenance, and its output never becomes the authority.

The microscope may trace broadphase pair creation, manifolds before and after
feature matching, prepared effective masses and bias values, constraint graph
colors, per-substep accumulated impulses and body velocity deltas, relaxation,
restitution, islands, and sleep transitions. Box2D's existing debug draw flags
for bounds, contact points, feature identifiers, normals, impulses, graph
colors, and islands should be projected into synchronized PhysBox playback.

For exact narrowphase comparison, call Box2D's public `b2CollideCircles`,
`b2CollidePolygonAndCircle`, and `b2CollidePolygons` functions directly. These
return the small public manifolds and stable feature identifiers that PhysBox
needs to reproduce; a complete Box2D world step is unnecessary for this layer.

### LiquidFun and PhysBox 3 fluid oracle

Only after the rigid fixture set passes should PhysBox 5 restore liquid. Use
LiquidFun and PhysBox 3 as the behavioral reference for:

- synchronous particle/body momentum exchange;
- displacement and buoyant support;
- current contact-point surface velocity and rotational effective mass;
- off-center fluid torque;
- floating, sinking, and wake behavior;
- dam-break impact and containers with moving boundaries;
- the natural, smooth subjective behavior that motivated the new engine.

As with Box2D, the oracle exists in validation and comparison tooling. The
product path remains the one GPU solver, and the fluid solver may differ from
LiquidFun internally as long as the resulting interactions meet the recorded
behavioral contract.

### PhysX 5 GPU

The pinned [PhysX source](https://github.com/NVIDIA-Omniverse/PhysX) is a
reference for GPU organization rather than a codebase to port wholesale.
Relevant areas include
[the GPU solver](https://github.com/NVIDIA-Omniverse/PhysX/tree/b4b286abff6f2b3debd1d1acb120dc428765cf2e/physx/source/gpusolver)
and
[the GPU simulation controller](https://github.com/NVIDIA-Omniverse/PhysX/tree/b4b286abff6f2b3debd1d1acb120dc428765cf2e/physx/source/gpusimulationcontroller).
The initial pinned reference is commit
`b4b286abff6f2b3debd1d1acb120dc428765cf2e`, PhysX 5.6.1.

Study:

- device-resident body and shape state;
- broadphase, contact generation, constraint preparation, solve, and
  integration phases;
- pair/contact compaction and work queues;
- fixed-capacity GPU memory and overflow policy;
- structure-of-arrays layouts;
- constraint batching and conflict management;
- warm starting, sleeping, waking, and direct GPU state access;
- CPU/GPU ownership boundaries that avoid automatic state synchronization.

Do not inherit:

- 3D vectors, quaternions, inertia tensors, or shape complexity;
- articulations and six-degree-of-freedom joint machinery;
- CUDA warp-size, cooperative-group, float-atomic, or device-barrier
  assumptions;
- CPU preparation/fallback paths that would recreate a required host feedback
  dependency;
- an API and compatibility surface intended for a general industrial engine.

PhysX supports planar-looking motion by locking Z translation and X/Y rotation,
but it remains a 3D engine internally. PhysBox should instead specialize its
data, collision math, torque, friction, manifolds, and joints for true 2D.

PhysX GPU source is BSD-3-Clause. Any copied rather than independently
reimplemented source must retain the required copyright and license notices.

PhysX documentation is also a capacity and performance warning. GPU buffers
are fixed-capacity in important parts of the pipeline and undersizing can
discard contacts, constraints, or pairs. PhysBox must turn the equivalent case
into an explicit validation failure. PhysX reports its largest GPU advantage at
several thousand active bodies, so PhysBox should not require GPU rigid bodies
to beat Box2D on tiny scenes before the architecture is considered useful. See
[PhysX GPU rigid bodies](https://nvidia-omniverse.github.io/PhysX/physx/5.2.1/docs/GPURigidBodies.html).

### `wgmath`, `wgparry`, and `wgrapier`

[`wgmath`](https://github.com/wgmath/wgmath) is the closest portable research
reference: Rust, WGSL, WebGPU, 2D/3D collision, LBVH broadphase, narrowphase,
warm starting, graph coloring, Soft-TGS, and Jacobi. Study its shader layouts,
radix/LBVH construction, body-to-constraint adjacency, indirect dispatches,
and solver variants before independently implementing those stages. The
research snapshot inspected for this proposal was commit
`ac89dceb8a33778e895303cd606da38afb015192`.

Do not adopt it as the product architecture or dependency without a separate
decision. The inspected work-in-progress implementation currently reads the
broadphase pair count to the CPU for resizing, reads graph-coloring convergence
to the CPU, drives a variable number of color passes from the host, and
documents a body-centric pseudo-Jacobi method that does not preserve one shared
equal-and-opposite contact impulse. These are useful experiments but violate
PhysBox's rule that no GPU result may be required to advance the next step.
The project explicitly describes the libraries as under heavy development.

### Other implementation and research references

- [Bullet3 OpenCL rigid bodies](https://github.com/bulletphysics/bullet3/tree/master/src/Bullet3OpenCL/RigidBody)
  contains GPU Jacobi/PGS contact solvers, batching, broadphase, and narrowphase.
  It is useful historical source, but its OpenCL GPU path is experimental and
  includes host-oriented preparation patterns that are not the PhysBox host
  contract.
- [Jax2D](https://github.com/MichaelTMatthews/Jax2D) is a compact functional,
  Box2D-inspired 2D engine and a useful secondary scenario/oracle reference.
  Its all-pairs `O(n^2)` model is appropriate for small validation worlds, not
  the scalable product broadphase.
- [Solver2D](https://box2d.org/posts/2024/02/solver2d/) is the model for keeping
  solver experiments isolated from production-engine complexity. Its PGS,
  TGS, soft-constraint, relaxation, and XPBD comparisons should inform the
  solver laboratory.
- [Small Steps in Physics Simulation](https://mmacklin.com/smallsteps.pdf)
  motivates the substep experiments and later interleaving of material
  constraints, but does not require rigid bodies themselves to use XPBD.
- [Mass Splitting for Jitter-Free Parallel Rigid Body Simulation](https://www.richardtonge.com/papers/Tonge-2012-MassSplittingForJitterFreeParallelRigidBodySimulation-preprint.pdf)
  is a required experiment for parallel Jacobi convergence and is referenced
  by the local PhysX GPU solver source.
- [GPU Gems 3: Broad-Phase Collision Detection with CUDA](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-32-broad-phase-collision-detection-cuda)
  is a useful scan/sort/spatial-subdivision reference, including duplicate-pair
  handling. Its CUDA scheduling assumptions are not requirements.
- [Unified Particle Physics for Real-Time Applications](https://matthias-research.github.io/pages/publications/flex.pdf)
  is a later reference for interleaving coupled material constraints. Rigid
  bodies represented only as particle clusters are not the initial route to
  Box2D-like 2D manifold behavior.

NVIDIA Warp projects such as [Newton](https://github.com/newton-physics/newton)
and [MuJoCo Warp](https://github.com/google-deepmind/mujoco_warp) may inform
profiling, event tracing, and large-batch testing. They target NVIDIA robotics
or many parallel worlds rather than one portable WebGPU sandbox and should not
drive the core architecture.

## What PhysBox 4 contributes

The fork should preserve and reuse the infrastructure that already serves one
shared GPU host:

- shared desktop, browser, Android, and headless frame boundary;
- Rust/`wgpu` device and surface ownership;
- fixed-step scheduling and catch-up policy;
- portable WGSL particle integration and PBF/XPBD passes;
- sparse particle grid, prefix scans, compaction, and diagnostics;
- analytic circle, rectangle, and other collider math;
- neutral input, camera, tools, and shared-renderer UI;
- versioned scene records and shared launch parsing;
- direct GPU rendering and offscreen capture;
- engine-owned cross-host validation scenarios and fresh provenance.

The following PhysBox 4 mechanisms should not define the new engine:

- Rapier as the authoritative live dynamic world;
- CPU rigid stepping followed by collider snapshot upload;
- GPU impulse packets read back for later CPU application;
- passivity/damping modes whose purpose is to stabilize that delay loop;
- body behavior that depends on asynchronous diagnostic availability.

Some related ideas remain useful but need reinterpretation. Sleeping bodies may
still select a cheaper or more stable fluid boundary path, for example, but
sleeping must be one state inside the GPU world rather than a CPU observation
used to reconcile two worlds.

## Fork and preservation policy

PhysBox 4 should be preserved as evidence, not rewritten until its history no
longer shows the problem that motivated the new direction.

Before or while creating PhysBox 5:

1. Record the exact PhysBox 4 source revision and dirty state.
2. Freeze the oracle provenance: PhysBox 3 revision and patch digest, Box2D
   revision and build/settings, and PhysX reference revision.
3. Retain the asynchronous-feedback topic, traces, numerical baselines, and
   relevant fresh captures.
4. Tag or otherwise name the final PhysBox 4 snapshot.
5. Create PhysBox 5 as a fork with its own vision and README rather than
   claiming the existing engine has already changed ownership.
6. Keep scene records backend-neutral where reuse is cheap, but do not preserve
   internal compatibility at the expense of the new GPU state model.
7. Remove old coupling code from PhysBox 5 only after the minimal GPU body path
   has its own native, browser, Android, and headless validation.

At the time this proposal was revised, the PhysBox 3 reference checkout was
commit `4169eba477201fe8220b49d95473555c4ac22995` with local modifications to
`worker/index.js` and `worker/wrangler.toml`. Confirm whether those changes are
relevant, then record or remove them deliberately; a commit hash alone is not
complete provenance for a dirty oracle checkout.

PhysBox 4 remains useful as an A/B reference. The new engine should first
reproduce its rigid-body product behaviors through the Box2D oracle, then
reproduce selected fluid/body scenes through the PhysBox 3/LiquidFun oracle
and PhysBox 4 regression evidence. PhysBox 4 is not intended to remain a second
product implementation indefinitely.

## Delivery stages

### P5.0: Preserve and fork

- Freeze and identify the PhysBox 4 evidence snapshot.
- Freeze PhysBox 3, Box2D, and PhysX reference provenance.
- Create the PhysBox 5 repository and update project identity.
- Carry forward the shared-host architecture, portable GPU infrastructure,
  and validation rules.
- Record which PhysBox 4 features are intentionally absent from the new
  minimal product.

### P5.0a: Oracle foundation and solver laboratory

- Define the versioned shared rigid scenario, precomputed-manifold, trace,
  metric, and failure-result schemas in Rust.
- Add the unmodified pinned Box2D authority lane, direct public narrowphase
  comparison, and optional patch-digested microscope build.
- Implement the scalar executable specification and pass-level GPU comparison.
- Compare portable contact-centric Jacobi/gather, graph-colored Soft-TGS, and
  optional rigid XPBD or split alternatives on the same precomputed contacts.
- Select the solver foundation from numerical behavior, cross-adapter behavior,
  GPU cost, memory, and one side-by-side review.

No product collision pipeline is built until this stage chooses a solver
foundation and records its accepted parameters and baseline results.

### P5.1: GPU body state and direct rendering

- Add one GPU-resident dynamic oriented rectangle with position, angle,
  velocity, inverse mass, and inverse inertia.
- Render it directly from GPU state.
- Support gravity, reset, and grab without per-step body readback.
- Exercise the selected solver against a precomputed or analytic fixed-floor
  manifold without yet treating it as the general narrowphase.
- Prove identical frame ownership on native, browser, Android, and headless.

### P5.2: Narrowphase and persistent contacts

- Add GPU-resident circles.
- Use all-pairs AABB candidates while body counts remain deliberately small.
- Implement circle/circle, circle/rectangle, and rectangle/rectangle manifolds
  with stable feature identifiers and at most two points.
- Compare every narrowphase pair against Box2D's public collision functions.
- Add sorted contact persistence, warm-start transfer, friction, restitution,
  speculative separation, and explicit penetration/capacity diagnostics.
- Render synchronized side-by-side oracle and GPU results for inspection.

### P5.3: Scalable broadphase

- Retain brute force as the pair-completeness oracle.
- Bake off sorted 1D sweep-and-prune and multi-cell grid candidates on the same
  correctness and performance corpus.
- Add LBVH only if measurements justify it.
- Select one shared portable algorithm and prove zero false negatives, stable
  canonical pairs, bounded memory, and explicit overflow behavior.

### P5.4: Rigid behavior parity

- Add sleeping, waking, and compact asynchronous diagnostics.
- Validate flat and edge landings, stacks, pyramids, impacts, sliding, rolling,
  bounce, off-center rotation, wake propagation, and grab/release against the
  Box2D oracle.
- Run fixed-step, substep, iteration, mass-ratio, material, capacity, and
  adapter sweeps without changing product behavior per host.
- Require numerical acceptance and synchronized interactive review before
  liquid work is authorized.

### P5.5: First construction loop

- Restore placement, erase, select, and grab for circles and rectangles.
- Save and reload the minimal authored scene.
- Add one joint only after free-body contact is stable.
- Decide whether the solver foundation is sufficient for chains and ragdolls
  before restoring broader tools.

### P5.6: LiquidFun-oracled fluid coupling

- Restore a bounded water-like particle solver after rigid parity is accepted.
- Define fluid/body oracle fixtures from LiquidFun and PhysBox 3 behavior.
- Couple particle/body contact inside the GPU simulation step.
- Apply equal-and-opposite translation and off-center rotational response.
- Include fluid boundary-density and iterative pressure effects rather than
  final projection alone.
- Demonstrate displacement, submerge, float or settle, wake, and dam-break
  response.
- Prove that no compact fluid-impulse readback exists in the simulation
  dependency graph.

## Oracle-driven development and failure artifacts

Cross-host validation scenarios have one source of truth in shared Rust. A
scenario owns authored bodies, stable handles, initial state, fixed-step and
solver settings, neutral actions, completion criteria, structural assertions,
metric bands, and a versioned result. Desktop, web, Android, and headless select
and transport it through the shared launch contract; Playwright, JavaScript,
shell, ADB, or an app crate must not restate its physics sequence or expected
numbers.

The Box2D and PhysBox 3 oracle adapters consume a versioned neutral projection
generated by that shared runner. They do not maintain a second hand-authored
scenario format. The projection records the source scenario hash and rejects
unsupported actions or shapes rather than approximating them silently.

Every validation run should produce:

- a unique run identifier and scenario hash;
- source revisions, dirty-state or patch digests, host, adapter, GPU, driver,
  browser where relevant, selected feature tier, and all solver settings;
- the complete shared scene/solver contract;
- canonical oracle and GPU results plus the structured diff;
- compact per-pass counters, timing, capacity high-water, and overflow state;
- a deterministic failure reason and minimized reproducer when available;
- fresh synchronized trace/playback or capture artifacts for visual fixtures;
- an explicit success sentinel only after all required artifacts exist.

Normal interactive frames may asynchronously copy compact metrics and bounded
trace summaries. They must not map body/contact state or wait for the GPU.
Bounded headless and diagnostic runs may explicitly flush complete state after
their final simulated frame. Replay consumes captured inputs and settings; a
pre-existing screenshot or trace is never proof of the current run.

The autonomous loop is: generate or select scenario, run the scalar/pass-level
checks, run Box2D and GPU worlds, evaluate invariants and metric bands, run
cross-host and performance lanes as required, minimize failures, and add the
reproducer to the corpus. Automation may fix an implementation to restore an
accepted baseline. It may not change solver family, physical defaults,
tolerances, oracle revisions, or checked numerical baselines without an
explicit reviewed decision and measured before/after report.

## Performance and capacity gates

Correctness and scale are separate axes. Benchmark at least:

- micro scenes with 1 to 32 bodies for latency and pass overhead;
- product scenes near the intended default and maximum sandbox body counts;
- scale scenes at approximately 1K, 4K, and 16K active bodies where supported;
- dense stacks, pyramids, high-pair-count grids, sparse motion, high mass ratio,
  and sleeping/waking churn;
- solver-only runs with precomputed contacts, collision-only runs, and complete
  end-to-end steps.

After pipeline warmup, record P50/P95/P99 GPU step and per-pass time where
timestamp queries are available, CPU encoding/submission time, dispatch count,
active bodies, broadphase pairs, candidate amplification, manifolds and points,
solver work, iterations/substeps, memory high-water, and every capacity counter.
Where GPU timestamps are unavailable, retain end-to-end frame/step timing and
mark the missing capability rather than fabricating pass timing.

Performance gates are first relative to a named revision on named adapters and
then absolute once representative hardware has established a product budget.
Do not invent one universal millisecond threshold before measurements exist.
Track native, browser, and Android separately but never obtain a faster lane by
changing solver quality, iteration count, material behavior, capacity, or scene.

A performance result is invalid if correctness failed, a pair/contact was
dropped, capacity overflowed, a required GPU-to-CPU synchronization occurred,
the benchmark did not reach its declared steady-state window, or provenance is
incomplete. Optimize the measured pass rather than refreshing a baseline after
an unexplained regression.

## Acceptance criteria for the direction

### Rigid-foundation acceptance

Fluid work does not begin until all of the following are true:

- No GPU-to-CPU result is required to advance rigid simulation.
- Desktop, browser, Android, and headless run the same body state, collision
  stages, solver, scene, and neutral action sequence.
- The selected solver passed the precomputed-contact laboratory against its
  scalar executable specification and recorded selection matrix.
- Broadphase has no false negatives against brute force across the accepted
  randomized/adversarial corpus.
- Circle and rectangle manifolds pass the direct Box2D narrowphase fixtures,
  including stable feature persistence near reference-face transitions.
- Circles and rectangles fall, collide, rotate, slide, roll, bounce, stack,
  sleep, wake, and respond to grabs without a CPU physics backend.
- Flat, corner, and edge landings have bounded penetration and settle without
  persistent artificial jitter or energy growth.
- Small stacks and pyramids remain coherent for their complete fixture window.
- Box2D oracle projection is generated from one shared scenario definition and
  is never used as a product or platform fallback.
- GPU trajectories and aggregate metrics remain inside justified
  scenario-specific bands relative to Box2D.
- Physical/metamorphic invariants pass independently of Box2D.
- No pair, manifold, contact, constraint, command, or diagnostic required for
  behavior is silently dropped; all accepted runs report zero overflow.
- Side-by-side playback is subjectively convincing for every accepted fixture.
- Optional native acceleration changes performance, not product behavior.

Rigid validation should record body position/angle, linear/angular velocity,
penetration, manifold/contact counts, sleep transitions, energy summaries,
overflow counters, candidate amplification, solver work, memory high-water,
and per-pass GPU time where supported. Exact full-trajectory equality with
Box2D is neither required nor expected; exact or tightly bounded pass-level
comparison with the PhysBox scalar specification is required where declared.

### Later coupled-fluid acceptance

The complete direction succeeds only when these additional conditions become
true:

- No GPU-to-CPU result is required to advance rigid or fluid simulation.
- A particle/body contact applies equal-and-opposite response in the same GPU
  fixed step.
- Desktop, browser, Android, and headless run the same scene, commands, solver
  stages, and material behavior.
- A default-density rectangle does not enter the PhysBox 4 delayed-feedback
  trampoline cycle.
- A light or floating body remains responsive without being pinned or having
  legitimate support removed by a passivity filter.
- Off-center fluid load creates prompt, correctly directed body rotation.
- Sleeping and waking do not require a special cross-world stationary state.
- The minimal sandbox feels at least as coherent as the comparable PhysBox 3
  interaction, even before matching PhysBox 4 particle counts.
- Browser WebGPU performance is sufficient for a useful default water scene.
- Optional native acceleration changes performance, not product behavior.

Coupled-fluid validation adds contact impulse or constraint work, fluid RMS
motion, particle population, density error, and fluid/rigid energy exchange.
Visual and interactive review remains required because the reason for the fork
includes feel that aggregate metrics did not adequately capture.

## Explicit non-goals for the first engine

- A general-purpose replacement for Box2D, Rapier, or PhysX.
- API or binary compatibility with any of those engines.
- A CUDA translation of PhysX GPU.
- 3D or 2.5D simulation.
- Arbitrary concave dynamic bodies.
- General articulations, vehicles, character controllers, or user-defined
  constraint shaders.
- Exact deterministic equality across GPU vendors.
- Native-only required physics.
- Liquid or particle coupling in the first rigid milestone.
- Immediate restoration of every PhysBox 4 tool or authored scene.
- Maximizing particle count before coupled behavior is convincing.

## Initial decisions and remaining experiments

The first rigid work begins with these defaults:

- Soft-Step/TGS-like velocity impulses are the leading rigid behavior
  hypothesis; rigid XPBD and split alternatives are laboratory comparisons.
- Contact-centric Jacobi plus per-body gather is the leading portable write
  strategy; GPU-colored TGS is the principal convergence competitor.
- All-pairs is the initial trusted broadphase and pair oracle; sorted SAP and a
  multi-cell grid are the first scalable bakeoff.
- Persistent contacts use canonical body handles with generations plus stable
  geometric feature identifiers, transferred by sorted merge.
- Box2D commit `3a4f0da8374af61293a03021c9a0b3ebcfe67948`
  is the initial rigid oracle source; exact build and world settings are frozen
  by P5.0a.
- Rapier or another CPU rigid backend is never a product fallback. Existing
  PhysBox 4 code may remain temporarily as isolated fork history until the GPU
  body path passes all hosts, then it is removed. Box2D remains test-only.

The solver laboratory and early pipeline must still answer:

- Does contact-centric Jacobi need mass splitting, additional iterations, or a
  hybrid colored treatment for tall stacks and extreme mass ratios?
- Can graph coloring be bounded and driven entirely on the GPU without too many
  global-barrier dispatches for portable WebGPU?
- At what measured body/contact distributions does SAP, grid, or later LBVH win
  on browser, desktop, and Android adapters?
- Which scenario-specific trajectory, manifold, energy, penetration, and sleep
  bands distinguish healthy parallel ordering variation from a materially
  different or unstable result?
- What fixed capacities and growth/restart policy cover the product defaults
  without silent drops or routine over-allocation?
- Which named adapter set and product scene establish the first absolute
  performance budget after relative regression tracking is operational?

Fluid formulation, particle/body reduction, coupled-iteration count, and the
exact PhysBox 3/LiquidFun oracle fixtures are later decisions. They should not
shape the first rigid solver prematurely beyond preserving current-state,
equal-and-opposite contribution and accumulation contracts.

Answer the remaining questions with minimal GPU prototypes, shared scenarios,
and recorded selection matrices, not by expanding the PhysBox 4 delayed
interface.
