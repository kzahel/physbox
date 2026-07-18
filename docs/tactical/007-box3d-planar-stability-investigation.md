# S7: Box3D planar stability investigation

Status: manual checkpoint, 2026-07-18. The clean v0.1.0 source study is
complete and recommends one static-late PhysBox order factorial. No
Box3D-derived PhysBox comparator, physical-default change, or authority change
is authorized until human review.

Governing topic:
[Box3D planar stability reference](../topics/box3d-planar-stability-reference.md).
Related work:
[S6 awake settling stabilization](006-awake-settling-stabilization.md) and
[S5 interactive rigid quality and scale](005-interactive-rigid-quality-and-scale.md).

## Goal

Determine whether Box3D's impressive planar and large-pile stability exposes a
dimension-independent contact, ordering, or solve-staging mechanism that can
explain a material PhysBox settling gap. Produce enough objective sleep-off
evidence to either justify at most two bounded future comparators or close the
reference study without product changes.

## Exclusions and ownership constraints

- Box3D is a source reference and optional process-isolated comparator. It is
  not a rigid authority, product dependency, CPU fallback, or second engine.
- Box2D retains rigid behavior authority; scalar invariants retain
  mathematical/pass precedence.
- The clean checkout is never edited or built in tree. Internal instrumentation
  requires a separate base-pinned, patch-digested microscope.
- No source is copied without explicit provenance and license preservation;
  prefer independent public-contract instrumentation.
- Do not reinterpret four-point manifolds, two-axis friction, twist/rolling
  resistance, or 3D inertia as direct 2D parity.
- Do not change PhysBox solver equations, accumulation, graph lifetime/order,
  friction staging, sleeping, quality bands, tolerances, or product defaults in
  this workstream without a new human decision.
- Stock Box3D timing is context only. PhysBox/Box2D performance claims continue
  to use the governing performance-and-scale contract.
- Keep all generated builds outside the checkout and re-run
  `npm run references:status` after every reference execution.

## Ordered delivery slices

### S7.1 — provenance, scope, and predeclared contract

- [x] Pin the clean MIT-licensed Box3D v0.1.0 checkout at
  `8441b4a06d6d09dcfb0b0f704df4d847d1437b92`.
- [x] Record the planar motion-lock mapping and the remaining 3D manifold,
  friction, and inertia differences.
- [x] Predeclare ranked hypotheses, exact-step observation points, metrics,
  forwarding gates, authority boundaries, and microscope trigger.
- [x] Add the governing topic and this tactical to their indexes.

Outcome: the study has a reproducible source and cannot turn a favorable visual
result into post-hoc product policy.

Validation: `npm run references:status`, `npm run check:boundaries`,
`cargo fmt --all -- --check`, and `git diff --check`.

Artifacts: tracked pin/reference map and the topic/tactical contract.

Completion gate: all scope, authority, projection, and forwarding boundaries
are explicit before a runtime result is measured.

Commit boundary: documentation and study contract only.

### S7.2 — clean build and stock reproduction controls

- [x] Configure an out-of-tree Release build with explicit SIMD, validation,
  worker, compiler, and dependency-cache settings; keep the checkout clean.
- [x] Build and run the upstream unit-test target.
- [x] Run the unmodified `large_pyramid` benchmark with one fixed worker and
  its stock 200-step/four-substep contract. Record counters and wall time only
  as a reproduction control.
- [x] Attest the stock `Pyramid2D` source, motion locks, dimensions, sleep
  default, and the `CreateLargePyramid` sleep-off setup directly from the pin.
- [x] Publish fresh provenance and a success sentinel, then rerun reference
  status.

Outcome: the pinned release builds and its relevant stock sources execute
without modifying the checkout.

Validation: exact configure/build/test/benchmark commands; fresh manifest;
`npm run references:status`; package verifier; repository boundary and format
checks.

Artifacts: `evidence/p5.2b-s7.2-box3d-stock/` with manifest, raw command output,
source attestations, and verifier.

Completion gate: tests pass, the stock benchmark exits successfully, source
and build provenance are exact, and the reference checkout remains clean.

Commit boundary: stock evidence and verifier only; no adapter or PhysBox code.

Completion checkpoint, 2026-07-18: the
[verified stock package](../../evidence/p5.2b-s7.2-box3d-stock/README.md)
records a clean out-of-tree Apple M4 Pro build with Release, SIMD, validation,
tests, and benchmark settings explicit. All upstream tests pass. The
unmodified one-worker 90-row benchmark completes 200 steps with 4,096 bodies,
12,105 contacts, and an upstream-reported 1,163.77 ms timed run. This is only a
build/execution control: no body-state observer exists yet, so no settling,
long-run structure, or cross-engine performance conclusion is drawn. S7.3 is
forwarded unchanged.

### S7.3 — public-API sleep-off observer

- [x] Add a process-isolated external harness that links the clean pin and
  independently authors the stock 12-row planar-locked pyramid and 90-row
  unlocked 3D pyramid without modifying Box3D.
- [x] Freeze 60 Hz, four substeps, one worker, sleeping disabled, stock contact
  recycling, initial transforms/materials, and steps 0/200/1,200/5,000.
- [x] Record body transforms, residual speeds, mass/inertia kinetic energy,
  structure/loss, contact/manifold/point counts, feature persistence/churn,
  separation, normal impulse, and central friction impulse through public APIs.
- [x] Keep every-step settled-window observations for the 12-row source and
  bounded checkpoints for the 4,095-body source.
- [x] Add deterministic replay, finite-value/schema checks, fresh atomic
  artifacts, and a success sentinel.

Outcome: visual tower stability becomes objective long-run awake evidence.

Validation: repeat each source twice; require byte-identical deterministic
results where the upstream contract promises determinism; compare body/contact
counts with source arithmetic; package verifier; reference status; normal
repository gates for new tooling.

Artifacts: `evidence/p5.2b-s7.3-box3d-planar-observer/`, including compact
tables and smallest observed structural loss.

Completion gate: the study can say whether each stock source stays upright,
loses structure, settles, or retains energy through 5,000 steps without using
sleeping.

Commit boundaries: harness/schema/tests first; clean long-run evidence and
interpretation separately.

Completion checkpoint, 2026-07-18: the
[verified public-API package](../../evidence/p5.2b-s7.3-box3d-planar-observer/README.md)
records two byte-identical 5,000-step executions per source from a clean pin.
The sleep-off 12-row motion-locked pyramid retains structure and stays below
the 0.05 m/s near-rest bar for every step from 900 onward. The 4,095-body
source is confirmed to be an unlocked 3D sheet, not a 2D mode, but it also
retains structure and reaches 4.30e-4 m/s maximum linear speed by step 5,000.
Both sources retain four persisted points per manifold with no fresh settled
points. The large source recycles all 12,105 contacts with zero settled SAT
calls; the small source requires one SAT call at step 2,907 across 4,101
settled observations. Static contacts occupy the dedicated color 22. No
structural loss exists to minimize. S7.4 is forwarded to distinguish the
dimension-independent lifetime/order mechanisms from four-point and 3D
confounders.

### S7.4 — mechanism differential and forwarding decision

- [x] Compare pinned Box3D, Box2D, and current PhysBox source contracts for
  contact identity/cache lifetime, color assignment/lifetime/priority, point
  ordering, warm start/store, normal/tangent ordering, friction staging,
  relaxation, and sleep.
- [x] Correlate the source differences with S7.3 feature churn, impulse, energy,
  and structural observations; distinguish evidence from inference.
- [x] Reject 3D-only or visually flattering explanations explicitly.
- [x] Name no more than two dimension-independent questions worth a neutral
  adapter or microscope, or close the study with no forwarded work.

Outcome: runtime results identify mechanisms rather than merely ranking
engines.

Gate: every forwarded hypothesis has a measured symptom, a specific source
difference, a cheapest falsifier, and a material decision it could change.

Artifacts: source-location matrix, observation correlations, rejected
alternatives, recommendation, and reconsideration triggers.

Commit boundary: differential report only.

Completion checkpoint, 2026-07-18: the
[verified differential](../../evidence/p5.2b-s7.4-box3d-mechanism-differential/README.md)
rejects sleeping, four-point/3D support, a manifold normal block,
relaxation-only friction, soft coefficients alone, and color lifetime alone as
the transferable explanation. Box3D's whole-contact recycling is forwarded to
one public-API on/off ablation in S7.5. The only product-facing question is the
static-late color priority shared by Box2D and Box3D: existing PhysBox controls
show lifetime alone is behaviorally identical, while changed assignment becomes
material at the 16-stack. Any PhysBox static-late factorial remains stopped for
human review.

### S7.5 — conditional neutral adapter or microscope

- [x] Proceed only if S7.4 forwards a specific unresolved hypothesis.
- [x] Prefer the clean process-isolated public API and use a stock-source
  recycling ablation instead of a neutral projection because the question is
  whether Box3D's own result depends on that public body policy.
- [x] Use a separate patch-digested microscope only if public observations
  cannot distinguish retained color order or per-pass energy.
- [x] Avoid a neutral adapter after the public ablation resolves the question;
  any future adapter must reuse the shared PhysBox scenario source and hashes.
- [x] Stop before implementing any Box3D-derived PhysBox solver comparator.

Outcome: one narrowly scoped diagnostic answers the surviving question without
changing authority or product code.

Gate: exact projection/provenance, deterministic replay, clean authority
separation, and a result that resolves the predeclared question.

Commit boundaries: adapter contract; optional microscope patch/digest and
automation; clean evidence; never mixed with product integration.

Completion checkpoint, 2026-07-18: the
[verified 5,000-step ablation](../../evidence/p5.2b-s7.5-box3d-recycling-ablation/README.md)
changes only public contact recycling and repeats both arms byte-identically.
The off arm remains upright and near rest for every settled observation, while
its peak speed and energy rise by 77.4% and 94.4% and its settled SAT work rises
from one call to 861,210. Recycling is a strong work/noise optimization, not a
requirement for the measured awake stability. The expensive unlocked 3D source
and an internal microscope are skipped by the predeclared cheapest-falsifier
branch. No PhysBox recycling comparator is forwarded; the human-reviewed
static-late ordering question remains.

### S7.6 — human review package

- [x] Assemble the compact source, runtime, energy, contact, structural, and
  provenance evidence with a specific recommendation.
- [x] State whether Box3D changed the explanation of PhysBox's settling gap,
  what was rejected, and what remains unknown.
- [x] Return for human review before any change to manifold accumulation,
  coloring lifetime/priority, friction staging, or physical policy.

Outcome: the reviewer can accept closure or authorize a named comparator
without inspecting the engines.

Completion gate: the package verifies from a clean tree, every claim links to
raw evidence, and no product or authority setting changed.

Commit boundary: review package and tactical/topic status only.

Completion checkpoint, 2026-07-18: the
[verified compact package](../../evidence/p5.2b-s7.6-box3d-review/README.md)
links and re-verifies every S7 evidence slice. The study rejects sleeping,
direct 3D support transfer, normal-block attribution, recycling as a stability
requirement, retained color lifetime alone, Box3D-only friction staging, and a
soft-policy reopening. It recommends one unpromoted static-late order factorial
with hard/direct manifold-block i2 fixed and a predeclared material-effect
gate. The workstream stops at this manual checkpoint for authorization or
closure.

## Autonomous boundary and stop conditions

S7.1 through S7.4 are autonomous under the approved source investigation.
S7.5 may implement only a process-isolated diagnostic already justified by
S7.4; it may not change PhysBox. Stop for human review before any product
comparator or physical-policy experiment.

Stop early if the clean pin cannot build without a source patch, upstream
public data is insufficient and no single microscope question survives, or a
result would require changing Box2D/invariant precedence. Ordinary build,
harness, schema, determinism, or evidence failures are diagnosed and minimized
without weakening the predeclared contract.

## Completion condition

The workstream is complete when the pinned stock sources have objective
sleep-off long-run results, the relevant Box3D/Box2D/PhysBox mechanisms are
compared, unhelpful hypotheses are rejected, and either no further work is
recommended or at most two bounded PhysBox experiments are presented for human
authorization. Completion never promotes Box3D or changes the product.
