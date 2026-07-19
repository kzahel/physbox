# S11: soft contact formulation study

Status: active, 2026-07-19. Authorized by human review at the S10.5
formulation fork recorded in the
[S10.6 review](../../evidence/p5.2b-s10.6-stationarity-review/README.md).
S10 measurably exhausted the authorized hard/direct correction space without
meeting the awake settling gate, while the pinned authority has only ever
demonstrated rest-level stillness with soft contacts and the frozen soft
lane's residual is roughly four orders of magnitude smaller in energy than
the hard one. This workstream root-causes the soft 16-stack rising-energy
mode with the S10 measurement discipline and derives at most two
mechanism-supported corrections. Every candidate is an unpromoted laboratory
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
with a minimal reproducer, and testing at most two corrections derived from
that mechanism. A survivor must pass the recorded reopening gate on Metal,
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
- At most two derived candidates, scalar first, each with at most one named
  unpromoted GPU port. Warm-start-off remains a diagnostic lane only.
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
the soft graph: compensated kinetic-energy attribution per pass family (warm
start, biased normal, tangent, normal relaxation where present, colored
support transfer, refresh, persistence, integration) at settled steps
900/1000/1100; stored-impulse vector maps labeled fixed, contraction, or
oscillation per captured pass boundary from exact settled `f32` state; the
S10.1 active-set and redistribution rules unchanged. Any S11-specific metric
must be frozen in S11.1 before measurement.

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

- [ ] Freeze the soft-graph attribution pass-family list, any S11-specific
  metric definitions, verdict rules, routing, and evidence naming before
  measurement.
- [ ] Extend the version-3 matched instrument to the frozen soft 16-stack
  graph without changing product paths; the scalar mirror must retain its
  proven pass-boundary exactness on the soft composition.
- [ ] Run matched-precision per-pass compensated energy attribution on the
  soft 16-stack at settled steps 900/1000/1100 with exact replay, and record
  the permanent four-body soft lane as secondary context.

Routing (predeclared): a material positive pass family routes hypothesis 1
or 2 by which family carries it, and S11.2 leads with that family's operator
map. No material single family routes hypothesis 3, and S11.2 leads with the
whole-recurrence discrete-energy accounting. A contradictory or ambiguous
attribution is named input to S11.2, not a rule change.

Artifacts: `evidence/p5.2b-s11.1-soft-pass-attribution/`.

Completion gate: exact replay and capacity cleanliness on every lane; the
attribution sums reconcile with the observed energy envelope; the routing
verdict is recorded here with its numbers.

Commit boundaries: contract and instrument extension; clean attribution
evidence.

### S11.2 — soft operator study, mechanism, and minimal reproducer

- [ ] Single-step operator study from captured settled soft `f32` state using
  the matched scalar mirror: label every pass-boundary stored-impulse map
  fixed, contraction, or oscillation, and name which family carries each
  change under the frozen S10.1/S10.3 rules.
- [ ] Depth sweep over the accepted centered stacks (`2, 4, 8..=16`) for the
  smallest structure exhibiting the identified soft signature; preserve it as
  a permanent reproducer with exact prefixes.
- [ ] Record the mechanism verdict: what a correction must change and what it
  must provably preserve (unilateral response, penetration correction,
  deterministic colored support transfer, refreshed geometry, final
  persistence, and soft impact response without sag).

Outcome: a named soft mechanism with a minimal reproducer, or a measured
demonstration that the drift is secular recurrence behavior, routing the
candidate space accordingly.

Artifacts: `evidence/p5.2b-s11.2-soft-mechanism/`.

Completion gate: the mechanism verdict is specific enough to derive a
correction from; recorded here with its reproducer hashes.

Commit boundaries: operator tooling; mechanism and minimization evidence.

### S11.3 — bounded corrections (at most two, scalar first, falsify early)

- [ ] Select at most two candidates strictly routed by the S11.1/S11.2
  verdicts, each with a scalar stationarity or discrete-energy argument.
  Free-form attenuation is outside the space.
- [ ] For each candidate, in order and failing closed at each step: the
  scalar argument; scalar quality on the two-step contract fixture, the
  permanent four-body reproducer, the S11.2 reproducer, and the full-length
  scalar 16-stack (support transfer and penetration preserved; quiet-by-sag
  rejected); the single named unpromoted GPU port; the early GPU
  falsification gate; the implementation gate (descriptor-exact control,
  dispatch-form exactness, fail-closed negatives, repository/platform
  gates); then the full-corpus evaluation against the recorded reopening
  gate.
- [ ] Record every candidate disposition — including a scalar-stage or
  falsification kill — as a dated checkpoint with evidence.

Artifacts: `evidence/p5.2b-s11.3-candidate-<name>/` per candidate.

Completion gate: both candidate slots have dispositions, or one survivor
advances with the other slot's disposition recorded.

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
derivations, the at-most-two GPU candidate ports as unpromoted policies,
falsification and corpus and gate runs, discard decisions strictly by the
predeclared rules, per-slice commits, and the final package.

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
both candidate slots have dispositions, and the S11.4 closure or S11.5
review package is returned. Every intermediate and discarded result is
preserved in evidence and in this document. Completion never changes product
defaults, quality bands, oracle precedence, or authority interpretation.
