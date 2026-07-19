# S13: exact authority-structure GPU execution probe

Status: active, 2026-07-19. Authorized by human decision at the S12.6
checkpoint. This workstream executes exactly one unpromoted portable-WebGPU
candidate: the S12 X1-X4 authority structure with the current PhysBox X5
restitution timing. It is an execution and decision probe, not a selected
solver or product change.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns response quality and the authority-band decision;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns the
exact GPU schedule, accumulation, and synchronization contract;
[scalar contact specification](../topics/scalar-contact-specification.md) owns
the qualified scalar endpoint;
[box2d rigid authority](../topics/box2d-rigid-authority.md) owns the frozen
native authority evidence; and
[performance and scale](../topics/performance-and-scale.md) owns measured work,
allocation, and time. Predecessor:
[S12 authority solve-structure decomposition](012-authority-structure-decomposition.md).

## Human fork resolution

Human decision, 2026-07-19:

- authorize the recommended minimal exact, unpromoted GPU execution probe of
  X1-X4 with current X5 timing, exactly as scoped by the verified
  [S12.6 review](../../evidence/p5.2b-s12.6-authority-structure-review/README.md);
- compare the exact candidate to the already frozen authority bands at stack
  depths 11 and 16;
- require exact replay, fail-closed capacity, a forced-overflow fixture, and
  measured 32/256/1K work, allocation, and complete-step time;
- do not admit an approximate selected-kernel port, comparator engine, wider
  scalar sweep, coefficient sweep, or unplanned structural variation; and
- stop again for human review before cross-host qualification or product
  promotion.

This authorization does not change the selected hard/direct manifold-block i2
product, constructor, physical defaults, numerical tolerances, oracle
precedence, source pin, or response ownership. The candidate is reachable only
through validation/probe construction and must reject product selection.

## Goal

Determine whether the exact S12 sufficient measured structure retains the
frozen native Box2D authority bands when executed with portable GPU arithmetic
and a bounded source-faithful colored schedule, and report its practical work,
allocation, complete-step time, replay, and capacity behavior. A qualified
probe establishes a concrete GPU solver candidate for later human review; it
does not promote that candidate. A clean negative is equally complete when
the exact structure fails a frozen response or hard execution constraint.

## Exact candidate contract

The sole candidate ID is `authority-x1-x4-current-x5-v1`. Its ingredients are
frozen before implementation:

- **X1 — preparation, identity, and impulse transport:** prepare once per
  outer step; use stable authority vertex-pair identity and shared midpoint
  anchors; retain prepared normal, anchors, and effective masses through all
  substeps; update separation analytically from normalized incremental-
  rotation transform deltas; retain evolving accumulated impulses without an
  inter-substep prepared-seed rewind.
- **X2 — accumulation and order:** construct the source-faithful authority
  constraint graph in stable source order; reserve 20 dynamic colors, three
  static-capable high colors, and one overflow lane; solve overflow serially
  before colors and solve colors in ascending authority order; update body
  state in place inside every body-disjoint color.
- **X3 — coefficient schedule:** use the pinned capped 30 Hz / damping 10 soft
  tuple, capped at `0.125 / h`, and recompute static-contact coefficients at
  twice the capped contact Hertz.
- **X4 — biased/relaxation composition:** execute exactly one biased pass and
  one no-bias relaxation pass per substep.
- **Common recurrence:** visit manifold points sequentially for the normal
  recurrence, then visit them sequentially for friction, using unilateral
  accumulated projection and the retained prepared basis.
- **Current X5:** apply restitution after every substep using the existing
  PhysBox timing, not the authority once-after-all-substeps endpoint.

The candidate also preserves the qualified scalar endpoint's fixed `dt`, four
substeps, semi-implicit gravity, material projection, zero rolling resistance,
inactive damping/clamps, finite `f32` state, exact requested step count, and
awake-only measurement. The GPU implementation must match the source contract,
not the current selected kernel's result or schedule.

An implementation correction is allowed only when it makes code conform to
this frozen contract. Changing an ingredient, adding an empirical adjustment,
or approximating a recurrence is a new experiment and is outside S13.

## Ownership and execution constraints

- The product remains GPU-resident. The probe may use bounded final diagnostic
  readback after a headless validation run; no GPU result may be required on
  the CPU to advance a step or choose a dispatch.
- Use portable WGSL and existing command-encoding boundaries. No subgroup
  feature, device-wide synchronization, unbounded loop/dispatch, CPU fallback,
  alternate backend, or reference dependency is allowed.
- Reuse existing bounded body, manifold, point, cache, color, indirect, and
  diagnostic storage where it preserves exact semantics. Any new field or
  buffer must be versioned and included in the allocation report.
- Authority color overflow is valid scheduled work, not a capacity failure.
  It must run serially before ordinary colors. Storage, arithmetic, indirect,
  bind-limit, or diagnostic overflow is a different condition and must publish
  an explicit rejected disposition with zero accepted/committed work.
- The clean Box2D pin remains a process-isolated oracle. S13 may replay the
  bound S12 native evidence and may obtain a fresh exact oracle replay for
  provenance; it may not edit, link, or copy the reference implementation.
- Repository and platform compilation/smoke gates do not qualify the candidate
  on another host. Browser/Android physics evidence and product promotion wait
  at the final human checkpoint.

## Frozen sources and response gates

The governed sources are the exact S12 depth-11 and depth-16 stack projections:
1,200 outer steps, inclusive settled window 900-1,200, four substeps, no sleep,
complete per-step observations, and unchanged material/gravity/cadence/body
generations. The S12 two-step and permanent four-body sources may be used only
as implementation diagnostics; they cannot add a response gate or rescue a
governed failure.

Each GPU governed run passes only when all of the following hold independently
for both depths:

- source/projection identity, cadence, requested steps, body generations,
  capacity, finite state, and exact candidate identity verify;
- final structure and support topology match the bound native authority;
- settled maximum linear and angular-edge speed are no greater than twice the
  native maximum plus `2e-5`;
- final dynamic center-height sum differs from native by no more than the
  larger of `2e-4 m` and native maximum penetration;
- settled maximum penetration is no worse than native plus `2e-4 m`;
- the last/first settled-energy-quartile ratio is no greater than the larger
  of one and the native ratio, with `1e-12 + 1e-5 * native-ratio` comparison
  slack;
- settled positive-energy run is no greater than the larger of four and the
  native run; and
- the unchanged `0.025 m/s` awake product ceiling is reported independently.

These are the frozen S12 laboratory bands, neither regenerated from candidate
results nor promoted into product tolerances. Every retained GPU run is
executed twice from the same input. Canonical observation bytes, final bodies,
cache/state diagnostics, capacity disposition, color evidence, and result
must match exactly between primary and replay.

## Forced-overflow and fail-closed capacity fixtures

The exact overflow fixture authors 21 dynamic manifolds incident on one
dynamic body in stable source order, forcing the twenty-first manifold into
the authority overflow lane while the first twenty occupy dynamic colors.
The evidence must prove the expected assignments, exactly one overflow item,
overflow-before-color execution, finite accepted completion, and exact replay.

The same source is rerun with one required bounded capacity set below demand.
The rejection must identify the exhausted capacity, publish overflow/rejected
status, accept and commit zero solver work, leave no partial candidate result,
and replay exactly. Silent truncation, aliased assignment, skipped overflow
work, or an accepted partial step is a hard failure.

If authored fixture details require a different neutral shape to reach the
same exact graph degree, that is an implementation fixture correction only
when it retains 21 dynamic manifolds sharing one dynamic body and the expected
20-plus-overflow assignment. It cannot change the candidate schedule.

## Frozen 32/256/1K measurement contract

Measure the selected hard/direct product control and the exact candidate on
the same named adapter, source family, capacity, build, and invocation. The
three retained buckets have 32, 256, and 1,024 authored solver manifolds with
bounded body-disjoint repetition of the governed contact shape. For each
bucket record:

- source/body/manifold/point counts and allocated capacity;
- encoded and executed dispatches by phase, shader invocations or exact
  logical work units where invocation count is unavailable, and overflow
  work;
- candidate-specific and total GPU allocation bytes, including deltas from
  the selected control; and
- GPU complete-step time from the first candidate step command through its
  final step command, using supported timestamp queries, with CPU encode time
  reported separately.

Use five warm-up samples and 20 retained samples per configuration/bucket,
fixed invocation order alternating control/candidate by sample, and publish
every raw sample plus nearest-rank median and p95. Unsupported timestamp
queries fail the timing package rather than substituting wall-clock or CPU
time. Complete-step timing and the provisional `16.67 ms` 60 Hz reference are
reported, not used to promote or silently reject the candidate. A completing
over-budget prototype is a measured optimization concern; definitive
architectural rejection requires a reproducible capacity, device-limit, or
watchdog failure that cannot be removed without changing the frozen exact
structure.

No approximate kernel, alternative color count, coefficient variant, source
size, batching variant, or post-result optimization arm may enter this matrix.
Corrections to instrumentation or exact implementation are rerun across the
whole affected bucket and recorded.

## Evidence and verifier contract

Every S13 package records run ID, source revision and dirty state, adapter and
backend, build profile/options, candidate descriptor/hash, shader hash,
source/projection hashes, capacity, complete settings, command/work vector,
and raw outputs. A package passes only when its verifier independently parses
and recomputes its declared gates; a README assertion is not evidence.

The final verifier recursively invokes every slice verifier from a clean-tree
revision and rejects missing files, duplicate run IDs, stale provenance,
non-finite values, replay disagreement, candidate-policy drift, band drift,
capacity ambiguity, missing raw timing samples, or a changed selected product
descriptor.

## Ordered delivery slices

### S13.1 — fork and exact probe contract

- [x] Record the human fork, exact candidate, frozen sources/bands, replay,
  overflow/capacity fixture, measurement method, exclusions, and stop gates.
- [x] Materialize a machine-readable versioned candidate descriptor and a
  verifier that rejects any policy or matrix expansion.
- [x] Prove the selected product descriptor remains byte-identical.

Artifacts: `evidence/p5.2b-s13.1-exact-gpu-probe-contract/`.

Completion gate: tactical, index, and governing-topic fork are committed before
candidate shader/runtime code; the descriptor names one candidate and the
verifier enumerates exactly two governed sources, one overflow fixture, one
forced-capacity rejection, and three performance buckets.

Commit boundaries: documentation authorization; executable contract.

### S13.2 — exact unpromoted GPU implementation

- [x] Implement the validation-only candidate policy, exact preparation,
  source-faithful 20+3+overflow construction, overflow-first schedule,
  point-sequential biased and relaxation recurrence, and current X5 timing.
- [x] Add focused controls for policy rejection, stable identity/midpoints,
  analytical separation, retained impulses, normalized rotation, coefficients,
  graph assignments/order, pass counts, point order, and restitution timing.
- [x] Prove selected construction and execution remain unchanged and cannot
  select the candidate through product configuration.

Artifacts: `evidence/p5.2b-s13.2-exact-gpu-implementation/`.

Completion gate: focused GPU diagnostics match the qualified scalar endpoint
at every exposed phase within the frozen S12 implementation-agreement bands;
all policy and selected-unchanged tests pass.

Commit boundary: versioned implementation and focused evidence.

### S13.3 — overflow and capacity qualification

- [x] Run and exactly replay the 21-manifold forced-overflow fixture.
- [x] Run and exactly replay the one-below-demand capacity rejection.
- [x] Verify overflow is executed first while true capacity overflow fails
  closed with zero accepted/committed work.

Artifacts: `evidence/p5.2b-s13.3-overflow-capacity/`.

Completion gate: assignment, execution-order, completion, rejection, no-
partial-result, and replay predicates all verify.

Commit boundary: forced-overflow/capacity evidence.

### S13.4 — governed authority qualification

- [x] Execute primary and exact replay for depth 11 and depth 16.
- [x] Compare both GPU results to the bound native authority using every
  frozen S12 band and report the unchanged product ceiling independently.
- [x] Retain complete raw observations, state/cache/color diagnostics, and
  recursively verified provenance.

Artifacts: `evidence/p5.2b-s13.4-gpu-authority-qualification/`.

Completion gate: both governed sources pass every frozen authority predicate
and replay exactly, or a verified failing predicate cleanly closes the quality
route without changing the candidate.

Commit boundary: governed GPU disposition.

### S13.5 — measured work, allocation, and complete-step time

- [x] Measure the selected control and exact candidate at 32, 256, and 1,024
  manifolds under the frozen sampling contract.
- [x] Publish raw samples, phase work/dispatches, overflow work, allocations,
  CPU encode time, GPU complete-step time, median, p95, and control deltas.
- [x] Classify any hard capacity/device/watchdog constraint separately from a
  completing performance cost.

Artifacts: `evidence/p5.2b-s13.5-gpu-work-time/`.

Completion gate: all six configuration/bucket cells have complete verified
work/allocation records and 20 valid retained timing samples, or a
reproducible hard execution failure is fully retained.

Commit boundary: measured GPU cost package.

### S13.6 — decision package and human checkpoint

- [ ] Assemble the exact policy, implementation controls, overflow/capacity,
  governed response, replay, work/allocation/time, constraints, and one
  recommendation.
- [ ] Update this tactical, the governing topics, and tactical index in the
  same change.
- [ ] Stop before cross-host candidate qualification or product promotion.

Artifacts: `evidence/p5.2b-s13.6-exact-gpu-probe-review/`.

Completion gate: a clean-tree verifier recursively checks every S13 input and
the review identifies exactly one of: qualified exact candidate for a later
human decision; exact candidate rejected by frozen authority quality; or exact
candidate blocked by a demonstrated non-removable execution constraint.

Commit boundary: final review and durable status only.

## Autonomous boundary and stop conditions

Autonomous after this authorization: versioned validation-only GPU code,
exact-policy implementation corrections, focused diagnostics, governed runs,
oracle replay, overflow/capacity fixtures, timing instrumentation, full reruns,
evidence/verifier tooling, repository/platform gates, per-slice commits, and
the final decision package.

Stop and return to the human when:

1. a frozen S12 authority band, source, candidate ingredient, or measurement
   contract would have to change;
2. exact execution requires CPU step feedback, fallback, nonportable features,
   unbounded scheduling, a product default change, or source copying;
3. invariant and native authority conclusions conflict;
4. a governed quality failure remains after exact-implementation defects are
   exhausted;
5. a reproducible capacity/device/watchdog constraint cannot be removed
   without changing X1-X4, the common recurrence, or current X5 timing; or
6. S13.6 reaches the recorded human checkpoint before cross-host candidate
   qualification or product promotion.

Ordinary compilation, shader validation, instrumentation, evidence, and
fixture defects are diagnosed and corrected autonomously. A completing but
slow measurement is evidence for the final decision rather than an
unrecorded experiment trigger.

## Completion condition

S13 completes when the one exact candidate has a verified governed disposition,
forced overflow and fail-closed capacity are proven, 32/256/1K work,
allocation, and complete-step time are retained, all evidence recursively
verifies, and the decision package reaches the human checkpoint. Completion
does not authorize cross-host candidate qualification, a product/default
change, a comparator, or a wider sweep.
