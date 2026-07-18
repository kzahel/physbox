# S10: warm-start and stored-impulse stationarity

Status: active, 2026-07-18. Authorized by human review after the S9 negative
closure. S9.2 localized warm start as the only material non-gravity energy
injector on the governed 16-stack; this workstream determines whether the
stored contact-impulse state is non-stationary at depth, root-causes the
mechanism, and derives at most two scalar-first corrections. Every candidate
is an unpromoted laboratory policy; the product remains hard/direct
manifold-block i2 with the selected scheduler throughout, and any winner
returns to human review before promotion.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns settling quality gates, the frozen measurement contract, and the deferred
soft direction with its recorded reopening gate;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns
scheduling/order ownership;
[scalar contact specification](../topics/scalar-contact-specification.md) owns
stage math;
[box2d rigid authority](../topics/box2d-rigid-authority.md) owns the oracle
execution contract;
[performance and scale](../topics/performance-and-scale.md) owns timing rows.
Predecessor: [S9 schedule differential](009-settling-schedule-differential.md),
closed with zero material survivors.

## Goal

Explain and, if possible, remove the depth-scaling awake settling residual by
following the S9.2 warm-start localization to its root cause. Establish
whether PhysBox's accumulated contact impulses oscillate at rest where the
authority's are stationary, localize the oscillation mechanism, and test at
most two derived corrections inside the selected family. If the hard
formulation is shown irreducible, produce the evidence package for the
deferred-soft reopening decision instead of more hard-family arms. A decisive
negative with a completed decision package is a valid outcome.

## Context and verified facts (2026-07-18)

- S9.2 matched attribution: warm start injects `+3.02338 / +0.148891 /
  +0.708266` compensated kinetic-energy units at 16-stack settled steps
  900/1000/1100; biased normal and relaxation are net sinks; tangent and
  refresh/non-gravity-integration positive energy are zero
  (`evidence/p5.2b-s9.2-pass-attribution/`).
- S9.1 pinned baselines: 4/8/16-stack settled max linear speeds
  `0.03873 / 0.17960 / 0.70853 m/s`, last-quartile energies
  `0.0017579 / 0.0553857 / 0.3366095`; pinned Box2D 16-stack final speed
  `0.000158 m/s`. The residual scales with depth, as do the stored support
  impulses (the bottom manifold of a 16-stack carries the weight of the whole
  column each substep).
- Warm start is toggleable on both sides today. PhysBox:
  `GpuSolverExecutionV4.warm_start`, consumed at
  `crates/physbox-engine/src/body_floor_solver.rs:3864`; experiment
  constructors may set it false while `SELECTED` keeps it true. Box2D: world
  warm starting scales the prepare-time impulse seed (`warmStartScale`,
  `reference/box2d/src/contact_solver.c:1507,1618-1619,1655-1656`).
- The version-2 settling traces already capture per-step pair-cache impulses,
  generational pair identity, reference owner, and point counts; the S6.5b/S9.2
  version-3 matched instrument captures exact `f32` state at every pass
  boundary with a proven scalar mirror. Most of S10's measurement surface
  exists.
- The pinned authority never executes a hard mode: its default 30 Hz contact
  stiffness sits exactly at its own `0.125/h` cap at the selected cadence
  (`reference/box2d/src/physics_world.c:779-781`), so authority-level
  stillness has only ever been demonstrated with soft contacts. This motivates
  the predeclared S10.5 decision fork; it does not itself reopen softness.

## Hypotheses under test

1. **Non-stationary stored impulses.** At rest the accumulated impulses do not
   converge to the static equilibrium distribution; they oscillate (for
   example the two points of a resting block manifold trading load, or
   cross-color support transfer cycling), so warm start re-injects a changing
   kick each substep and the solve reabsorbs it imperfectly. Prediction:
   settled-window impulse trajectories show material amplitude or active-set
   flips that the authority's do not, and the residual collapses when warm
   start is disabled.
2. **Stationary impulses, lossy composition.** The stored impulses are
   stationary but the warm-start/bias/relax composition of the hard
   formulation cannot cancel the churn below the gate at depth. Prediction:
   impulse trajectories are quiet, warm-start-off changes little, and
   single-step operator analysis shows the residual is re-created by the
   solve recurrence itself. This routes to the S10.5 formulation decision.
3. **Shared mechanism with the deferred soft failure.** The S6.5 soft
   rising-energy mode at depth is the same impulse non-stationarity amplified
   by soft feedback. Prediction: the frozen unpromoted soft lane shows the
   same oscillation signature, larger. Confirming this unifies the two
   negative records and sharpens any reopening decision.

## Exclusions and ownership constraints

- No product default, tolerance, quality-band, capacity, oracle-precedence, or
  authority change anywhere in this workstream. Product constructors stay on
  the selected hard/direct manifold-block i2 and selected scheduling.
- Warm-start-off is a diagnostic lane only. It is never a candidate, never
  ranked, and never promotable; quiet-by-sag or quiet-by-lost-convergence is
  measurement context, not success.
- No new soft recurrence, coefficient, or soft candidate. This authorization
  permits exactly one bounded re-observation of the already-implemented,
  frozen, unpromoted S6.4 soft-candidate lane solely to measure impulse
  stationarity (hypothesis 3). Any further soft work requires the recorded
  reopening decision.
- No free-form empirical knobs: no warm-start scaling factors, impulse clamps,
  damping multipliers, or deadbands without a scalar stationarity or
  discrete-energy argument. The authority uses full warm start; a correction
  must explain, not merely attenuate.
- No new reference engine. A Rapier2D or other comparator lane is deliberately
  deferred; reconsidering one is a recorded trigger only if S10 closes
  negative, and requires its own explicit decision in the authoritative topic.
- No sleeping, no tolerance widening, no baseline refresh to a worse result,
  no structural-gate weakening. Solver family, dispatch forms, capacity
  fail-closed semantics, no-readback frame contract, launch/scenario
  ownership, and oracle process isolation all remain as recorded.
- Oracle lanes use the process-isolated native runner against the clean pinned
  checkout; a warm-start-off authority lane records the exact world flag in
  its provenance and does not alter the authority pin or build contract.

## Frozen measurement and gate contract

Quality measurements reuse the frozen S6.2 policy v2 exactly as pinned by S9.1
(1,200 exact steps, settled window 900–1200, `0.025 m/s` rest bands, energy
epsilons `1e-12`/`1e-5`, positive-run limit 5, churn definitions, exact
replay, capacity cleanliness), the S9.1 baseline manifest, and S9's three gate
tiers verbatim — implementation gate, material-effect gate, winner gate — from
[tactical 009](009-settling-schedule-differential.md). Corpus for candidate
quality runs: 4/8/16 centered stacks, the permanent four-body reproducer, the
seven frozen S6.2 fixtures, 32/64 diagnostics, and both buckets.

Stationarity metrics are frozen in S10.1 before any measurement, and are
comparative rather than invented: for each settled-window manifold, per-point
and per-manifold accumulated normal impulse amplitude (max minus min),
variance, block active-set flip count, intra-manifold redistribution rate,
and tangent-impulse amplitude, reported beside the same observables measured
from the pinned authority on the same structures. The authority's measured
band is the stationarity reference; PhysBox observables are reported as
ratios against it. Verdict rules bind to those frozen definitions.

S10.1 metric freeze, 2026-07-18, before measurement:

- The stationarity window is the 301 post-step states at exact completed steps
  900 through 1,200 inclusive. PhysBox manifold identity is the generational
  canonical body pair and point identity is its complete persistent feature
  key. Authority identity is the corresponding source-body pair plus the
  public `b2ManifoldPoint.id`; no nearest-point or geometry-based rematching is
  allowed. A disappearance splits a point track. Amplitude and variance use
  only continuous tracks with at least two samples; all appearances and splits
  remain counted as identity churn.
- Per-point and per-manifold accumulated-normal amplitude is `max - min`.
  Per-manifold normal impulse is the sum of its points in each sample.
  Variance is the population variance over the exact captured `f32` values,
  accumulated in `f64` without rounding the published inputs. Tangent
  amplitude uses the same rule per point and for the signed per-manifold sum.
- The block active set is the ordered bit set `normalImpulse > 0.0`; there is
  no epsilon or deadband. A flip is one change between consecutive samples
  having the identical ordered point-id set. For an identical two-point set,
  redistribution is `0.5 * sum(abs(share_i(t) - share_i(t-1)))`, where
  `share_i = impulse_i / manifoldTotal`. Two zero totals contribute zero; one
  zero and one nonzero total contributes one. Report transition count, mean,
  and maximum; never average across an identity break.
- Centered-stack manifold classes are source-defined. `floor-support` is the
  fixed-floor/dynamic pair, `top` is the support pair immediately below the
  highest authored dynamic body, and `interior` is every other dynamic support
  pair. The same source-pair depth matches PhysBox and authority; point ranks
  are reported but do not authorize cross-engine feature remapping.
- For every observable and class, the authority band is the maximum warm-on
  value measured on the matched 8- and 16-stack authority tracks. The PhysBox
  class maximum is divided by that band when the authority value is positive.
  Exact zero is not hidden behind a denominator floor: report `both-zero` when
  both are zero and `authority-zero-candidate-positive` when only the
  authority is zero. A class is `stationary` only when every PhysBox maximum
  is at or below its authority band; any strict exceedance signs
  `oscillating` and names the exceeding metrics.
- Local residual speed for a support manifold is the settled-window maximum
  center speed of its supported dynamic body. Correlation is the population
  Pearson correlation across the class's matched manifold tracks between that
  speed and per-manifold normal-impulse amplitude, accumulated in `f64`;
  publish `null` when either population variance is exactly zero.
- The S10.1 majority-removal route uses the 16-stack's frozen v2 settled
  maximum linear speed and last-energy-quartile maximum. For each,
  `removed = (warmOn - warmOff) / warmOn`. Hypothesis 1 is favored only if the
  PhysBox warm-off lane preserves structure and both removals are strictly
  greater than `0.5`; otherwise hypothesis 2 is favored. Authority warm-off
  is context only. The fresh PhysBox warm-on summaries must equal the S9.1
  pinned summaries exactly; a mismatch is diagnosed rather than accepted as a
  refreshed baseline.
- S10.2 signs hypothesis 1 only if the S10.1 majority-removal route favors it
  and at least one `floor-support` or `interior` normal/active-set/
  redistribution metric exceeds its authority band. All other outcomes sign
  hypothesis 2; a contradictory oscillation remains named input to the S10.3
  operator study rather than silently changing this rule.

S10.3 operator freeze, 2026-07-18, before the operator/depth measurement:

- The operator source is the exact selected hard/direct `f32` state entering
  completed step 904. The GPU trace maps only after that outer step and its
  persistence store. Its matched scalar mirror starts at every captured pass
  boundary, follows the exact compacted manifold/color order, and records the
  existing full-state difference at the corresponding GPU boundary. No
  widened scalar-replay tolerance is a verdict input.
- The stored-impulse vector is the ordered prepared-point-capacity vector of
  accumulated normal and tangent impulses, with invalid slots represented by
  exact zero. For each pass, `fixed` means the vector is bit-exact unchanged;
  otherwise `contraction` means its Euclidean distance to the exact final
  persisted vector strictly decreases; every other changed map is
  `oscillation`. This endpoint-residual label describes the captured operator
  path and is not a spectral-radius claim.
- Intra-block active-set flips retain S10.1's exact `normalImpulse > 0` rule.
  Intra-manifold redistribution retains S10.1's half-L1 two-point share rule.
  The scalar mirror records every color boundary; cross-color support
  transfer is reported as opposing signed color energy changes and as the
  cancellation between absolute per-color normal-load changes and their net.
  Warm-start/bias interplay is a warm-start kinetic-energy change followed by
  a non-fixed biased-normal impulse map in the same substep.
- An inter-substep persistence reseed is named only when a refresh has reset
  mode `1`, its output impulse vector is bit-exact equal to the first prepared
  vector of the outer step, and its input differs. The discarded L1 distance
  is reported. The stale-reseed mechanism requires all three selected
  inter-substep refreshes to meet that exact condition, plus non-fixed biased
  normal work after every warm start. A correction therefore must preserve
  refreshed geometry/effective mass, unilateral impulses, penetration and
  support transfer while preventing solved within-step impulses from being
  overwritten by the older persistence seed.
- The governed depth sweep is `2, 4, 8..=16`, the complete accepted centered
  stack set below 16. A depth exhibits the harmful signature only if the exact
  stale-reseed mechanism is present and the frozen S6.2-v2 speed or energy
  envelope gate fails; mere nonzero roundoff at a reseed is insufficient. The
  first such depth is preserved by source hash, target step, trace hash, and
  an exact replay hash. This rule chooses a reproducer; it does not relax the
  quality bands.
- Hypothesis 3 applies the unchanged S10.1 stationarity metric implementation
  to the frozen S6.4 `30 Hz / 10 / static×1 / prior-contact` 16-stack. The
  existing frozen trace lacks raw impulse/cache observables, so exactly one
  fresh bounded observation series is authorized. It is measurement-only;
  there is no soft coefficient, recurrence, baseline, or selection change.

Repository gates for every code slice are the standard block:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown -p physbox-web
cargo check --target aarch64-linux-android -p physbox-android
cargo run -p physbox-headless -- --output /tmp/physbox5-smoke.png --report /tmp/physbox5-smoke.json --scenario bootstrap
npm run check:boundaries
npm run test:web
```

Every evidence lane emits `PHYSBOX5_VALIDATION_SUCCESS` with a run id and
fresh complete provenance and ships a `verify.mjs`. `npm run
references:status` must pass after every oracle execution.

## Recording discipline

Identical to S9: every lane, diagnostic verdict, candidate, and discard —
including negative ones — gets a dated checkpoint paragraph in this tactical
and a verified package under `evidence/p5.2b-s10.*`, committed per slice with
conventional-commit subjects. No result may exist only in a commit message or
session memory.

## Ordered delivery slices

### S10.1 — contract freeze and warm-start differential

- [x] Freeze the stationarity metric definitions, verdict rules, routing, and
  evidence naming before measurement.
- [x] Add a named warm-start-off laboratory execution for the selected
  hard/direct graph (policy attested in the scene contract; product
  constructors unchanged; fail closed for unsupported compositions).
- [x] Extend the process-isolated authority runner with a warm-start-off world
  option recorded in provenance.
- [x] Run the four-lane differential on the 4/8/16 stacks and the permanent
  four-body reproducer, sleep off, 1,200 steps, S6.2 policy v2: PhysBox
  warm-on (fresh, must reconcile with the S9.1 pin), PhysBox warm-off,
  authority warm-on, authority warm-off. Record settled speed, energy,
  penetration, structure, and churn for all lanes.

Routing (predeclared): if PhysBox warm-off removes the majority of the settled
16-stack residual speed and energy, hypothesis 1 is favored and S10.2/S10.3
proceed as ordered. If the residual persists warm-off, hypothesis 2 is
favored: S10.2 still runs (its data is required either way), but S10.3 leads
with the composition operator study rather than oscillation minimization.
Authority warm-off behavior is recorded as context in both routes. No lane in
this slice is a candidate.

Outcome: the cheapest direct test of the S9.2 localization, using toggles
that already exist on both sides.

Artifacts: `evidence/p5.2b-s10.1-warm-start-differential/`.

Completion gate: all four lanes replay exactly with clean capacity and fresh
provenance; the routing verdict is recorded here with its numbers.

Commit boundaries: contract and lane implementation; clean differential
evidence.

Implementation checkpoint, 2026-07-18: the frozen metric contract is
serialized by the lab, the product lane remains parameter-free, and the named
warm-start-off diagnostic accepts only the exact selected hard/direct graph
with warm starting disabled. The process-isolated Box2D awake-world lane now
accepts the same on/off choice, records it in provenance, and can capture its
public per-step body/contact trace without modifying the pinned checkout.

Completion checkpoint, 2026-07-18: clean run
`p5.2b-s10.1-warm-start-differential-20260718-r2` passes every S10.1 gate.
All four PhysBox and authority lanes replay exactly; both PhysBox lanes are
capacity-clean and instrumentation-exact, every PhysBox structure survives,
and each fresh warm-on summary equals the S9.1 pin. On the 16-stack, PhysBox
settled maximum linear speed falls from `0.70852673` to `0.000023823235`
(`0.9999663` removed) and last-quartile maximum energy falls from `0.3366095`
to `2.307887e-9` (`1.0` removed) with warm starting off. The strict-majority
route therefore favors hypothesis 1. Authority warm-on remains bounded at
`0.0005379421` maximum settled linear speed; authority warm-off is contextual
and loses the 8- and 16-stack structures. Evidence and its passing verifier
are in `evidence/p5.2b-s10.1-warm-start-differential/`.

### S10.2 — stored-impulse stationarity measurement

- [x] Capture settled-window (900–1200) per-step stored-impulse observables on
  the 8- and 16-stacks under the selected product policy: per-manifold total
  and per-point accumulated normal impulse, tangent impulse, block active-set
  case, generational pair identity, and body residual speed, using the
  existing version-2 trace surface extended only where an observable is
  missing.
- [x] Capture the matched authority lane: the same structures, cadence, and
  window through the public contact-impulse API in the process-isolated
  runner.
- [x] Apply the frozen stationarity metrics; publish the per-manifold-class
  verdict (floor support, interior, top) with authority ratios and the
  correlation between impulse amplitude and local residual speed.

Outcome: hypothesis 1 versus 2 is decided by measurement against the
authority band, not by inference.

Artifacts: `evidence/p5.2b-s10.2-impulse-stationarity/`.

Completion gate: exact replay of both engines' lanes; every frozen metric
computed; a signed stationary/oscillating verdict per manifold class recorded
here.

Implementation checkpoint, 2026-07-18: the S6.2 v2 observation surface now
has a stationarity-only companion capture for the two observables it lacked:
supported-body center speed and the active-set case derived by the frozen
exact `normalImpulse > 0` rule. The analyzer retains only steps 900–1200,
splits discontinuous identities, computes all eight frozen maxima and
population correlations, and applies the exact-zero authority ratio semantics.
The runner recaptures the selected PhysBox 8/16 stacks with exact replay and
reuses both exact public Box2D warm-on traces from S10.1; no product path or
oracle checkout changes.

Completion checkpoint, 2026-07-18: clean run
`p5.2b-s10.2-impulse-stationarity-20260718-r1` exactly replays both PhysBox
captures, reproduces their S10.1 raw-series hashes, and exactly replays both
public authority inputs. Every class is `oscillating`. Floor-support
manifold normal amplitude is `1.246864803135395` versus the authority band
`0.00025260448455810547` (`4936.0359x`); the matching point maximum is
`0.623433031141758` versus `0.0006193220615386963` (`1006.6379x`). Interior
manifold/point maxima are `2793.9682x`/`1162.0351x` authority, with `46`
active-set flips versus exact authority zero and redistribution `1.0` versus
`0.0020854763602072557`. Top support has the same signature (`50` flips
versus zero), while all PhysBox tangent maxima remain below authority. The
local speed/normal-amplitude correlations (PhysBox/authority) are `1.0/1.0`
for floor, `0.018146322/0.730651603` for interior, and `1.0/1.0` for top.
Together with S10.1 majority removal, the frozen conjunction signs hypothesis
1: non-stationary stored impulses. Evidence and its independently
recalculating verifier are in
`evidence/p5.2b-s10.2-impulse-stationarity/`.

Commit boundaries: observation tooling; clean measurement evidence.

### S10.3 — mechanism localization and minimization

- [x] Single-step operator study from captured settled `f32` state using the
  matched scalar mirror: replay one outer step; report how the stored-impulse
  vector maps across each pass boundary — fixed point, contraction, or
  oscillation — and which family (intra-block active-set flips, intra-manifold
  point redistribution, cross-color support transfer, warm-start/bias
  interplay) carries the change.
- [x] Depth sweep for the smallest structure exhibiting the identified
  signature; preserve it as a permanent reproducer with exact prefixes.
- [x] Hypothesis 3 unification: apply the frozen stationarity metrics to
  existing frozen soft-candidate captures, plus the single authorized bounded
  re-observation of the unpromoted soft lane on the 16-stack if existing
  captures lack impulse observables. Record whether the soft rising-energy
  mode shares the hard mode's signature. This is measurement only.

Outcome: a named mechanism with a minimal reproducer, or a measured
demonstration that the stored state is already stationary and the residual
belongs to the composition (routing to S10.5).

Artifacts: `evidence/p5.2b-s10.3-mechanism/`.

Completion gate: the mechanism verdict is specific enough to name what a
correction must change and what it must provably preserve; recorded here with
its reproducer.

Commit boundaries: operator tooling; minimization evidence; unification
evidence.

Hard-mechanism checkpoint, 2026-07-18: clean run
`p5.2b-s10.3-hard-operator-and-depth-20260718-r1` exactly replays the full
series, step-904 pass trace, and operator report at every governed depth
`2,4,8..=16`; matched scalar color-boundary differences are retained without
a widened tolerance, and all lanes are capacity-clean and structurally intact.
Every depth exhibits three exact
inter-substep reset-mode-1 maps back to the first prepared impulse vector and
four warm-start/non-fixed-biased-normal compositions. The named mechanism is
`inter-substep-stale-persistence-reseed`. Depth 2 remains below both quality
gates (`8.5901223e-7 m/s`, nonincreasing `7.3734763e-13` last energy), while
depth 4 is the first harmful structure (`0.03872548 m/s`, rising
`0.0017579256` last energy). Its step-904 refreshes discard
`0.00020720600332838917` impulse L1 despite zero active-set flips and only
`8.945056e-8` redistribution, so an intra-block rule cannot explain the
minimum failure. The permanent exact-prefix reproducer has source hash
`f9a8e6138ebf9f1539014eda7f62c4b08c213c7ee866a88835ed2c3fbcefe447`.
The routed correction must retain current within-step impulses across geometry
and effective-mass refresh while preserving unilateral response, penetration
correction, deterministic colored support transfer, and final persistence.
The clean evidence and passing independent verifier are in
`evidence/p5.2b-s10.3-mechanism/`.

Completion checkpoint, 2026-07-18: the single authorized soft observation
ran once from clean revision `f4034ab7adc321032b89659c490b045f1afed944`
and preserved the frozen rising-energy mode (fresh first/last energy maxima
`1.9504352e-8`/`0.000043430242`, positive run `300`, rising envelope). Floor
and interior point-normal amplitudes exceed the unchanged authority bands by
`1.8396131x` and `2.2314258x`, respectively, while top support is stationary
and every tangent maximum remains below authority. Unlike hard/direct,
however, soft manifold-normal totals are only `0.0087305x`/`0.0048996x`
authority for floor/interior, active-set flips stay exact zero, and
redistribution remains below authority. Hypothesis 3 therefore signs
`partial-unification-point-normal-only`: the soft failure shares the
point-normal/tangent disposition, but not the hard manifold-total,
active-set, or redistribution signature. S10.3 closes with the named
`inter-substep-stale-persistence-reseed` mechanism and centered four-body
minimum reproducer; S10.4 is routed to candidate (b), retaining current
within-step impulses through geometry/effective-mass refresh. Candidate (a)
has no root-mechanism support because the minimum reproducer has zero
active-set flips and negligible redistribution. The combined verifier
reconstructs both phases from `evidence/p5.2b-s10.3-mechanism/` and passes.

### S10.4 — bounded corrections (at most two, scalar first)

- [x] Select at most two candidates routed by S10.2/S10.3 from the predeclared
  space: (a) a stationarity-preserving intra-block rule — deterministic
  active-set/tie handling or regularized near-rest load distribution with a
  scalar fixed-point or contraction argument; (b) a warm-start/bias
  composition correction with a discrete-energy or stationarity argument at
  every colored boundary. Free-form attenuation knobs are outside the space.
- [ ] For each candidate, in order and failing closed at each step: the scalar
  argument; scalar quality on the two-step contract fixture, the permanent
  four-body reproducer, and the S10.3 reproducer (support transfer and
  penetration preserved; quiet-by-sag rejected); a single GPU port as a named
  unpromoted policy; the S9 implementation gate; the S9 material-effect gate
  on the full corpus against the S9.1 baseline.
- [ ] Record every candidate disposition — including a scalar-stage kill —
  as a dated checkpoint with evidence.

Outcome: the mechanism evidence either produces a material survivor inside
the selected family or exhausts the derived-correction space with recorded
proof.

Artifacts: `evidence/p5.2b-s10.4-candidate-<name>/` per candidate.

Selection and scalar checkpoint, 2026-07-18: S10.3 routes exactly candidate
(b), `retain-within-step-impulses-through-refresh`; candidate (a) is
`not-routed` because the minimum has zero active-set flips and only
`8.945056e-8` redistribution. The clean scalar gate at revision
`2b34abb09435fe9b1a11b30486d410ef5287062e` passes its 180-sample
stationarity argument. It preserves the exact two-step body response. On the
permanent mixed pile it reduces settled max linear/angular-edge speed from
`0.07749471/0.03475067` to `0.00665991/0.00308007`, changes the first/last
energy maxima from `0.00222324/0.00456408` to
`0.0000455469/0.0000443481`, and reduces the positive run from 5 to 3. On the
S10.3 centered four-body reproducer it reaches
`0.00627111/0.00934313`, a non-rising
`0.0000733514/0.0000704171` envelope, and positive run 2. Both long cases
preserve support counts, do not increase penetration, and pass structure and
anti-sag checks. The verified package in
`evidence/p5.2b-s10.4-candidate-retained-refresh/` admits one named,
unpromoted GPU port; it does not change the selected product.

Completion gate: both candidate slots have dispositions, or one survivor
advances with the other slot's disposition recorded.

Commit boundaries: scalar derivation; GPU port; quality evidence — separately
per candidate.

### S10.5 — survivor qualification or formulation decision package

- [ ] If a material survivor exists: apply S9.5 verbatim — Metal 256/1K
  complete-step timing, Chrome WebGPU, hardware-backed AVD, available
  physical Android structure through step 1,200, an AVD matched-boundary
  overlay if any rising-energy mode remains, then the winner gate. A winner
  stops for human promotion review.
- [ ] If no survivor: assemble the formulation decision package for human
  review — the warm-start differential, stationarity verdicts, mechanism
  reproducer, candidate dispositions, and the hypothesis-3 unification
  result, presented against the recorded soft-reopening gate (remove the
  16-stack rising-energy mode; hold candidate 16-stacks through step 1,200 on
  AVD and physical Android; AVD permanent reproducer positive runs under 5).
  The package must state specifically what a soft reopening would now do
  differently than S6.5 and what evidence says the hard formulation cannot
  meet the awake gate. It recommends; the human decides.

Outcome: either a promotion candidate at human review, or the settling
direction's next decision is fully briefed instead of implied.

Artifacts: `evidence/p5.2b-s10.5-survivor-<name>/` or
`evidence/p5.2b-s10.5-formulation-decision/`.

Completion gate: winner-gate clauses individually recorded, or the decision
package verifies with every claim linked to raw evidence.

Commit boundary: qualification or decision package only.

### S10.6 — review package (always runs)

- [ ] Assemble the compact review: differential, stationarity, mechanism,
  candidate dispositions, cross-host results if any, and one specific
  recommendation — promote-candidate, take the formulation decision, or close
  with named reconsideration triggers (including the deferred comparator-
  engine question).
- [ ] Update this tactical, the governing topics, and the tactical index in
  the same change.

Outcome: a reviewer can act without reading either engine.

Artifacts: `evidence/p5.2b-s10.6-stationarity-review/`.

Completion gate: package verifies from a clean tree; no product or authority
setting changed.

Commit boundary: review package and status updates only.

## Autonomous boundary and stop conditions

Autonomous after this authorization: measurement tooling, the warm-start-off
lanes on both engines, stationarity analysis, operator studies, minimization,
the bounded soft-lane re-observation, scalar derivations, the at-most-two
GPU candidate ports as unpromoted policies, corpus and gate runs, discard
decisions strictly by the predeclared rules, per-slice commits, and the final
package.

Check back with the human only when:

1. the winner gate passes — promotion is always a human decision;
2. S10.5 resolves to the formulation decision package — reopening softness or
   accepting a different physical policy is the recorded human fork;
3. a correction would require leaving the predeclared candidate space (new
   accumulation architecture, empirical attenuation knob, coefficient set) —
   stop before implementing it;
4. an authority or invariant conflict appears — preserve the reproducer and
   stop;
5. a gate could only be met by weakening a frozen band, tolerance, or
   structural contract; or
6. a required lane cannot run without changing product code paths.

Ordinary build, harness, schema, determinism, or evidence failures are
diagnosed and minimized autonomously without weakening the contract.
Negative verdicts are recorded and the program continues to its completion
condition; exhaustion with a complete decision package is a designed outcome.

## Completion condition

The workstream is complete when either a winner-gate survivor is packaged for
human promotion review, or the stationarity question is answered with
authority-referenced measurements, the mechanism is named with a permanent
reproducer, both candidate slots have dispositions, and the S10.5 decision or
closure package is returned. Every intermediate and discarded result is
preserved in evidence and in this document. Completion never changes product
defaults, quality bands, oracle precedence, or authority interpretation.
