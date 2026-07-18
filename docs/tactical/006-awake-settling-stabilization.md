# S6: Awake settling stabilization

Status: S6.5d scalar-first recurrence derivation active, 2026-07-18. S6.1–S6.4
are complete and the S6.5c no-survivor result is accepted. One analytically
justified, scalar-qualified coupled candidate may reach the GPU. Hard/direct
manifold-block i2 remains the product; every settling candidate remains
unpromoted.

Governing topic:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md).
Related work:
[S5 interactive rigid quality and scale](005-interactive-rigid-quality-and-scale.md),
[contact solver parallelism](../topics/contact-solver-parallelism.md), and
[collision and contact persistence](../topics/collision-and-contact-persistence.md).

## Goal

Make ordinary rectangles visibly come to rest while sleeping remains disabled,
using objective replayable evidence rather than subjective tuning alone. A
reviewer should be able to compare the current product, each serious candidate,
and pinned Box2D on perfect stacks and realistic seeded drops without reading
the implementation.

The workstream ends with an accepted awake contact policy, a cross-host
quality package, and an explicit handoff to broadphase selection and sleeping.
It does not itself select a broadphase or sleep policy.

## Ownership constraints

- Shared Rust owns sources, actions, completion, observations, settings, and
  neutral projection. Hosts transport input and presentation only.
- The selected product and every candidate use the same GPU body state,
  generated contacts, pair slots, manifold representation, solver family,
  renderer, frame path, and no-catch-up clock.
- Clean pinned Box2D v3 is the behavior authority; independent invariants own
  mathematical correctness. Direct authority results and optimized CPU timing
  remain separate processes and artifacts.
- Sleeping is disabled in every acceptance run. No body may be frozen to pass
  the awake gate.
- A candidate may exist behind an explicit laboratory launch option, but no
  contact-identity or physical coefficient becomes the product default without
  human review.
- Capacity rejection remains atomic. No CPU physics fallback, CPU body mirror,
  required readback, native-only path, or hidden dropped contact is permitted.

## Exclusions

This tactical excludes a new solver family, different accumulation
architecture, broadphase selection, sleep implementation, CCD, joints,
polygons, capsules, liquid, editable materials, pacing changes, tolerance
widening, oracle changes, and deployment. It does not promise Box2D's exact
trajectory or low-microvelocity residual as a hard gate.

## Ordered delivery slices and commit boundaries

### S6.1 — scheduler baseline and comparison control

- [x] After explicit approval, promote the separately reviewed S5.4c bounded
  parallel scheduler without changing manifold-block i2 or physical settings.
- [x] Retain the accepted serial full-rebuild/per-color execution as an
  explicit validation and direct-launch control.
- [x] Prove scenario, policy, assignment/order, replay, capacity, and query
  ownership through `physbox-launch`; do not create a second engine path.

Integration checkpoint, 2026-07-17: product constructors now select bounded
full-rebuild parallel speculative coloring with per-color indirect dispatch.
`solver-parallel-coloring` exercises that selected policy explicitly, while
`solver-manifold-i2` names the preserved serial full-rebuild/per-color control.
Historical arbitrary-policy factorial helpers continue to use the serial
control so promotion cannot silently relabel old evidence. Physical
coefficients, manifold-block i2, substeps, iterations, tolerances, and oracle
precedence are unchanged. Clean host evidence and the compact promotion
manifest remain the final S6.1 gate.

Completion checkpoint, 2026-07-17: the clean
[promotion package](../../evidence/p5.2b-s6.1-scheduler-promotion/README.md)
pins both policy digests and the immutable S5.4c 7/7 quality, exact-replay, and
capacity result. Native Metal, Chrome BrowserWebGPU, and the hardware-backed
`jstorrent-tablet` AVD pass selected and serial scenes with identical source
and solver execution. All retained frames are visually complete. An attached
Pixel 9 was locked, so the physical lane stopped before install and is an
explicit availability gap. Bounded parallel scheduling is now the product
standard; serial remains the named direct control.

Outcome: settling experiments run on the intended product schedule and can
attribute any behavior difference against the preserved serial control.

Likely boundaries: `physbox-launch`, engine configuration and scheduler policy,
shared validation scenarios, browser query mapping, Android argv transport,
and scheduler governing docs. No contact equations change.

Validation: repository gates; generated-contact exact replay; the seven S5.4c
accepted structures; scheduler capacity/33-color rejection; native Metal,
Chrome BrowserWebGPU, hardware-backed `jstorrent-tablet` AVD, and an available
physical Android device; fresh visually inspected captures.

Artifacts: a compact promotion manifest with both policy digests, exact
settings, cross-host reports, fresh provenance, and success sentinels.

Documentation committed with the slice: S5.4c acceptance/integration status,
contact-solver topic status, launch help/query contract, and this checkbox.

Gate: bounded parallel scheduling is the one product standard; serial remains
explicitly named and byte-equivalent to its accepted control; physical values,
solver family, tolerances, and oracle results are unchanged.

Commit boundary: scheduler promotion and controls only.

### S6.2 — settling observations and realistic shared fixtures

- [x] Add versioned reference-owner, feature transition, point-count,
  contact-normal, cache miss-reason, time-to-rest, angular-edge-speed, and
  kinetic-energy-envelope observations.
- [x] Freeze observation cadence, stable-support classification, settled
  windows, and gate arithmetic before running candidates.
- [x] Add shared seeded near-flat floor drops, box-on-resting-box drops,
  sequential 3–5 rectangle construction, slide-to-rest, and a small mixed pile.
- [x] Project the same sources to Box2D and add exact deterministic replay,
  random generation, minimization, and permanent reproducers.

Contract checkpoint, 2026-07-17: schema version 2, the exact step/window
arithmetic, actual GPU manifold/cache observation, seven shared neutral
fixtures, Box2D projection, deterministic random-source selection, and a
dense-slot-preserving minimizer are implemented without changing a solver
setting. The bounded recorder retains GPU-ordered per-step copies and maps only
after the final step. Short replay, cadence rejection, classification,
projection, and minimization tests pass. The final checkbox remains open until
the clean baseline/authority artifacts and permanent observed reproducers are
published.

Completion checkpoint, 2026-07-17: the clean
[S6.2 package](../../evidence/p5.2b-s6.2-settling-baseline/README.md) retains 11
canonical one-step-cadence GPU traces, 88 fresh isolated Box2D runs, 14 random
sources, one minimized unique failure family plus its distinct second seed,
and synchronized playback. All selected-GPU runs are exact, uninstrumented-
equivalent, capacity-clean, and structurally valid; all authority finals replay
exactly. Three sources pass the full gate and eight preserve a measured
baseline failure. Every unexplained settled feature change exactly coincides
with a reference-owner change and two cache misses, with zero point-count or
material-normal changes. No candidate or product default exists in the
package. S6.3 may now test that attributed mechanism in isolation.

Outcome: the existing perfect-stack evidence is complemented by repeatable
human-realistic motion, and every proposed cause can be measured directly.

Likely boundaries: `physbox-lab` versioned metrics/traces, shared scenario and
neutral projection modules, Box2D oracle protocol/worker, headless report
contracts, validation runners, and evidence verifier. Existing schema versions
remain readable.

Validation: schema round trips and negative fixtures; scalar/GPU observation
agreement; exact replay; process-isolated authority runs; deterministic seeded
generation/minimization; repository gates. Run native first, then browser and
Android only after the shared contract is stable.

Artifacts: frozen source/projection hashes, baseline and authority traces,
cache/churn attribution tables, time-series data, minimized baseline failures,
and synchronized baseline/Box2D playback.

Documentation committed with the slice: metric/fixture contract, scenario
catalog, evidence README, governing topic evidence links, and this checkbox.

Gate: all fixtures are host-neutral and replay exactly; instrumentation does
not change body/contact results; legitimate impact/rolling/recontact is not
misclassified as settled support churn; no candidate result exists yet.

Commit boundary: contracts and fixtures only, with no physical-default change.

### S6.3 — contact-identity attribution and comparators

- [x] Record the exact chain from reference-owner or clipped-feature change to
  cache hit/miss, accumulated impulse, velocity response, and later topology.
- [x] Implement isolated deterministic near-tie ownership, true prior-contact
  hysteresis, and safe canonical-correspondence comparators where justified.
- [x] Preserve normal/topology safety, pair generations, point uniqueness,
  exact replay, and atomic capacity behavior.
- [x] Reject or minimize every comparator that transfers an impulse across an
  incompatible normal, destabilizes a seed, or merely trades churn for drift.

Outcome: contact-identity churn is either proven causal and reduced, or ruled
out as the dominant settling mechanism with permanent evidence.

Likely boundaries: scalar analytic narrowphase, portable narrowphase WGSL,
pair-slot cache diagnostics, versioned lab settings, comparator launch names,
and targeted collision/persistence tests. The product policy stays unchanged.

Validation: scalar/GPU feature agreement; randomized/adversarial rectangles;
pair erase/reuse/recontact; exact replay; first divergence; baseline and
realistic fixtures; direct public Box2D narrowphase retained as declared
authority evidence without demanding identical valid tie choices.

Artifacts: feature/cache causality matrices, minimized flapping and unsafe-
transfer cases, candidate playback, complete cost, and rejected alternatives.

Documentation committed with the slice: provisional identity conclusions,
candidate status, validation commands, evidence links, and this checkbox.

Gate: at least one conclusion is attributable; no comparator is silently
promoted; geometry tolerances and authority precedence remain unchanged.

Commit boundary: identity instrumentation and each materially distinct
comparator may be separate commits; the clean comparison/recommendation is its
own commit.

Completed 2026-07-17. The clean
[S6.3 package](../../evidence/p5.2b-s6.3-contact-identity/README.md) records 44
exact per-step traces, 112 fresh randomized final executions, matched timing,
and synchronized playback. The selected direct policy records 3,068 settled
owner/feature changes and 6,136 cache misses. Stateless relative and canonical
coherence reduce churn but catastrophically lose the 16-stack at 10.38 and
8.42 m/s, so both are rejected. The bounded prior-contact lane uses the
existing sorted generational pair records, changes geometry only within a 1 mm
band with identical point count and compatible normal, and records zero
settled owner changes or cache misses. It adds one dispatch and no allocation.

Identity is causal but insufficient: prior-contact hysteresis brings the
4-stack into the `4.59e-6 m/s` band and passes 7/11 sources, while the 8-stack
still rings at `0.4005 m/s` and the 16-stack remains just outside structure and
speed bands. Prior-contact hysteresis is forwarded only into the S6.4 combined
candidate. It is not selected. Cross-feature impulse correspondence is
rejected because no remaining identity failure justifies an unproven transfer.

### S6.4 — soft-contact factorial and combined candidate

- [x] Reproduce the pinned Box2D hertz/damping-to-coefficient equations in the
  scalar specification with explicit provenance, then implement the portable
  GPU comparator independently against that contract.
- [x] Measure bounded contact-hertz, damping-ratio, and static-contact variants
  against the unchanged `0.2/1/0` control.
- [x] Compare identity-only, softness-only, and combined candidates over all
  accepted and realistic sources at complete-step cost.
- [x] Preserve impact, friction, restitution, penetration, stack height,
  structure, capacity, and deterministic replay; minimize sag, ringing, or
  slow-response failures rather than hiding them in aggregate scores.

Outcome: one compact package identifies whether contact softness supplies the
missing damping and whether it composes safely with contact identity changes.

Likely boundaries: scalar contact settings/preparation, solver settings,
generated-contact WGSL coefficient preparation, shared launch candidates,
factorial runner, and evidence verification. Product defaults remain fixed.

Validation: analytic and metamorphic coefficient tests; scalar/GPU first-
divergence traces; full fixture/seed matrix; pinned authority and optimized CPU
lanes; native/browser/AVD/available-device candidate execution; complete GPU
timing and work vectors; repository gates.

Artifacts: objective quality/cost matrix, synchronized current/candidate/
Box2D playback, direct launch instructions, minimized failures, cross-host
captures, raw samples, recommendation, and reconsideration triggers.

Documentation committed with the slice: scalar provenance, measured
alternatives, evidence package, topic conclusion, and this checkbox.

Gate: candidates are correctness- and capacity-qualified; the report names
every physical response and performance tradeoff; no default or quality band
is accepted automatically.

Factorial checkpoint, 2026-07-17: the clean
[S6.4 GPU package](../../evidence/p5.2b-s6.4-soft-contact/README.md) screens eight
bounded coefficient triples and retains both top static-multiplier variants
after a development run exposed a material identity interaction. The combined
`30 Hz / damping 10 / static×1 / prior-contact` lane ranks first at 9/11 gates,
preserves 11/11 structures, records zero settled churn, and stays below the
`0.025 m/s` speed bands. It adds one dispatch and no allocation. Its 16-stack
and small mixed pile still fail the frozen rising-energy gate, so promotion is
not authorized and failure reduction/authority/cross-host evidence remain.

Failure-minimization checkpoint, 2026-07-17: the clean
[minima](../../evidence/p5.2b-s6.4-soft-contact/failures/README.md) preserve the
leader's two energy families without changing the frozen cadence or gate. A
depth sweep finds a non-monotonic small-signal boundary at 9 bodies, while the
16-stack is the severe mode with 300 rising intervals. Dense reduction shrinks
the mixed pile from 8 to 4 dynamic bodies in 37 GPU attempts and retains an
exact, capacity-clean 1,200-step reproducer. All governed response probes,
penetration/height records, structures, capacity, and replay are now retained;
fresh authority and cross-host review evidence remain before S6.5.

Authority/cost checkpoint, 2026-07-17: the clean
[authority package](../../evidence/p5.2b-s6.4-soft-contact/authority-performance/README.md)
runs unmodified pinned Box2D twice at step 1200 on all 11 sources and retains
600/900 checkpoints for four review scenes. Every payload replays exactly.
The leader is quality-qualified inside the `0.025 m/s` speed band but remains
far above Box2D on the 16-stack (`0.02324` versus `0.000158 m/s`). At 1,024
bodies it records a complete 7.917 ms Metal P50, essentially neutral against
hard/direct's 7.875 ms and inside 60 Hz, but optimized four-worker Box2D is
0.132 ms. The roughly 60× CPU gap is explicit; cross-host launch/capture and
the compact S6.5 review package remain.

Cross-host checkpoint, 2026-07-17: all four direct hard/candidate 8- and
16-stack lanes reach at least 1,200 steps with clean capacity on native Metal,
Chrome WebGPU, and the hardware-backed `jstorrent-tablet` AVD. Visual review
finds the candidate 8-stack centered everywhere and both 16-stack lanes
centered on Metal/Chrome, but the candidate 16-stack is collapsed on AVD
Vulkan by captured step 1,203 while the AVD hard control remains centered. The
[checked platform package](../../evidence/p5.2b-s6.4-settling-cross-host/README.md)
therefore fails the portability structure gate and blocks promotion. No
physical Android device was connected. The AVD pacing samples are explicitly
not a cost comparison because unrelated emulator load caused variable slip.

Commit boundary: scalar contract, GPU comparator, factorial automation,
failure minima, authority/cost, direct shared lanes, cross-host automation,
and clean evidence are independently reviewable commits.

### S6.5 — mandatory visible-quality review

- [x] Stop and present the S6.4 package before changing product contact
  identity or physical coefficients.
- [ ] Ask the reviewer to judge synchronized playback and, after they choose
  to deploy the named commit, the direct web candidate against the current
  baseline on touch and desktop.
- [x] Record acceptance, rejection, requested follow-up, and the exact chosen
  settings without promoting them in the review-record commit.

Outcome: human judgment resolves the visible feel/quality tradeoff with the
objective matrix beside it.

Artifacts: a short decision report, three-way synchronized playback, URL and
native launch recipes, candidate hashes, objective gate table, costs, known
failures, and a one-paragraph recommendation.

Gate: explicit human selection or direction. This is the normal unattended
stopping point.

Commit boundary: decision record only after human response.

Review checkpoint, 2026-07-17: the clean
[S6.5 package](../../evidence/p5.2b-s6.5-settling-review/README.md) composes the
frozen factorial, Box2D authority, optimized CPU timing, minimized failures,
and native/browser/AVD evidence without rerunning or changing them. Its
three-way exact-step playback shows the candidate's large 8-stack improvement
beside Box2D and the promotion-blocking AVD 16-stack collapse. The package
recommends holding promotion and authorizing a targeted cross-backend
diagnosis/minimization before another review. Product defaults, physical
coefficients, tolerances, and oracle precedence remain unchanged. The next two
web/deployed-feel checkbox remains open because no comparison commit has been
pushed or deployed.

Decision record, 2026-07-17: human review accepted the package recommendation.
The exact `prior-contact-hysteresis / 30 Hz / damping 10 / static×1` candidate
remains the unpromoted laboratory leader. Review explicitly authorized a
targeted AVD diagnosis and minimization, with the first question being whether
the Vulkan collapse and the already-minimized rising-energy modes share one
feedback root cause. This acceptance does not authorize coefficient retuning,
a widened identity band, a changed quality gate, a production default, or a
portability exception.

### S6.5a — authorized portability and energy diagnosis

- [x] Reproduce candidate 16 on Metal and the hardware-backed AVD at exact
  checkpoints; sweep stack depth and first structural divergence without
  changing the frozen source, solver, scheduler, or gate.
- [x] Run hard/direct, identity-only, softness-only, and combined controls to
  attribute the failure to identity state, soft-contact feedback, their
  interaction, or backend execution.
- [x] Compare the 16-stack and minimized four-body energy reproducer using
  bounded final-flush body, energy, contact, cache, and identity traces. No
  interactive frame may require readback.
- [x] Minimize any backend divergence into a permanent shared scenario and
  first-divergence artifact with exact replay, capacity, provenance, and
  success sentinel.
- [x] Implement only reversible laboratory corrections justified by the
  traces; do not retune coefficients or promote a policy speculatively.
- [x] Re-run the frozen quality, energy, response, capacity, performance,
  authority, browser, AVD, and available physical-device matrix for every
  forwarded correction. No correction was forwarded, so the full matrix was
  not triggered; each rejected comparator has its bounded clean matrix.
- [x] Publish a compact diagnosis and recommendation, then stop before any
  product-default change.

Outcome: determine whether the AVD collapse is the retained energy instability
crossing a backend-dependent threshold or a distinct identity/backend defect,
and preserve the smallest reproducer for either conclusion.

Gate: a conclusion must be supported by comparable exact-step traces on Metal
and AVD. A correction advances only if it removes the backend structure loss
and the attributable energy failure without weakening structure, response,
capacity, portability, or complete-step cost. Missing physical hardware is
reported as a gap, never inferred as a pass.

Commit boundary: decision record; diagnostic contract; each distinct
minimizer or correction; clean cross-host evidence; and final recommendation
are separate reviewable commits.

Factorial-attribution checkpoint, 2026-07-17: the clean
[S6.5a package](../../evidence/p5.2b-s6.5a-contact-attribution/README.md)
crosses direct/prior identity with hard/soft contact on the same 16-stack at
or after step 1,200 on native Metal, Chrome WebGPU, and the hardware-backed
`jstorrent-tablet` AVD. Hard/direct remains centered everywhere. Identity-only
collapses everywhere; softness-only collapses on Metal/AVD and is strongly
leaning on Chrome. Combining them restores the stack on Metal/Chrome but not
on AVD Vulkan. The desktop success is therefore a non-additive,
backend-sensitive cancellation between two individually unstable changes,
not robust promotion evidence. The package does not yet name the numerical
first divergence; bounded four-policy traces remain the next gate. The
maintained physical-device lane was attempted and found no connected device.
Product defaults, coefficients, tolerances, solver, scheduler, capacity, and
oracle precedence remain unchanged.

Trace-contract checkpoint, 2026-07-17: the shared generated-contact runner now
supports an explicit one-outer-step-per-submission diagnostic cadence while
retaining GPU ordering and mapping only after the final requested step. The
native `physbox-settling-backend-trace` adapter and
`android/trace-settling.sh` run the same shared 16-stack policies and permanent
four-dynamic-body energy reproducer on Metal or Android Vulkan. The minimized
source is reconstructed by the engine-owned scenario builder and locked to its
published canonical hash; neither adapter authors a fixture or advances an
interactive frame from readback.

Cross-backend trace checkpoint, 2026-07-17: the clean
[trace package](../../evidence/p5.2b-s6.5a-backend-traces/README.md) preserves
all four 16-stack controls plus the minimized four-body reproducer on Metal and
the hardware-backed AVD at exact per-step cadence. Candidate body values differ
at step 1, but feature topology and cache keys remain identical through step
796. Vulkan has already accumulated 298 consecutive frozen-epsilon energy
increases from steps 499 through 796; its first identity transition occurs at
797 and structural loss follows at 892. The minimized case has 300 settled
energy increases and zero settled churn/cache misses on both backends, with
Vulkan reaching 168.59× Metal's last-quartile peak energy. Therefore the AVD
collapse is the shared contact-feedback instability amplified earlier, with a
secondary identity transition after the unstable run is established—not a
discrete identity flip initiating a separate failure. Promotion remains
blocked. A depth sweep and smallest AVD structural reproducer remain open.

Depth-sweep contract checkpoint, 2026-07-17: the bounded trace adapter accepts
every candidate stack depth from 8 through 16, all built by the same shared
centered-stack and frozen review-policy functions. This extends diagnostic
source coverage only; product launch aliases, defaults, coefficients, and
accepted quality bands are unchanged.

Depth-minimum checkpoint, 2026-07-17: the clean
[depth package](../../evidence/p5.2b-s6.5a-depth-minimum/README.md) sweeps every
candidate depth from 8 through 16 on Metal and the hardware-backed AVD. Metal
retains structure at every depth. AVD depths 8 through 14 retain structure,
depth 15 first fails at step 1,057, and depth 16 first fails at step 892. Depth
15 is therefore the smallest governed structural reproducer and depth 14 its
immediate structural control. Both retained depth-15 traces replay exactly and
all retained samples remain capacity-clean. Depth 14 is not a quality pass: it
has 359 consecutive frozen-epsilon energy increases before its first feature
transition. Depth 15 similarly has 311 increases before its feature transition
and loses structure 108 steps later. The backend minimization and first-
divergence gates are complete; reversible laboratory correction remains next.

Single-iteration diagnostic contract checkpoint, 2026-07-17: the engine-owned
settling sources now expose a bounded `i1` comparator for candidate stack
depths 14 through 16 and the permanent four-body energy reproducer. It changes
only the laboratory source identity and contact iteration count from two to
one; prior-contact identity, 30 Hz softness, damping 10, static multiplier 1,
authored bodies, solver family, scheduler, capacity, tolerances, and product
defaults remain fixed. This tests the trace-supported hypothesis that repeating
the soft update amplifies a backend-sensitive small-signal mode. It is not an
accepted quality policy: a favorable result still requires the frozen matrix
and human review before any product or accepted-quality change.

Single-iteration rejection checkpoint, 2026-07-17: clean Metal and
hardware-backed AVD traces reject the first reversible correction. `i1`
introduces structure loss on Metal depth 16 and AVD depth 14, and advances the
existing AVD depth-15/16 failures. The four-body case retains 300 settled
energy increases with zero cache misses on both backends. The
[checked package](../../evidence/p5.2b-s6.5a-soft-i1/README.md) preserves all
eight traces and the exact `i2` comparisons. No coefficients, solver family,
scheduler, tolerances, capacity, oracle precedence, or product defaults
changed. This comparator is rejected, not forwarded; isolating the second
pass's soft-feedback term while retaining `i2` is the next reversible question.

Soft-feedback isolation contract checkpoint, 2026-07-17: a laboratory-only
execution policy now keeps both selected colored manifold-block iterations but
applies the softness bias, mass scale, and accumulated-impulse decay only in
the first biased iteration. The second iteration remains an unbiased normal
constraint solve; the later selected relaxation passes are unchanged. Tests
prove the operation count, solver/scheduler, and every parameter except the
second biased iteration's existing `use_bias` word are identical. Standard and
product constructors are pinned to `every-biased-iteration`, while native and
Android trace aliases explicitly attest `first-biased-iteration-only`. No
scenario setting, coefficient, shader, tolerance, allocation, dispatch,
readback, or product default changed. Cross-backend evidence decides whether
this comparator is rejected or earns the full frozen matrix.

Soft-feedback isolation rejection checkpoint, 2026-07-17: the clean
[cross-backend package](../../evidence/p5.2b-s6.5a-soft-first/README.md) rejects
the staged `i2` comparator. It creates new Metal depth-15/16 and AVD depth-14
structure failures, advances the existing deeper AVD failures by hundreds of
steps, and retains the identity-clean four-body energy mode. All eight runs are
capacity-clean and hardware-backed; representative full traces and every raw
hash are retained. No product or accepted-quality setting changed. The next
bounded diagnostic may compare the existing point-normal and manifold-block
lab lanes under the same frozen soft policy, but any solver-family change or
promotion remains a human checkpoint.

Point/block attribution contract checkpoint, 2026-07-17: the bounded trace
runner now accepts the existing compact colored point-normal execution as a
laboratory-only comparator. Candidate depths 14 through 16 and the permanent
four-body energy reproducer retain the exact shared source hash, two
iterations, prior-contact identity, 30 Hz softness, damping 10, static
multiplier 1, forward order, compact indirect dispatch, warm start,
relaxation, capacities, and measurement policy. The existing point lane
changes the normal solve and color unit from manifold-block/manifold to
point-normal/point. It must also use the historical serial full-rebuild
scheduler because the selected speculative parallel constructor was
qualified only for manifold units and changes assignment/order. The raw
artifact attests both dimensions. This is therefore a truthful bundle-level
attribution control, not a claim that the block solve alone changed. Product
and standard trace constructors remain selected manifold-block plus the
selected parallel scheduler. If the point control removes the failure, a
point-unit parallel constructor or any solver-family change requires human
review before a larger matrix or product-default change.

Point-control rejection and S6.5a completion checkpoint, 2026-07-17: the clean
[ten-row package](../../evidence/p5.2b-s6.5a-point-control/README.md) rejects
the last bounded existing solver bundle. Point-normal/point coloring with its
required serial full-rebuild control creates a Metal depth-16 structure loss
at step 399 and advances AVD depth-15/16 loss to steps 290/130. The permanent
four-body energy envelope still rises on Metal, AVD, and a physical Pixel 7a;
Metal and Pixel do so with zero cache misses. The physical depth-14 control is
quiet like Metal, while the AVD retains 300 consecutive increases, isolating
the severe amplification to MoltenVK without resolving the common formulation
failure. All retained runs are exact-step, final-flush, hardware-backed, and
capacity-clean. No correction advances to the full frozen matrix, and no
product policy changes.

Recommendation: keep the soft candidate unpromoted and stop for human review.
If continued, instrument per-pass energy transfer on the permanent four-body
source and compare GPU point/block recurrence with the scalar specification
and pinned Box2D authority. Fix an attributable implementation divergence
inside the selected family. If all three agree but violate the awake gate under
colored order, choose any new formulation/accumulation or physical policy only
through a new human-reviewed decision.

Stop early for human direction before a coefficient/identity-band default
change, tolerance or quality-band revision, scheduler/solver/accumulation
change, portability exception, simulation-dependent readback, CPU fallback,
or another difficult-to-reverse architecture fork.

### S6.5b — authorized matched-precision per-pass diagnosis

- [x] Freeze a versioned per-pass trace that brackets every state-changing
  logical generated-contact pass for the permanent four-body energy
  reproducer. Each boundary retains the complete ordered per-color dispatch
  sequence. Copy bounded body, contact, and accumulated-impulse state in GPU
  command order and map only after the final requested step.
- [x] Mirror the selected manifold-block recurrence in scalar Rust with `f32`
  state and rounding at the same operation boundaries. Report both strict
  ordered-`f32` kinetic energy and compensated wider accumulation over the
  identical captured `f32` values.
- [x] Compare Metal with the scalar trace first. Attribute the first material
  body/impulse divergence independently of energy-summation precision and
  retain the exact input, operation index, ULP/absolute delta, and surrounding
  contact state.
- [x] Keep clean pinned Box2D as the world-behavior authority. Use its public
  process-isolated trace first; create a separate patch-digested microscope
  only if the localized question cannot be answered from public state.
- [ ] After the common energy-producing pass is known, run the same final-flush
  trace on the hardware-backed `jstorrent-tablet` AVD to localize where
  MoltenVK amplifies it. Attempt the physical-device lane when available and
  report absence or lock state as a gap.
- [x] If GPU state diverges from the scalar recurrence, fix only the
  attributable implementation defect inside the selected manifold-block
  family and re-run the frozen quality/energy/response/capacity/performance/
  authority/platform matrix. If they agree while violating the energy gate,
  publish the evidence and stop for the formulation/policy decision.

Outcome: distinguish a correctable GPU implementation defect from an accepted-
equation recurrence that injects energy under colored order, without using
different accounting precision as evidence and without changing the product.

Artifacts: versioned pass dictionary; exact operation schedule and hashes;
GPU and scalar pass traces; strict-`f32` and compensated energy tables; first-
divergence report; public Box2D behavior trace; Metal/AVD amplification overlay;
capacity/provenance records; verifier; and a compact branch recommendation.

Gate: every compared trace uses the same source, policy, step, operation
boundary, and captured `f32` state. No per-pass or per-step CPU result advances
simulation. The product remains hard/direct with the selected manifold-block
solver and scheduler. An implementation fix cannot be inferred from energy
accumulation alone; a formulation change cannot be made without human review.

Commit boundaries: this authorization contract; pass-trace schema and scalar
mirror; GPU final-flush instrumentation; clean Metal attribution; an
attributable fix if the branch permits it; AVD localization; and the verified
decision package are independently reviewable commits.

Pass-trace checkpoint, 2026-07-18: schema version 1 names all 56 logical
state-changing boundaries in the selected four-substep/manifold-block-i2
schedule. A colored logical pass contains its complete 32-color indirect
sequence, so the trace preserves Gauss–Seidel order without producing 1,296
CPU-visible synchronization points. The diagnostic copies actual body and
prepared-contact `f32` state after every boundary into one bounded buffer and
maps once after the target step and persistence store. A two-step Metal test
is bit-exact against the ordinary unsplit graph, retains the selected parallel
scheduler and soft/prior policy, and reports clean construction/color
capacity. No product constructor or interactive frame path calls this API.

Scalar-recurrence checkpoint, 2026-07-18: report version 1 independently
replays gravity, pose integration, warm start, manifold-block normal solve,
sequential per-manifold friction, relaxation, and restitution from each
captured `f32` pre-pass state in the exact compacted color order. Preparation
and refresh are declared synchronization boundaries: they must leave body
state bit-exact, then the mirror consumes their actual prepared `f32` contact
output so host trigonometry cannot manufacture a solver divergence. Every row
reports strict ordered-`f32` and compensated-`f64` kinetic energy plus raw
exact-bit, maximum absolute, and ULP state differences. A two-step Metal
fixture first differs at pass 33 by one ULP; no pass/fail bound, physical
tolerance, or product coefficient changes with this diagnostic contract.

Metal/authority decision checkpoint, 2026-07-18: the clean
[S6.5b package](../../evidence/p5.2b-s6.5b-pass-energy/README.md) brackets step
900 of the permanent reproducer. GPU kinetic energy rises
`1.9601904193100975e-8`; independent scalar pass deltas sum to
`1.9598340244749856e-8`, only `3.563948351118385e-12` apart. All eight normal-
relaxation passes inject energy in both recurrences, totaling
`2.7251938749029784e-6` on GPU and `2.7251939285323583e-6` in scalar Rust.
Pinned unmodified awake Box2D remains near `8.4e-13` kinetic energy and changes
by only `3.6376413761573665e-15` across steps 899–900. The evidence therefore
does not support an attributable GPU implementation fix; it triggers the
predeclared formulation/policy stop. The AVD per-pass trace remains unchecked
because that stop occurs before the optional amplification-localization lane,
not because AVD behavior is inferred. Hard/direct manifold-block i2 remains
selected and every candidate remains unpromoted.

### S6.5c — authorized relaxation-policy factorial

- [x] Freeze the arm definitions, order, and survival criteria in governing
  documentation before implementing or measuring an arm.
- [x] Add laboratory-only normal-relaxation policy controls, a version-2
  policy-attested pass trace, and a matched scalar recurrence. Keep every
  product-facing constructor on hard/direct relaxation.
- [x] Add exact-count backend-trace aliases for both four-body energy arms;
  attest the relaxation policy separately from the unchanged source hash.
- [x] Arm 1, `soft-no-normal-relax`: omit normal-relaxation passes when contact
  softness is enabled. Retain biased manifold-block normals, tangent
  relaxation, restitution, `i2`, coloring, coefficients, and every other pass.
- [x] Arm 2, `soft-coefficient-normal-relax`: retain normal relaxation, but for
  penetrating contacts set velocity bias to zero while retaining the selected
  soft `massScale` and `impulseScale`. Retain the existing speculative-contact
  branch for positive separation. This is an experimental feedback equation,
  not Box2D parity: pinned Box2D v3 relaxation instead uses zero bias,
  `massScale = 1`, and `impulseScale = 0`, which is the current hard-relax
  shape.
- [x] Arm 3, `soft-point-normal-relax`, is conditional. Implement and run a
  point-sequential normal-relaxation pass while retaining manifold-block
  biased solves only if arms 1 and 2 disagree about whether block coupling is
  the injector. Arm 1 passing energy but showing drop overshoot or bounce does
  not trigger arm 3.
- [x] Reuse the S6.5b matched-precision trace at the same boundaries. Every
  present normal-relaxation pass must be an energy sink or neutral in both GPU
  and scalar accounts under the frozen per-boundary epsilon; arm 1 must record
  that the normal-relaxation passes are absent, not infer a pass from the
  endpoint.
- [x] Apply the cheapest kill first on Metal: the permanent four-dynamic-body
  reproducer must pass the unchanged S6.2 settled-window energy gate over
  steps 900–1,200. The gate retains absolute epsilon `1e-12`, relative epsilon
  `1e-5`, a positive-energy run limit of 5, and the first-versus-last-quartile
  non-increase check.
- [x] A surviving arm must introduce no new Metal structural loss relative to
  the current soft/prior candidate in the governed 4-, 8-, and 16-body stack
  matrix. Run realistic seeded drop fixtures only after the energy and
  structure kills; record overshoot, bounce, residual, churn, capacity, and
  replay failures without weakening a band.
- [x] Forward at most one arm through the complete Metal matrix, then the
  hardware-backed `jstorrent-tablet` AVD and available physical hardware.
  Run one AVD matched-boundary overlay for that survivor and require the
  MoltenVK amplification to collapse with the common defect. Declare a locked
  or absent physical device as a gap.
- [x] Publish a fresh verified package and return to human review before any
  promotion. The package must retain both rejected arms, the conditional-arm
  disposition, per-pass attribution, full survivor matrix, synchronized
  captures, provenance, and reconsideration triggers.

Outcome: decide whether a narrowly changed normal-relaxation policy removes
the common retained-energy mode without trading it for a stack, drop-quality,
or portability regression.

Gate: arm definitions and kill criteria precede every measurement. The product
constructor, launch default, physical coefficients, authority precedence,
solver/scheduler family, tolerances, and frozen quality bands remain
unchanged. Passing endpoints without a neutral-or-sink relaxation signature is
not survival. No result promotes a candidate without a new human decision.

Commit boundaries: this authorization contract; laboratory policy and
matched-trace support; each independently killed or forwarded arm; the Metal
survivor matrix; cross-host survivor evidence; and the verified promotion
checkpoint are separate reviewable commits.

Laboratory-mechanics checkpoint, 2026-07-18: version-2 traces name the normal-
relaxation policy explicitly. `soft-no-normal-relax` removes the eight normal
passes while retaining all eight tangent-relaxation passes in the four-
substep/`i2` schedule. `soft-coefficient-normal-relax` retains all 56 logical
boundaries and marks only the eight normal-relaxation descriptors for the
experimental coefficient equation. Both execute on Metal and replay through
the independent matched-`f32` scalar recurrence within the existing diagnostic
comparison. Hard/direct remains selected in every wrapper and product
constructor. These are mechanics checks, not factorial measurements or arm
survival results.

Arm 1 checkpoint, 2026-07-18: the clean
[omit-normal package](../../evidence/p5.2b-s6.5c-arm1-no-normal-relax/README.md)
is rejected at the first gate. Its settled energy envelope rises from
`0.0010513754` to `0.0061409203` (5.84×), all 300 settled intervals form a
positive run, and maximum linear speed reaches `0.09048607 m/s`. The matched
step-900 trace contains zero normal-relaxation passes and all eight tangent-
relaxation passes, so omission is direct evidence rather than an endpoint
inference. Capacity, structure, contact identity, and warm cache remain clean.
Arm 1 does not reach stacks or drops; product construction remains unchanged.

Arm 2 checkpoint and Arm 3 trigger, 2026-07-18: the clean
[soft-coefficient package](../../evidence/p5.2b-s6.5c-arm2-soft-coefficient-relax/README.md)
is also rejected before stacks and drops. Residual linear speed falls within
the frozen band at `0.01083445 m/s`, but the settled energy envelope still
rises 1.273× with a 299-observation positive run. More decisively, all eight
normal-relaxation passes inject in matched GPU and scalar recurrences, totaling
`0.00034883377174823997` on Metal. Arm 1 shows that the overall energy mode
survives without block normal relaxation; Arm 2 shows that retained block
relaxation still injects. This is the predeclared ambiguity about block
coupling, so conditional Arm 3 is triggered. No arm is forwarded.

Arm 3 checkpoint, 2026-07-18: the clean
[point-sequential package](../../evidence/p5.2b-s6.5c-arm3-point-sequential-relax/README.md)
is rejected before stacks and drops. Biased normals remain coupled manifold
blocks; only the relaxation points are visited sequentially with the existing
hard/direct equation. The settled energy envelope rises from `0.00010415906`
to `0.0006075374` (5.83×), the positive run spans all 300 observations, and
linear plus angular-edge residuals reach `0.02844868 m/s`, above the frozen
rest band. All eight normal-relaxation passes inject in matched GPU and scalar
recurrences, totaling `2.2802872491491276e-6` on Metal. Point sequencing does
not remove the injector, so every authorized arm is rejected. With no
survivor, the realistic drops, governed stacks, AVD pass overlay, and physical
device forwarding are correctly not run; their conditional checklist items
are closed by no-survivor disposition rather than inferred as passes.

S6.5c review checkpoint, 2026-07-18: the
[verified comparative package](../../evidence/p5.2b-s6.5c-relaxation-factorial-review/README.md)
indexes all three independently verified arm packages and emits a no-survivor
success sentinel. No arm satisfies the frozen energy gate; both retained-
normal arms inject during all eight normal-relaxation passes in GPU and scalar
accounts. No promotion candidate exists. The recommendation is to retain the
hard/direct manifold-block i2 product and, only with human authorization,
derive an energy-stable coupled soft-contact recurrence in scalar Rust before
another bounded GPU factorial. S6.6 is not authorized by this result.

### S6.5d — authorized scalar-first coupled recurrence

- [x] Freeze the authority source, scalar gates, one-candidate limit, forwarding
  order, and stop branches before deriving or measuring a candidate.
- [ ] Independently transcribe pinned unmodified Box2D v3 `3a4f0da...`
  softness and contact updates into the scalar laboratory. Record its MIT
  provenance and validate the transcription against analytic single-point
  cases without copying reference implementation code.
- [ ] Replay the authority recurrence over the captured compacted PhysBox color
  order. Box2D already runs sequential graph colors with disjoint work inside
  each color, so attribute coupled-block, normal/tangent stage, and any other
  schedule differences separately rather than calling coloring itself novel.
- [ ] Derive at most one coupled candidate with a discrete-energy argument that
  holds at every colored normal-relaxation boundary. A proof that assumes a
  different sequential order does not qualify.
- [ ] Before a GPU port, require neutral-or-dissipative scalar relaxation plus
  penetration resolution and support transfer on the permanent four-body
  reproducer and the existing two-step contract fixture. Reject zero-correction,
  excessive-damping, sagging, or endpoint-cancellation solutions.
- [ ] If and only if the scalar gate passes, port that one candidate as a named
  laboratory policy while leaving every product constructor on hard/direct
  manifold-block i2. Reuse matched-precision per-pass accounting.
- [ ] Apply the permanent Metal four-body energy/rest kill first, then require
  no new structural loss on the governed 4/8/16 Metal stacks. Run realistic
  drops only after both gates pass.
- [ ] Forward a survivor to one matched-boundary `jstorrent-tablet` AVD overlay
  and available physical hardware. Require the MoltenVK amplification to
  collapse with the common defect; record unavailable hardware as a gap.
- [ ] Publish a verified review package with the authority control, derivation,
  scalar quality evidence, the one GPU candidate or scalar-stop disposition,
  minimized failures, provenance, and a recommendation. Do not promote or
  push.

Outcome: determine whether one authority-derived coupled recurrence can make
soft settling energy-stable under the selected compact colored schedule while
retaining the quality benefit that motivated softness.

Gate: energy stability is necessary but insufficient. Scalar support transfer,
penetration resolution, replay, and frozen quality bands must pass before GPU
work. The GPU candidate limit is one. Product defaults, authority precedence,
tolerances, scheduler/solver family, pacing, and capacity policy remain
unchanged.

Stop branches: if the authority control exposes no derivable coupled recurrence
with a colored-order energy argument and scalar quality retention, stop before
GPU work. If the one GPU port diverges from its scalar recurrence, fix only an
attributable implementation defect inside that candidate and rerun the cheap
gate. If scalar and GPU agree but a governed gate fails, preserve the minimized
failure and return for the choice between deferring softness and accepting a
different physical policy. No second recurrence is tried autonomously.

Commit boundaries: this authorization contract; authority transcription and
analytic controls; scalar derivation and quality decision; the single GPU port
if authorized by scalar results; Metal kill/matrix; cross-host forwarding; and
the final verified checkpoint are separately reviewable commits.

### S6.6 — accepted policy integration

- [ ] Promote only the explicitly accepted identity and coefficient policy
  through the one product graph; retain named baseline controls.
- [ ] Re-run the complete native, browser, Android AVD, available physical
  device, headless, authority, capacity, replay, and timing matrix.
- [ ] Publish a deployed-review-ready comparison and leave the tree clean; do
  not push `main`.

Outcome: the reviewed candidate, rather than a laboratory alias, becomes the
shared product behavior and is ready for the user's deployed feel test.

Likely boundaries: shared product settings and launch projection, engine
construction, regression baselines explicitly accepted in the decision, and
governing status. No broadphase or sleep work is mixed into the commit.

Gate: the accepted policy passes the awake completion gate on every required
fixture and host, or the integration is rejected and the failure is preserved.

Commit boundary: product integration and clean cross-host evidence are
separate commits if the evidence package is large.

### S6.7 — conditional residual mechanisms

- [ ] Run only if the accepted identity/softness direction still misses an
  awake gate and attribution identifies bias pumping or tangential drift.
- [ ] Evaluate a penetration deadband for attributable bias pumping, or
  friction anchors for attributable tangential drift, independently first.
- [ ] Stop for renewed human review before either becomes a product default.

Outcome: the workstream answers a remaining measured question without turning
into open-ended tuning.

Gate: the trigger, comparator, costs, benefits, and regressions are explicit.
If neither trigger fires, mark the slice not required rather than implementing
speculative mechanisms.

Commit boundary: one mechanism per comparator, clean evidence separately, and
a human decision record before integration.

### S6.8 — awake gate closure and downstream handoff

- [ ] Publish the final accepted awake matrix, synchronized playback,
  minimized failures, cross-host results, complete cost, and aspiration gap to
  Box2D.
- [ ] Mark broadphase product-scene ranking/selection and sleeping design
  unblocked only when every hard awake gate passes.
- [ ] Preserve any failed seed and state which work remains blocked if the
  gate does not pass.

Outcome: reviewers can verify settling quality without inspecting the entire
implementation, and S5.5/S5.7 receive an explicit evidence-backed dependency.

Gate: every condition in the governing topic's awake completion gate passes
on fresh clean provenance, followed by human confirmation of the deployed feel
before final broadphase selection or sleeping thresholds are accepted.

Commit boundary: final evidence/status/handoff only.

## Autonomous execution boundary

Once the user explicitly says to begin, S6.1 through the experimental and
evidence work of S6.4 are autonomous. That includes routine implementation
choices, reversible refactors, failing experiments, deterministic
minimization, oracle runs, native/browser/Android testing, physical-device
attempts, documentation, and one clean commit per completed slice. Ordinary
test failures are diagnosed and fixed without interrupting the user.

The normal stop is S6.5. Product-default promotion waits for human review of a
clearly visible comparison and the objective matrix. After explicit selection,
S6.6 and final evidence are autonomous. S6.7 causes another stop only if its
trigger fires and a new physical mechanism is recommended. S6.8 returns for a
deployed feel confirmation and downstream handoff.

Stop earlier when progress requires a new solver family or accumulation
architecture, a tolerance/baseline/authority change, an accepted quality-band
revision, CPU fallback, native-only requirement, lost portability,
simulation-dependent readback, silent capacity loss, pacing change, or scope
expansion into broadphase selection, sleeping, joints, new shapes, or liquid.

## Completion condition

S6 is complete only when an explicitly reviewed product policy passes the
awake gate on perfect and realistic sources, all required hosts execute the
same shared contract, failures are minimized and permanent, complete cost is
reported against optimized Box2D, the user can see the improvement through a
compact playback/deployed review, and S5.5/S5.7 are updated from evidence.

Completion does not authorize a broadphase, sleep threshold, liquid work, or a
claim that GPU rigid physics beats optimized CPU Box2D.
