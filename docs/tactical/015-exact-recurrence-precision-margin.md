# S15: exact-recurrence precision margin

Status: active, 2026-07-20. Authorized by human decision at the S14.7
checkpoint. This workstream rejects the S14 reformulation and measures whether
the unchanged S12 recurrence has a usable arithmetic-precision margin before
admitting at most one error-free-transformation candidate on the already
identified cancellation path.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns response quality and the frozen authority bands;
[scalar contact specification](../topics/scalar-contact-specification.md) owns
the independent recurrence and arithmetic proof;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns the
portable colored execution contract; and
[performance and scale](../topics/performance-and-scale.md) owns the unchanged
32/256/1K work and timing contract. Predecessor:
[S14 branch-robust authority-band formulation](014-branch-robust-authority-band-formulation.md).

## Human fork resolution

Human decision, 2026-07-20:

- reject `authority-band-cancellation-robust-v1` as recorded by S14;
- measure the exact S12 recurrence with its original retained cancellation
  path promoted to `f64`, and separately with the complete scalar solve in
  `f64`, on governed depths 11 and 16;
- measure the depth-16 perturbation-amplitude threshold at which stillness
  fails as a margin curve;
- only if the precision-limit lanes are still and the measured margin exceeds
  a conservative compensated-arithmetic error bound, admit one candidate:
  the unchanged exact recurrence with error-free-transformation two-float
  arithmetic only on the identified cancellation path;
- route that sole candidate through the frozen scalar perturbation gate and,
  only if it qualifies, the unchanged S13 GPU probe contract; and
- if precision-limit behavior is not still, or the margin cannot dominate the
  candidate bound, admit no candidate and return a decomposition-addendum
  fork.

No reformulation arm, coefficient sweep, authority-band change, comparator,
approximate kernel, cross-host qualification, or product promotion is
authorized. The hard/direct manifold-block-i2 product remains selected.

## Goal

Decide whether the exact S12 soft recurrence has enough numerical margin for
portable compensated evaluation of its diagnosed separation cancellation, or
whether the remaining depth-16 behavior belongs to solve structure rather
than attainable arithmetic precision. A positive result is one exact,
unpromoted two-float candidate qualified under the existing scalar and GPU
contracts. A negative result is a measured decomposition-addendum fork with
no candidate.

## Precision-limit lanes

Both lanes start from the same materialized binary32 neutral scenario and the
exact `authority-x1-x4-current-x5-v1` mathematical recurrence, including
authority preparation, identity, point/color order, coefficients, one biased
plus one relaxation pass, current X5 restitution timing, unilateral
projection, warm start, cache, cadence, substeps, and requested steps.

1. `cancellation-path-f64` retains binary32 bodies, contacts, coefficients,
   impulses, branch result, and every other operation. Only the original S12
   authority-separation DAG is evaluated from its materialized operands in
   binary64 and rounded once to binary32 at the existing separation state
   boundary. It is an attribution lane, not a candidate.
2. `full-solve-f64` independently evaluates narrowphase, preparation,
   coefficients, warm start, normal/tangent solves, integration, the original
   separation DAG, relaxation, restitution, cache, and observations in
   binary64 from the materialized source. It preserves the same discrete
   identity, ordering, contact admissibility, and branch predicates. Values
   are converted only for canonical evidence serialization, never fed back
   into the solve. This is the precision-limit lane, not a product backend.

Each lane runs twice at governed depths 11 and 16 for exact replay and is
evaluated against the unchanged S12 native-authority structure, support,
speed, height, penetration, energy-ratio, and positive-run predicates over
the frozen 1,200-step, four-substep, sleep-off contract. Candidate admission
requires all four governed rows to pass. In particular, a passing full-solve
lane cannot rescue a failing cancellation-path-only lane: that result would
show that compensation on the identified path is insufficient. Both the
precision matrix and the margin curve are required measurement-only work; a
precision failure prevents candidate admission but does not skip the curve.

## Frozen depth-16 margin curve

The margin measurement perturbs only the final real value of the original
authority-separation expression before its existing `separation > 0` use. It
does not perturb anchors, positions, velocities, impulses, coefficients,
topology, time, or another pass. The baseline is `cancellation-path-f64`.

Use the seven already frozen S14 schedules: zero-amplitude nearest,
uniform-low, uniform-high, point-alternating, substep-alternating,
step-alternating, and `hashed-sign-v1`. Non-nearest schedules add signed
absolute amplitude `A` in binary64; the hash and parity definitions remain
unchanged. Measure the fixed binary lattice
`A = 2^e m` for every integer `e` from `-40` through `-16`, plus `A = 0`, at
depth 16. Every cell runs twice. A schedule's certified margin is the largest
amplitude for which every lattice cell from zero through that amplitude passes
every unchanged authority predicate and exact replay; any later re-entry is
reported but does not extend the prefix margin. The study margin is the
minimum certified margin across the six signed schedules. This is a response
margin measurement, not a coefficient or tolerance sweep.

Before candidate admission, derive an outward-rounded worst-case absolute
error bound `E_eft` for the exact two-float DAG over operand magnitudes retained
by the precision and margin runs. Use binary32 unit roundoff `u = 2^-24`,
standard `gamma(n)`, and proved error-free `TwoSum`, `TwoDiff`, and `TwoProd`
preconditions. Fitted constants and empirical attenuation are forbidden.
Admission requires the full governed precision gate and strict separation
`study_margin > E_eft`; equality, an unbounded precondition, or a bound that
depends on a changed expression routes to the no-candidate fork.

## Sole conditional candidate

The sole conditional candidate ID is
`authority-exact-separation-two-float-v1`. It retains the original S12
expression, grouping, operands, and branch predicate. It may represent
intermediate rounding residuals as a non-overlapping binary32 high/low pair
using error-free transformations and round only at the existing separation
state boundary. `f64`, shader `f64`, empirical damping, a deadband,
hysteresis, alternate algebraic grouping, coefficient changes, and expanded
precision in bodies, velocities, impulses, contacts, cache, or product state
are forbidden.

The implementation must include a written operation DAG and error proof,
adversarial/local controls, exact replay, finite-state checks, and retained-run
validation that observed absolute error never exceeds `E_eft`. A defect may be
corrected to conform to this one definition. A second compensation scheme or
expression requires a new human decision.

## Frozen scalar and GPU gates

An admitted candidate reruns the S14 seven-lane perturbation gate unchanged at
depths 11 and 16. Perturbation endpoints use the candidate's proved `E_eft`,
the same outward rounding and sign schedules, and every one of 14 governed
rows must pass the unchanged S12/S13 authority predicates and exact replay.
No margin-curve cell substitutes for this candidate gate.

Only a 14/14 scalar result admits the unchanged S13 GPU contract:

- exact authority-band primary and replay at depths 11 and 16;
- the 21-manifold star with colors 0-19 and exactly one overflow item at lane
  23, executed overflow-first;
- one-below-demand manifold capacity rejected fail-closed with zero accepted
  and committed work; and
- selected control versus candidate at 32/256/1,024 two-point manifolds on one
  named adapter, five warmups, 20 alternating retained samples, timestamped
  GPU complete-step time, separate CPU encode time, exact work/dispatches,
  allocation, replay, and nearest-rank median/p95.

The candidate must be an exact portable WGSL implementation of the proved
two-float DAG. No approximate selected-kernel port, subgroup requirement,
CPU feedback, fallback, source copy, new dispatch topology, capacity change,
or quality-invalid timing claim is allowed.

## Ownership, evidence, and exclusions

- All code is validation-only until a later human promotion decision. It may
  read complete state only after a bounded diagnostic run; no result advances
  the next interactive GPU frame.
- The `f64` lanes are independent scalar evidence, not a CPU product backend,
  alternate authority, or permission to change response bands.
- The selected product descriptor, oracle pins and precedence, sources,
  projections, physical coefficients, branch predicates, order, capacity, and
  timing method remain unchanged and digest-bound.
- No reformulation, coefficient/band sweep, damping, separation deadband,
  hysteresis, contact filtering, alternate restitution timing, comparator,
  wider scalar source sweep, cross-host qualification, or promotion.
- Every package records run ID, source revision/dirty state, exact arithmetic
  scope, source/projection/descriptor hashes, settings, raw observations,
  predicate outcomes, replay hashes, and admitted route. Verifiers recompute
  the route; prose assertions are not evidence.

## Ordered delivery slices

### S15.1 — fork and frozen study contract

- [x] Record the S14.7 authorization, two precision lanes, fixed margin
  lattice, strict route predicate, sole conditional candidate, unchanged
  scalar/GPU contracts, exclusions, and stops.
- [x] Materialize an executable contract that rejects lane, amplitude,
  schedule, candidate, source, band, overflow/capacity, or timing expansion.
- [x] Bind the selected product, S12 endpoint, S13 contract, S14 disposition,
  retained cancellation derivation, and reference pins.

Artifacts: `evidence/p5.2b-s15.1-precision-margin-contract/`.

Completion gate: tactical, index, governing-topic fork, and executable contract
are committed before precision response implementation.

### S15.2 — binary64 precision-limit response

- [x] Implement the independent cancellation-path-only and complete-solve
  binary64 lanes with scope-attestation controls.
- [x] Run depths 11 and 16 twice, retain full observations and predicates, and
  verify exact replay and unchanged authority ownership.
- [x] Record the candidate-admission precision predicate without using a
  failed row to skip the separately authorized margin measurement.

Artifacts: `evidence/p5.2b-s15.2-f64-precision-limit/`.

Completion gate: all four rows have verified dispositions and the study
continues to the required measurement-only margin curve. A failure records
the no-candidate route but does not stop before S15.3.

### S15.3 — depth-16 margin and EFT feasibility bound

- [ ] Run the frozen amplitude lattice under all seven schedules twice and
  retain the full metric curves, prefix margins, failures, and re-entry.
- [ ] Derive and mechanically verify the conservative `E_eft` feasibility
  bound over retained operand high-water values without implementing a
  response candidate.
- [ ] Admit the sole candidate only when `study_margin > E_eft` and all bound
  preconditions hold.

Artifacts: `evidence/p5.2b-s15.3-depth16-margin/`.

Completion gate: the strict candidate route is verified from both S15.2 and
S15.3, or close with no candidate and a decomposition-addendum fork at the
human checkpoint.

### S15.4 — exact two-float derivation and scalar controls

Conditional on S15.3 admission.

- [ ] Freeze the sole candidate DAG/hash and written error-bound argument.
- [ ] Implement only the exact error-free-transformation cancellation path in
  the independent scalar laboratory.
- [ ] Pass analytic, adversarial, identity, branch, bound, finite-state,
  scope-attestation, and exact-replay controls before governed response.

Artifacts: `evidence/p5.2b-s15.4-two-float-scalar/`.

Completion gate: one implementation conforms to its exact recurrence and
bound, or close at the scalar human checkpoint.

### S15.5 — frozen scalar perturbation qualification

Conditional on S15.4 qualification.

- [ ] Run the unchanged seven schedules at governed depths 11 and 16 twice.
- [ ] Retain complete observations, error high-water, structures, support,
  branch counts, replay, and every unchanged authority predicate.
- [ ] Admit GPU work only if all 14 governed rows qualify.

Artifacts: `evidence/p5.2b-s15.5-two-float-robustness/`.

Completion gate: 14/14 rows pass, or close at the scalar human checkpoint.

### S15.6 — unchanged GPU response, capacity, work, and timing

Conditional on S15.5 qualification.

- [ ] Port only the proved two-float DAG and retain focused scalar/GPU traces.
- [ ] Rerun unchanged depth-11/depth-16 authority and replay gates plus the
  overflow and fail-closed capacity fixtures.
- [ ] Rerun the unchanged selected/candidate 32/256/1K work and timing matrix,
  separating response validity, execution constraints, and completing cost.

Artifacts: `evidence/p5.2b-s15.6-two-float-gpu-probe/`.

Completion gate: every admitted S13 contract field verifies or the exact
quality/execution disposition is retained.

### S15.7 — decision package and human checkpoint

- [ ] Recursively verify every admitted slice and publish exactly one final
  disposition and recommendation.
- [ ] Update this tactical, tactical index, and governing topics in the same
  change.
- [ ] Stop before cross-host candidate qualification or product promotion.

Artifacts: `evidence/p5.2b-s15.7-precision-margin-review/`.

Completion gate: a clean-tree recursive package reaches the human checkpoint
without changing a frozen authority, product, response, or execution contract.

## Autonomous boundary and stop conditions

Autonomous after authorization: contract freeze, scalar `f64` implementation,
precision measurements, fixed amplitude curve, feasibility proof, the one
conditionally admitted two-float implementation and correction, frozen scalar
gate, exact GPU port, unchanged S13 reruns, evidence/verifiers, per-slice
commits, and repository/platform gates.

Stop and return to the human when:

1. after both measurement-only slices complete, either precision-limit lane
   has failed a governed row;
2. the study margin does not strictly exceed the proved compensated bound;
3. an EFT precondition or unchanged-expression proof fails;
4. any governed candidate perturbation lane fails after implementation defects
   are exhausted;
5. GPU qualification fails after exact implementation defects are exhausted;
6. a frozen source, band, coefficient, branch, schedule, capacity fixture, or
   timing contract would have to change; or
7. S15.7 reaches the final human checkpoint.

## Completion condition

S15 completes when the precision limit and margin have verified dispositions,
the one conditional candidate and GPU work have run only when mechanically
admitted, every admitted slice recursively verifies, and the study stops at
the recorded human checkpoint. Completion alone authorizes neither cross-host
qualification nor product/default promotion.
