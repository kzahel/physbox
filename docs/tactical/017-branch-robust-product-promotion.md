# S17: branch-robust product-relative promotion study

Status: active, 2026-07-20. Human review authorizes Track A: port the one
S14 cancellation-robust separation expression onto the proven S13 X1-X4 /
current-X5 GPU pipeline and evaluate it against the current selected
hard/direct product under the already recorded product-relative material-
effect tier. This workstream stops at human promotion review. Track B, full
authority parity, is paused rather than closed; its next authority-robustness
study is not part of S17.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns the two-track decision, response gates, and program ledger;
[scalar contact specification](../topics/scalar-contact-specification.md) owns
the recurrence and S14 algebraic identity;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns the
colored execution and capacity contract; and
[performance and scale](../topics/performance-and-scale.md) owns complete-step
timing claims.

## Goal

Determine whether the already-derived S14 branch-robust formulation is a
material product improvement even though it did not satisfy every frozen
authority-parity perturbation row. Port exactly that formula onto the proven
S13 validation pipeline, compare it with the current selected product under a
fresh product-relative baseline, qualify its response and portability, and
deliver a per-scene better/worse promotion decision package.

This is one candidate, not an open formulation search:

- base execution: exact S13 X1-X4, normal-then-friction point-sequential
  recurrence, current after-each-substep X5 timing;
- sole change: replace the original analytical separation expression in the
  biased and relaxation refresh path with the frozen S14
  `authority-band-cancellation-robust-v1` displacement expression;
- coefficients: authority-capped 30 Hz / damping 10 / static×2, unchanged;
- candidate identity: `branch-robust-product-promotion-v1`;
- product status: named validation-only and unselected until human review.

## Human fork and two-track ownership

The 2026-07-20 human decision explicitly separates two questions:

1. **Track A — product promotion, active in S17.** Does the S14 formulation
   materially beat the current selected hard/direct product without losing
   required structure, contact response, portability, capacity, replay, or
   complete-step viability?
2. **Track B — full authority parity, paused.** The native authority bands,
   S14 perturbation result, S15 precision evidence, and S16 event evidence
   remain unchanged. The next authority-robustness study requires a later
   human release. S17 evidence cannot pass, fail, weaken, or close Track B.

The S14 authority-band rejection remains true historical evidence. Authorizing
Track A changes response ownership for this promotion study only: the baseline
is the current selected product and the gate is the recorded S9 product-
relative material tier. It does not reinterpret the authority bands.

## Frozen corpus and baseline

Before candidate results are retained, S17 freezes and then freshly executes
the current selected hard/direct manifold-block i2 product at the candidate
implementation revision. Product construction and every `SELECTED` descriptor
must remain identical to the S13 snapshot.

The full governed corpus is the 15-scene S9 set:

- accepted: centered 4/8/16 stacks, the permanent four-body reproducer, all
  seven S6.2 fixtures, and the circle/rectangle 100-body buckets;
- diagnostics: centered 32/64 stacks with first structural-loss step;
- 1,200 exact steps and the frozen S6.2-v2 900–1,200 settled window for every
  non-bucket scene; 600 exact steps for both buckets;
- exact replay, instrumented/final equality, finite state, and capacity
  cleanliness on every row.

The five recorded S6.4 response probes also run for selected and candidate:
bilateral impact, off-center angular response, friction, restitution, and
short-stack order. Their existing pass predicates are unchanged.

## Product-relative gate

The recorded S9 material-effect gate is applied verbatim before any promotion
recommendation:

- every accepted structure is preserved;
- exact replay and capacity cleanliness hold on every scene;
- 32/64 diagnostic first-loss steps do not regress;
- settled maximum linear speed and last-quartile kinetic energy both improve
  by at least 25% on at least two of the 4/8/16 stacks, including depth 16;
- any remaining stack regresses by no more than 5% on either metric;
- depth-16 positive-energy-run maximum does not regress and its first/last
  settled quartile envelope is non-rising under the unchanged `1e-12` /
  `1e-5` arithmetic.

The human hard requirements add these non-negotiable promotion predicates:

- the centered 16-stack preserves order and support through step 1,200 on
  Metal, the hardware-backed AVD, and physical Android;
- on every accepted 1,200-step scene, settled penetration and separation do
  not exceed the selected control by more than `2e-4 m`; final stack height
  differs by at most the larger of `2e-4 m` and the selected settled maximum
  penetration; center drift may improve but may not exceed the selected
  control by more than `2e-4 m`;
- all five response probes pass in both selected and candidate lanes;
- no accepted scene has worse final finite-state, support, failure, or
  capacity disposition;
- matched complete-step results report selected and candidate at 32, 256, and
  1,024 two-point manifolds with five warmups, twenty alternating retained
  samples, raw GPU timestamp samples, CPU encode/submit samples, work,
  dispatches, allocation, and replay. Metal candidate P50 at 256 and 1,024
  must be within +5% of the selected control; AVD and physical-device timings
  are qualification evidence and are not used as CPU-crossover claims.

These geometry margins reuse the already recorded S12/S13 absolute
penetration/height allowance; they are applied product-relative here and do
not change the authority bands or product tolerances.

Every final review row is classified independently as `better`, `same`,
`mixed`, or `worse`, with the deciding metric deltas shown. Aggregate wins
cannot hide one structural or response regression.

## Exclusions and ownership constraints

- No sleeping implementation, threshold, mask, or evidence lane.
- No second formulation, empirical damping, deadband, coefficient sweep,
  band change, comparator, approximate selected-kernel port, two-float path,
  or event-routing arm.
- No product constructor or launch default changes before the human promotion
  decision. The candidate remains explicit validation-only code.
- No change to S12/S13 authority bands, S14 perturbation outcomes, reference
  pins, oracle precedence, capacity semantics, fixed `dt`, or one-world GPU
  ownership.
- No CPU product backend, frame-advancing readback, native-only candidate, or
  host-authored scenario. Android uses the same engine candidate and shared
  scenario catalog.
- A true capacity overflow remains atomic and fail-closed. The S13 forced-
  overflow and one-below-demand fixtures rerun for the branch-robust candidate.
- An AVD is shut down immediately after its qualification/benchmark command,
  including failure or interruption. Physical Android is run first when
  attached.

## Ordered delivery slices

### S17.1 — promotion contract, ledger, and baseline ownership

- [x] Record this tactical, the exact Track A authorization, Track B pause,
  one candidate, frozen corpus, product-relative gate, geometry/response
  additions, timing matrix, exclusions, and human stop.
- [x] Add the compact S9-S16 program-status ledger to the governing topic.
- [x] Publish an executable contract binding the S9 baseline/gate, S13
  pipeline, S14 formula/proof/disposition, S15/S16 status, selected-product
  snapshot, and reference pins.
- [x] After the candidate implementation commit, freshly re-pin the unchanged
  selected product over all 15 scenes before retaining candidate corpus
  results.

Artifacts: `evidence/p5.2b-s17.1-promotion-contract/` and
`evidence/p5.2b-s17.2-selected-baseline/`.

Completion gate: the contract is committed before candidate measurement, and
the later baseline proves the current selected descriptor and constructors are
unchanged at the measured implementation revision.

S17.1 baseline checkpoint, 2026-07-20: the clean
[fresh Metal baseline](../../evidence/p5.2b-s17.2-selected-baseline/README.md)
at the committed implementation revision passes all 15 replay, final-only,
finite, capacity, accepted-structure, and five response controls. It pins the
current 4/8/16 metrics and diagnostic first-loss steps before any candidate
corpus result is retained; the selected policy remains the S13 snapshot.

### S17.2 — exact branch-robust GPU port

- [x] Add one unpromoted engine descriptor and runner entry point that reuses
  the S13 pipeline and selects only the frozen S14 expression.
- [x] Add WGSL/Rust operation-order controls proving the original and robust
  paths differ only at the declared analytical separation DAG.
- [x] Rerun the S13 two-step implementation comparison, exact replay, forced-
  overflow, one-below-demand capacity rejection, work-vector, allocation, and
  selected-policy negative controls.

Artifacts: `evidence/p5.2b-s17.3-branch-robust-gpu-port/`.

Completion gate: the implementation is finite, exact-replay, fail-closed,
product-inert, and mechanically identical to S13 except for the frozen S14
formula. An implementation defect is fixed and rerun; a structural or
capacity failure is a measured candidate rejection.

S17.2 checkpoint, 2026-07-20: the clean Apple M4 Pro Metal
[implementation package](../../evidence/p5.2b-s17.3-branch-robust-gpu-port/README.md)
passes exact series/pass/authored replay, final-only equality, the independent
Rust/WGSL robust-expression bit control, selected-policy negative controls,
the declared overflow-color schedule, and atomic one-below-demand rejection.
All 28 mapped first-step scalar/GPU pass states remain inside `5e-5`. The
two-step endpoint delta is `5.556643e-4`, outside the inherited comparison
band but below S13's already-recorded `9.260178e-4`; this historical S13 miss
is retained and does not replace the S17 product-relative quality gate.

### S17.3 — Metal full-corpus material qualification

- [ ] Run selected and candidate over the complete 15-scene corpus with the
  frozen observation policy and retain per-step evidence.
- [ ] Apply the product-relative material, 16-stack, geometry, finite-state,
  replay, capacity, and diagnostic first-loss predicates mechanically.
- [ ] Run the five selected/candidate response probes and retain raw outputs.

Artifacts: `evidence/p5.2b-s17.4-metal-product-relative/`.

Completion gate: every predicate and every per-scene delta is explicit. A
failed material gate does not authorize another candidate, but cross-host
execution still runs so the human review receives the complete requested
better/worse accounting unless execution is unsafe or impossible.

### S17.4 — complete-step cost

- [ ] Measure matched selected/candidate complete steps at 32/256/1,024
  two-point manifolds on Metal with the frozen 5/20 alternating sample policy.
- [ ] Retain dispatch, work, allocation, raw timing, replay, and capacity
  evidence; classify the +5% P50 gate at 256 and 1,024.

Artifacts: `evidence/p5.2b-s17.5-metal-complete-step/`.

Completion gate: no missing sample, adapter ambiguity, overflow, or partial-
kernel timing is accepted.

### S17.5 — physical Android and AVD qualification

- [ ] Run the same selected/candidate full corpus, response probes, and timing
  matrix on the attached physical Android device first.
- [ ] Run the same package on the hardware-backed `jstorrent-tablet` AVD,
  require a non-software adapter, and shut the AVD down immediately afterward.
- [ ] Prove the AVD is no longer running even after a failed lane.

Artifacts: `evidence/p5.2b-s17.6-android-qualification/`.

Completion gate: Metal, physical Android, and AVD each have fresh adapter,
source, scene, replay, capacity, response, and 16-stack evidence. A locked,
disconnected, or unavailable required device is a recorded blocker rather
than an inferred pass.

### S17.6 — promotion review

- [ ] Recursively verify the contract, implementation, baseline, Metal,
  timing, physical-device, and AVD packages.
- [ ] Publish the clear `better` / `same` / `mixed` / `worse` accounting for
  every scene and every host, including all regressions and timing deltas.
- [ ] Recommend promote or reject exactly this candidate, then stop before
  changing a product selector, constructor, launch default, or sleeping.

Artifacts: `evidence/p5.2b-s17.7-promotion-review/`.

Completion gate: the decision package answers whether the candidate is a
material, response-safe, portable, and cost-qualified improvement. Promotion
requires the recorded human review even when every gate passes.

## Autonomous boundary and stops

Autonomous work includes the exact port, validation-only runners, baseline
re-pin, all three host lanes, failure diagnosis inside the frozen candidate,
complete evidence/verifiers, repository gates, and per-slice commits.

Stop and return to the human:

1. at S17.6 with the promotion package, before any product selection;
2. if the frozen S14 formula cannot be represented exactly in portable WGSL
   without changing another S13 operation or contract;
3. if a required physical Android device is unavailable after all other safe
   work is complete; or
4. before any second candidate, sleeping change, authority-band change,
   coefficient variation, comparator, Track B study, or wider architecture.

## Completion condition

S17 reaches its required manual checkpoint when the one candidate has a
recursively verified better/worse decision package covering the full corpus,
response probes, Metal, physical Android, AVD, capacity/replay, and matched
complete-step timing. The product remains hard/direct until a separate human
promotion decision.
