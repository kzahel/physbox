# S6: Awake settling stabilization

Status: active, 2026-07-17. Human review accepted the S5.4c scheduler
recommendation and authorized autonomous execution through S6.4. S6.1 is
complete; S6.2 is complete and S6.3 contact-identity comparators are next.

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

- [ ] Record the exact chain from reference-owner or clipped-feature change to
  cache hit/miss, accumulated impulse, velocity response, and later topology.
- [ ] Implement isolated deterministic near-tie ownership, true prior-contact
  hysteresis, and safe canonical-correspondence comparators where justified.
- [ ] Preserve normal/topology safety, pair generations, point uniqueness,
  exact replay, and atomic capacity behavior.
- [ ] Reject or minimize every comparator that transfers an impulse across an
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

### S6.4 — soft-contact factorial and combined candidate

- [ ] Reproduce the pinned Box2D hertz/damping-to-coefficient equations in the
  scalar specification with explicit provenance, then implement the portable
  GPU comparator independently against that contract.
- [ ] Measure bounded contact-hertz, damping-ratio, and static-contact variants
  against the unchanged `0.2/1/0` control.
- [ ] Compare identity-only, softness-only, and combined candidates over all
  accepted and realistic sources at complete-step cost.
- [ ] Preserve impact, friction, restitution, penetration, stack height,
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

Commit boundary: scalar contract, GPU comparator, factorial automation, and
clean evidence are independently reviewable commits.

### S6.5 — mandatory visible-quality review

- [ ] Stop and present the S6.4 package before changing product contact
  identity or physical coefficients.
- [ ] Ask the reviewer to judge synchronized playback and, after they choose
  to deploy the named commit, the direct web candidate against the current
  baseline on touch and desktop.
- [ ] Record acceptance, rejection, requested follow-up, and the exact chosen
  settings without promoting them in the review-record commit.

Outcome: human judgment resolves the visible feel/quality tradeoff with the
objective matrix beside it.

Artifacts: a short decision report, three-way synchronized playback, URL and
native launch recipes, candidate hashes, objective gate table, costs, known
failures, and a one-paragraph recommendation.

Gate: explicit human selection or direction. This is the normal unattended
stopping point.

Commit boundary: decision record only after human response.

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
