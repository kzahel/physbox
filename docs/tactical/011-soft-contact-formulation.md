# S11: soft contact formulation study

Status: active, 2026-07-19. Authorized by human review at the S10.5
formulation fork recorded in the
[S10.6 review](../../evidence/p5.2b-s10.6-stationarity-review/README.md).
S10 measurably exhausted the authorized hard/direct correction space without
meeting the awake settling gate, while the pinned authority has only ever
demonstrated rest-level stillness with soft contacts and the frozen soft
lane's residual is roughly four orders of magnitude smaller in energy than
the hard one. This workstream root-causes the soft 16-stack rising-energy
mode with the S10 measurement discipline and derives only
mechanism-supported corrections under a bounded, versioned mechanism verdict.
Every candidate is an unpromoted laboratory
policy; the product remains hard/direct manifold-block i2 with the selected
scheduler throughout, and any winner returns to human review before
promotion.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns settling quality gates, the frozen measurement contract, and the
recorded soft reopening gate;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns
scheduling/order ownership;
[scalar contact specification](../topics/scalar-contact-specification.md)
owns stage math;
[box2d rigid authority](../topics/box2d-rigid-authority.md) owns the oracle
execution contract;
[performance and scale](../topics/performance-and-scale.md) owns timing rows.
Predecessors: [S6 soft closure](006-awake-settling-stabilization.md) (variant
selection, closed negative without attribution) and
[S10 hard exhaustion](010-warm-start-impulse-stationarity.md) (mechanism
discipline, closed negative with a taken decision).

## Goal

Explain and, if possible, remove the frozen soft 16-stack rising-energy mode
by measuring where the soft formulation injects energy, naming the mechanism
with a minimal reproducer, and testing corrections derived from that mechanism.
At most two candidates may be active under one frozen mechanism verdict; a
discarded candidate can reopen attribution only when its evidence exposes a
distinct previously unmeasurable injector, and that reroute must be frozen as
a versioned mechanism addendum before another candidate. A survivor must pass
the recorded reopening gate on Metal,
the hardware-backed AVD, and available physical Android independently before
stopping for human promotion review. If the soft mechanism cannot be removed
inside the authorized space, produce the closure decision package — including
the recorded comparator-engine fork — instead of more soft arms. A decisive
negative with a completed decision package is a valid outcome.

## Context and verified facts (2026-07-19)

- The pinned authority never executes a hard contact mode at the selected
  cadence: its default 30 Hz stiffness sits at its own `0.125/h` cap
  (`reference/box2d/src/physics_world.c:779-781`). Authority-level stillness
  is soft-only demonstrated.
- S10 closed the hard direction: warm start carries the 16-stack residual
  (`0.9999663`/`1.0` removed warm-off), stored impulses oscillate up to
  `4936.04x` the authority band, the named
  `inter-substep-stale-persistence-reseed` correction improves 4/8 stacks but
  regresses 16-stack energy `29.24%`, and the second slot has no mechanism
  support (`evidence/p5.2b-s10.6-stationarity-review/`).
- The frozen unpromoted soft lane is the S6.4
  `30 Hz / 10 / static×1 / prior-contact` configuration. Its fresh S10.3
  re-observation preserves the rising-energy mode with first/last settled
  energy maxima `1.9504352e-8`/`0.000043430242` and positive run 300 —
  roughly four orders of magnitude below the hard 16-stack residual energy
  `0.3366095`.
- The soft failure signature is mild and distinct from the hard one:
  floor/interior point-normal amplitudes `1.8396131x`/`2.2314258x` authority,
  manifold-normal totals `0.0087305x`/`0.0048996x` authority, exact zero
  active-set flips, redistribution and all tangent maxima below authority
  (`evidence/p5.2b-s10.3-mechanism/soft/`). Hypothesis 3 signed
  `partial-unification-point-normal-only`: the hard stale-reseed rule is not
  presumed transferable.
- The S6.5 soft study selected identity/coefficient/normal-relaxation
  variants without per-pass attribution; its S6.5d review recorded that any
  future soft proposal must begin with matched-precision per-pass energy
  attribution on the governed 16-stack and that the permanent four-body
  reproducer alone is insufficient evidence.
- Measured scalar-to-GPU predictive gap (S10.4): the same permanent four-body
  scene passed its full-length scalar lane at `0.006659909 m/s` with positive
  run 3 and failed the GPU corpus at `0.04818385 m/s` with positive run 199.
  A scalar-only pass is never a qualification signal.
- The S6.5b/S9.2/S10.3 version-3 matched instrument captures exact `f32`
  state at every pass boundary with a proven scalar mirror, and the S10.1
  stationarity metrics and authority bands are frozen and already applied
  once to the soft lane. Most of S11's measurement surface exists.

## Hypotheses under test

1. **Soft warm-start/bias composition injector.** Matched attribution finds
   warm start, the soft biased normal, or their composition as the material
   positive injector on the soft 16-stack, with the point-normal oscillation
   as its signature. Prediction: one or two pass families carry compensated
   positive energy at settled steps while the others are sinks, and the
   injector's single-step operator map is non-contractive at the oscillating
   points.
2. **Support-transfer or persistence composition injector.** The injector is
   colored support transfer, refresh, or persistence seeding rather than the
   warm-start/bias pair. Prediction: attribution localizes positive energy to
   those boundaries and the operator study shows the corresponding
   non-fixed maps.
3. **Secular soft recurrence drift.** No single pass family is material; the
   discrete soft update accumulates energy slowly through the recurrence
   itself at depth. Prediction: attribution shows small alternating
   contributions that sum positive across substeps, and the operator study
   shows endpoint contraction per pass with net growth per outer step. This
   routes to a discrete-energy correction argument or, failing that, to the
   closure package.

## Exclusions and ownership constraints

- No product default, tolerance, quality-band, capacity, oracle-precedence,
  or authority change anywhere in this workstream. Product constructors stay
  on the selected hard/direct manifold-block i2 and selected scheduling.
- No coefficient, identity, or normal-relaxation re-selection and no
  free-form empirical knobs (scaling factors, clamps, deadbands, damping)
  without a measured-mechanism route and a scalar stationarity or
  discrete-energy argument. The S6.5 selection space is not re-entered.
- At most two candidates under one frozen mechanism verdict, scalar first,
  each with at most one named unpromoted GPU port. A discarded candidate may
  route another only through a versioned mechanism addendum supported by new
  attribution evidence; candidate count alone can never justify closure.
  Warm-start-off remains a diagnostic lane only.
- The mandatory early GPU falsification (frozen below) runs before any
  full-corpus or cross-host investment; skipping it is a contract violation.
- No sleeping, no tolerance widening, no baseline refresh to a worse result,
  no structural-gate weakening, no capacity relaxation. Dispatch forms,
  fail-closed semantics, the no-readback frame contract, launch/scenario
  ownership, and oracle process isolation remain as recorded.
- No new reference engine. The Rapier2D comparator question stays deferred
  with its recorded trigger (this study closing negative) and requires its
  own explicit decision in the authoritative topic.
- Oracle lanes use the process-isolated native runner against the clean
  pinned checkout with complete provenance.

## Frozen measurement and gate contract

Quality measurements reuse the frozen S6.2 policy v2 exactly as pinned by
S9.1 (1,200 exact steps, settled window 900–1200, `0.025 m/s` rest bands,
energy epsilons `1e-12`/`1e-5`, positive-run limit 5, churn definitions,
exact replay, capacity cleanliness), the S9.1 baseline manifest, and the
S10.1 stationarity metric definitions and authority bands verbatim. The soft
lane under study is exactly the frozen S6.4 configuration re-observed in
S10.3; changing that configuration is candidate work, not measurement.
Corpus for candidate quality runs: 4/8/16 centered stacks, the permanent
four-body reproducer, the seven frozen S6.2 fixtures, 32/64 diagnostics, and
both buckets.

Attribution and operator rules follow the S9.2 and S10.3 freezes applied to
the soft graph. Exact full-state/contact traces at settled steps 900, 1000,
and 1100 retain compensated kinetic-energy attribution and matched scalar
replay for the pass families `integrate-velocity`, `prepare-contacts`,
`refresh-contacts`, `warm-start`, `normal-biased`, `tangent-biased`,
`integrate-position`, `normal-relaxation`, `tangent-relaxation`, and
`restitution`. In addition, one motion-only trace captures the exact `f32`
body motion after every one of those pass boundaries for every settled step
900 through 1200 in GPU command order and maps only after step 1200. It emits
strict ordered-`f32` and compensated-`f64` kinetic energy, per-step and
full-window pass-family totals, and retains the raw captured motion values.
The exact anchor initial/final motions must equal their corresponding window
states. `persistence-store` has zero direct kinetic-energy delta by ownership
because it changes cache state rather than body motion; its impulse handoff is
still an explicit cross-step operator boundary in S11.2 and may route the
persistence-composition hypothesis.

A pass boundary is materially positive only when its compensated energy
delta exceeds the frozen S6.2 predicate
`max(1e-12, 1e-5 * max(abs(E_before), abs(E_after)))`. A pass family is a
consistent material injector only when its net compensated delta is positive
beyond that predicate over the complete window and at each of the three exact
anchors. Gross positive delta, material-boundary count, net delta, and step
occupancy are always retained so an alternating composition cannot disappear
inside a net total. Attribution reconciliation is accepted only when the sum
of all pass deltas matches both every captured outer-step endpoint and the
complete-window endpoint within `1e-10` absolute plus `1e-8` relative error.
One or more consistent non-gravity families route hypothesis 1 or 2 by their
owned mechanism; no such family routes hypothesis 3. Cross-hypothesis,
sign-changing, or reconciliation-clean but non-consistent results are recorded
as ambiguous input to S11.2 rather than reclassified after measurement.

Stored-impulse vector maps remain labeled fixed, contraction, or oscillation
per captured pass boundary from exact settled `f32` state; the S10.1
active-set and redistribution rules are unchanged. These definitions and the
artifact name below are frozen before S11.1 measurement.

The winner bar is the recorded reopening gate, verbatim: remove the soft
16-stack rising-energy mode; keep 16-stack linear and angular-edge settled
speed at or below `0.025 m/s` with positive-energy runs strictly below 5;
preserve the 16-stack through step 1,200 on the hardware-backed AVD and
available physical Android; keep the AVD permanent reproducer positive run
strictly below 5; preserve the same structures on Metal, AVD, and physical
Android; never via sleeping, tolerance widening, capacity relaxation, or an
oracle-precedence change. The 16-stack is the primary derivation gate; the
permanent four-body reproducer is required evidence but cannot qualify a
candidate by itself.

Early GPU falsification gate (S11 addition, frozen before any candidate):
after a candidate passes its scalar stages, its single named unpromoted GPU
port immediately runs exact-replay full-length lanes on the 16-stack and the
permanent four-body reproducer only. The falsification passes only if the
16-stack settled energy envelope is non-rising and both scenes meet the same
speed, energy, positive-run, structure, and anti-sag verdicts their scalar
lanes passed. A falsification failure is recorded and routes back to
mechanism work or discards the candidate; wider corpus and cross-host lanes
must not run first. This rule exists because of the measured S10.4
scalar-to-GPU divergence and does not weaken any band.

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

Identical to S9/S10: every lane, diagnostic verdict, candidate, and discard —
including negative ones — gets a dated checkpoint paragraph in this tactical
and a verified package under `evidence/p5.2b-s11.*`, committed per slice with
conventional-commit subjects. No result may exist only in a commit message or
session memory.

## Ordered delivery slices

### S11.1 — soft attribution contract freeze and matched pass attribution

- [x] Freeze the soft-graph attribution pass-family list, any S11-specific
  metric definitions, verdict rules, routing, and evidence naming before
  measurement.
- [x] Extend the version-3 matched instrument to the frozen soft 16-stack
  graph without changing product paths; add the single-run settled-window
  motion trace described above, and retain the scalar mirror's proven
  pass-boundary exactness on the soft composition.
- [x] Run matched-precision per-pass compensated energy attribution on the
  soft 16-stack over the complete settled window and at exact steps
  900/1000/1100 with exact replay, and record the permanent four-body soft
  lane as secondary context under the same contract.

Routing (predeclared): a material positive pass family routes hypothesis 1
or 2 by which family carries it, and S11.2 leads with that family's operator
map. No material single family routes hypothesis 3, and S11.2 leads with the
whole-recurrence discrete-energy accounting. A contradictory or ambiguous
attribution is named input to S11.2, not a rule change.

Artifacts: `evidence/p5.2b-s11.1-soft-pass-attribution/`.

Completion gate: exact replay and capacity cleanliness on every lane; the
attribution sums reconcile with the observed energy envelope; the routing
verdict is recorded here with its numbers.

Implementation checkpoint, 2026-07-19: the attribution contract is frozen to
the ten named motion-changing pass families, the S6.2 material predicate, and
the per-step/full-window reconciliation bands above. A native-only version-1
motion-window capture now records raw `f32` body motion at all 57 selected
motion states spanning 56 pass boundaries for every step 900–1200 in GPU
command order and maps once after step 1200. The lab emits strict
ordered-`f32` and compensated-`f64` energy, family net/gross/material totals,
occupancy, anchors, and routing.
The existing version-3 exact trace remains the full-state/contact/scalar
anchor and is required to equal the window motion at steps 900/1000/1100.
The extension leaves product constructors and all physical defaults inert.
Standard repository gates, the APK build, and the hardware-backed AVD lane
pass; the attached physical Pixel 7a lane was available but locked and is not
an instrument-completion dependency. Clean attribution evidence remains the
last S11.1 action.

Completion checkpoint, 2026-07-19: clean run
`p5.2b-s11.1-soft-pass-attribution-20260719-r1` passes every frozen S11.1
gate in the
[verified package](../../evidence/p5.2b-s11.1-soft-pass-attribution/README.md).
Both lanes exactly replay their 1,200-step observation series, complete
settled-window motion captures, and three full anchor traces; capacity,
structure, matched scalar state, static-support accounting, and energy
reconciliation pass. The primary 16-stack grows from
`3.901851147423373e-9` to `4.34302409180226e-5` compensated kinetic energy
over steps 900–1200, with positive run 300. `normal-biased` is its only
consistent non-gravity material injector (`+5.953993913608667e-7`, positive
on all 301 steps and all three anchors), while normal relaxation is a
`-2.385762499016329e-6` sink. The larger secular balance includes a
`+4.526201848124244e-5` deficit between gravity integration and warm-start
support, so the signed route is hypothesis 1 at composition level rather
than bias in isolation. The required four-body lane reverses the soft-normal
signs — biased normal is `-7.207121141519029e-4` and normal relaxation is the
consistent `+8.186273718758345e-4` injector — while retaining the rising mode
and positive run 300. S11.2 therefore leads with the primary normal-biased
operator but must retain bias, relaxation, gravity/warm support cancellation,
and the persistence-to-next-step handoff as one scene-dependent recurrence.
Prepare, refresh, integrate-position, and restitution have exact zero direct
motion delta and do not route hypothesis 2 by themselves. No product or
contract default changed.

Commit boundaries: contract and instrument extension; clean attribution
evidence.

### S11.2 — soft operator study, mechanism, and minimal reproducer

- [x] Single-step operator study from captured settled soft `f32` state using
  the matched scalar mirror: label every pass-boundary stored-impulse map
  fixed, contraction, or oscillation, and name which family carries each
  change under the frozen S10.1/S10.3 rules.
- [x] Depth sweep over the accepted centered stacks (`2, 4, 8..=16`) for the
  smallest structure exhibiting the identified soft signature; preserve it as
  a permanent reproducer with exact prefixes.
- [x] Record the mechanism verdict: what a correction must change and what it
  must provably preserve (unilateral response, penetration correction,
  deterministic colored support transfer, refreshed geometry, final
  persistence, and soft impact response without sag).

Outcome: a named soft mechanism with a minimal reproducer, or a measured
demonstration that the drift is secular recurrence behavior, routing the
candidate space accordingly.

Artifacts: `evidence/p5.2b-s11.2-soft-mechanism/`.

Completion gate: the mechanism verdict is specific enough to derive a
correction from; recorded here with its reproducer hashes.

Operator/minimization contract freeze, 2026-07-19, before governed S11.2
measurement: the exact operator prefix is completed step 1100 with the
adjacent step-1099 final state retained for the persistence-store handoff.
Every pass map uses the frozen S10 fixed/contraction/oscillation rule and the
same `2e-5` scalar ceiling. The depth catalog is exactly
`2, 4, 8, 9, 10, 11, 12, 13, 14, 15, 16`; every depth runs the full S6.2
series with exact replay plus exact replay of both operator prefixes. The
identified primary composition signature requires all of: the full-length
rising-energy mode; materially positive biased normal at step 1100; all eight
biased-normal maps oscillatory; all eight normal-relaxation maps contractive;
three exact refresh rewinds to the first prepared impulse; a bit-exact
outer-step persistence handoff; and nonzero within-step impulse discarded by
those rewinds. The individual gravity/warm deficit is retained as composition
evidence but is not an onset predicate because S11.1 already measured
scene-dependent sign changes between bias and relaxation. The smallest depth
meeting the conjunction must remain the exact shared
`p5_2_s11_soft_composition_reproducer_scenario` alias at depth 11 or the clean
lane fails closed. Candidate derivation may not begin from dirty diagnostic
numbers; it begins only after the full clean sweep confirms this contract.

Implementation checkpoint, 2026-07-19: the native-only version-1 soft
operator runner now binds the verified S11.1 manifest, executes the complete
frozen depth catalog plus the permanent four-body context, and records exact
1,200-step replay, adjacent step-1099/1100 pass traces, matched scalar rows,
capacity and structure verdicts, per-family maps, the outer persistence
handoff, and inter-substep discarded-impulse totals. Clean runs fail unless
the shared depth-11 source is the first exact primary-signature match. Product
constructors and all physical defaults remain inert. Standard repository
gates, APK construction, and the hardware-backed AVD lane pass; clean operator
and minimization evidence remains the last S11.2 measurement action.

Completion checkpoint, 2026-07-19: clean run
`p5.2b-s11.2-soft-mechanism-20260719-r1` passes the frozen operator and
minimization contract in the
[verified package](../../evidence/p5.2b-s11.2-soft-mechanism/README.md). All
12 lanes replay the 1,200-step series, adjacent step-1099/1100 prefixes, and
operator report exactly with clean capacity, accepted structure, and maximum
scalar difference below `4.3593536e-7`. The named mechanism is
`depth-triggered-inter-substep-soft-seed-rewind`: outer persistence retains
all active points and both impulses exactly, but each of three later-substep
refreshes restores the first prepared impulse and discards the current
within-step correction. Biased normal is oscillatory on all eight maps and
normal relaxation contractive on all eight. The complete rising signature
first appears at depth 11 (positive run 6, energy maxima
`1.3462258e-9`/`1.350559e-9`), falls below the predicate at depths 12–14,
and reappears at 15/16, so this is depth-triggered rather than a monotonic
threshold. The exact permanent reproducer hash is
`9c02153e801e463f5e693f2187ee8ca497fce817a52933676916c7b614644658`.
At depth 16, all 32 points hand off exactly while the refreshes discard
`1.925067298338412e-4` impulse L1; the outer-step gravity/warm deficit is
`+4.8204905025994726e-8`. The four-body context retains the same three
rewinds and exact handoff while reversing the normal-family energy signs.
Therefore a correction must refresh geometry/effective mass without
overwriting current outer-step impulses, and must preserve unilateral
response, penetration correction, deterministic colored support transfer,
final persistence, exact replay/capacity, and soft impact response without
sag. S11.3 routes candidate 1 to the existing isolated retained-impulse
refresh policy as a soft-specific unpromoted experiment; S10's hard quality
disposition is not transferred and no product default changed.

Commit boundaries: operator tooling; mechanism and minimization evidence.

### S11.3 — bounded corrections (mechanism-routed, scalar first, falsify early)

- [x] Select at most two candidates under the frozen S11.1/S11.2 mechanism
  verdict, each with a scalar stationarity or discrete-energy argument.
  Free-form attenuation is outside the space. If both are discarded without
  establishing a closure constraint, another candidate requires new measured
  attribution, a versioned S11.2 mechanism addendum, and its own predeclared
  scalar argument before implementation.
- [x] For each candidate, in order and failing closed at each step: the
  scalar argument; scalar quality on the two-step contract fixture, the
  permanent four-body reproducer, the S11.2 reproducer, and the full-length
  scalar 16-stack (support transfer and penetration preserved; quiet-by-sag
  rejected); the single named unpromoted GPU port; the early GPU
  falsification gate; the implementation gate (descriptor-exact control,
  dispatch-form exactness, fail-closed negatives, repository/platform
  gates); then the full-corpus evaluation against the recorded reopening
  gate.
- [x] Record every candidate disposition — including a scalar-stage or
  falsification kill — as a dated checkpoint with evidence.

Artifacts: `evidence/p5.2b-s11.3-candidate-<name>/` per candidate.

Completion gate: every routed candidate has a disposition and either one
survivor advances or no unresolved mechanism-supported correction remains
inside the ownership constraints. Consuming two slots alone is not closure.

Candidate-1 scalar contract freeze, 2026-07-19, before measurement: the one
routed policy is `retain-within-step-impulses-through-refresh`. It changes
only which already-existing impulse vector seeds the three full inter-substep
geometry/effective-mass refreshes; contact regeneration, anchors, effective
masses, separation, unilateral projection, soft coefficients, penetration
bias, friction, restitution, colored order, and final persistence remain
unchanged. It has zero free parameters and is not a product policy. The
soft-specific scalar argument must prove the active one-constraint endpoint
is warm-seed independent algebraically because the pinned soft coefficients
satisfy `massScale + impulseScale = 1`, then retain and bound the actual `f32`
coefficient-complement residual; retention removes the measured coupled-color
rewind transient rather than hiding rounding. The exact scalar lanes are the
two-step soft four-body contract, full-length permanent soft four-body
context, full-length depth-11 S11.2 reproducer, and full-length soft 16-stack.
Every lane must remain finite, structurally accepted, no more penetrated than
its stale-seed
control, support-preserving, and non-sagging; the two-step body response must
remain exact, while every full-length candidate must independently remain
inside both `0.025 m/s` bands with positive run below 5 and a non-rising
energy envelope. Any scalar failure discards candidate 1 before a GPU port.
Only a complete scalar pass admits the existing single unpromoted GPU policy
to the frozen early 16-stack/four-body falsification; no wider GPU corpus may
run first. Artifact name:
`evidence/p5.2b-s11.3-candidate-retained-refresh/`.

Candidate-1 scalar implementation checkpoint, 2026-07-19: the native-only
runner binds the verified S11.2 manifest, evaluates 720 soft unilateral seed
combinations, and executes control/candidate plus exact replay on all four
frozen scalar lanes. It retains complete scalar observations in deterministic
compressed captures and fails closed on finite state, structure, penetration,
support, anti-sag, two-step exactness, awake speed, positive-run, and
energy-envelope predicates. A scalar rejection is a successful measured
disposition with `gpuPortAdmitted=false`; it cannot emit authorization for a
GPU run.
Product constructors and all physical defaults remain inert. At this
implementation checkpoint, clean candidate evidence remained open.

Candidate-1 disposition, 2026-07-19: clean run
`p5.2b-s11.3-retained-refresh-scalar-20260719-r1` is independently verified
in the
[candidate package](../../evidence/p5.2b-s11.3-candidate-retained-refresh/README.md)
and discards raw retained refresh at the scalar stage. The local soft argument
passes 720 samples: the actual `f32` coefficient complement residual is
`1.862645149230957e-8`, with maximum endpoint impulse/velocity differences
`3.7252903206663746e-8`/`2.607703215584678e-7`. The coupled lanes do not.
Depth 11 improves from `0.17856713` to `0.00850862 m/s`, but its candidate
energy maxima rise from `1.0909919e-4` to `1.1369673e-4` and penetration
worsens from `0.01701543` to `0.02646441 m`. The scalar 16-stack is
structurally invalid in both lanes and candidate penetration worsens from
`0.03863653` to `0.04534828 m`; the four-body candidate changes the final
active support count from 7 to 8. Every policy replays exactly and the
two-step response is exact, so this is a quality/structure rejection rather
than nondeterminism. `gpuPortAdmitted=false`: no S11 GPU candidate, corpus,
or cross-host lane ran. Candidate 1 is recorded but the mechanism is not
closed; candidate 2 now requires a distinct soft-consistent impulse-transport
derivation rather than raw retention, attenuation, or a weakened penetration
gate.

S11.2 mechanism addendum v2 and candidate-2 scalar contract freeze,
2026-07-19, before measurement: candidate 1 exposes a previously unresolved
refresh-basis/impulse-state composition. Raw retention with full contact
regeneration reduces depth-11 speed but worsens penetration and fails the
energy envelope, so impulse continuity alone is insufficient. The pinned
Box2D authority prepares each contact constraint once per outer step, retains
its evolving impulse through all substeps, and analytically updates separation
from body position/rotation deltas; it does not regenerate anchors or
effective masses between substeps (`reference/box2d/src/solver.c:1093-1174`,
`reference/box2d/src/contact_solver.c:24-158`). This routes the distinct
zero-parameter candidate `fixed-anchor-soft-impulse-transport`: prepare the
contact basis once per outer step, retain the current impulse in that fixed
basis, and update separation analytically before relaxation. It preserves
unilateral projection, the pinned soft coefficients, penetration bias,
friction, restitution, deterministic colored order, final persistence, and
the outer-step geometry preparation. This addendum deliberately revises the
S11.2 “refreshed geometry” preservation clause only for candidate 2; the new
candidate-1 evidence and exact authority schedule are the route, not candidate
count. Candidate 2 runs the same four scalar lanes and unchanged gates as
candidate 1. Any scalar failure discards it without a GPU port. A complete
scalar pass admits exactly the existing unpromoted fixed-anchor analytical-
separation GPU policy to the mandatory 16-stack/four-body falsification before
any wider corpus. Artifact name:
`evidence/p5.2b-s11.3-candidate-fixed-anchor-transport/`.

Candidate-2 disposition, 2026-07-19: clean run
`p5.2b-s11.3-fixed-anchor-scalar-20260719-r1` is independently verified in
the
[fixed-anchor package](../../evidence/p5.2b-s11.3-candidate-fixed-anchor-transport/README.md)
and discards fixed-basis soft impulse transport at the scalar stage. It is
not uniformly worse: the permanent four-body lane passes, depth-11 maximum
linear speed falls from `0.17856713` to `0.03549986 m/s`, and depth-11
penetration improves from `0.01701543` to `0.01385809 m`. The unchanged gates
still reject it. The exact two-step body response changes; depth 11 has a
positive energy run of 6; and the scalar 16-stack changes final active support
points from 34 to 33 while maximum penetration rises from `0.03863653` to
`0.09379554 m`. Every lane remains finite and control/candidate replay
exactly. `gpuPortAdmitted=false`: the mandatory scalar-first order excludes
the early GPU port, corpus, and cross-host lanes.

The measured `depth-triggered-inter-substep-soft-seed-rewind` route is now
exhausted inside the ownership constraints. Candidate 1 tested impulse
continuity through regenerated bases; candidate 2 tested the distinct pinned-
authority fixed-basis transport schedule exposed by candidate 1. The remaining
changes are coefficient re-selection, empirical attenuation, a new
accumulation architecture, or weakened quality/structure gates, all explicitly
outside S11. S11.3 is complete with no survivor and routes S11.4 to the
closure decision package.

Commit boundaries: scalar derivation; GPU port and falsification; quality
evidence — separately per candidate.

### S11.4 — survivor cross-host qualification or closure package

- [ ] If a survivor exists: Metal 256/1K complete-step timing, Chrome WebGPU
  structure, hardware-backed AVD and available physical Android 16-stack
  structure through step 1,200 measured independently, the AVD permanent
  reproducer positive-run clause, and an AVD matched-boundary overlay if any
  rising-energy mode remains. A candidate meeting every reopening-gate clause
  stops for human promotion review; no cross-host result is inferred.
- [ ] If no survivor: assemble the closure decision package — attribution,
  mechanism, candidate dispositions, and the remaining options, including
  the recorded comparator-engine reconsideration trigger. It recommends; the
  human decides.

Artifacts: `evidence/p5.2b-s11.4-survivor-<name>/` or
`evidence/p5.2b-s11.4-soft-closure-decision/`.

Completion gate: reopening-gate clauses individually recorded, or the
closure package verifies with every claim linked to raw evidence.

Commit boundary: qualification or closure package only.

### S11.5 — review package (always runs)

- [ ] Assemble the compact review: attribution, mechanism, candidate
  dispositions, cross-host results if any, and one specific recommendation —
  promote-candidate, take the closure decision, or close with named
  reconsideration triggers.
- [ ] Update this tactical, the governing topics, and the tactical index in
  the same change.

Artifacts: `evidence/p5.2b-s11.5-soft-formulation-review/`.

Completion gate: package verifies from a clean tree; no product or authority
setting changed.

Commit boundary: review package and status updates only.

## Autonomous boundary and stop conditions

Autonomous after this authorization: measurement tooling, the soft
instrument extension, attribution and operator lanes, minimization, scalar
derivations, the bounded mechanism-routed GPU candidate ports as unpromoted
policies, falsification and corpus and gate runs, discard decisions strictly
by the predeclared rules, per-slice commits, and the final package.

Check back with the human only when:

1. a candidate meets every reopening-gate clause — promotion is always a
   human decision;
2. S11.4 resolves to the closure decision package — reopening the comparator
   question or accepting a different direction is a recorded human fork;
3. a correction would require leaving the predeclared candidate space (a new
   accumulation architecture, empirical attenuation knob, or coefficient
   re-selection) — stop before implementing it;
4. an authority or invariant conflict appears — preserve the reproducer and
   stop;
5. a gate could only be met by weakening a frozen band, tolerance, or
   structural contract; or
6. a required lane cannot run without changing product code paths.

Ordinary build, harness, schema, determinism, or evidence failures are
diagnosed and minimized autonomously without weakening the contract.
Negative verdicts are recorded and the program continues to its completion
condition; exhaustion with a complete decision package is a designed
outcome.

## Completion condition

The workstream is complete when either a reopening-gate survivor is packaged
for human promotion review, or the soft injector question is answered with
matched attribution, the mechanism is named with a permanent reproducer,
every routed candidate has a disposition, no unresolved mechanism-supported
correction remains inside the ownership constraints, and the S11.4 closure or
S11.5 review package is returned. Every intermediate and discarded result is
preserved in evidence and in this document. Completion never changes product
defaults, quality bands, oracle precedence, or authority interpretation.
