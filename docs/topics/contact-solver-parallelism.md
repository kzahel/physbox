# Contact solver parallelism and scheduling

Status: bounded parallel scheduler selected and qualified, 2026-07-18.
Forward manifold-block i2 remains the selected shared solver. Product
constructors select bounded speculative parallel coloring. The 64-body
interactive capacity tier uses one workgroup-per-island dispatch while larger
capacities retain per-color indirect dispatch; serial full rebuild/per-color
remains an explicit control.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Execution tracker:
[S5 interactive rigid quality and scale](../tactical/005-interactive-rigid-quality-and-scale.md).
Cost and scale claims are owned by
[performance and scale](performance-and-scale.md); pacing and any frame-rate
retarget are owned by
[simulation time and overload](simulation-time-and-overload.md); stage math is
owned by the [scalar contact specification](scalar-contact-specification.md).
Primary evidence:
[S5.4 awake settling bakeoff](../../evidence/p5.2b-s5.4/README.md) and
[S5.4a parallel-normal factorial](../../evidence/p5.2b-s5.4a/README.md), with
the current [S5.4c scheduler review](../../evidence/p5.2b-s5.4c-parallel-scheduling/README.md).

## The concern

Rigid contact impulses are interdependent: the bottom contact of a stack must
carry the weight of every body above it, but learns that only through the
chain of contacts. How those coupled solves are ordered on a massively
parallel device decides both structural stability and portable scheduling
cost. This topic records why the independent-parallel (Jacobi) formulation
fails tall structures, which alternatives were measured, why sequential-style
(colored Gauss-Seidel) solving is the selected next experiment, and what its
WebGPU dispatch floor may imply for the product.

## Why independent-parallel solving fails stacks

A Gauss-Seidel sweep solves contacts one at a time against continuously
updated velocities, so support information crosses the whole chain in one
sweep. Box2D does this and keeps every test structure upright. A Jacobi pass
instead solves every contact against the same start-of-pass snapshot and
applies all impulses together. The product graph is Jacobi: each point uses a
standalone two-body effective mass (`effective_mass`), solves against the
snapshot (`emit_normal`), and the gather stage sums every contribution at full
strength with no contact-count scaling (`gather`, all in
`crates/physbox-engine/src/gpu_solver.wgsl`).

Two mechanisms follow:

- **Propagation delay.** Support information moves one contact layer per
  pass, so a deep chain cannot equilibrate within a feasible pass budget
  while gravity keeps injecting new penetration.
- **Simultaneous overshoot.** An interior stack body receives full-strength
  corrections from the manifolds above and below at once. Whether the
  overshoot damps or grows is governed by the spectral radius of the Jacobi
  iteration matrix; chain length destroys the diagonal dominance that
  convergence requires.

The measured signature is decisive: past a depth threshold, more iterations
make the same structure fail *earlier* (the 16-stack fails at step 5 with
eight iterations versus step 14 with one). That is divergence, not slow
convergence. A second, independent defect is intra-manifold rocking: the two
points of a box-box manifold each compute a full standalone correction
including the angular term, so their simultaneous sum over-rotates the body
and pumps energy — the dominant visible jitter on otherwise passing scenes.

## Measured alternatives

S5.4 evaluated nine reversible executions inside the point-Jacobi family
(substeps, iterations, warm start, relaxation, bias, contact policy): every
candidate lost the awake 8-stack while awake Box2D held every structure below
`2.72e-6 m/s` / `1.78e-6 rad/s` settled residuals. S5.4a then ran a factorial
of five formulations at 1/2/4/8 iterations over seven structures including a
16-stack. Condensed result (complete matrix in the evidence package):

| Family | Best structural result | Interpretation |
| --- | --- | --- |
| Point Jacobi | 3/7 at every iteration count | Depth-limited; iteration-amplified divergence |
| Scalar mass splitting | 6/7 at i8; 16-stack falls at step 44 | Contracts the frozen rocking probe but is too slow in the awake chain; sags ("memory foam"), 8-stack residual 0.377 m/s |
| Manifold block normal | 6/7 from i2; 16-stack falls even at i8 | Removes rocking, moves the depth horizon, does not remove it |
| Block + mass splitting | 3/7, worse with iterations | Negative interaction; do not compose these fixes |
| Colored in-place | **7/7 at i4 and i8** | Only family that preserves every tested structure through the 16-stack; scheduling cost and residual quality remain blockers |

Cost at 1,024 bodies (Apple M4 Pro Metal, complete graph P50): manifold block
i2 `3.564 ms` / 135 dispatches; colored i4 `19.308 ms` / 2,360 dispatches;
fastest optimized Box2D `0.185 ms`. The colored comparator encodes 32
worst-case color passes times five separate stages, while actual color counts
are only 4 (16-stack) to 12 (4-row pyramid). Scheduling waste is therefore the
leading cost hypothesis, not a measured decomposition of the `19.308 ms` row.

Conclusions recorded as decided:

- The tested Jacobi-family changes do not preserve the 16-stack. Colored
  propagation is the selected bounded experiment, not yet a production rigid
  direction or proof that no other parallel formulation can succeed.
- Mass splitting is excluded from the first colored factorial because no
  measured colored failure requires it and it would confound attribution. The
  measured block-plus-mass-split regression occurred under Jacobi and does not
  prove the same interaction under colored execution.
- The two-point manifold block solve is validated independently. Whether it
  reduces colored iteration count is an explicit S5.4b question, not a
  conclusion.

## The portable dispatch floor

Sequential-by-color execution turns every color transition into a dispatch
boundary with fixed driver/queue cost, independent of pass size. The frame
contract forbids CPU readback of GPU results, so the CPU encodes the command
stream blind: the current comparator therefore encodes the 32-pass worst case
even when four colors exist, and empty passes still launch full grids that
exit on a color check. The optimized CPU lane has a fundamentally cheaper
small-scene scheduling path and retains its measured advantage.

The floor is an arithmetic, not a mystery: optimized passes scale as
(actual colors) x (fused stages) x (iterations) x (substeps). With 4–12
colors, ~2 fused stages, <=2 iterations, and 4 substeps, the expected count is
a few hundred dispatches. Whether that reaches the cost territory of the
Jacobi/gather lanes is plausible but unmeasured; S5.4b exists to measure it.
Two structural facts frame the decision:

- The dispatch floor is roughly constant in body count (colors track contact
  graph degree, not N), while solve work and any CPU engine scale with N. The
  floor may amortize as scenes approach the product target of thousands of
  solids plus liquid. Small-fixture ratios such as 20.8x or 104.3x versus
  Box2D remain material decision evidence even though they cannot establish
  the eventual crossover; scale claims belong to the performance-and-scale
  contract.
- Per-dispatch overhead differs by host. The floor must be measured on
  browser WebGPU and Android, not extrapolated from Metal.

## Authorized next experiment (S5.4b)

One bounded, attributable factorial over a shared compact scheduler:

1. **Scheduling-only control:** preserve the existing point coloring and
   scalar point solve while compacting GPU-owned color ranges, emitting
   indirect zero-work arguments for absent colors, and fusing only stages that
   remain byte-exact. This owns the scheduling-cost attribution gate.
2. **Manifold-colored block lane:** color whole manifolds, keep both points in
   one colored unit, and run the validated active-set 2x2 normal solve. Sweep
   i1/i2/i4; i4-only preservation would show that block coupling did not buy a
   lower iteration count.
3. **Ordering lane:** compare stable forward color order with deterministic
   alternating forward/reverse sweeps. Do not add gravity-directed or random
   ordering unless this comparison leaves a material question.
4. **Depth, degree, and churn diagnostics:** retain the seven S5.4a acceptance
   sources, add 32/64 stacks as depth diagnostics rather than new accepted
   quality bands, and measure both 100-body buckets plus deterministic
   construction/erase churn. Record actual/allocated colors, color occupancy,
   compaction and recoloring work, empty indirect launches, dispatches, memory,
   and complete time.

The compact control must match the existing point-colored result byte for byte
at the same iteration count. All lanes retain the same generated contacts,
pair slots, body-edge views, caches, capacities, physical settings, one world,
and no-readback contract. Mass splitting, pacing changes, production
broadphase, liquid, and product promotion remain excluded.

Gates repeat S5.4a: all seven accepted structures over every one of 1,200 exact
steps, complete 32/256/1K cost, exact replay, explicit capacity/failure data,
and browser plus hardware-backed AVD execution for a potential candidate.
The 16.67 ms boundary is only feasibility; a liquid-heavy product still needs
material rigid-solver headroom and later scale evidence. Promotion and any
residual-motion band remain separate explicit human decisions.

Scheduling-control checkpoint, 2026-07-16: the first S5.4b implementation
keeps the exact greedy point assignments, forward color order, scalar solve,
and stage sequence. A GPU-owned stable compaction writes per-color counts,
prefix offsets, original point indices, and 32 indirect dispatch records. The
color-construction pass and indirect solve are separated by a compute-pass
boundary so the argument buffer is never simultaneously writable storage and
an indirect source. The eight-body/eight-step/i2 stack matches the legacy
fixed-full-grid comparator byte for byte in bodies, persistent cache, prepared
contacts, contribution impulses, and replay. A 33-incident-point overflow
zeros every indirect workload and retains the diagnosed atomic failure.

No stage fusion qualifies as scheduling-only: moving tangent, relaxation, or
restitution work inside a per-color dispatch changes the global stage/color
order for bodies shared across later colors. The control therefore compacts
work without fusing equations. This is a successful attribution floor, not a
quality or production selection; manifold coloring and block math remain the
next authorized slice.

Manifold-block checkpoint, 2026-07-16: compact coloring can now treat one
complete generated manifold as the disjoint unit. Both points receive the same
color; the representative point enters the stable compact range once. Normal
stages execute the validated active-set 2x2 equations against the current
in-place body motions, while warm start, friction, and restitution retain
deterministic point order inside the manifold. Matrix, active-set, impulse,
occupancy, compaction, and indirect-work evidence all remain explicit.

The four-body native stack replays exactly with coupled block evidence. A
one-point restitution manifold is byte-identical to compact scalar point
coloring and records the scalar active set. A 33-manifold shared-body fixture
exceeds the declared 32-color capacity, emits zero work for all indirect solve
passes, and preserves one atomic failure. This closes implementation and
degeneration gates only; i1/i2/i4 quality and cost belong to the later
factorial, and forward versus alternating order is the next slice.

Ordering checkpoint, 2026-07-16: the compact scheduler now encodes either
stable forward colors or deterministic alternating forward/reverse sweeps
without reading the GPU-produced color count. Alternating parity is
`substep * iterations + iteration`: normal and tangent retain one coherent
direction, relaxation mirrors its corresponding iteration, warm start uses
the substep's first parity, and restitution uses its last. Thus i1 alternates
across substeps, while i2/i4 also alternate inside each substep.

Native i1/i2/i4 manifold-block runs replay exactly, retain identical color
assignments and capacities, and produce a real ordering difference on the
stack corpus rather than aliasing forward order. A one-color manifold remains
byte-identical in both policies. This records the comparator semantics and
mechanical result only; the seven-source 1,200-step matrix, residuals, and cost
decide whether either order is promising.

Cost-instrumentation checkpoint, 2026-07-16: compact execution now separates
the unchanged greedy color assignment from stable range compaction with an
explicit GPU pass boundary. The generated-contact benchmark retains ordinary
complete-step timing and adds a diagnostic ordered phase-group lane that
attributes color construction, compaction, active-color solve work, zero-work
indirect launches, and the remaining solver graph. Adjacent operations already
assigned to the same declared phase share one timestamped pass; phase changes,
operation order, dispatches, assignments, stable index order, indirect
arguments, and atomic overflow semantics remain unchanged. No cost or quality
conclusion is recorded until the clean factorial is published.

Evidence-automation checkpoint, 2026-07-16: one bounded S5.4b runner freezes
the fixed/compact point i4 controls and manifold forward/alternating i1/i2/i4
matrix. The seven governed structures remain the only acceptance corpus;
32/64 stacks and 100-body circle/rectangle buckets are separately labeled
diagnostics. A shared five-snapshot source records deterministic construction,
erase, and generation reuse and counts full rebuild work plus surviving color
changes. Fresh failure reproducers, Box2D authority, raw 32/256/1K timing,
platform provenance, and the conditional comparator triggers are mandatory
outputs. No result is claimed until a clean atomic package completes.

Transient-failure checkpoint, 2026-07-16: the first clean-at-start factorial
attempt completed all seven accepted sources and the 32-stack diagnostic, then
found a stack-64 color-capacity rejection at post-step 357. This is the
declared atomic 32-color boundary, not an accepted-source regression. The
native diagnostic-history contract now returns versioned `FailureV1` records
for transient construction or solver-coloring rejection alongside the bounded
body history. The legacy strict history call still rejects the same run. This
lets the factorial preserve the exact first failing step and permanent
reproducer instead of aborting or allowing a later clean checkpoint to hide a
transient capacity failure. The clean package and its conclusions remain
pending.

Timestamp-budget checkpoint, 2026-07-16: the next clean-at-start attempt
completed all seven accepted sources, all four diagnostics, and deterministic
churn, then rejected the first performance row before measurement because the
fixed-grid i4 control requested 4,660 queries from one WGPU query set; the
portable limit is 4,096. Diagnostic timestamps now partition safely across
bounded query sets and, more importantly, group only consecutive operations of
the same declared phase. The exact fixed-grid i4 regression completes and
decodes construction, active, empty, and other solver time without the prior
2,330-pass measurement explosion. Ordinary complete-step timing is still a
separate unchanged graph. The clean publishable rerun remains pending.

Measured factorial checkpoint, 2026-07-16: the clean
[S5.4b package](../../evidence/p5.2b-s5.4b/README.md) completes all eight
executions, seven accepted sources, four diagnostics, five churn snapshots,
and 24 GPU timing rows. Manifold forward i2, forward i4, and alternating i4
preserve every accepted structure through every exact step. Forward i2 is the
leading qualified lane at `17.685 ms`/1,337 dispatches for 1K, versus
`0.170 ms` for the fastest optimized Box2D row. Forward/alternating i1 lose the
16-stack at steps 40/22; alternating i2 transiently loses it at step 480 and
therefore does not qualify despite final recovery.

The leader's diagnostic P50 attributes `8.742 ms` to color construction,
`7.998 ms` to compaction, `4.080 ms` to empty color launches, and only
`0.393 ms` to active color solves. Every surviving unit also retains its color
through the governed churn corpus while the implementation rebuilds all
units. Both recorded conditional triggers are therefore true: implement the
stable-pair incremental-color comparator and the workgroup-per-island reserve
before renewed review. These are laboratory comparators, not authorization to
promote a solver, accept a residual band, retarget pacing, or begin S5.5.

Incremental-maintenance checkpoint, 2026-07-16: a versioned scheduling policy
now varies color maintenance independently from the V4 physical execution.
The stable-pair lane preserves compact manifold assignments across steps using
a double-buffered GPU pair-to-color table keyed by the body slots and body
generations already carried by generated contacts. The table follows a pair
when contact compaction changes its point slot. It retains a prior color only
when the complete identity matches and neither body has already retained that
color; new or conflicting units enter the same deterministic greedy allocator.
Retained, new, recolored, and removed unit counts, identity-table storage, and
its capacity are explicit evidence. No CPU readback, second world, physical
equation, color limit, or selected product default changed. Preserving colors
can preserve a different valid Gauss-Seidel order than a fresh rebuild after
churn; exact replay and governed structure, not rebuild byte-equivalence, own
that comparison.

The native forward-i2 four-stack is byte-exact with full rebuilding after
eight steps in bodies, cache, and prepared contact evidence, replays exactly,
reports every initial unit as new, and reports every final unit as retained.
The 33-manifold shared-body fixture still rejects atomically and emits zero
indirect solve work. Body generations are embedded in both generated-contact
bridge protocols so slot reuse cannot inherit an earlier body's color. This
checkpoint establishes mechanics only: stable maintenance still scans the
bounded contact storage and does not remove range compaction or topology work,
so the clean cost matrix must decide whether it helps.

Workgroup-island checkpoint, 2026-07-16: the second triggered scheduling
comparator constructs dynamic-body connected components entirely on GPU after
coloring. Fixed endpoints neither unite otherwise independent islands nor
receive solver writes. A deterministic one-invocation construction pass
assigns dense island ids, compacts manifold representatives by
`(island, color)`, publishes body/manifold occupancy, and writes one bounded
indirect workgroup count. Each dispatched island workgroup executes the
existing manifold block equations in the declared forward or reverse order,
with storage visibility and a `workgroupBarrier()` between colors. Thus each
physical solver stage uses one indirect dispatch rather than 32 per-color
dispatches; equations,
color assignments, the 32-color atomic rejection, and the selected product
execution remain unchanged.

A connected four-stack is byte-identical to per-color execution through four
steps. Two independent rectangles sharing one fixed floor form two islands,
execute concurrently without a fixed-body race, and remain byte-identical to
the per-color result. Full-rebuild/island and incremental/island policies both
replay exactly; 33 manifolds incident on one body still reject with one
zero-work island argument. Capacity evidence includes parent/body-island
maps, roots and occupancies, three bounded `body_capacity * 32` color-bin
views, compacted manifold indices, and the extra indirect triple. This is a
mechanical comparator checkpoint, not a cost or production-selection result;
the renewed factorial must measure whether serial island construction and the
dense bin views cost less than the dispatch work they replace.

Cross-host validation checkpoint, 2026-07-16: the shared launch/host contract
now names `solver-manifold-i2`, an eight-step neutral four-stack that runs the
same forward manifold-block i2 execution with full color rebuild and per-color
indirect dispatch. Scene contract v4 records the complete generated solver
execution and scheduling policy, so native/headless, browser, and Android
artifacts can prove which laboratory lane actually ran. The compact Android
marker carries lossless numeric encodings of those same versioned fields while
remaining below the log-line bound. This opt-in validation scene does not alter
`GpuSolverExecutionV4::SELECTED`, the default scenario, or any physical
setting. Native exact-step and Chrome BrowserWebGPU smokes pass; the clean AVD
and final evidence captures also pass and are retained in the renewed package.

Renewed scheduling-factorial checkpoint, 2026-07-16: the clean
[review package](../../evidence/p5.2b-s5.4b-scheduling/README.md) holds forward
manifold-block i2 fixed and crosses full versus stable-pair incremental color
maintenance with per-color indirect versus workgroup-per-island dispatch. All
four policies preserve all seven accepted structures with exact replay and
clean capacity; both bucket diagnostics pass, while every policy retains the
same step-25/29 failure on the 32/64-stack diagnostics. Scheduling therefore
does not resolve the remaining deep-chain quality limit.

Full rebuild plus per-color indirect is the measured 1K leader at `17.623 ms`,
1,337 dispatches, and 31.15 MB. Incremental/per-color costs `30.821 ms` and
34.29 MB; full/island costs `30.946 ms` and 31.38 MB despite reducing dispatches
to 98; the combined policy costs `45.741 ms` and 34.52 MB. The temporal probes
prove real incremental retention and removal accounting, but the current
serial stable scan raises color construction from `8.741` to `18.999 ms`.
Likewise island scheduling removes `4.048 ms` of empty-color work but raises
compaction from `7.993` to `26.221 ms`. Both are rejected as current production
schedules and retained as reserve implementations.

The recommendation returned to human review is forward manifold-block i2 with
full rebuild/per-color scheduling. It narrowly misses 16.67 ms and is 107.6x
slower than the fastest optimized Box2D 1K row, so this is an architecture and
quality recommendation, not a rigid performance victory or liquid-heavy
crossover claim. Point Jacobi/Jacobi-gather remains selected until review;
S5.5 remains blocked.

Human acceptance, 2026-07-16: forward manifold-block i2 with full color rebuild
and per-color indirect dispatch is the accepted product integration baseline.
Incremental maintenance and workgroup-island dispatch remain measured reserves.
This accepts the solver family and present scheduling baseline; it does not
accept the 32/64-stack diagnostic failures, a residual/tolerance change, the
current 1K cost, or a liquid-heavy crossover claim.

The measured scaling also changes the status of scheduler construction.
Between 256 and 1,024 bodies the accepted GPU lane adds about `11.84 us` per
body-step. Depending on whether Box2D workers are held constant or chosen per
count, that marginal gap is roughly 55x to 106x. Diagnostic construction and
compaction dominate the identified work, although their instrumented pass
boundaries mean their times are attribution rather than a literal percentage
of the ordinary graph. The current crossover premise is therefore
unsatisfied: parallel color construction and byte-stable compaction are
required S5.4c work, not discretionary tuning.

S5.4c first preserves assignments and within-color order while parallelizing
counts, prefix placement, and compaction. Only if serial greedy assignment
remains material may a deterministic speculative/conflict-repair coloring
comparator be added. Any changed assignment changes Gauss-Seidel order and
requires full quality evidence plus human review before promotion. S5.4c may
run beside the independent S5.5 broadphase bakeoff; neither result can by
itself establish a scale advantage.

Product-integration checkpoint, 2026-07-16: `GpuSolverExecutionV4::SELECTED`
now names colored in-place accumulation, manifold-block normals, compact
per-color indirect dispatch, manifold units, stable forward order, warm start,
and relaxation. Shared P5.2 generated-contact product scenarios fix two solver
iterations; the older S1.6 authored-contact point-Jacobi i1 lane remains an
explicit laboratory control rather than an alternate product engine. Normal
product construction and the cross-host `solver-manifold-i2` validation scene
use the same selected graph and standard full-rebuild/per-color scheduling.
Scene-contract v4 already carries this complete identity, so no schema fork or
host-local selection was added.

The promotion changes a physical product default from point-Jacobi i1 to the
human-accepted manifold-block i2 lane. Exact native replay and capacity tests
pass; 2/4/8 centered rectangle stacks now all preserve their structural gate
through 1,200 steps. This integration does not accept the measured 17.623 ms
1K cost, the 32/64-stack diagnostic failures, a residual band, or a liquid
crossover. Fresh Metal headless, Chrome BrowserWebGPU, and hardware-backed
`jstorrent-tablet` AVD validation pass; no physical device was attached. The
serial selected lane remains the byte-exact S5.4c control.

Scheduler-contract checkpoint, 2026-07-16: work-vector v1 hashes both the
accepted assignment and stable compacted order while recording the actual
slot ranges visited by the serial passes. The shared benchmark runner applies
that contract to generated 32/256/1K complete steps and authored
1K/10K-contact solver steps, retains raw ordinary and phase samples, and emits
marginal slopes. A candidate cannot enter the matrix without exact replay,
coherent compact ranges, and clean capacity. This establishes the comparison
contract only; it does not change assignment, compaction, or product order.

Byte-exact compaction checkpoint, 2026-07-16: an unselected scheduling policy
now replaces the serial stable compactor with four portable passes. Per-block
color counts, per-color block prefixes, one 32-lane global range scan, and
stable scatter ranks reproduce the accepted assignment and within-color order
byte for byte. The original serial greedy assignment remains the only color
chooser, and the selected product policy remains `full-rebuild`. Exact stack,
pyramid, bucket, authored-chain, replay, and overflow tests pass. Initial dirty
Metal smoke is materially faster, but robust ordinary-graph slope evidence is
still required before deciding whether the remaining serial assignment
justifies an order-changing parallel-coloring comparator.

Clean compaction measurement, 2026-07-16: 21 retained Metal samples at clean
revision `42c5bff` confirm that the byte-exact candidate reduces generated 1K
ordinary cost from `17.575` to `12.347 ms` and the 256-to-1K marginal slope
from `10.691` to `5.355 us/body`. All five assignment and compacted-order
digests match. The 10K authored row improves by 24.6%; the 1K authored row is
13.9% slower because three added dispatches dominate that small scope. The
remaining serial greedy construction measures `9.618 ms` in the separately
instrumented candidate 1K row, while compaction is `0.098 ms`. The accepted
trigger therefore authorizes a bounded deterministic parallel-coloring
comparator. Because that comparator can change Gauss-Seidel order, it remains
unpromoted until full quality evidence and human review. The product policy is
still the serial `full-rebuild` control.

Parallel-coloring implementation checkpoint, 2026-07-16: the authorized
unselected comparator uses four stable-identity-seeded initial colors, bounded
speculative proposals, per-body/color atomic-min winners, and deterministic
conflict repair. Thirty-two fixed rounds provide a portable upper bound; a
final GPU pass rejects any uncolored unit, while the existing status-gated
parallel prefix zeros all solve dispatches on failure. Scheduler work-vector v2
records the 98 construction dispatches, winner-slot visits, rounds used,
conflict retries, and final uncolored units. Focused stack, pyramid, sparse,
replay, disjointness, and 33-color overflow tests pass. Because assignments and
Gauss-Seidel order differ from the accepted serial control, this implementation
is a comparator only and cannot be promoted without the full quality matrix and
human review.

Cross-host comparator checkpoint, 2026-07-16: the shared launch contract now
owns a validation-only `solver-parallel-coloring` scene over the same neutral
four-stack and selected manifold-block execution as `solver-manifold-i2`.
Engine construction chooses the comparator scheduling policy explicitly, and
scene completion rejects a policy mismatch or capacity failure. Native Metal,
Chrome BrowserWebGPU, and the hardware-backed `jstorrent-tablet` AVD all pass
the actual comparator path. The scene is excluded from the ordinary product
menu and does not alter `GpuSolverSchedulingPolicyV1::STANDARD`; no physical
device was attached for the attempted lane.

S5.4c clean review checkpoint, 2026-07-16: the
[parallel-scheduler package](../../evidence/p5.2b-s5.4c-parallel-scheduling/README.md)
holds forward manifold-block i2 fixed and compares serial full rebuild,
byte-exact parallel compaction, and bounded speculative parallel coloring. All
three preserve the seven accepted stack/pyramid structures, both 100-body
buckets, exact replay, and capacity. The speculative lane delays but does not
resolve the known 32/64-stack failures, reaching steps 27/40 versus 25/29 for
the controls. Its assignment and compacted-order digests differ on all five
scaling rows, making the changed Gauss-Seidel order explicit rather than a
silent implementation substitution.

The candidate finishes generated sparse rows in one bounded round and authored
1K/10K chains in four, with zero uncolored units. The permanent 33-color case
atomically zeros solve work for serial, parallel-compaction, and speculative
policies. Native Metal, BrowserWebGPU, and hardware-backed AVD execution pass.
At generated 1K the quality factorial records `8.780 ms`; the separate scaling
runner records `8.781 ms`, versus `16.794 ms` serial. The recommendation is to
make the bounded parallel scheduler the next product scheduling baseline while
retaining the same solver family and physical settings. Because assignment and
order change, `STANDARD` remains serial until human acceptance.

Human acceptance and S6.1 integration checkpoint, 2026-07-17: review accepted
the S5.4c recommendation. Product generated-contact constructors now choose the
bounded parallel policy explicitly, while the historical `STANDARD` constant
continues to name the byte-equivalent serial control used by earlier
factorials. The named shared scenes expose selected and serial policies without
creating alternate engine state or host-owned configuration. This changes
construction/order only: manifold-block i2, physical coefficients, substeps,
iterations, tolerances, capacity rejection, and oracle precedence are held
fixed. Clean cross-host promotion evidence remains the completion gate.

S6.1 completion, 2026-07-17: the clean
[promotion package](../../evidence/p5.2b-s6.1-scheduler-promotion/README.md)
pins both policy digests and the prior S5.4c qualification digest. Native
Metal, BrowserWebGPU, and hardware-backed AVD product/control scenes share the
same scenario hash and manifold-block i2 execution, remain capacity-clean, and
emit success sentinels. The attached Pixel 9 was locked and is recorded as an
availability gap. Product selection is now bounded parallel; serial is the
explicit direct control rather than an implicit former default.

Small-world dispatch checkpoint, 2026-07-18: physical Pixel 7a bootstrap
measurement closed the S6.1 device gap and fired this topic's explicit
dispatch-bound reconsideration trigger. The selected per-color graph presented
the two-body bootstrap at only `13.51 FPS` with markers off (`74.03 ms` mean,
`94.83 ms` P95) while the shared free-flight control held `60.07 FPS`. Turning
all frame markers on changed the result only to `13.31 FPS`, rejecting
reporting as the primary cause. The fixed 32-color stage sequences, not live
contact work, dominated the small scene.

The already qualified workgroup-per-island reserve is now selected only for
declared body capacities at or below 64. On the same Pixel it reduces bootstrap
from 1,434 to 195 dispatches and from 1,573,916 to 1,290,800 allocated solver
bytes; the maintained pacing lane reports `60.06 FPS` presentation, `16.75 ms`
P95, `59.38` fixed steps/s, and `0.990` effective scale. A same-Metal diagnostic
moved generated 32-body GPU P50/P95 from the prior clean `6.069/6.627 ms` to
`1.893/2.073 ms`. The reserve is deliberately not global: a diagnostic 1K run
regressed from the prior `8.781/8.901 ms` to `21.884/21.901 ms`, so larger
capacities retain selected per-color indirect dispatch. Direct tests prove
body, cache, and work equality between the two dispatch forms. Solver
equations, color assignment/order, fixed `dt`, physical coefficients,
tolerances, and capacity rejection are unchanged.

## Fallback and reserve options

- **Accepted baseline.** Manifold block i2 improves structural preservation and
  measured 1K cost over the provisional point-Jacobi execution, but it does not
  dominate every residual-motion metric. Human review accepted this exact
  physical-default change; later scheduler work may not silently change its
  assignment/order or quality contract.
- **Workgroup-per-island sequential solve.** The bounded S5.4b comparator now
  implements this reserve for dynamic contact islands using one workgroup and
  barriers between color phases. Its current deterministic island builder is
  intentionally attributable rather than presumed scalable; renewed quality,
  memory, and timing evidence decides whether the reserve is useful.
- **Not portable, rejected:** persistent/cooperative kernels with device-wide
  sync and driver-generated command techniques (CUDA graphs, device-generated
  commands) do not exist in portable WebGPU.
- **Pacing retarget with render interpolation.** If the measured floor misses
  16.67 ms on a required host, a lower fixed outer rate plus interpolated
  presentation is a product option — but display rate is not the cost driver;
  substeps per second are. Halving the outer rate while preserving substep dt
  doubles per-frame solve work along with the budget, amortizing only the
  once-per-frame stages; real savings require a larger substep dt, which
  trades back convergence. Interpolated presentation is GPU-friendly (body
  states are already double-buffered) but adds up to one outer step of input
  latency, which lands on grab feel. This decision is owned by the
  simulation-time-and-overload contract, is not authorized here, and should
  not be taken before the S5.4b floor exists. The existing no-catch-up slip
  already degrades overload gracefully without a retarget.

## Anticipated end-state (exploratory)

One world with per-constraint-family schedules: colored sequential solving
for rigid contacts, relaxed Jacobi-style iteration for later particle/liquid
and coupling constraints, which tolerate it well (the FleX precedent chose
Jacobi for everything and accepted soft rigid stacking; PhysBox 5 does not).
The product bar comes from the PhysBox 3 oracle, which is built on Box2D:
crisp stacking is required sandbox behavior, and a structure that collapses
at feasible iteration budgets is a product regression, not an aesthetic
trade. This section is exploratory context for P5.6 planning, not a decision;
liquid remains downstream of accepted rigid quality.

## Prior art and references

Pinned checkouts (see `reference/pins.toml`; source material only —
independent implementation, no copying without provenance):

- **PhysX 5 GPU solver** (`reference/physx`, revision `b4b286a`):
  `physx/source/gpusolver/` — `PxgConstraintPartition` is graph coloring
  under another name (32 partitions per slab, `PX_PARTITION_COMPACTION`,
  incremental `addEdge`/`removeEdge` maintenance under churn — the model for
  our recoloring-cost question); `PxgTGSCudaSolverCore` is substepped
  Gauss-Seidel on GPU. PhysX targets large scenes and leaves small scenes on
  the CPU; its CUDA affordances (cooperative grid sync, CUDA graphs) have no
  portable WebGPU equivalent.
- **Box2D v3** (`reference/box2d`, revision `3a4f0da`): CPU constraint-graph
  coloring for worker-thread/SIMD parallelism with a serial overflow lane.
  Box2D colors for speed and would be stable solved serially; PhysBox colors
  for correctness. Its TGS Soft substepping is the accepted behavior target.
- **wgmath** (`reference/wgmath`, revision `ac89dce`): the closest prior art
  under identical portable-WebGPU constraints; read for colored-pass
  scheduling before S5.4b implementation.

Literature (conclusions replicated by S5.4/S5.4a evidence):

- Tonge, Benevolenski, Voroshilov, *Mass Splitting for Jitter-Free Parallel
  Rigid Body Simulation*, SIGGRAPH 2012 — Jacobi damping that guarantees
  convergence; measured here as too slow for deep chains.
- Harada, *A Parallel Constraint Solver for a Rigid Body Simulation*, 2011
  (Bullet 3 GPU lineage) — batched/colored Gauss-Seidel on GPU.
- Fratarcangeli, Tibaldo, Pellacini, *Vivace: a Practical Gauss-Seidel Method
  for Stable Soft Body Dynamics*, SIGGRAPH Asia 2016 — fast randomized graph
  coloring for parallel Gauss-Seidel.
- Macklin, Storey, Lu, Terdiman, Chentanez, Jeschke, Müller, *Small Steps in
  Physics Simulation*, SCA 2019 — substeps beat iterations; the basis of TGS
  and the reason the substep budget, not display rate, is the cost driver.
- Macklin, Müller, Chentanez, Kim, *Unified Particle Physics for Real-Time
  Applications*, SIGGRAPH 2014 (FleX) — the deliberate all-Jacobi design and
  its known-soft rigid stacking; the road not taken for rigid contacts.
- Catto, Box2D v2 block solver — the 2x2 coupled manifold normal solve
  validated here as the manifold block family.

## Reconsideration triggers

Revisit this topic's direction only with fresh governed evidence showing:

- compact scheduling still spends material time rebuilding colors — authorize
  a stable-pair incremental-coloring comparator under deterministic churn;
- the optimized colored lane remains dispatch-bound or misses the 16.67 ms
  feasibility boundary on a required host — evaluate the workgroup-per-island
  reserve before any pacing retarget under the simulation-time-and-overload
  contract;
- bucket or other high-degree scenes push color counts past the bounded pass
  budget, or recoloring under construction churn dominates frame cost —
  revisit coloring granularity and PhysX-style incremental maintenance;
- portable WebGPU gains device-scope synchronization or device-generated
  commands — recompute the dispatch-floor arithmetic;
- P5.6 liquid coupling requires an accumulation architecture incompatible
  with colored rigid contacts — renewed human architecture decision;
- a Soft-TGS or disjoint-batch formulation matches colored structure at
  materially lower measured cost; or
- a genuine invariant-versus-Box2D conflict requires authority review under
  the declared oracle precedence.

Do not reconsider by widening tolerances, accepting toppled or transient
structural failures, hiding unused color passes, or extrapolating rigid
small-fixture timings to liquid-heavy scenes.
