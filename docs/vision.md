# Vision: one GPU-resident 2D physics world

Status: active direction, 2026-07-15.

This is the compact entry point. The authoritative, research-backed direction
is [PhysBox 5: one GPU-resident 2D physics world](topics/physbox5-unified-gpu-physics.md),
which owns the detailed solver, contact, broadphase, oracle, and acceptance
contracts. The sibling [performance and scale contract](topics/performance-and-scale.md)
owns benchmark workloads, CPU/GPU comparison, timing validity, capacity, and
scale claims. The [simulation time and overload contract](topics/simulation-time-and-overload.md)
owns fixed-step pacing and the default slow-motion response to missed budgets.

PhysBox 5 is a small, purpose-built 2D physics world whose live mechanical
state stays on the GPU. It should eventually feel like a deliberately
simplified Box2D with a native liquid material—not a 3D engine constrained to
a plane, and not separate CPU-rigid and GPU-fluid simulators connected by
delayed messages.

The required implementation is one Rust, `wgpu`, and WGSL engine on desktop,
browser WebGPU, Android, and headless validation. Optional native capabilities
may accelerate a shared algorithm; the portable browser path defines the
behavioral contract.

## Why a new engine

PhysBox 3 is the behavioral reference: its Box2D/LiquidFun rigid bodies and
particles participated in one synchronous mechanical world. PhysBox 4 tested
a split Rapier/GPU-fluid design. That work made delayed feedback measurable and
bounded, but reactions still reached a later rigid state and required
stabilizers. The architectural lesson is that interactive bodies cannot remain
authoritative in a different simulator if natural two-way material coupling is
the product.

PhysBox 4 remains the evidence archive. PhysBox 5 has fresh history and admits
only code that serves this direction.

## First product slice

Liquid is explicitly deferred. The rigid foundation eventually contains:

- fixed and dynamic circles and oriented rectangles;
- gravity, placement, erase, select, and grab;
- circle/circle, circle/rectangle, and rectangle/rectangle contacts;
- stable manifolds, warm starting, friction, restitution, stacking, sleeping,
  and waking;
- direct rendering from GPU-resident body state;
- side-by-side validation against a test-only Box2D oracle.

Joints, capsules, general polygons, chains, ragdolls, machines, soft bodies,
terrain, CCD, fluid, and the PhysBox 4 tool catalog wait until this minimal
world is trustworthy.

## State and frame contract

```text
CPU authored commands and neutral input facts
    -> bounded GPU command records
    -> one shared fixed-step compute graph
       -> predict bodies
       -> generate canonical candidate pairs
       -> generate and persist contact manifolds
       -> solve equal-and-opposite constraints
       -> integrate final poses and sleep/wake state
    -> render directly from GPU buffers
```

The CPU owns authored records, UI policy, lifecycle, persistence, and compact
delayed diagnostics. No GPU result may be required to advance the next
interactive step. Bounded validation may flush complete state only after its
last step.

Interactive pacing admits zero or one outer 60 Hz fixed step per engine frame.
If wall time advances faster than the engine can present frames, excess time is
slipped permanently and the entire world runs in slow motion; later frames do
not batch steps to catch up. Exact-count headless validation remains independent
of wall time.

The initial body contract is a stable generational handle, shape and
dimensions, position and angle, linear and angular velocity, inverse mass and
scalar inverse inertia, material/collision flags, and sleep state. True 2D
means no quaternions, 3D inertia tensors, gyroscopic terms, or tangent planes.

## Selected solver before collision complexity

The first real physics milestone is an engine-owned solver laboratory fed by
authored manifolds plus a scoped analytic fixture contact generator for
multi-step fixtures. A scalar Rust executable specification and GPU variants
consume the same contact records:

1. contact-centric Jacobi with per-body gather (selected portable foundation);
2. graph-colored/disjoint-batch Soft-TGS (retained convergence comparator);
3. optionally rigid XPBD or a split position/velocity formulation only after a
   new measured ambiguity.

Fixtures cover one contact, symmetric and off-center impact, chains, stacks,
pyramids, thin-edge landings, friction, restitution, mass ratios, and
step/substep sweeps. Selection measures penetration, stack error, jitter,
energy creation, friction/restitution behavior, convergence, repeatability,
GPU time, dispatches, scratch memory, and future particle/body gather fit. The
solver family was a human decision gate; automation may not silently switch it.

S1.6 review selected contact-centric Jacobi with equal-and-opposite records and
GPU body gather. The starting checked policy is fixed `dt` 1/60, four substeps,
one iteration, bias rate 0.2, mass coefficient 1.0, impulse coefficient 0.0,
and restitution threshold 1.0. Colored Soft-TGS remains a convergence
comparator; the optional XPBD/split comparator was not triggered. The
[review package](../evidence/p5.0a-s1.5/README.md) records the measurements,
rejected alternative, and reconsideration triggers.

The selected initial behavior uses Box2D Soft-Step-like velocity impulses:
locally anchored contacts retained across substeps, accumulated projected
impulses, a biased solve, and unbiased relaxation.

## Collision contracts

Broadphase, narrowphase, persistence, and solve are independently testable:

1. Broadphase emits canonical `(min_handle, max_handle)` pairs.
2. Narrowphase emits a canonical normal and at most two manifold points.
3. Points contain stable feature identifiers, local anchors, separation, and
   material data.
4. Persistence keys include generational handles and features.
5. Current/previous keys are radix-sorted and merged for warm-start transfer.
6. Capacity and overflow are explicit; no pair or contact is silently lost.

Circle contacts are analytic. Rectangle/rectangle uses SAT, reference and
incident edges, segment clipping, and a documented bias against reference-face
flip-flop. Restitution is separate from penetration stabilization; friction
clamps accumulated tangent impulse against accumulated normal impulse.

Broadphase development starts with all-pairs AABB as the small-scene oracle,
then compares sorted 1D sweep-and-prune with a multi-cell grid. LBVH is added
only if measured object-size variation or sparse-world behavior justifies it.
Every scalable candidate is checked for zero false negatives against brute
force and reports amplification, duplication, high-water, and overflow.

## Oracle stack

The oracles form layers, not a product fallback:

| Layer | Authority | Required comparison |
|---|---|---|
| Invariants | PhysBox specification | finite state, equal/opposite response, symmetry, monotonic projected impulses, no overflow |
| Pass-level | scalar Rust specification | exact or tightly bounded prepared constraints and one-iteration deltas |
| Narrowphase | public Box2D collision functions | manifold normal, points, separation, and stable features |
| Rigid behavior | native pinned unmodified Box2D v3 | trajectories, settling, energy, sleep/wake, and aggregate bands |
| Later coupled behavior | native PhysBox 3 sidecar plus its Box2D v3 | particle/body exchange, buoyancy, torque, wake, and aggregate bands |
| Historical fluid reference | native original LiquidFun | algorithms, fixtures, and secondary behavior comparison |
| Product calibration | fresh PhysBox 3 web traces and synchronized playback | settings, interaction sequence, and subjective feel |
| Performance | [performance and scale contract](topics/performance-and-scale.md) | correctness-qualified CPU/GPU curves, capacity, crossover, and end-to-end timing |

One shared Rust scenario owns bodies, actions, completion, metrics, and result
schema. Routine oracle adapters are separate native processes that consume its
versioned neutral projection and emit the canonical result schema. JavaScript,
Playwright, shell, ADB, and platform crates do not recreate physics scenarios.
PhysBox 3 browser automation is an infrequent calibration lane, not a routine
correctness dependency.

Use an unmodified pinned Box2D build as the behavioral authority. A separate
instrumented “microscope” build may expose contact and solver traces, but its
patch digest and output are diagnostic evidence, never the acceptance oracle.
Random/adversarial generators should minimize failures into permanent corpus
fixtures.

Scale evidence begins in the solver laboratory rather than after rigid parity.
The initial shorthand is that 10K bodies prove useful scale, 30K should begin
to demonstrate an advantage on realistic contact-rich scenes, and 100K is a
qualified frontier whose meaning depends on awake bodies, candidate pairs,
contacts, solver work, and shape mix. Phones are a first-class target with a
smaller early-measured envelope owned by the performance contract.

Oracle precedence is explicit rather than a majority vote. Invariants and the
scalar specification own mathematical/pass correctness; Box2D owns the rigid
behavior target; the native PhysBox 3 sidecar later owns the coupled-behavior
target. Original LiquidFun and any future Rapier2D lane are secondary evidence
that may reveal overfitting or an engine-sensitive scenario but do not silently
change acceptance.

## References and boundaries

The reproducible checkout pins and the workspace-local paths into the PhysBox
3 behavior oracle are maintained in the
[local reference map](../reference/README.md). Run
`npm run references:status` before producing oracle evidence.

- Box2D commit `3a4f0da8374af61293a03021c9a0b3ebcfe67948` is the first
  rigid authority. Inspect its public collision functions and solver stages;
  do not import its dynamic tree merely because it is the oracle.
- PhysX 5.6.1 commit `b4b286abff6f2b3debd1d1acb120dc428765cf2e`
  is an organization reference for resident state, SoA layouts, compaction,
  batching, fixed-capacity buffers, and overflow policy. Do not import its 3D,
  CUDA, warp, float-atomic, or CPU-fallback assumptions. See the
  [PhysX GPU rigid-body guide](https://nvidia-omniverse.github.io/PhysX/physx/5.6.1/docs/GPURigidBodies.html).
- `wgmath` commit `ac89dceb8a33778e895303cd606da38afb015192` is the
  closest WebGPU research reference for LBVH, graph coloring, Soft-TGS, and
  Jacobi. Study it, but do not accept GPU-count readbacks or host-driven
  convergence as a frame dependency.
- [Solver2D](https://box2d.org/posts/2024/02/solver2d/) motivates the isolated
  solver laboratory; [Small Steps](https://mmacklin.com/smallsteps.pdf),
  [Bullet3 OpenCL](https://github.com/bulletphysics/bullet3/tree/master/src/Bullet3OpenCL/RigidBody),
  [Jax2D](https://github.com/MichaelTMatthews/Jax2D), and
  [GPU Gems broadphase](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-32-broad-phase-collision-detection-cuda)
  are secondary algorithm references.
- PhysBox 3 commit `4169eba477201fe8220b49d95473555c4ac22995`, its
  LiquidFun checkout at `7f20402173fd143a3988c921bc384459c6a858f2`, and its
  actual nested Box2D/WASM source become the later fluid-behavior oracle.
- Rapier2D is a real compile-time 2D specialization despite sharing source with
  Rapier3D. It remains an optional, non-gating dissent comparator after the
  Box2D lane is established, not an initial dependency or implementation
  authority. See the [Rapier repository architecture](https://github.com/dimforge/rapier/blob/master/ARCHITECTURE.md).

## Acceptance before liquid

Liquid work requires all of the following: one GPU-owned rigid state on every
host; the selected solver passes the scalar laboratory; broadphase has no false
negatives; manifolds pass direct Box2D comparison; circles/rectangles collide,
slide, roll, stack, bounce, sleep, wake, and grab convincingly; numerical bands
and invariants pass; all accepted runs have zero overflow; and optional native
acceleration changes performance rather than behavior.

Exact full-trajectory equality across GPU vendors is not required. Explicit
invariants, narrow pass-level tolerances, durable behavioral bands, and fresh
side-by-side playback are required.
