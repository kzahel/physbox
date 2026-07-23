# Bring-up plan

The milestone order is a dependency graph. Later product breadth cannot hide a
weak solver, incomplete pair generation, or host synchronization.
Performance evidence follows the
[performance and scale contract](topics/performance-and-scale.md) from the
solver laboratory onward; P5.4 is the first complete rigid-engine claim, not
the first time scale is measured. Interactive scheduling follows the
[simulation time and overload contract](topics/simulation-time-and-overload.md):
fixed `dt`, at most one outer step per engine frame, and permanent wall-time
slip instead of catch-up.

The compact [product capability ledger](topics/product-capability-status.md)
records what is implemented, limited, missing, and deferred without requiring
the historical tacticals to be reconstructed. S20 owns the validation diet;
S21 owns the now-complete P5.3 broadphase bakeoff, winner integration, and
physical-Android qualification. S23 has reached its manual promotion checkpoint
with opt-in effective sleeping, coordinated bounded-neighbor admission/wake,
awake-driven solve work, and matched sleeping-city evidence. S24's current-
default native discriminator passes 2/4/8 but the upright 12-stack misses the
awake quiet band. The authorized S25 follow-up is now complete: 60/8/i1 passes
the native 2/4/8/12 gate, preserves 14, reports 16 loss at step 840, and has
matching Browser WebGPU/physical-Pixel identities. The equation, bands,
sleeping policy, and 60/4/i1 product default remain fixed. Human review
declined the costlier alternative and accepts an 8-box awake quality claim
plus demonstrated 14-box structural survival as the current limitation.

The [current rigid quality target](topics/rigid-quality-targets.md) supersedes
the historical 16-stack promotion gate for new work: 2/4/8 remain the
regression foundation, 12 is the blocking awake product target, 14 is stretch,
and 16 is a non-blocking matched-Box2D frontier diagnostic. Historical
tacticals retain their original gates and results.

Active cross-milestone execution from the remaining P5.2/S4 closure through
P5.4 is tracked by
[S5 interactive rigid quality, construction, and scale](tactical/005-interactive-rigid-quality-and-scale.md).
That tactical orders easy-stack and bucket fixtures, the selected combined
contact graph, circle/rectangle authoring, settling review, broadphase
selection, scalable buckets, and sleeping without changing the milestone
dependencies below.

The selected physics direction and configuration migration are tracked by
[S19 soft default and physics configuration](tactical/019-soft-default-and-physics-configuration.md).
S19 promotes the interactively preferred soft-coupled family into the one
ordinary shared path and removes hardcoded fixed-rate/resolution assumptions
before broadphase or sleeping can normalize the old hard trajectory as the
continuing product baseline.

## P5.0 — lean shared-host bootstrap (complete)

- Fresh Git history and PhysBox 4 provenance.
- One GPU-resident bootstrap body, one compute step, and direct rendering.
- One `EngineHost::render_frame` path across desktop, web, Android, headless.
- Minimal launch contract and shared bootstrap scenario.
- Fresh artifacts, boundary tests, and exact-pushed-SHA deployment.
- No Rapier, liquid, product tools, settings, or old scene compatibility.

Exit: all locally available gates pass, Wrangler dry-run passes, hook is
installed, and the tree contains only the retained platform rim.

## P5.0a — oracle foundation and solver laboratory (complete 2026-07-15)

Completed execution tracker:
[S1 rigid solver laboratory](tactical/001-rigid-solver-laboratory.md).

1. Replace the bootstrap eight-step catch-up accumulator with the shared
   slow-clock/explicit-step modes, diagnostics, and timing-sequence tests.
2. Define versioned body, authored-manifold, scenario, trace, metric, capacity,
   and failure schemas.
3. Add scalar Rust contact preparation and solve specification.
4. Add the scoped analytic fixture contact generator so multi-step fixtures
   can regenerate contacts; fixture geometries only, checked against Box2D
   public collision functions, replaced by P5.2 and never promoted.
5. Add a process-isolated native, pinned, single-threaded Box2D authority
   executable and direct public narrowphase harness; keep both outside the
   product dependency graph.
6. Add invariant/metamorphic generators and failure minimization.
7. Implement GPU Jacobi/gather and graph-colored Soft-TGS against precomputed
   manifolds; optionally add one XPBD/split comparator.
8. Benchmark the shared prefix-scan, radix-sort, unique, and compaction
   primitives on named adapters; their measured cost is a solver-selection
   input.
9. Record the numerical, cross-adapter, capacity, and 1K/10K/30K authored-contact
   performance matrix required by the scale contract, including at least one
   measurement on the named Android reference target and any explicit physical
   device availability gap.
10. Select the solver family and freeze its first checked baselines through
    one explicit human review.

Exit: exact/tightly bounded scalar pass comparisons, behavior matrix, no
readback dependency, no overflow, no interactive multi-step catch-up, and a
documented solver decision. Do not build a production broadphase before this
exit. PhysBox 3 browser automation, LiquidFun, and a Rapier2D comparator are
not part of this stage.

Exit decision: human review selected contact-centric Jacobi with
equal-and-opposite records and GPU per-body gather. The starting checked policy
is fixed `dt` 1/60, four substeps, one iteration, bias rate 0.2, mass
coefficient 1.0, impulse coefficient 0.0, and restitution threshold 1.0.
Colored Soft-TGS remains a convergence comparator. The full evidence and
reconsideration triggers are in the
[S1.6 review package](../evidence/p5.0a-s1.5/README.md).

## P5.1 — minimal rigid state and fixed-floor contact (complete 2026-07-15)

Execution tracker: [S2 GPU body state and fixed-floor contact](tactical/002-gpu-body-state.md).

Completion, 2026-07-15: S2.1 through S2.5 are complete. The
generational body allocation, direct rendering, and selected fixed-floor
Jacobi/gather solve have fresh native, browser, AVD, physical Android, scalar,
and Box2D evidence. Bounded reset/grab behavior also has exact replay and
clean cross-host evidence. Clean 1K/10K/100K scale curves retain complete
state, work, memory, and capacity validation on all four target lanes. Human
review accepted the S2.6 recommendation: the limited 64-byte `BodyCoreV1`,
generational handle, existing 48-byte command record, and explicit
capacity-failure semantics are frozen as the P5.2 baseline. Scratch meanings,
numeric capacities, the hold scan, and all future shape/contact/joint/liquid
buffers remain unfrozen. The
[decision package](../evidence/p5.1-s2.6/README.md) contains the exact scope and
direct evidence links.

- Replace bootstrap state with generational SoA body storage and bounded GPU
  commands.
- Keep one analytic rectangle plus fixed floor and direct rendering.
- Exercise the selected solver with a precomputed floor manifold.
- Add reset and engine-owned neutral grab actions.
- Prove the same scenario and scene contract on every host.
- Record bytes per body and free-flight integration/rendering curves at 1K,
  10K, and 100K on the named reference adapters, including Android, before
  freezing the body layout.

Exit decision: human acceptance closed P5.1 and authorized the P5.2 tactical.
The accepted layout is the dynamic body core, not a frozen universal
body-plus-shape-plus-joint allocation.

## P5.2 — small-world collision and persistence (active)

Execution tracker:
[S3 small-world collision and contact persistence](tactical/003-small-world-collision-persistence.md).

Current checkpoint, 2026-07-15: human review accepted the renewed S3.6
direction. Stable generational pair slots and compact stable body-edge index
views with segmented gather are selected for product integration; the
full-key sort and canonical incidence scan remain correctness baselines. The
measured `0.02` speculative distance and `3.0 m/s` per-contact depenetration
limit are provisional P5.2 defaults while deep coincident spawn remains
diagnosed. The first post-selection slice must integrate and revalidate both
representation views and publish structural stack curves. S3 closure and any
CPU crossover claim remain gated on that combined evidence.

S5.4 awake-quality checkpoint, 2026-07-16: selected pair-slot plus stable-edge
integration and bounded GPU construction are complete. Nine reversible
executions have now been compared on six awake stack/pyramid sources against
clean awake Box2D. Every GPU candidate loses the 8-body stack while Box2D
preserves it, and additional substeps/iterations are non-monotonic. The
[review package](../evidence/p5.2b-s5.4/README.md) preserves all first failures,
complete cost, synchronized playback, and platform evidence. No candidate is
promoted. Human review must choose whether to authorize the recommended
manifold-coupled two-point laboratory experiment (and possible colored/in-place
comparison) before P5.3 broadphase work begins.

Human follow-up decision, 2026-07-16: authorize a laboratory-only factorial
comparison of current point Jacobi, scalar mass splitting, coupled two-point
manifold normal blocks, and block mass splitting at 1/2/4/8 iterations. Add a
generated-contact colored/in-place comparator only if those candidates cannot
preserve the existing awake corpus monotonically inside the fixed 60 Hz
feasibility boundary. This does not promote a solver or unblock P5.3; publish a
renewed package and return to human review.

S5.4a renewed checkpoint, 2026-07-16: the clean
[parallel-normal package](../evidence/p5.2b-s5.4a/README.md) evaluates all 20
authorized point-Jacobi, scalar-mass-split, manifold-block, combined, and
colored/in-place executions. Only colored i4/i8 preserve all seven structures,
including the 16-stack, but their fixed 32-pass graphs cost `19.308`/`28.042
ms` at 1K and every family still violates the declared strict monotonic gate.
No solver is promoted. Human review must decide whether to authorize an
optimized GPU-owned manifold-color/block lane; P5.3 remains blocked.

S5.4b authorization, 2026-07-16: human review authorized an attributable
compact-coloring factorial: a byte-exact scheduling-only point-color control,
manifold-colored active-set block solving, and forward versus alternating
color order. The seven accepted scenes remain the quality gate; 32/64 stacks,
100-body buckets, and construction churn are added as diagnostics. Incremental
coloring or workgroup-per-island solving is conditional on measured rebuild or
dispatch cost. No solver promotion, residual band, pacing change, or P5.3 work
is authorized.

S5.4b scheduling checkpoint, 2026-07-16: the compact-indirect point-color
control is implemented with explicit occupancy, compaction, indirect-work,
capacity, and overflow evidence. It is byte-identical to the fixed-grid
control on the eight-body/eight-step/i2 native replay. No stage fusion met the
required exact-order gate. Manifold-colored active-set block solving is the
next authorized slice; product selection remains unchanged.

S5.4b manifold checkpoint, 2026-07-16: compact manifold coloring now keeps two
contact points in one disjoint unit and runs the validated active-set normal
block. Exact replay, scalar one-point degeneration, matrix/active-set evidence,
and zero-work atomic color overflow pass on native Metal. Deterministic
forward-versus-alternating order is next; no product solver is promoted.

S5.4b ordering checkpoint, 2026-07-16: stable forward and deterministic
alternating manifold-color sweeps now share one compact scheduler. The global
substep/iteration parity covers i1/i2/i4 without readback, replays exactly, and
is not a silent forward-order alias. The depth/degree/churn and complete cost
matrix is next; product selection remains unchanged.

S5.4b cost-instrumentation checkpoint, 2026-07-16: greedy coloring and stable
compaction now have distinct GPU pass/timestamp identities. Complete-step
timing remains an ordinary graph measurement; a separate diagnostic lane
attributes construction, compaction, active solve, and empty indirect work.
The clean depth/degree/churn factorial remains next and no solver is promoted.

S5.4b automation checkpoint, 2026-07-16: the atomic evidence runner now owns
the eight-lane factorial, seven accepted structures, four separately labeled
depth/degree diagnostics, deterministic construction/erase recoloring source,
raw cost matrix, first failures, Box2D comparison, and conditional comparator
triggers. A clean package run remains next; no evidence result or solver
promotion is implied by the automation itself.

S5.4b renewed review checkpoint, 2026-07-16: the clean
[scheduling package](../evidence/p5.2b-s5.4b-scheduling/README.md) crosses both
conditional scheduling factors around the qualified forward manifold-block i2
execution. Every policy preserves all seven accepted structures with exact
replay and clean capacity. Full rebuild plus per-color indirect leads at
`17.623 ms` for 1K; incremental/per-color costs `30.821 ms`, full/island
`30.946 ms`, and their combination `45.741 ms`. The recommendation is to
accept the full/per-color forward manifold-block path as the next integration
baseline while retaining the other policies as reserves. This is the mandatory
human checkpoint: point Jacobi/Jacobi-gather remains selected and P5.3 remains
blocked until review.

Human S5.4 acceptance, 2026-07-16: integrate forward manifold-block i2 with
full rebuild/per-color indirect scheduling as the product baseline. The
current 17.623 ms 1K result and 32/64-stack diagnostic failures are not accepted
quality or performance bands. Because the measured 256-to-1K marginal cost is
roughly two orders of magnitude worse than best-per-count Box2D, parallel
scheduler construction/compaction is required S5.4c work and may proceed beside
the now-authorized P5.3 broadphase bakeoff. Stop before selecting a parallel
coloring implementation that changes Gauss-Seidel order and before selecting a
product broadphase.

S5.4 integration checkpoint, 2026-07-16: forward manifold-block i2 with full
rebuild/per-color indirect dispatch is now the selected P5.2 product graph.
Shared generated-contact scenarios fix two iterations and publish the exact
execution through scene contract v4; the S1.6 authored point-Jacobi i1 lane is
retained only as an explicit validation control. Native exact replay,
capacity, and 2/4/8-stack structural tests pass. Cross-host validation is part
of the same reviewable integration slice. The next unattended work is the
S5.4c scheduler benchmark/byte-exact parallel compaction and the independent
S5.5 SAP/grid bakeoff, stopping before either order-changing coloring or a
product broadphase is selected.

S5.4c renewed review checkpoint, 2026-07-16: the clean
[parallel-scheduler package](../evidence/p5.2b-s5.4c-parallel-scheduling/README.md)
compares the serial control, byte-exact parallel compaction, and bounded
speculative parallel coloring. All three preserve the seven accepted
structures, both bucket diagnostics, exact replay, and capacity. The parallel
candidate explicitly changes color/order, retains no uncolored fallback, and
leads at `8.780 ms` for the quality-factorial 1K row. Dedicated scheduler
scaling reports `8.781 ms` at generated 1K and `7.535 ms` for authored 10K
contacts. This is still 50.8x optimized Box2D at 1K with a roughly 19.8x worse
marginal curve, so it is a scheduler recommendation rather than a crossover.
The serial product policy remains selected pending the required human review;
no broadphase, residual band, or liquid claim is accepted.

S6.1 scheduler integration checkpoint, 2026-07-17: human review accepted the
bounded parallel recommendation. Product generated-contact constructors now
select that schedule explicitly; the named manifold-i2 validation scene keeps
the serial full-rebuild/per-color control. Solver equations and physical
settings are unchanged. Clean native, browser, hardware-backed AVD, and
available-device qualification remains before S6.1 closes, after which S6.2
freezes realistic settling observations and fixtures.

S6.1 completion, 2026-07-17: the clean
[promotion package](../evidence/p5.2b-s6.1-scheduler-promotion/README.md)
qualifies selected bounded parallel scheduling and the serial direct control on
native Metal, BrowserWebGPU, and the hardware-backed AVD with one scene hash
and one solver execution. The attached physical Pixel 9 was locked and remains
an explicit availability gap. S6.2 now owns observation schemas and seeded
realistic settling fixtures; it may not change contact policy.

S6.1a pacing checkpoint, 2026-07-18: the unlocked Pixel 7a closed that gap and
exposed the selected per-color dispatch floor at `13.51 FPS` on the two-body
bootstrap. Product construction now selects the exact-equivalent
workgroup-per-island reserve only for the 64-body interactive capacity tier;
larger capacities retain the scalable per-color path. A maintained physical
lane gates both rendering cadence and fixed-step scale. Bootstrap passes at
`60.06 FPS` and `0.990` effective scale with no fixed-delta, quality, or
physics-tolerance change.

The first S3.5a representation checkpoint implements the stable pair-slot
comparator with full feature verification and deterministic generational slot
recycling. Its correctness/adversarial gates pass and its combined
collision/persistence schedule is 25 rather than 212 dispatches. It remains an
isolated bakeoff lane pending the body-edge comparator, scale evidence, and
renewed S3.6 review; the product path and contact ABI are not yet changed.

The second representation checkpoint adds the stable body-edge index/range
comparator to the generated-contact solver. It preserves canonical
equal-and-opposite contributions and byte-exact body/cache outcomes while
removing the candidate's per-body scan across every point-capacity slot. The
baseline remains selected because the comparator adds radix dispatches and
index memory; contact-rich measurements and the combined matrix must resolve
that tradeoff before S3.6 review.

The evidence-automation checkpoint adds a single fresh atomic command for the
complete S3.5a matrix. It consumes the existing generated scenarios and shared
authored-chain source, retains representative versus instrumented timing
lanes, and requires exact replay/capacity qualification. It records no
representation selection and leaves the product baseline unchanged until the
clean native and cross-host artifacts are published for renewed S3.6 review.

The clean S3.5a package is now published from revision `27a0e8c`. Stable
pair-slot persistence is exact and reduces its isolated stage P50 by 68x to
416x across the governed generated counts. Stable body-edge views are exact,
improve the complete generated graph by 1.04x to 1.38x, and improve isolated
1K/10K contact-rich rows by 1.54x/2.28x at about 6% total-allocation overhead.
The approved physical-policy candidate materially mitigates both minimized
failures but retains visible residual response. Native, browser, and the
hardware-accelerated AVD pass; no physical device was connected. Because the
pair and edge timings are not yet one combined product graph, the package
makes no CPU crossover claim and returns to S3.6 for human selection.

- Add circles and oriented rectangles.
- Use all-pairs AABB candidate generation.
- Implement analytic/SAT/clipped manifolds and direct Box2D comparison.
- Add sorted persistent contact keys, warm starting, friction, restitution,
  speculative separation, and explicit capacity diagnostics.
- Produce synchronized oracle/GPU failure playback.

## P5.2a — interactive evaluation workbench (authorized)

Execution tracker:
[S4 interactive evaluation workbench](tactical/004-basic-scene-navigation-camera.md).

Human review authorized camera, UI, diagnostics, and anchored-grab work after
S3.6 direction acceptance so subjective rigid feedback is practical before
deeper tuning. The minimal place/erase slice waits for the combined S3 pair
integration because deletion and generational slot reuse must invalidate the
selected persistent cache. Complete both this checkpoint and the separate S3
closure gate before P5.3.

Keep the browser a fullscreen canvas: the shared engine renders the toolbar,
menu, glyph diagnostics, and tether, and owns scene selection, camera state,
gesture arbitration, tools, GPU hit requests, construction commands, and
pointer-to-world conversion. Platform adapters transport raw mouse, touch,
wheel, lifecycle, and resize facts only.

S4.3 automated implementation and evidence are complete. Camera and input-trace
contracts are versioned in shared Rust; right-drag pan, cursor-anchored wheel
zoom, two-touch pan/pinch, pointer
loss, resize, and all-contacts-up rearming replay deterministically. The shared
engine now renders a bounded glyph toolbar, responsive scenario menu, restart,
recenter, and an honestly labeled declared-fact overlay in one UI pass and two
draws. Scenario activation atomically replaces the world without stale state.
Headless, desktop/phone browser, and hardware-accelerated AVD captures pass; no
physical Android device was connected. GPU selection now consumes versioned
body-free intents, retains the selected generation and body-local anchor on
device, and applies a sparse compliant point constraint without teleporting or
disabling collision. Tether geometry is compute-prepared into a 192-byte GPU
vertex buffer only while held. Twenty-one exact GPU replays and 21 clean pinned
Box2D public motor-joint processes pass. Deployed human review accepts the
camera, scene discovery, and current anchored-grab feel as the continuing
workbench baseline. The values remain reconsiderable rather than universal
final defaults. S4.4 bounded GPU construction now passes clean exact
lifecycle, capacity, browser, and hardware-backed AVD evidence through S5.3.
Construction feel remains part of the combined S5.4 human checkpoint.

- Make the maintained shared scenarios discoverable from the default deployed
  canvas without requiring query-string editing.
- Make one empty-world touch inert; use two touches for pan/pinch, right mouse
  drag for pan, wheel for cursor-anchored zoom, and primary input for tools.
- Add GPU-selected body-local anchored grab with a visible tether and collisions
  retained during dragging.
- Add an efficient engine-rendered glyph overlay with honest pacing, body,
  contact, capacity, overflow, adapter, and asynchronous-sample-age facts.
- Add a minimal toolbar for Grab, Circle, Rectangle, and Erase after the
  selected pair-cache generation-invalidation gate passes.
- Preserve direct URL/argument launches for validation and deep links.
- Prove that camera/UI state does not affect physics, pacing, scenario hashes,
  or cross-host outcomes, and that construction owns no CPU body mirror.

## P5.2b — soft default and configurable product physics (complete 2026-07-21)

Human review on 2026-07-21 selects the soft-coupled point-sequential family as
the product direction. Its initial 60 Hz/four-substep/one-biased-plus-
relaxation 30/10/staticx2 seed is much quieter and subjectively closer to
Box2D than hard/direct. Its visible tall-stack penetration and step-407
16-stack loss are accepted starting limitations, not hidden successes. Under
the later [rigid quality target](topics/rigid-quality-targets.md), that
16-stack result is diagnostic. S24's fresh native discriminator subsequently
passes 2/4/8, preserves 12/14 structurally but misses their quiet observation,
and loses 16 at step 463; browser and physical qualification were not admitted.

- Make soft-coupled the ordinary default on desktop, browser, Android, and
  headless; retain hard/direct as an explicit comparison only.
- Give fixed rate, substeps, family-specific iterations, and contact response
  one bounded versioned configuration owned by shared launch/engine code.
- Parameterize the shared clock and soft GPU pass graph while preserving
  zero-or-one interactive outer steps, permanent slip, GPU residency,
  portability, replay, and fail-closed capacity.
- Expose named presets and advanced values through the engine-rendered menu,
  with requested/effective values, internal rate, cost, and pacing visible.
- Characterize ordinary structures and the known tall-stack limitation across
  Metal, browser, and physical Android; match Box2D configuration whenever a
  parity claim is made.

Exit: the exact deployed ordinary build selects soft-coupled from a fresh
launch; all hosts project the same configuration; supported rate/resolution/
contact settings replay and report honestly; hard/direct never activates as a
fallback; and fresh response/performance artifacts plus subjective review are
recorded. Detailed ordering, measurements, and closure evidence are in S19.

## P5.3 — scalable broadphase bake-off

Execution tracker:
[S21 scalable broadphase](tactical/021-scalable-broadphase.md), after
[S20 validation diet](tactical/020-validation-diet-and-feature-ledger.md).

Selection checkpoint, 2026-07-21: clean 32-through-100K Metal and Browser
WebGPU matrices qualify both candidates, and human review selects full-sort
one-axis SAP. SAP wins every 10K/30K/100K native row with roughly one quarter
of grid's fixed evidence allocation. S21.5 now integrates SAP into the one
shared product graph with an 8N candidate bound and no automatic fallback.
Clean integrated 100/1K/10K Metal curves replay exactly and remain capacity
safe; the maintained 100-body Browser WebGPU scene sustains `0.993` effective
scale with zero slip. Physical Pixel 7a Vulkan closes S21: default bootstrap
pacing sustains `1.0003` effective scale, and the 103-body bucket executes
correctly and capacity-clean while honestly reporting `0.591` slow time.
Shared 64/256/1K/10K world-capacity presets make larger graphs explicit
opt-ins; they are not additional sustained-rate claims.

Active-work follow-up: [S22 active-work compaction](tactical/022-active-work-compaction.md)
is complete. The GPU live-body set and committed collision/persistence counts
own integration, rendering, SAP, narrowphase, pair-slot lifecycle, topology,
and solver dispatch. The durable 64/256/1K/10K probe is approximately flat for
the same sparse scene while its dense control expands the work vector.
[S23 GPU sleeping and waking](tactical/023-gpu-sleeping-and-waking.md) has now
reached its manual checkpoint: generation-safe GPU sleep state, coordinated
four-round admission/wake propagation, awake-driven solve work, the persisted
toggle, state visualization, and matched sleeping-city evidence are complete.
Effective sleeping remains opt-in and off by default; S23.7's promotion/default
decision is the next separate human checkpoint. S25's qualified eight-substep
setting was not promoted, and sleeping may not conceal the retained
four-substep awake limitation.
[S26 exact contact-component observation](tactical/026-exact-contact-component-observation.md)
is complete as a no-behavior GPU measurement. Exact labels and structural
counts match CPU through the admitted split/churn, 1K connected-pile, and 10K
reserve rows. Its 29–61 dispatch and 0.110–0.577 ms P50 upper-bound cost is
recorded. Work is stopped before component reductions or an exact-island
policy.

- Preserve brute force as pair-completeness oracle.
- Implement sorted sweep-and-prune and multi-cell grid candidates.
- Run the shared randomized/adversarial completeness corpus and applicable
  32-through-100K performance scene families.
- Choose from measured correctness, candidate amplification, memory, and GPU
  time. Add LBVH only if those measurements require it.

## P5.4 — rigid parity and performance gates

- Deliver the default-on, compile-removable
  [interactive Box2D comparison](topics/interactive-box2d-comparison.md)
  through the existing shared UI on desktop, browser, and Android. It is a
  non-authoritative shadow for visible review, never fallback or performance
  evidence.
- Resolve the completed-negative
  [S24 current rigid quality qualification](tactical/024-current-rigid-quality-qualification.md):
  the native 2/4/8 foundation passes, but the upright 12-stack exceeds the
  awake quiet band and stopped browser/physical expansion.
- Record the completed
  [S25 bounded awake configuration qualification](tactical/025-bounded-awake-configuration-qualification.md),
  whose sole 60/8/i1 finalist passes native 2/4/8/12 plus matching browser and
  physical-Android execution while exposing higher cost and the 16 diagnostic
  failure.
- Record the human decision to retain the unqualified 60/4/i1 default and
  accept its 8-quality/14-structural limitation without moving the 12-stack
  target.
- Complete the S23.7 sleeping promotion/default review over the implemented
  bounded wake propagation, coordinated admission, awake-driven solve, compact
  diagnostics, and sleeping-city evidence.
- Retain S26's completed observation-only exact contact-component correctness
  and cost report; its labels affect no sleep or solver decision.
- Qualify the selected default against the current centered-stack hierarchy:
  2/4/8 foundation, 12 blocking awake, 14 stretch, and 16 matched-Box2D
  frontier diagnostic. Sleep-enabled survival may not satisfy the awake gate.
- Accept landing, stack, pyramid, impact, friction, bounce, mass-ratio, and
  grab/release fixtures against Box2D and independent invariants.
- Publish correctness-qualified Box2D CPU/GPU crossover curves for solver-only,
  collision-only, end-to-end physics, and full-frame scopes at 32, 256, 1K,
  3K, 10K, 30K, and 100K where each scene family is applicable. Extend sparse
  and mostly-sleeping cases to 300K+ where useful.
- Track P50/P95/P99 GPU/CPU timing, dispatches, active work, amplification,
  memory high-water, every overflow, effective interactive time scale, and
  slipped wall time. A performance result is invalid when correctness or
  capacity fails; slow-time pacing cannot be reported as sustained 60 Hz
  throughput.

## P5.5 — first construction loop

- Extend the P5.2a minimal place/erase loop with selection, editable body
  properties, and deliberate shape sizing.
- Add minimal authored save/load and scene-document semantics.
- Add one joint only after free-body contact remains green.

## P5.6 — LiquidFun-oracled coupled material

- Add bounded water only after P5.4 acceptance.
- Generate fluid fixtures from the same shared scenario projection.
- Add separate native oracle executables for the pinned PhysBox 3 particle
  sidecar plus Box2D v3 and for original LiquidFun/Box2D 2.3.
- Treat the native PhysBox 3 sidecar as the coupled-behavior authority and
  original LiquidFun as a secondary algorithm/scenario reference.
- Calibrate a small canonical corpus against fresh runs of the actual PhysBox
  3 web product; do not require that browser lane for routine correctness.
- Couple particle/body response inside the same GPU fixed step, including
  translation, off-center torque, pressure/boundary effects, and wake policy.
- Compare displacement, float/sink, wave impact, and moving boundaries with
  PhysBox 3/LiquidFun.
- Prove there is no simulation-dependent fluid impulse readback.

## Autonomous development loop

For each accepted slice: select or generate a shared scenario, project it to
the required process-isolated native oracle, run invariants and scalar checks,
run authority/GPU comparison, enforce metric bands, run capacity/performance
lanes, minimize any failure, and promote its reproducer to the corpus. Every
failure artifact records run/scenario ids, source and oracle revisions,
dirty/patch digests, adapter/browser where relevant, full contract, pass
counters, structured diff, and deterministic failure reason.

Automation may repair code to restore a baseline. It may not change the solver
family, oracle precedence or revision, physical defaults, tolerance bands, or
checked baseline without an explicit measured decision.
