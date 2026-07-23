# PhysBox 5: One GPU-Resident 2D Physics World

Status: active direction, updated 2026-07-23. The lean bootstrap and P5.0a solver
laboratory are complete at `~/code/physbox5`; the fresh-history root is commit
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

### Product physics decision, 2026-07-21

Direct interactive review selects the soft-coupled, point-sequential GPU
family as the continuing product direction. Its quiet Box2D-derived response
is preferred to the visibly active hard/direct manifold-block solver even
though the current 60 Hz/four-substep seed permits more penetration and loses
the governed 16-stack. Those limitations remain measured and visible; they are
accepted as a foundation to improve rather than used to keep the subjectively
worse solver by default.

[Physics configuration and solver profiles](physics-configuration-and-solver-profiles.md)
owns the detailed decision. The initial seed is 60 Hz outer fixed rate, four
substeps, one biased point-sequential pass plus relaxation, and soft contact
30 Hz/damping 10/static multiplier 2. Hard/direct remains a named comparison,
not an automatic fallback. The [S19 tactical](../tactical/019-soft-default-and-physics-configuration.md)
has made this path the ordinary code default and turned fixed rate, substeps,
iterations, contact response, and recovery policy into one bounded shared
configuration. Its cross-host qualification and exact live deployment are
complete.

Historical S6-S18 dispositions remain accurate for their frozen
questions; they do not override the later human product decision. Invariants,
capacity safety, exact replay within a configuration, portable WebGPU, one
authoritative GPU-resident product state, and honest cost/response reporting
remain mandatory. The default-enabled, compile-removable
[interactive Box2D comparison](interactive-box2d-comparison.md) may maintain
one non-authoritative shadow for human review. It cannot feed mechanics,
become fallback, satisfy gates, or enter PhysBox performance evidence.

The [rigid quality target contract](rigid-quality-targets.md) now makes the
awake stack hierarchy explicit for new work: 2/4/8 are the regression
foundation, 12 is blocking, 14 is stretch, and 16 is the non-blocking boundary
diagnostic for the exact matched Box2D fixture. This supersedes the historical
use of 16 as an automatic promotion veto without rewriting those experiments,
weakening their findings, or allowing sleeping to satisfy an awake gate.

### Liquid-entry decision, 2026-07-23

Human review accepts the current measured rigid foundation as sufficient to
begin bounded liquid, UI, and interaction work. This is a product-priority
decision, not a retroactive pass of every P5.4 rigid-parity or performance
gate. The selected 60/4/i1 default retains its eight-box awake quality claim,
12/14 quietness failures, demonstrated 14-box structural survival, and
16-stack loss. Comprehensive rigid parity and correctness-qualified CPU/GPU
crossover remain open debt.

S23 and S30 make sleeping an implemented, reversible capability. The later
S32 human decision promotes sleeping on with exact
current-contact-component grouping as the ordinary default; local-4, local-8,
and sleeping-off remain explicit review choices. Sleeping or liquid may not
relabel an awake rigid failure.

The [coupled liquid/rigid constraint surface](coupled-liquid-rigid-constraint-surface.md)
owns the entry architecture, reuse limits, rigid-only Box2D boundary, and
acceptance contract. The approved
[S31 coupled-liquid sandbox](../tactical/031-coupled-liquid-sandbox.md) has
implemented the bounded GPU water substrate, same-step coupled response,
fluid wake roots, Water/Surface/Erase interactions, compact 32K-capacity
neighborhoods, direct rendering, native oracle corpus, and Browser/authorized
hardware-backed AVD qualification. It is now at the human behavior-band,
subjective-feel, and product/default checkpoint. This implementation state does
not change the retained rigid limitations above.

The first simulation slice is a rigid-only foundation:

- fixed and dynamic circles;
- fixed and dynamic oriented rectangles;
- gravity, direct placement, erase, and grab;
- circle/circle, circle/rectangle, and rectangle/rectangle contact;
- friction, restitution, stacking, sleeping, and waking;
- direct rendering from GPU-resident body state;
- side-by-side validation against a test-only Box2D oracle.

That rigid-only entry period is complete under the explicit liquid-entry
decision above. Particle coupling now runs in the one authoritative PhysBox
GPU world through the bounded S31 contract. The PhysBox 3 sidecar compiled
natively is its primary behavioral oracle, original LiquidFun is secondary
evidence, and the web-only PhysBox 3 product is a calibration lane. Measured
sidecar disagreements remain human review inputs rather than silently accepted
bands. Body joints, capsules, convex polygons, chains, ragdolls, machines,
soft bodies, terrain, CCD, multi-material fluid, and the complete PhysBox 4
tool set remain later breadth.

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
require scene-specific damping. Correctness does not make scale optional: the
sibling [performance and scale contract](performance-and-scale.md) requires
qualified CPU/GPU crossover and stress evidence from the solver laboratory
onward. The [simulation-time contract](simulation-time-and-overload.md) keeps
the outer delta fixed and makes coherent slow motion, rather than multi-step
catch-up, the default response to overload.

The initial rigid experience should remain playable throughout development:

- place circles and rectangles;
- grab, drop, stack, and throw bodies;
- build a small stack, container, or ramp;
- observe falling, sliding, rolling, edge impacts, friction, restitution, and
  sleep/wake transitions;
- reset and replay the same scene on desktop and in the browser.

Circle and rectangle placement use one shared touch-first gesture policy. A
quick tap commits once on release. Holding for 0.25 seconds enters placement
paint mode, attempts another primitive every 0.10 seconds, and follows the
current pointer target until release or cancellation. Hold time advances only
during active interactive wall time, independent of the selected fixed-step
rate, but a repeat emits only when an outer fixed step is admitted. Pause and
overload therefore cannot accumulate a burst of deferred placements. The
release after paint mode does not add another primitive, and erase remains a
deliberate release-committed action. This also preserves the two-contact
transition to camera navigation without committing a short cancelled
construction gesture.

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

The shared interactive clock advances that graph by zero or one outer fixed
step per engine frame. When wall time outruns completed frames, excess
whole-step time slips permanently and all world behavior slows together; it is
not retained for a later catch-up batch. Exact-count validation and throughput
benchmarks request explicit steps independently of wall pacing. The full
contract is [simulation time and overload](simulation-time-and-overload.md).

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

P5.1 S2.1 implementation status, 2026-07-15: the reviewable initial allocation
uses four 16-byte storage-buffer lanes per body slot: pose, motion,
rectangle/mass properties, and generational metadata. The current total is 64
bytes per slot. Metadata retains the live/fixed/sleep/rectangle flags and the
exact material bits needed by the later contact path. Reset and grab transport
uses a separate bounded 48-byte command record with a target slot and
generation plus monotonic sequence. The allocation checks portable device
limits before buffer creation, and stale generations or capacity overflow are
diagnosed rather than ignored. This is a provisional P5.1 layout to measure at
1K, 10K, and 100K; it is not frozen until the S2.6 human checkpoint.

P5.1 S2.2 implementation status, 2026-07-15: the shared host now has a
`body-free-flight` product-state lane that integrates active rectangles with
gravity in portable WGSL and renders one six-vertex instance per live body
directly from the GPU pose, property, and metadata lanes. The fixed floor is a
fixed body in the same allocation and draw. A bounded exact-step capture
matches the independent scalar semi-implicit `f32` update within `1e-5`, while
zero-step rendering leaves mechanics unchanged. The lane passed on native
Metal, browser WebGPU, the `jstorrent-tablet` AVD's Vulkan SwiftShader, and a
physical Pixel 9 Mali-G715/Vulkan adapter. That dedicated free-flight lane
intentionally retains no collision response; S2.3 supplies the precomputed
fixed-floor manifold and accepted Jacobi solve for the default bootstrap.

P5.1 S2.3 implementation status, 2026-07-15: the default bootstrap and the
explicit `rigid-floor` lane now use the generational body allocation and the
accepted contact-centric Jacobi schedule. Each outer step builds the
two-point incidence topology on the GPU with count, prefix, and scatter,
then runs four substeps with equal-and-opposite records, deterministic body
gather, ping-pong motion, bias, relaxation, friction, and restitution. The
checked schedule is 68 dispatches per outer step and 552 bytes of bounded
solver scratch for the two-body/two-point scene; the scene contract reports
both values and overflow state. The authored floor manifold supplies only
fixed local anchors. Live separation and effective mass are recomputed from
GPU pose, while general contact generation and outer-step warm-start transfer
remain deferred. The inherited bootstrap mechanics buffer and compute path
have been removed; the old visualization shader is retained only for the S1
candidate laboratory.

S2.3 cross-host evidence is complete in the
[fixed-floor evidence package](../../evidence/p5.1-s2.3/README.md). The exact
product playback settles by step 60 and remains bounded through step 180.
At step 120 its vertical position differs from clean pinned Box2D by
`0.00042979`, with approximately `6.05e-7` residual vertical speed and
`0.000547588` maximum penetration. Native Metal, browser WebGPU, the
`jstorrent-tablet` SwiftShader/Vulkan AVD, and a physical Pixel 7a
Mali-G710/Vulkan device all report the same structural scene contract and no
overflow. These are objective observations rather than a new tolerance or
baseline.

P5.1 S2.4 implementation checkpoint, 2026-07-15: reset and neutral grab now
enter the shared GPU world as bounded, ordered body commands at fixed-step
boundaries. Zero-step interactive frames retain pending input without
advancing mechanics, pointer targets coalesce before admission, and explicit
multi-step validation interleaves the engine-owned scheduled command sequence
at exact authored steps. The GPU applies commands before solving and holds a
grabbed body after each step without body-state readback. Reset restores the
authored initial pose and motion; release clears held state and prevents stale
pointer motion from changing the body. Command overflow is persistent scene
diagnostic data and rejects the complete pending batch. The provisional
48-byte record and capacity of 64 remain review inputs, not a frozen ABI.

The shared `reset-grab` source contains the scenario, six scheduled commands,
and canonical hash. Platform adapters transport only its launch name and raw
pointer facts. Engine GPU tests prove exact targets, release, bit-identical
replay after reset, coalescing, and no partial execution on overflow.

S2.4 cross-host evidence is complete in the
[reset/grab evidence package](../../evidence/p5.1-s2.4/README.md). Clean exact
headless, desktop, browser, `jstorrent-tablet` AVD, and physical Pixel 7a runs
report the same scenario hash, six commands, two resets, released state, and
no overflow. The browser retains one raw-event transport proof. AVD and
physical pacing diagnose 0.15 s and 0.05 s of permanent slip respectively,
without catch-up. This evidence completes action transport and replay only;
it does not freeze the provisional body or command representation.

P5.2a S4.3 implementation checkpoint, 2026-07-15: live pointer interaction now
uploads versioned body-free begin/target/end intents rather than a CPU-selected
body. A begin deterministically scans current analytic circle/rectangle state
on the GPU, rejects fixed/no-hit candidates, and retains the generational body
identity plus touched body-local anchor in a 64-byte device record. Each held
outer step applies one bounded compliant point constraint before contact
solving; collision stays enabled, target samples coalesce, release velocity is
bounded, and stale generations clear atomically. No selection or body state is
read back to advance an interactive frame.

The tether is also GPU-derived. During an active pointer, a 12-thread compute
pass reads the retained grab state and current pose and writes a 192-byte vertex
buffer for the existing shared render pass. This portable bridge was required
because the AVD MoltenVK path presented stale data when the compute-written
grab record was consumed directly as vertex-stage storage or vertex input. It
does not cross the CPU, delay mechanics, or create a second state owner. The
serial begin scan and provisional `45/10/80` spring/damping/acceleration values
are explicitly temporary: scalable picking follows broadphase evidence, and
human deployed feel review precedes any final grab-quality decision.

The [S4.3 evidence package](../../evidence/p5.2a-s4.3/README.md) preserves 21
exact GPU replays, 21 process-isolated clean Box2D public motor-joint results,
objective deltas, invariant/selection/generation tests, synchronized playback,
tether captures, and native/web/AVD provenance. It changes neither solver
defaults nor numerical tolerances.

P5.1 S2.5 evidence is complete in the
[body scale package](../../evidence/p5.1-s2.5/README.md). The same exact-step
body world integrates, scans held state, and directly renders 1K, 10K, and
100K deterministic rectangles on native Metal, Chrome WebGPU, the
`jstorrent-tablet` SwiftShader/Vulkan AVD, and a physical Pixel 7a. All 12
clean release rows validate complete final pose/motion/metadata state, retain
64 bytes per body and all capacity/work counters, and report zero overflow.
At 100K, native Metal GPU time is 0.926 ms P50 and Chrome is 1.503 ms P50;
Pixel host completion is 11.239 ms P50 because its advertised timestamp-query
path returns unusable all-zero samples. The AVD remains a software portability
lane.

No material hot/cold comparator is exposed by these curves. An 8-byte
identity/8-byte material split preserves the same total bytes while adding a
binding before general contacts establish their access pattern. The grouped
SoA and 48-byte bounded command record therefore entered S2.6 as freeze
proposals, while the full-capacity hold scan remained an explicit
reconsideration target. The decision below resolves that checkpoint without
freezing the hold architecture.

Human review accepted the limited freeze proposed by the
[S2.6 decision package](../../evidence/p5.1-s2.6/README.md): the stable 64-byte
four-lane `BodyCoreV1`, generational handle, existing 48-byte command record,
and explicit capacity-failure semantics are the P5.2 baseline. Scratch meaning,
numeric capacities, hold-dispatch architecture, future shape flags, and all
P5.2+ buffers remain outside the freeze. `BodyCoreV1` is the dynamic core, not
a universal body-plus-shape-plus-joint allocation: analytic circles and
rectangles use the existing shape payload, while future polygons, compound
shapes, and joints require separate versioned geometry/constraint buffers
rather than silent field reinterpretation. P5.1 is complete and P5.2 is active
under the [S3 tactical](../tactical/003-small-world-collision-persistence.md).

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

Precomputed manifolds cover single-step and few-step fixtures. The most
diagnostic fixtures — stacks settling over hundreds of fixed steps, long
friction slides, and sleep candidates — need contacts regenerated as bodies
move, and anchor-based separation updates only cover substeps within one fixed
step. The laboratory therefore includes a deliberately scoped analytic fixture
contact generator limited to the fixture geometries: circle/floor,
rectangle/floor, and axis-aligned box-on-box resting contact. It is validation
tooling checked against Box2D's public collision functions on those
geometries. It must not grow SAT clipping, general feature persistence, or
broadphase behavior; P5.2 replaces it and never promotes it into the product
narrowphase.

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

The S1.4 laboratory implements both portable experiments without selecting
between them. Its Jacobi lane constructs body incidence on the GPU through
integer-atomic count, prefix, and scatter passes, emits one equal-and-opposite
pair per contact, and gathers per body. Its colored lane assigns a deterministic
GPU greedy color and executes a fixed 32-color schedule; a 33-edge shared-body
fixture produces an explicit failed capacity record. Both lanes retain authored
anchors across Soft-Step substeps, refresh separation from current poses, and
place unbiased relaxation after pose integration. The scalar-compatible
contact-pass mode remains available for first-divergence traces. These are
candidate facts from S1.4; the later S1.6 decision is recorded below.

The completed S1.5 review package is
[`evidence/p5.0a-s1.5`](../../evidence/p5.0a-s1.5/README.md).

#### S1.6 solver selection — accepted 2026-07-15

Human review selected contact-centric Jacobi with equal-and-opposite contact
records and GPU per-body gather as the initial rigid solver foundation. The
portable accumulation path is GPU-built body incidence through count, prefix
scan, and scatter, followed by deterministic body-owned `f32` gather and
ping-pong velocity buffers. This decision authorizes P5.1; it does not claim
full rigid acceptance or authorize liquid work.

The first checked laboratory baseline is fixed `dt` 1/60 second, four solver
substeps, one iteration, bias rate 0.2, mass coefficient 1.0, impulse
coefficient 0.0, and restitution threshold 1.0. These values are a starting
policy, not permission to weaken later scenario-specific quality bands.
Changing the family, accumulation architecture, or these baseline parameters
requires new measured evidence and review.

Graph-colored Soft-Step/TGS is rejected as the initial production foundation
but retained as a convergence comparator and regression lane. It converges
faster per substep, yet the measured four-substep implementation requires 786
dispatches, is materially slower on both named adapters, and has a diagnosed
32-color topology bound. The optional XPBD/split comparator was not triggered:
the two required candidates answered the current accumulation and convergence
question without a material third-family ambiguity. Native float atomics and
CPU physics remain ineligible as required product paths.

Reconsider the selection if physical Android hardware reverses the measured
dispatch/cost relationship; accepted tall-stack, mass-ratio, friction,
restitution, or sleep fixtures fail Jacobi quality bands; gather contention or
adjacency memory scales materially worse on a representative portable adapter;
a GPU-driven colored or segmented path removes the current dispatch and
topology limits; later contact persistence changes the work topology enough to
invalidate the authored-manifold evidence; or a genuine scalar-invariant
versus Box2D behavior conflict is demonstrated.

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

S3.4 checkpoint, 2026-07-15: the first executable persistence graph implements
this contract exactly with a 48-byte persistent entry and a 64-byte current
record, stable four-bit radix passes over all eight key words, and sorted exact
lookup. The bounded scalar/GPU matrix passes replay, churn, permutation,
generation-reuse, and atomic-overflow gates. The resulting 64 radix passes and
212 total candidate/narrowphase/persistence dispatches are deliberately
provisional until the S3.5 end-to-end measurements and S3.6 representation
review; they are not a frozen performance architecture.

S3.5a authorization, 2026-07-15: after the first S3.6 evidence review and a
pinned PhysX organization study, human review authorized a measured comparison
against that exact baseline. Stable pair-slot, pair-local two-point
persistence is the lead cache candidate, and compact stable body-edge index
views plus deterministic segmented gather are the lead adjacency candidate.
The complete semantic key remains acceptance truth and the current full-key
sort/body scan remain correctness baselines. This authorization does not
select the candidates, accept the proposed `0.02` speculative distance or
`3.0 m/s` depenetration limit as defaults, or permit P5.3/liquid work.

Pair-slot implementation checkpoint, 2026-07-15: the lead persistence lane is
now an isolated portable GPU comparator over the same candidate and
narrowphase contracts. Stable pair slots retain an independent generation;
deterministic scans recycle free slots; pair identity and complete local
features remain the authority for warm-start transfer, with a compact
fingerprint used only as a shortlist. The comparator passes replay,
permutation, churn, body-reuse, deliberate fingerprint-collision, and atomic
overflow tests without intermediate mapping. Its 10 persistence dispatches
reduce the combined collision/persistence work shape from 212 to 25, but the
product still uses the full-key correctness baseline until the complete S3.5a
matrix and renewed human review select a representation.

Body-edge implementation checkpoint, 2026-07-15: the accepted Jacobi solver
now has an isolated adjacency comparator that keeps one canonical contribution
payload and builds an alternate compact `{ body, contribution }` index view.
Stable body-key radix grouping and explicit body ranges feed the same
body-owned segmented gather, preserving exact point/side accumulation order
under deliberately reversed build scheduling. The current body-by-point scan
remains the product and correctness baseline. Exact generated behavior,
high-contention, churn, and overflow comparisons pass; the additional radix
dispatches and ping-pong index bytes remain visible for the S3.5a timing and
scale matrix rather than being accepted architecturally here.

The S3.5a evidence command measures these views without introducing a
benchmark-only engine: generated rows use the real contact graph, while 1K
and 10K authored-contact rows feed the accepted solver from the shared neutral
scenario source and skip only the deliberately out-of-scope all-pairs stage.
Representative timestamps and instrumented subphase timestamps are distinct;
both are correctness-qualified by exact result digests, replay, permutation,
capacity, and atomic-failure records. Fresh artifacts always state that no
representation or physical default has been selected.

The clean S3.5a matrix now qualifies both views without promoting either.
Stable pair-slot persistence preserves full equality verification and exact
replay while reducing isolated persistence P50 by 68x to 416x and comparable
allocation by 27% to 47% across 32/256/1K bodies. Stable body edges preserve
exact solver output and reduce complete generated-graph P50 by 1.04x to 1.38x;
on 1K/10K authored contacts they reduce P50 by 1.54x/2.28x while adding about
6% total allocation and approximately doubling topology bytes. The candidate
policy reduces the preserved crossing penetration from `0.005069` to
`0.000508` and six-body coincident maximum speed from `126.27` to `27.41 m/s`.
These residuals, the absence of a combined pair-slot-plus-edge full-step
timing, and the physical-device availability gap remain explicit S3.6 review
inputs. No default, contact ABI, or product representation changed.

Human review accepted the renewed S3.6 recommendation on 2026-07-15: stable
generational pair slots plus stable body-edge index views are the P5.2
production direction, with the complete semantic key and canonical scan
retained as validation oracles. The `0.02` speculative distance and `3.0 m/s`
per-contact depenetration limit are accepted as provisional defaults while
deep coincident placement remains a diagnosed exceptional state. The first
post-selection slice must combine both views in the real graph, preserve exact
output and atomic failure, and publish structural stack curves and combined
timing before S3 closes or any crossover is claimed.

Pair-slot product integration checkpoint, 2026-07-15: the real
generated-contact graph now selects stable generational pair slots while the
full semantic point-key sort remains an explicit correctness oracle. Pair
records retain full pair generations and both complete local features; the
fingerprint cannot authorize transfer. The bridge keeps the existing sparse
two-point manifold indices so solver contribution order is byte-exact under
reversed scheduling, feature churn, and forced fingerprint collision. This
reduces collision/persistence from 212 to 25 dispatches without readback or a
second contact payload. Stable body-edge integration and the combined
performance/structural gate remain pending, so S3 is not closed here.

Stable body-edge product integration checkpoint, 2026-07-15: the real graph
now combines the selected pair-slot cache with deterministic sorted body-edge
ranges. The canonical scan remains available only as an explicit validation
topology. Combined output is exact against both full-key/canonical and
pair-slot/canonical lanes across governed intermediate behavior steps,
permuted construction, pair churn, high contention, reset/grab coverage, and
atomic overflow. The maintained two-body floor path is 97 dispatches and
19,640 bytes, while the 32-body performance fixture is 100 dispatches. These
are accounting results; S3 still requires fresh combined timings, structural
curves, cross-host artifacts, minimized failures, and the optimized Box2D
comparison before closure.

Combined representation completion, 2026-07-15: clean same-revision evidence
qualifies the selected pair-slot plus stable-edge product graph across exact
directed/randomized replay, atomic capacity failure, 32/256/1K generated
timing, 1K/10K authored contacts, structural stacks, 100-body buckets,
optimized Box2D, and native/browser/hardware-backed AVD hosts. The integrated
1K GPU P50 is `3.159 ms`, about 7.4x faster than the former full-key product
but still about 20.7x slower than the fastest equivalent optimized Box2D row.
The representation gate passes without accepting the two minimized physical
failures or the 4/8-body awake stack failures. Generational place/erase work is
therefore unblocked, while final solver/contact quality remains a mandatory
S5.4 human decision and P5.3 remains downstream.

Construction lifecycle checkpoint, 2026-07-15: the shared contract now sends
bounded body-free place-circle, place-rectangle, and erase-at intents to a
separate GPU lifecycle pass. Device-owned occupancy selects the lowest free
slot; retirement increments its generation and clears pose, motion, material,
shape flags, and a matching active grab. Placement publishes explicit full and
deep-overlap failures, while erase publishes miss and generation-exhaustion
failures. No body identity or occupancy result is required on the CPU to
advance the next frame. Interactive worlds reserve the next power-of-two slot
tier above their authored population, currently at least 64, and all downstream
render, selection, collision, persistence, and solver work sees that declared
capacity. The small-world memory and scan cost is intentionally visible and is
qualified only for the bounded S5.3 construction scope described below.

Shared authoring-toolbar checkpoint, 2026-07-16: UI contract version 2 exposes
Grab, Circle, Rectangle, and Erase through the same engine-rendered glyph pass
on every host. Placement and erase commit once on primary/contact release at
the press position; a second touch therefore cancels the pending construction
tool before it emits an intent and continues to own pan/pinch. Right-drag and
wheel remain camera facts. Tool changes explicitly end an active grab, and all
construction intents join the existing bounded, monotonic, first-admitted-step
interaction stream. The UI reports selection and dispatch counts but never
owns occupancy, body handles, or construction outcomes.

Construction qualification, 2026-07-16: clean revision `4d44af0` replays the
complete shared authoring sequence byte-for-byte, proves warm contact graph
state clears across erase and generation-2 reuse, admits 62 additional bodies
into the 64-slot bootstrap world before one explicit atomic capacity failure,
and rejects deep coincidence without changing collision policy. Browser and
the hardware-backed `jstorrent-tablet` AVD each project one raw placement into
the same engine contract and observe the later asynchronous GPU result. The
[evidence package](../../evidence/p5.2b-s5.3/README.md) records the absent
physical-device lane. This closes bounded construction mechanics, not
large-world allocation, arbitrary geometry, physical defaults, or subjective
construction feel.

Awake solver-quality checkpoint, 2026-07-16: the clean
[S5.4 package](../../evidence/p5.2b-s5.4/README.md) compares the selected 4x1
execution with eight reversible substep, iteration, warm-start, relaxation,
bias, and governed contact-policy variants on 2/4/8 rectangle stacks and
2/3/4-row pyramids. Clean pinned Box2D remains awake and effectively
motionless while preserving every structure. All GPU variants lose the
8-body stack; improvements on smaller fixtures are non-monotonic, and a lane
can report negligible residual motion only after toppling. The bounded-policy
lane is byte-identical to the selected execution on these initially touching
sources, so speculation/depenetration does not explain the observed failure.

No physical default, quality band, tolerance, oracle precedence, product
constructor, or solver architecture changes at this checkpoint. Independent
two-point Jacobi coupling/convergence is the leading measured hypothesis, not
a proven root cause. The recommended next bounded laboratory experiment is a
manifold-coupled two-point normal solve that preserves equal-and-opposite
records and the selected stable views where possible; if it cannot, compare
the documented colored/in-place family before changing product architecture.
Human review is required before that experiment or P5.3 broadphase work.

S5.4a authorization, 2026-07-16: human review accepted a sequenced
laboratory-only comparison of the current point Jacobi solve, scalar mass
splitting, a coupled two-point manifold normal block, and block mass splitting.
Scalar splitting uses valid point incidence; block splitting uses distinct
live manifolds/body pairs. Split inverse mass/inertia changes effective-mass
preparation only, while equal-and-opposite impulses are applied through the
existing gather with full physical mass. The factorial matrix at 1/2/4/8
iterations must distinguish chain convergence from intra-manifold rocking.

The generated-contact colored/in-place comparator is conditional: add it only
if no mass-split/block candidate preserves the existing S5.4 corpus with
monotonic iteration behavior inside the fixed 60 Hz feasibility boundary. The
old precomputed-contact laboratory has useful colored-stage machinery, but the
current generated-contact product shader does not; porting it is therefore an
explicit comparator slice rather than a hidden switch. This authorization does
not select a solver, change a constructor/default/tolerance, or unblock P5.3.

S5.4a colored comparator checkpoint, 2026-07-16: the corrected every-step
factorial record triggered the authorized fallback comparison. Execution
schema v3 now names `jacobi-gather` versus `colored-in-place` independently of
the normal formulation. The new laboratory path performs deterministic
GPU-side point coloring over generated contacts and applies each disjoint color
in place to the same body motion buffers; collision generation, stable pair
slots, stable body-edge ranges, contact preparation/cache, and physical
settings remain shared. The 32-color portable bound is an explicit atomic
capacity failure, and every encoded color pass is included in dispatch and
timing evidence. Point Jacobi plus Jacobi/gather remains the product selection.
No quality band, tolerance, oracle precedence, constructor, or physical default
changed; measured comparison and renewed human review are still required.

S5.4a measured decision checkpoint, 2026-07-16: the clean
[factorial evidence](../../evidence/p5.2b-s5.4a/README.md) shows that colored
in-place is the only measured accumulation architecture to preserve all seven
awake structures: i4 and i8 retain the added 16-stack through every one of
1,200 steps. Scalar mass split i8 and manifold block i2/i4/i8 preserve the
original six scenes but lose the 16-stack; combined block mass splitting
regresses. All executions are exact, finite, and capacity-clean, but every
family has a strict residual or first-failure monotonic violation.

The structural result does not authorize promotion. The deliberately complete
32-color schedule costs `19.308 ms`/2,360 dispatches at 1K for colored i4 and
`28.042 ms`/4,408 dispatches for i8, compared with `0.185 ms` for optimized
Box2D. Point Jacobi/Jacobi-gather therefore remains the provisional selected
execution. The recommendation awaiting human review is a bounded optimized
colored comparator that compacts GPU-owned manifold colors, couples the two
normal points, and fuses compatible stages without simulation-dependent CPU
readback or a second world. A resulting potential candidate must pass the same
seven-scene, residual, capacity, cost, browser, and AVD gates before selection.

S5.4b authorization, 2026-07-16: the colored follow-up must keep scheduling
attribution separate from solver math. First make the existing point-colored
execution compact/fused and byte-identical; then compare manifold-colored
active-set blocks and deterministic alternating color order at i1/i2/i4.
Extended 32/64 stacks, buckets, and churn diagnose depth and scheduling scale
without silently creating a new accepted quality band. Incremental coloring
and workgroup-per-island solving remain measured conditional comparators.
Point Jacobi/Jacobi-gather remains the product selection.

S5.4b scheduling checkpoint, 2026-07-16: a laboratory-only V4 execution
contract now distinguishes fixed-full-grid and compact-indirect point-color
schedules. GPU stable compaction and indirect dispatch preserve the existing
greedy assignment, forward order, scalar equations, and exact stage sequence;
the native eight-step i2 stack is byte-identical to the fixed-grid control and
the overflow path schedules zero solve work. No stage fusion was admitted
because it would change color/stage ordering. `SELECTED` remains the V4
point-Jacobi/Jacobi-gather execution with no color schedule.

S5.4b manifold checkpoint, 2026-07-16: the same compact scheduler can color a
whole one- or two-point generated manifold and execute the existing active-set
normal block in place. Tangent and restitution remain ordered scalar point
operations within the disjoint manifold unit. Native replay, exact one-point
degeneration, complete block traces, and atomic 33-manifold color overflow
pass. The lane remains laboratory-only pending ordering and full factorial
evidence; `SELECTED` is unchanged.

S5.4b ordering checkpoint, 2026-07-16: compact manifold blocks can use stable
forward order or a deterministic alternating sweep keyed by the global
substep/iteration ordinal. Normal/tangent and their relaxation counterpart
keep coherent direction; no GPU color-count readback is introduced. Native
i1/i2/i4 runs replay exactly and expose a genuine ordering delta, while a
single-color case remains byte-identical. Full quality/cost evidence is still
required before recommendation or selection.

S5.4b measured checkpoint, 2026-07-16: the clean
[compact-coloring factorial](../../evidence/p5.2b-s5.4b/README.md) qualifies
manifold forward i2/i4 and alternating i4 across all seven accepted awake
structures. Forward i2 is the cost leader at `17.685 ms` and 1,337 dispatches
for the 1K rigid fixture, versus `0.170 ms` for optimized Box2D. It spends only
`0.393 ms` in active colored solves while color rebuild/compaction and empty
dispatches dominate. Both predeclared conditional comparators are therefore
required: stable-pair incremental coloring and bounded workgroup-per-island
sequential solve. Point Jacobi/Jacobi-gather remains `SELECTED`; no physical
default, residual band, pacing, broadphase, or liquid decision changed.

S5.4b incremental-maintenance checkpoint, 2026-07-16: color maintenance and
colored dispatch are now a versioned laboratory scheduling policy orthogonal
to V4 solver execution. Existing hosts and product construction use the
standard full-rebuild/per-color-indirect policy. An explicit generated-contact
experiment may instead retain generation-safe stable-pair manifold colors,
with deterministic conflict repair and complete maintenance/capacity
evidence. Native forward-i2 exactness, replay, stable retention, and atomic
overflow pass without changing solver math or requiring readback. This does
not select the lane or close S5.4b.

S5.4b workgroup-island checkpoint, 2026-07-16: an explicit scheduling policy
can now build dynamic contact islands on GPU, compact manifold representatives
by island and color, and execute each physical stage as one indirect
workgroup per island with barriers between color phases. Fixed bodies neither
connect islands nor receive raced motion writes. Connected-stack and shared-
floor independent-island results match the per-color equations byte for byte;
full and incremental maintenance policies replay exactly and retain atomic
color overflow. The additional island/bin storage is capacity-accounted.
Existing hosts and product construction still use the standard selected
Jacobi/gather path. Renewed cross-host cost and quality evidence remains
required before the S5.4b human checkpoint.

S5.4b cross-host validation checkpoint, 2026-07-16: the shared host owns one
opt-in `solver-manifold-i2` scene using the neutral four-stack source and the
exact forward manifold-block i2/full-rebuild/per-color policy. Scene contract
v4 serializes that complete execution and scheduling identity for desktop,
headless, browser, and Android rather than relying on a host-local label.
Default product construction still uses `GpuSolverExecutionV4::SELECTED` and
standard scheduling. The validation scene is a shared adapter test surface,
not a second engine or a solver promotion. Clean headless, BrowserWebGPU, and
hardware-backed AVD artifacts are retained in the final renewed evidence slice.

S5.4c cross-host comparator checkpoint, 2026-07-16: the launch-only shared
`solver-parallel-coloring` scenario projects the same neutral four-stack into
the same engine-owned generated-contact world, selecting only the unpromoted
parallel scheduling policy. Scene-contract v4 exposes that exact policy and
completion rejects a mismatch or capacity failure. Headless Metal, Chrome
BrowserWebGPU, and hardware-backed AVD Vulkan/MoltenVK pass. The ordinary scene
menu excludes this evidence lane, and the standard product constructor remains
unchanged.

S5.4b renewed human checkpoint, 2026-07-16: the clean
[scheduling package](../../evidence/p5.2b-s5.4b-scheduling/README.md) finds all
four full/incremental and per-color/island policies physically equivalent on
the seven accepted sources, exact in replay, and clean in capacity. Full
rebuild plus per-color indirect is the clear 1K cost leader at `17.623 ms`.
The current incremental implementation costs `30.821 ms`, the island
implementation `30.946 ms`, and their combination `45.741 ms`; each remains a
measured reserve rather than a product path. Clean headless, BrowserWebGPU,
and hardware-backed AVD captures prove the recommended laboratory lane through
the shared host. Human review must decide whether to accept forward
manifold-block i2/full-per-color as the next product integration baseline.
`GpuSolverExecutionV4::SELECTED` remains point Jacobi/Jacobi-gather, and no
broadphase or liquid work is unblocked before that decision.

Human acceptance, 2026-07-16: forward manifold-block i2 with full rebuild and
per-color indirect dispatch is the next product integration baseline. The
shared generated-contact graph may replace the provisional point-Jacobi
selection with that exact execution and scheduling policy. Incremental and
workgroup-island policies remain laboratory reserves. The promotion must keep
one GPU world, require no result readback, preserve all versioned contracts,
and pass native, browser, and Android validation in its own commit.

Acceptance also authorizes S5.4c parallel scheduler construction alongside the
S5.5 broadphase bakeoff. The present serial construction/compaction marginal
cost makes the large-N crossover contract unsatisfied. Parallel compaction must
first preserve accepted assignments and order byte for byte; any later
parallel coloring comparator that changes order remains laboratory-only until
fresh quality evidence and human review. This is not authorization for liquid,
CPU fallback, native-only scheduling, or weaker capacity behavior.

S5.4 product-integration checkpoint, 2026-07-16: the selected generated-
contact product graph is now forward manifold-block i2 with colored in-place
accumulation, compact per-color indirect dispatch, warm start, relaxation, and
the standard full-rebuild scheduling policy. Shared P5.2 product scenarios
carry the two-iteration setting, while the S1.6 authored point-Jacobi i1
specification remains an explicit validation control. Product, headless,
browser, Android, and the named validation scene all use the same constructor
and scene-contract-v4 identity; no adapter owns a solver choice or second
world. This is a human-approved physical-default promotion, not acceptance of
the current performance curve, deep-stack diagnostics, or liquid readiness.

S5.4c renewed review checkpoint, 2026-07-16: the clean
[parallel-scheduler package](../../evidence/p5.2b-s5.4c-parallel-scheduling/README.md)
qualifies the bounded GPU-only speculative scheduler on the full accepted
quality, churn, replay, capacity, performance, and platform matrix. It changes
assignment and Gauss-Seidel order explicitly but keeps one manifold-block i2
solver, one generated-contact state, one frame graph, and no result-dependent
CPU path. Generated 1K improves from `16.794` to `8.781 ms` and authored 10K
contacts from `16.181` to `7.535 ms`; native Metal, BrowserWebGPU, and the
hardware-backed AVD execute the actual comparator. The result recommends a
scheduler promotion only. `GpuSolverSchedulingPolicyV1::STANDARD` remains
serial until human review, and the 50.8x 1K Box2D gap, deep-stack diagnostics,
liquid coupling, broadphase, and final scale advantage remain unresolved.

S6.1 scheduler integration checkpoint, 2026-07-17: human review accepted the
bounded parallel scheduler recommendation. The one product generated-contact
constructor now selects full-rebuild parallel speculative coloring with
per-color indirect dispatch. The accepted serial full-rebuild/per-color
schedule remains an explicit validation scene and historical factorial
control. Both schedules use the same generated-contact state, manifold-block
i2 equations, frame graph, renderer, and host boundary. No physical setting,
tolerance, capacity rule, oracle, or scale claim changes with this promotion;
fresh clean cross-host qualification remains pending.

S6.1 completion, 2026-07-17: the clean
[scheduler-promotion package](../../evidence/p5.2b-s6.1-scheduler-promotion/README.md)
qualifies selected and serial-control paths on native Metal, Chrome
BrowserWebGPU, and the hardware-backed AVD. It pins the unchanged solver
execution, policy digests, scene hash, clean capacity, and the immutable S5.4c
7/7 quality result. The locked attached Pixel 9 remains an explicit host gap.
Settling observation work can now measure against the intended product
schedule without reopening scheduler architecture.

S6.1a mobile pacing repair, 2026-07-18: an unlocked physical Pixel 7a proved
that the selected two-body product graph's 1,434-dispatch fixed floor sustained
only `13.51 FPS`, while the shared free-flight path sustained `60.07 FPS`.
Continuous marker reporting changed the slow result by less than two percent.
The one product constructor now selects the already qualified
workgroup-per-island dispatch form for body capacities through 64 and retains
per-color indirect dispatch above that tier. Bootstrap falls to 195 dispatches
and passes the maintained physical lane at `60.06 FPS` presentation and `0.990`
effective simulation scale. The dispatch forms replay body/cache/work exactly;
this changes neither color assignment/order nor any physics default,
physics tolerance, fixed delta, state ownership, or host behavior.

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

S21 evidence checkpoint, 2026-07-21: clean Metal and Browser WebGPU matrices
qualify both declared candidates through 100K, and human review selects
full-sort one-axis SAP. SAP wins every native scale row at 10K and above with
roughly one quarter of grid's fixed evidence allocation. Grid is the recorded
rejected alternative and no LBVH addendum is justified. S21.5 now wires SAP
into the shared product graph: live GPU body buffers produce speculative AABBs
and sorted canonical generational pairs under an 8N candidate bound, while
all-pairs remains an isolated bounded oracle with no automatic fallback.
Integrated Metal, Browser WebGPU, and physical Pixel 7a Vulkan qualification
passes, closing S21. The physical 103-body bucket remains deliberately reported
as a correct slow-time result rather than a sustained-rate claim. The measured
record, configurable 64/256/1K/10K capacity policy, and reconsideration
triggers are in [S21](../tactical/021-scalable-broadphase.md) and the
[performance topic](performance-and-scale.md#interactive-world-capacity-policy).

## What to learn from existing engines

### Box2D rigid oracle

The first milestone should run the GPU solver beside an unmodified Box2D
world. One engine-owned scenario definition supplies the same authored bodies,
initial states, gravity, fixed step, and neutral action sequence to both.
Box2D is an oracle and comparison harness, not a product backend, browser
fallback, or second scene format. Authoritative automation remains a
process-isolated native test tool; the separately authorized interactive
shadow may run inside the shared host only as visibly labeled validation
presentation.

The initial source reference is commit
`3a4f0da8374af61293a03021c9a0b3ebcfe67948` (currently
`v3.1.1-18-g3a4f0da`). The [local reference map](../../reference/README.md)
distinguishes the clean `reference/box2d` authority checkout from the same
revision under the `../physbox3` behavioral oracle and its WASM build. Freeze
the exact revision, build options, world settings, worker count, and scenario
projection. The deterministic oracle lane should use a
single-threaded/unthreaded configuration; a separate PhysBox 3 product lane
may preserve the original worker and LiquidFun settings only for web
calibration. An oracle result is not reproducible if it records only "Box2D"
without this configuration.

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
| Full rigid scenario | Native pinned unmodified Box2D v3 world | Scenario-specific trajectory and stability bands |
| Product calibration | Fresh PhysBox 3 web traces and synchronized playback | Intended neutral interaction and subjective feel |
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

### Oracle execution topology and precedence

Routine authoritative oracle automation uses process-isolated native
executables rather than driving a browser or linking a CPU engine into the
PhysBox acceptance result. The shared Rust runner owns the only authored
scenario and result contract:

```text
shared versioned scenario
    |-> neutral projection -> native oracle process -> canonical oracle result
    `-> PhysBox GPU/headless run                 -> canonical GPU result
canonical results -> shared diff, metrics, minimization, and playback
```

Each adapter accepts a versioned projection containing the source scenario
hash, fixed step, settings, bodies, shapes, and neutral actions. It must reject
an unsupported feature with a deterministic reason instead of approximating it
or inventing a private fixture. It emits the canonical result/trace schema plus
its source revision, build options, runtime settings, and dirty or patch
digest. The shared runner, not the foreign-engine adapter, owns tolerances,
expected structural outcomes, comparison, minimization, and presentation.

Start with an inspectable canonical JSON request and result written to explicit
paths. A thin Rust CLI for each adapter should depend on the shared schema-only
validation crate, parse and emit that schema, and call the foreign C/C++ engine
through a narrow C ABI or direct C FFI. Foreign source, CMake glue, and shims
belong under test/oracle tooling; no engine or app crate may depend on an
adapter. The runner validates the pin before launch, applies a timeout, requires
an atomic fresh result plus `PHYSBOX5_ORACLE_SUCCESS`, and treats adapter logs
as diagnostics rather than schema. Canonical JSON uses UTF-8, stable struct
field ordering and stable identifier ordering, finite numbers only, and no
unordered object maps in hashed payloads. Preserve the source scenario hash
from the shared runner and record a separate projection hash over the exact
request bytes. Version the protocol before replacing JSON with a denser trace
encoding.

The [interactive comparator](interactive-box2d-comparison.md) is the sole
exception to the product-dependency wording above: its compile feature may
package an enumerated redistributable subset of the same Box2D pin behind one
shared-host shadow API. It never produces authority artifacts, feeds PhysBox,
or participates in product performance. Compiling the feature out removes the
foreign code, assets, UI, and runtime work.

Keep authority engines in separate processes. In particular, Box2D v3 and
original LiquidFun's Box2D 2.3 are distinct source generations and must not
force their APIs, symbols, build systems, or lifetime rules into one authority
binary. Process isolation keeps this boundary mechanically visible and
prevents a direct interactive fallback path.

Authority is declared by layer; engines do not vote:

| Question | Authority | Status of other engines |
| --- | --- | --- |
| Mathematical and physical invariants | PhysBox specification | A violation fails regardless of engine agreement |
| Declared pass equations and ordering | Scalar Rust executable specification | Exact or tightly bounded hard gate |
| Rigid manifold and world behavior | Native unmodified pinned Box2D v3 | Primary rigid acceptance authority |
| Later coupled particle/body behavior | Native PhysBox 3 sidecar plus its pinned Box2D v3 | Primary coupled-behavior authority |
| Historical LiquidFun algorithms and scenarios | Native original LiquidFun/Box2D 2.3 | Secondary reference, not an exact PhysBox 3 substitute |
| Product settings and subjective feel | Fresh PhysBox 3 web calibration corpus | Calibration and review, not routine CI execution |
| Independent rigid disagreement | Optional native Rapier2D | Non-gating diagnostic evidence only |

When two engines disagree, label the fixture engine-sensitive and inspect the
invariants, scenario projection, settings, and intended product behavior. Do
not choose the majority result, widen a band, or change authority silently.
Rapier2D is not part of P5.0a; it may be added later only when the Box2D lane is
green and a measured ambiguity or suspected Box2D overfit justifies the cost.

### Box2D authority and microscope builds

Keep two distinct native Box2D lanes:

- The process-isolated authoritative executable links an unmodified pinned
  Box2D library. Its public state, events, manifolds, and debug draw data own
  pass/fail comparisons.
- A separately patched microscope build records internal stages for diagnosis.
  Its patch digest is provenance, and its output never becomes the authority.

On 2026-07-14 the pinned Box2D source built successfully as a native static
library on arm64 macOS with CMake 4.2.3 and AppleClang 21. P5.0a subsequently
froze the process protocol, single-threaded authority settings, validation-on
and SIMD-off build, fresh atomic artifacts, and success sentinel. The separate
optimized performance lane retains SIMD and named one-/four-worker settings.
No patched microscope was needed for the accepted fixtures.

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

### LiquidFun and PhysBox 3 fluid oracles

The original plan deferred liquid until the complete rigid fixture set passed.
The 2026-07-23 entry decision now authorizes the bounded S31 phase while the
remaining rigid failures stay explicit debt. Use LiquidFun and PhysBox 3 as the
behavioral reference for:

- synchronous particle/body momentum exchange;
- displacement and buoyant support;
- current contact-point surface velocity and rotational effective mass;
- off-center fluid torque;
- floating, sinking, and wake behavior;
- dam-break impact and containers with moving boundaries;
- the natural, smooth subjective behavior that motivated the new engine.

As with Box2D, these oracles exist only in validation and comparison tooling.
The product path remains the one GPU solver, and the fluid solver may differ
from LiquidFun internally as long as the resulting interactions meet the
recorded behavioral contract.

The workspace-local source of record is the PhysBox 3 sibling at
`../physbox3`: its LiquidFun checkout is `../physbox3/reference/liquidfun`, its
actual Box2D v3 source is
`../physbox3/vendor/box2d3-wasm/box2d`, and the binding/build layer is
`../physbox3/vendor/box2d3-wasm`. Revisions and clickable local paths are kept
in the [reference map](../../reference/README.md). The exact reduced particle
solver used by PhysBox 3 lives under
`../physbox3/vendor/box2d3-wasm/box2d3-wasm/csrc/particle_sidecar/`; its
Emscripten bindings are separate in `csrc/glue.cpp`.

Use three deliberately different lanes:

1. Compile the PhysBox 3 particle sidecar natively with its pinned Box2D v3 and
   box2cpp sources. This is the routine primary coupled-behavior authority
   because it reproduces the algorithm and bridge used by the product without
   requiring browser automation.
2. Compile [original LiquidFun](https://github.com/google/liquidfun) and its
   Box2D 2.3 world as a separate native executable. It is a secondary source
   for particle algorithms, historical tests, baseline patterns, and scenes
   such as body contacts, confinement, dam break, and wave machines. It is not
   an exact oracle for the reduced PhysBox 3 sidecar.
3. Drive the actual PhysBox 3 web product only for a small canonical
   calibration corpus and subjective review. Refresh that corpus when the
   native sidecar adapter, PhysBox 3 revision, build flags, defaults, or
   scenario projection changes; do not make every PhysBox 5 correctness run
   depend on browser automation.

The pinned original LiquidFun repository is now read-only upstream, which is
acceptable for a frozen source reference. Its native test infrastructure
already records body properties across a simulation and compares them to
checked baselines with tolerances. On 2026-07-14 its complete Box2D 2.3 plus
particle solver built successfully as a native static library with
`CMAKE_POLICY_VERSION_MINIMUM=3.5`. On the same machine PhysBox 3's
`particle_bridge.cpp` and `particle_system.cpp` compiled as native C++20
objects against its Box2D v3 and box2cpp headers without Emscripten includes.
These checks establish feasibility, not behavioral equivalence; the native
sidecar must still be calibrated against fresh web-product traces.

Do not promote LiquidFun, the sidecar, or the web product's internal
representation into the PhysBox 5 scene or GPU state contract.

### Optional Rapier2D dissent lane

Rapier2D is a genuine 2D specialization: it uses 2D vectors, scalar angular
state, and the `parry2d` collision crate. It is not a 3D world constrained to a
plane. However, the
[Rapier repository deliberately shares source](https://github.com/dimforge/rapier/blob/master/ARCHITECTURE.md)
between 2D and 3D builds with dimension-selected types and conditional paths.
That makes it a useful independent engine but a less direct implementation
reference for a small true-2D WGSL solver than Box2D's C source.

Do not add Rapier to the initial solver laboratory. If the accepted Box2D lane
later leaves a measured ambiguity, an isolated native Rapier2D adapter may run
the same high-level scenario projection and report trajectory, penetration,
settling, energy, and sleep metrics. It is not authoritative for Box2D feature
identifiers, exact manifolds, solver pass values, or product feel. Agreement
between Box2D and Rapier strengthens evidence; disagreement identifies a
scenario for investigation and never triggers majority voting. PhysBox 4's
existing Rapier2D adapter may reduce implementation cost, but any reuse must be
selective, test-only, and carry exact PhysBox 4 provenance.

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
[PhysX GPU rigid bodies](https://nvidia-omniverse.github.io/PhysX/physx/5.6.1/docs/GPURigidBodies.html).

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
- fixed-step scheduling ownership, while replacing its catch-up behavior with
  the PhysBox 5 slow-time contract;
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

The local sibling and all nested reference revisions are enumerated in
[`reference/pins.toml`](../../reference/pins.toml). The LiquidFun checkout was
at `7f20402173fd143a3988c921bc384459c6a858f2`, and PhysBox 3's nested Box2D
source was at the same `3a4f0da8374af61293a03021c9a0b3ebcfe67948`
revision as the independent authority lane.

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
  metric, native-adapter request/result, and failure-result schemas in Rust.
- Add the process-isolated native unmodified pinned Box2D authority lane,
  direct public narrowphase comparison, and optional patch-digested microscope
  build.
- Implement the scalar executable specification and pass-level GPU comparison.
- Add the scoped analytic fixture contact generator for multi-step fixtures;
  fixture geometries only, checked against Box2D's public collision functions,
  replaced by P5.2.
- Compare portable contact-centric Jacobi/gather, graph-colored Soft-TGS, and
  optional rigid XPBD or split alternatives on the same precomputed contacts.
- Benchmark the shared prefix-scan, radix-sort, unique, and compaction
  primitives the adjacency build requires; several of them run more than once
  per step, so their measured cost is a solver-selection input that caps
  broadphase, persistence, and solver scale.
- Name the desktop and Android reference adapters and record at least one
  solver-laboratory Android measurement; record a physical-device availability
  gap explicitly rather than substituting emulator evidence silently.
- Select the solver foundation from numerical behavior, cross-adapter behavior,
  GPU cost, memory, and one side-by-side review.

No product collision pipeline is built until this stage chooses a solver
foundation and records its accepted parameters and baseline results. Original
LiquidFun, the PhysBox 3 sidecar and web product, and Rapier2D remain outside
this stage.

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

S3.1 contract checkpoint, 2026-07-15: P5.2 assigns metadata bit 9 to centered
circles while retaining rectangle bit 8 and the accepted 64-byte body core.
Circle radius repeats in `properties.xy`; rectangles retain half extents. The
versioned collision surface defines canonical generational pairs, provisional
circle/face/vertex features, persistent keys, exact 16-byte-aligned GPU record
strides, full work accounting, atomic capacity failure, first-divergence
traces, and source/projection/provenance identity. Four engine-owned sources
and neutral Box2D projections have pinned canonical hashes. Details and
reconsideration triggers live in the
[collision/persistence topic](collision-and-contact-persistence.md) and
[S3.1 evidence](../../evidence/p5.2-s3.1/README.md). No product GPU collision
behavior is present until S3.2.

### P5.3: Scalable broadphase

- Retain brute force as the pair-completeness oracle.
- Bake off sorted 1D sweep-and-prune and multi-cell grid candidates on the same
  correctness and performance corpus.
- Add LBVH only if measurements justify it.
- Select one shared portable algorithm and prove zero false negatives, stable
  canonical pairs, bounded memory, and explicit overflow behavior.

### P5.4: Rigid behavior parity

- Retain the implemented S23/S30 sleeping, waking, grouping, and compact
  asynchronous diagnostics with S32's sleeping-on/exact ordinary defaults and
  the retained sleeping-off/local-4/local-8 comparison controls.
- Validate flat and edge landings, stacks, pyramids, impacts, sliding, rolling,
  bounce, off-center rotation, wake propagation, and grab/release against the
  Box2D oracle.
- Run fixed-step, substep, iteration, mass-ratio, material, capacity, and
  adapter sweeps without changing product behavior per host.
- Preserve these numerical and synchronized-review rows as the complete
  rigid-parity claim. The 2026-07-23 decision permits bounded S31 liquid work
  before every row closes, without treating an open row as passed.

### P5.5: First construction loop

- Extend the P5.2a minimal circle/rectangle place/erase workbench with
  selection, editable body properties, and deliberate shape sizing.
- Save and reload the minimal authored scene.
- Add one joint only after free-body contact is stable.
- Decide whether the solver foundation is sufficient for chains and ragdolls
  before restoring broader tools.
- S31 may pull Water, Surface, particle-aware Erase, and a shared render-style
  refresh forward because they are required to evaluate coupled behavior.
  Persistent properties, save/load, joints, and the complete tool catalog
  remain in this broader construction milestone.

### P5.6: LiquidFun-oracled fluid coupling

- Restore a bounded water-like particle solver under the reviewed coupled
  constraint topic and S31 plan. This explicit entry decision replaces the
  former blanket “after rigid parity” blocker without claiming P5.4 completion.
- Define fluid/body oracle fixtures in the shared projection rather than in
  any foreign adapter.
- Add separate native adapters for the PhysBox 3 sidecar plus Box2D v3 and for
  original LiquidFun/Box2D 2.3.
- Calibrate the native sidecar against a small fresh PhysBox 3 web corpus, then
  use native processes for routine unattended comparisons.
- Couple particle/body contact inside the GPU simulation step.
- Apply equal-and-opposite translation and off-center rotational response.
- Include fluid boundary-density and iterative pressure effects rather than
  final projection alone.
- Demonstrate displacement, submerge, float or settle, wake, and dam-break
  response.
- Prove that no compact fluid-impulse readback exists in the simulation
  dependency graph.
- Keep the interactive Box2D shadow rigid-only: it receives no particle state,
  fluid forces, or fluid-tool actions, and Box2D view hides PhysBox water.

## Oracle-driven development and failure artifacts

Cross-host validation scenarios have one source of truth in shared Rust. A
scenario owns authored bodies, stable handles, initial state, fixed-step and
solver settings, neutral actions, completion criteria, structural assertions,
metric bands, and a versioned result. Desktop, web, Android, and headless select
and transport it through the shared launch contract; Playwright, JavaScript,
shell, ADB, or an app crate must not restate its physics sequence or expected
numbers.

The native Box2D and later PhysBox 3-sidecar and original-LiquidFun adapters
consume a versioned neutral projection generated by that shared runner. An
optional Rapier2D comparator must use the same protocol. They do not maintain a
second hand-authored scenario format. The projection records the source
scenario hash and rejects unsupported actions or shapes rather than
approximating them silently.

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
their final simulated frame. Replay consumes captured inputs and settings. A
reviewed checked trace may be a named calibration or numerical baseline, but
it is never proof that the current oracle or web build produced the same
result; claims about current behavior require a fresh run with provenance.

The autonomous loop is: generate or select scenario, run the scalar/pass-level
checks, run the declared native authority and GPU worlds, evaluate invariants
and metric bands, run cross-host and performance lanes as required, minimize
failures, and add the reproducer to the corpus. Automation may fix an
implementation to restore an accepted baseline. It may not change solver
family, oracle precedence, physical defaults, tolerances, oracle revisions, or
checked numerical baselines without an explicit reviewed decision and measured
before/after report.

## Performance and capacity gates

The standalone [performance and scale contract](performance-and-scale.md) owns
the workload vector, scene families, count sweep, Box2D authority and optimized
CPU lanes, timing protocol, result validity, capacity accounting, provisional
targets, and regression/claim gates. It is the authoritative source when a
short milestone summary omits performance detail.

The durable direction-level constraints are:

- performance evidence begins in the authored-manifold solver laboratory and
  influences solver, layout, dispatch, persistence, sleeping, and broadphase
  decisions;
- body count is never reported without awake work, candidate pairs, contacts,
  solver work, shape mix, memory high-water, and capacity state;
- primary claims require correctness-qualified rectangles or mixed shapes in
  addition to any large circle-only result;
- the deterministic Box2D oracle lane is not substituted for an optimized
  release/SIMD/worker CPU performance competitor;
- solver-only, collision-only, end-to-end physics, and full-frame scopes remain
  separable;
- GPU timestamps, CPU encode/submit time, CPU step time, and full-frame wall
  time are labeled rather than conflated; and
- correctness failure, silent work loss, overflow, simulation-dependent
  readback, unequal physical quality, or incomplete provenance invalidates a
  performance result.

The initial shorthand is that 10K bodies demonstrate useful scale, 30K should
begin to expose a material GPU advantage on realistic contact-rich workloads,
and 100K is a scene-qualified frontier. These are planning hypotheses, not a
universal promise or a substitute for measured curves on named adapters.

## Acceptance criteria for the direction

### Rigid-foundation acceptance

The complete rigid-parity claim requires all of the following:

The 2026-07-23 liquid-entry decision accepts the current partial foundation for
the bounded S31 workstream before every item is true. It does not delete,
weaken, or mark the remaining items complete; they remain the P5.4 acceptance
and crossover debt, and coupled results may not substitute for them.

- No GPU-to-CPU result is required to advance rigid simulation.
- Interactive frames execute zero or one outer fixed step, slip excess wall
  time without debt, and report effective scale; explicit-step validation
  completes the authored count independently of machine speed.
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
- Small stacks and pyramids remain coherent for their complete fixture window;
  the current centered-stack roles and cross-host gate are owned by
  [rigid quality targets](rigid-quality-targets.md), with 12 blocking and
  14/16 retained as stretch/diagnostic evidence.
- The process-isolated native Box2D oracle consumes the projection generated
  from one shared scenario definition, emits a fresh canonical result, and is
  never used as a product or platform fallback.
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
per-pass GPU time where supported, pacing mode, effective time scale, and
slipped wall time. Exact full-trajectory equality with Box2D is neither
required nor expected; exact or tightly bounded pass-level comparison with the
PhysBox scalar specification is required where declared.

### Later coupled-fluid acceptance

The complete direction succeeds only when these additional conditions become
true:

- No GPU-to-CPU result is required to advance rigid or fluid simulation.
- The native PhysBox 3 sidecar oracle is calibrated against fresh web-product
  traces for the accepted coupled corpus, with revision, build flags, defaults,
  and projection recorded.
- The GPU coupled metrics remain in justified scenario-specific bands relative
  to the native sidecar; original LiquidFun results are retained as secondary
  evidence rather than substituted silently.
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

The first rigid work proceeds with these decisions:

- Soft-Step-like velocity impulses with the accepted S1.6 parameters are the
  selected initial rigid behavior foundation; rigid XPBD and split alternatives
  require a new measured ambiguity before implementation.
- Contact-centric Jacobi plus per-body gather is the selected portable write
  strategy; GPU-colored TGS remains the principal convergence comparator.
- All-pairs is the initial trusted broadphase and pair oracle; sorted SAP and a
  multi-cell grid are the first scalable bakeoff.
- Persistent contacts use canonical body handles with generations plus stable
  geometric feature identifiers. Stable generational pair slots with full
  local equality verification are the selected P5.2 product direction; the
  exact full-key sorted merge remains the correctness baseline.
- Generated-contact scenarios use the accepted provisional P5.2 defaults of
  `0.02` speculative distance and `3.0 m/s` per-contact maximum depenetration.
  Scalar and GPU stages share that contract; policy-free authority fixtures
  retain exact overlap admission. Deep coincident placement remains a
  diagnosed exceptional state, not an accepted ordinary spawn condition.
- Interactive frames execute at most one outer fixed step and permanently slip
  excess wall time; solver substeps and explicit validation steps are separate
  concepts.
- Box2D commit `3a4f0da8374af61293a03021c9a0b3ebcfe67948`
  is the initial rigid oracle source; a process-isolated native adapter plus
  exact build and world settings are frozen by P5.0a.
- Rapier or another CPU rigid backend is never a product fallback. Rapier2D is
  excluded from P5.0a and may become only a non-gating native comparator after
  a measured ambiguity and explicit decision. Box2D remains validation-only;
  its explicitly authorized interactive shadow is not product authority.
- The S31 routine coupled oracle is the PhysBox 3 particle sidecar compiled
  natively with its Box2D v3 source. Original LiquidFun is secondary evidence,
  and the web-only PhysBox 3 product is a calibration lane.
- The first coupled material is bounded water in the PhysBox GPU world only.
  The interactive Box2D shadow remains rigid-only and receives no fluid state,
  contact, forces, or fluid-tool actions.
- Water and particle-aware Erase are bounded per-step GPU commands admitted by
  the shared interactive clock; zero-step frames cannot build a later emission
  burst. Surface authoring is a PhysBox-only construction-v3 intent whose
  drag endpoints allocate one thin fixed oriented rectangle on the GPU.
  Water/Surface are hidden in Box2D view, Box2D-visible Erase cannot mutate
  hidden particles, and Surface is filtered from both comparison projection
  directions. The PhysBox liquid world continues stepping while hidden.

The solver laboratory and early pipeline must still answer:

- Does contact-centric Jacobi need mass splitting, additional iterations, or a
  hybrid colored treatment for tall stacks and extreme mass ratios?
- Can graph coloring be bounded and driven entirely on the GPU without too many
  global-barrier dispatches for portable WebGPU?
- At what measured body/contact distributions does SAP, grid, or later LBVH win
  on browser, desktop, and Android adapters?
- Which future evidence would justify changing the implemented S23/S30/S32
  sleep policy? Exact and sleeping-on are the selected defaults; local-4,
  local-8, and sleeping-off remain explicit choices. Coupled work must retain
  GPU fluid wake roots without changing those defaults implicitly.
- Which scenario-specific trajectory, manifold, energy, penetration, and sleep
  bands distinguish healthy parallel ordering variation from a materially
  different or unstable result?
- What fixed capacities and growth/restart policy cover the product defaults
  without silent drops or routine over-allocation?
- Which named adapter set and product scene establish the first absolute
  performance budget after relative regression tracking is operational?

Fluid formulation, particle/body reduction, coupled-iteration count, and the
exact PhysBox 3/LiquidFun oracle fixtures are now bounded S31 decisions. The
coupled topic limits the first schedule comparison, requires GPU body gather
and same-step equal-and-opposite response, and defines seven initial sources.

Answer the remaining questions with minimal GPU prototypes, shared scenarios,
and recorded selection matrices, not by expanding the PhysBox 4 delayed
interface.
