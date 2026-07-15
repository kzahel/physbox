# S5: Interactive rigid quality, construction, and scale

Status: active, 2026-07-15.
Direction: [one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Scale contract: [performance and scale](../topics/performance-and-scale.md).
Dependencies: [S3 collision/persistence](003-small-world-collision-persistence.md)
and [S4 evaluation workbench](004-basic-scene-navigation-camera.md).
Bring-up stages: [P5.2 through P5.4](../bringup-plan.md#p52--small-world-collision-and-persistence-active).

## Goal

Turn the working small-world circle/rectangle prototype into an interactive
rigid-body evaluation surface with objective settling evidence and a credible
path to scale. A reviewer must be able to load an intentionally easy rectangle
stack, create and erase circles and rectangles through the shared toolbar,
inspect 100-body bucket demonstrations, and later exercise 1K and 10K bucket
variants after a scalable broadphase is selected.

The workstream closes the selected S3 contact-representation integration,
completes the S4 construction loop, performs the P5.3 broadphase decision, and
reaches the P5.4 sleeping, quality, and correctness-qualified performance
review. It coordinates those existing milestones; it does not replace their
technical contracts or create a second engine path.

## Human direction recorded

Deployed review on 2026-07-15 accepted the S4.3 camera, scene discovery, and
anchored grab feel as the continuing workbench baseline. The current mixed
rectangle-on-circle stack is useful interaction evidence but is not a fair
settling target because its support is intrinsically unstable. A centered
rectangle resting on another rectangle still jitters visibly and is a quality
issue to diagnose rather than dismiss or conceal.

The approved sequence adds an easy rectangle stack, objective settling
metrics, 100-circle and 100-rectangle bucket demonstrations, the already
selected pair-slot plus stable-edge product integration, interactive circle
and rectangle authoring, settling experiments, the P5.3 broadphase bakeoff,
scalable bucket variants, and sleeping/waking. Current grab constants remain
the accepted provisional workbench baseline with explicit reconsideration
triggers; this feedback does not freeze them as universal final physical
defaults.

## Ownership and invariants

- Shared Rust owns every scenario, generator, fixed wall, action, completion
  rule, toolbar tool, construction command, metric, and expected structural
  outcome. Apps transport raw facts and do not recreate scenes or tools.
- The product continues to have one GPU body store, one collision graph, one
  persistent pair representation, one solver path, one renderer, and one
  scene catalog. Comparator pipelines remain isolated laboratory lanes.
- Bucket walls are ordinary fixed rectangles in the shared body/shape model,
  not platform geometry, special terrain, or a new collision subsystem.
- Placement and erasure allocate or retire generational slots on the GPU.
  Current occupancy, hit results, pair invalidation, and capacity decisions do
  not require simulation-dependent readback or a CPU body mirror.
- One-touch empty space stays inert in Grab mode. In Circle or Rectangle mode,
  primary input requests one bounded placement; two-touch navigation still
  cancels the tool and owns pan/pinch until all contacts lift.
- Brute-force all-pairs remains the pair-completeness oracle. A 100-body bucket
  may be shown before P5.3 as a labeled small-world baseline; 1K and 10K bucket
  performance claims wait for the selected scalable broadphase.
- Awake settling is measured before sleeping is allowed to hide residual
  motion. Sleeping is then evaluated as required product behavior with an
  explicit wake contract for grabs, impacts, construction, and changed
  contacts.
- Box2D remains the rigid behavior and optimized CPU authority through the
  existing process-isolated projections. No comparator becomes a product
  dependency.
- Capacity overflow, deep-overlap rejection, unsupported shape state, and
  timing invalidity are explicit artifacts. No scene silently drops bodies,
  pairs, contacts, constraints, commands, or diagnostics.

## Scope and exclusions

This workstream includes centered circles and rectangles, fixed rectangle
bucket walls, easy stacks and small pyramids, generated bucket populations,
bounded place/erase tools, the selected S3 representation integration,
settling/convergence experiments, SAP/grid broadphase candidates, sleeping and
waking, and correctness-qualified CPU/GPU performance matrices.

It excludes arbitrary polygons, triangles, capsules, compounds, joints,
liquid, CCD, deformables, editable materials, drag-to-size authoring, save/load,
general terrain, a CPU product fallback, native-only required behavior, and a
second platform-specific menu or simulation. One fixed circle size and one
fixed rectangle size are sufficient for the first construction loop; deliberate
sizing and properties remain P5.5 work.

Do not change the no-catch-up clock, oracle precedence, accepted solver family,
equal-and-opposite accumulation architecture, numerical tolerances, or portable
WebGPU requirement inside a routine slice. Do not accept a broadphase, physical
quality band, sleep threshold, or materially different product default without
the human checkpoints below.

## Ordered delivery slices

### S5.1 — shared evaluation scenes and settling contract

- [x] Add deterministic 2-, 4-, and 8-rectangle centered stack fixtures, with
  the four-rectangle form discoverable in the shared scene menu.
- [x] Add shared 100-circle and 100-rectangle bucket demonstrations using three
  fixed rectangle walls and declared initial spacing/seed/capacities.
- [x] Add neutral Box2D projections and exact-step replay for the stack and
  bucket sources; reject any unsupported projection explicitly.
- [x] Define settling observations for residual linear/angular speed, center
  drift, stack height, penetration/separation, contact-feature churn, impulse
  variation, energy, and capacity/overflow over declared transient and settled
  windows.
- [x] Record the present all-pairs graph, solver settings, no-sleep state, work
  vector, and timing as a baseline without changing tuning.

Gate: every source hashes and replays deterministically; fixed walls use the
ordinary body contract; the centered stack has a structural pass/failure
result rather than subjective screenshots alone; bucket scenes either complete
at declared capacity or fail atomically; menu selection changes no scene
semantics.

Artifacts: source and projection hashes, exact GPU/Box2D traces, settling time
series and summary, work/capacity records, synchronized stack playback, and
fresh desktop/phone/headless/AVD captures. Physical Android is attempted when
available.

Commit boundaries: land the stack source plus metric/oracle contract first;
then land the two 100-body bucket sources, menu exposure, captures, and labeled
all-pairs baseline. Do not mix solver tuning or representation integration into
either commit.

Stack-contract progress, 2026-07-15: the shared source now authors centered
2/4/8 rectangle stacks and exposes the four-body form as `rectangle-stack`
without replacing the mixed `rigid-stack` review scene. Version 1 settling
observations distinguish awake/sleep-enabled lanes and record every governed
motion, geometry, persistence, impulse, energy, and capacity dimension. Exact
1,200-step GPU replay is bit-for-bit for all three sources: the 2-body stack
retains its upright support chain, while the 4- and 8-body sources
deterministically topple and are preserved as structural failures for S5.4
rather than relabeled as settled. Clean pinned Box2D world projections accept
all three sources; its default sleeping behavior remains explicitly distinct
from the awake GPU lane.

S5.1 completion, 2026-07-15: the
[scene and settling package](../../evidence/p5.2b-s5.1/README.md) records five
exactly replayable shared sources, 38 clean pinned Box2D processes, objective
settling series, synchronized playback, 100-body Metal timing, and fresh
headless/web/AVD captures. Both buckets complete without overflow at 5,253
all-pairs tests, but remain visibly active; the 4/8 rectangle stacks topple
while the sleep-enabled Box2D projections remain upright. These failures are
the input to S5.4, not permission to tune before the selected S3 representation
is integrated. Physical Android was unavailable; the hardware-backed
`jstorrent-tablet` AVD passed.

### S5.2 — selected combined product graph and S3 evidence

- [x] Replace product full-key persistence with the accepted stable
  generational pair-slot path while retaining full semantic pair/feature
  verification and the full-key lane as a correctness oracle.
- [x] Replace the product body-by-capacity scan with accepted stable body-edge
  index views and deterministic segmented gather while retaining the canonical
  scan as a correctness oracle.
- [x] Integrate both selected views in one product graph; do not infer the
  combined result by adding isolated candidate timings.
- [x] Prove churn, recontact, reversed scheduling, body-generation reuse,
  forced fingerprint collision, reset, grab, erase preparation, and whole-stage
  overflow against exact baselines.
- [x] Re-run generated 32/256/1K rows, authored-contact 1K/10K rows, the easy
  stacks, 100-body buckets, optimized Box2D, and cross-host evidence with
  complete dispatch/memory/timing attribution.

Gate: product and correctness-oracle digests are exact for every governed case;
the combined graph has no readback dependency or silent capacity loss; the two
preserved physical-policy reproducers do not regress; structural stack curves
remain finite and diagnosed; all fresh artifacts describe the combined path
rather than isolated candidates.

Artifacts: combined graph byte/dispatch table, exactness and generation traces,
minimized failures, structural curves, native/browser/Android matrix, optimized
CPU comparison, and an updated S3 closure recommendation.

Commit boundaries: pair-slot product integration and negatives; stable-edge
product integration and exactness; combined performance/structural evidence;
then S3 status documentation. Stop early only if combination requires a new
representation or accumulation architecture, or if its failed quality gate
cannot be resolved inside the accepted direction.

Pair-slot integration, 2026-07-15: the shared generated-contact product graph
now uses stable generational pair slots and pair-local two-point caches. The
256-bit full-key graph remains selectable only as an explicit validation
oracle. Complete pair and local-feature equality still gate every impulse
transfer; reversed scheduling and a zero-bit forced fingerprint retain exact
body/cache output across every behavior family and every intermediate step.
The pair bridge preserves the existing sparse two-point manifold slots so
Jacobi contribution order stays byte-exact. Existing churn, recontact,
body-generation reuse, and whole-stage overflow negatives continue to pass.
With the canonical body scan still intentionally unchanged, the generated
graph drops from 282 to 95 dispatches and the maintained floor allocation from
23,700 to 19,024 bytes. These are implementation accounting results, not the
required combined timing or S3 closure claim.

Stable-edge integration, 2026-07-15: the same product graph now also builds
stable body-edge records and gathers deterministic sorted body ranges. The
canonical body-by-point scan remains explicitly selectable as a validation
oracle. The combined pair-slot-plus-edge path matches the full-key/canonical
lane at every checked intermediate behavior step, and matches pair-slot plus
canonical gathering under permuted edge construction, pair churn, high
contention, and atomic overflow. Shared reset and grab tests also exercise the
new product default. The maintained two-body floor graph is 97 dispatches and
19,640 bytes; the 32-body performance fixture is 100 dispatches. The increase
from the pair-only checkpoint is the bounded edge topology and two additional
dispatch parameter records, while body-by-capacity contribution scans are
removed. Timing, structural curves, erase/reuse preparation, cross-host
evidence, and S3 closure remain separate pending gates.

Combined-evidence automation checkpoint, 2026-07-15: the generated-contact
authority runner now names the selected pair-slot and stable-edge topology in
every behavior and performance row, compares generated 32/256/1K output to the
full-key/canonical oracle, retains raw timing samples, and re-runs exact
canonical-versus-selected authored-contact rows at 1K/10K. The scene runner
records the selected topology, dispatches, and allocation at every stack and
bucket checkpoint and can require a clean source tree. Representation
acceptance is reported separately from the still-open solver-quality gate so
known minimized failures cannot be hidden by exact layout equivalence. Fresh
clean packages and their measured interpretation remain the next commit.

S5.2 completion, 2026-07-15: the clean
[combined graph package](../../evidence/p5.2-s3.6-combined/README.md) and
[scene package](../../evidence/p5.2b-s5.2/README.md) qualify the actual selected
graph. All 9 directed and 128 randomized representation cases are exact, both
authored-contact rows match the canonical gather result, every accepted
capacity is clean, and the deliberate dense overflow is atomic. Native,
headless, browser WebGPU, and hardware-backed `jstorrent-tablet` AVD artifacts
are fresh; no physical Android device was connected.

The integrated 1K generated GPU row is `3.159 ms` P50 versus the prior
full-key product's `23.422 ms`, but remains about 20.7x slower than the fastest
equivalent optimized Box2D row. The 100-body circle/rectangle bucket GPU P50 is
`0.589`/`0.745 ms` with 100 dispatches. The 2-body easy stack passes while the
4/8-body awake stacks still topple. Representation acceptance is therefore
complete and S5.3 is unblocked; solver quality, final defaults, and any CPU
crossover remain unaccepted S5.4 decisions.

### S5.3 — GPU construction loop and interactive authoring

Dependency: S5.2 generational pair invalidation passes.

- [x] Implement versioned bounded place-circle, place-rectangle, and erase-at
  intents using existing material defaults and one declared size per shape.
- [ ] Allocate and retire body slots on the GPU, increment generations, and
  invalidate stale pair, manifold, cache, grab, command, and render identity.
- [x] Expose `Grab`, `Circle`, `Rectangle`, and `Erase` in the existing shared
  glyph toolbar with deterministic UI/tool/gesture priority.
- [x] Make an empty placement request finite and deterministic; reject deep
  coincident placement or full capacity explicitly rather than ghosting,
  disabling collision, silently moving the body, or losing work.
- [ ] Exercise place, fall, collide, stack, grab, erase, reuse, restart, scene
  switch, two-touch cancellation, and high-cadence input on every host.

Gate: a user can author circles and rectangles without a CPU occupancy mirror;
erased generations cannot transfer stale contacts or grabs; the interactive
frame never waits for selection/allocation readback; all failures are bounded
and visible; the toolbar remains one shared engine-rendered UI.

Artifacts: construction schema/byte table, lifecycle traces, generation and
cache invalidation evidence, capacity/deep-overlap failures, fresh captures,
and a deployed human interaction checklist.

Commit boundaries: command and GPU lifecycle contract; toolbar/tool wiring;
cross-host evidence and S4.4/S4.5 status. Construction feel remains provisional
until the first mandatory human checkpoint after S5.4 evidence is ready.

Construction-contract checkpoint, 2026-07-15: contract version 1 carries only
an ordered sequence, operation kind, and finite world target; it never carries
a CPU-selected body identity. The GPU ABI uses one 32-byte intent record
(`kind/sequence/padding`, `target/padding`), a 64-byte parameter record, and one
80-byte cumulative state record. Intent batches are bounded at 64 and reject
partial overflow. Results distinguish placed, erased, full capacity, deep
overlap, erase miss, and exhausted generation.

The first lifecycle pass reserves interactive body slots at the next power of
two above the authored population with a current minimum of 64. It runs before
grab/command processing and contact generation, chooses the lowest free slot,
increments generations on retirement, clears the matching active grab, and
never maps occupancy to the CPU while advancing. Empty slots retain a valid
nonzero generation and no live/shape flags. The two-body floor consequently
reserves 64 body slots and 2,048 contact-point slots: its declared graph changes
from 97 dispatches/19,640 bytes to 100 dispatches/1,190,404 bytes. This is an
explicit small-scene headroom cost, not a scale or performance improvement;
S5.3 evidence must still prove stale pair/cache invalidation and qualify this
capacity policy before the second lifecycle checkbox is complete.

Toolbar checkpoint, 2026-07-16: UI contract version 2 renders four mutually
exclusive touch-sized glyph tools in the one shared canvas pass and records the
selected tool in every frame report. UI capture precedes world tools; secondary
mouse input remains camera-owned. Grab preserves its anchored begin/target/end
stream. Circle, Rectangle, and Erase capture a primary/contact press and emit
exactly one construction intent on release at the press position, so a second
touch cancels the pending tool before it can place or erase anything. Changing
tools ends an active grab explicitly. Pending construction shares the existing
monotonic interaction sequence, zero-step retention, first-admitted-step
dispatch, and atomic overflow boundary without becoming a second engine path.
Exact GPU tests cover circle/rectangle placement, erase/reuse to generation 2,
tool-change grab release, and two-touch cancellation. Cross-host traces,
high-cadence input, scene reset/switch, and visible asynchronous failure
diagnostics remain in the S5.3 evidence boundary.

Asynchronous-diagnostics checkpoint, 2026-07-16: every surface host now copies
the 80-byte cumulative construction state only after a construction dispatch,
maps it without waiting, and publishes the result on a later frame solely for
reports and the shared glyph overlay. The overlay names the last placement or
erase result, cumulative failure count, current GPU live-body count, and sample
age. Scene activation/restart epochs discard stale in-flight samples. Frame
reports state explicitly that construction diagnostics never block simulation
advancement. A headless-only blocking evidence readback records the live
candidate, manifold, point, and persistent-pair counts; exact erase/reuse tests
show a warm graph clearing to zero, slot generation 2 starting with an empty
graph, and contacts rebuilding only after recontact. Cross-host freshness and
the complete shared interaction trace remain the final S5.3 evidence commit.

### S5.4 — awake settling and tuning bakeoff

- [ ] Run 2/4/8 rectangle stacks and small pyramids with sleeping disabled in
  both GPU and Box2D lanes to separate solver residuals from sleep policy.
- [ ] Attribute visible jitter to contact churn, warm-start transfer,
  two-point torque balance, convergence, relaxation, speculative distance,
  depenetration, or another measured source; preserve the first divergence.
- [ ] Evaluate reversible tuning candidates inside the accepted solver family,
  including substep/iteration count, relaxation, warm-start policy, and the
  already governed bias/contact policy. Include their complete GPU cost.
- [ ] Minimize every structural failure and add it as a permanent reproducer.
- [ ] Publish synchronized playback and an objective recommendation without
  accepting a new physical default or quality band silently.

Gate: the package distinguishes stable awake behavior, bounded residual motion,
and behavior that only sleeping could conceal. Every candidate is compared at
equivalent quality and complete cost; tolerances remain unchanged unless a
human explicitly accepts a measured revision.

Artifacts: metric matrix, first-divergence traces, minimized reproducers,
Box2D comparison, cost/convergence curves, authored-scene playback, and current
deployed construction/stack review instructions.

Commit boundaries: measurement instrumentation; candidate experiments;
evidence/recommendation. Stop for the first mandatory human checkpoint before
accepting stack-quality bands, solver/contact defaults, or construction feel.

### S5.5 — P5.3 scalable broadphase bakeoff

Dependency: the human accepts an S5.4 path, S3 closes, and the S4 construction
checkpoint is adequate for continued evaluation.

- [ ] Keep brute force as the complete pair oracle and implement portable
  sorted sweep-and-prune plus multi-cell grid candidates over the same shape
  AABBs and canonical output contract.
- [ ] Compare pair completeness, duplicates before unique, amplification,
  churn, memory/high-water, dispatches, overflow, and GPU time across the
  standard count/scene matrix.
- [ ] Include uniform and skewed sizes, sparse motion, packed circles, mixed
  bodies, rain, buckets, giant statics, high-speed crossing, and adversarial
  overlap without hiding quadratic physical cases.
- [ ] Run native Metal, Chrome WebGPU, host-GPU AVD portability, available
  physical Android, and optimized Box2D comparison where scopes are equivalent.
- [ ] Add LBVH only if SAP and grid leave a material measured question; do not
  expand to it as speculative architecture work.

Gate: every candidate emits the complete canonical pair set or rejects the
whole stage; no timing row survives a correctness/capacity failure; scene
families expose rather than average away each candidate's weakness.

Artifacts: correctness corpus, amplification/memory/timing matrices,
minimized failures, platform matrix, rejected alternatives, and a compact
broadphase recommendation.

Commit boundaries: shared AABB/candidate benchmark contract; SAP candidate;
grid candidate; any evidence-justified comparator; final matrix/recommendation.
Stop for the second mandatory human checkpoint before selecting or promoting a
product broadphase.

### S5.6 — selected broadphase integration and scalable buckets

Dependency: explicit human broadphase selection.

- [ ] Integrate only the selected broadphase into the one product graph while
  retaining brute force as the bounded correctness oracle.
- [ ] Extend circle, rectangle, and mixed bucket generators through 100, 1K,
  and 10K bodies, stopping only at an explicit capacity or time boundary.
- [ ] Record collision-only, end-to-end physics, and full-frame timing plus
  work vectors, simulation scale/slip, dispatches, memory, candidate
  amplification, contact counts, and render/UI cost.
- [ ] Compare optimized Box2D at equivalent scene, fixed step, and structural
  quality; do not substitute slow-time presentation for 60 Hz throughput.
- [ ] Preserve 30K and 100K claims for applicable bounded-neighbor, sparse, or
  mostly-sleeping families rather than implying every dense bucket must reach
  those counts.

Gate: 1K and 10K bucket results exercise the selected product graph, not the
quadratic baseline or isolated solver; every published row is correct,
capacity-clean, provenance-complete, and scoped honestly.

Artifacts: scalable bucket captures, exact reports, four-scope performance
curves where applicable, CPU comparison, memory/capacity tables, and
reconsideration triggers.

Commit boundaries: product integration; scalable scene generators; native
matrix; browser/Android matrix and report. A failed provisional target remains
visible evidence and does not justify silent quality reduction.

### S5.7 — sleeping, waking, and P5.4 rigid review

- [ ] Define a versioned GPU sleeping/island-wake contract with measurable
  awake compaction, thresholds, timers, and generation-safe state.
- [ ] Wake on grab, construction, explicit action, material impact, relevant
  contact change, and support motion without a CPU island mirror or readback.
- [ ] Re-run awake-only and sleep-enabled stacks, pyramids, buckets, rain,
  impacts, and sleeping-city scenes against invariants and Box2D behavior.
- [ ] Publish 1%, 10%, and 100% awake performance, wake latency/propagation,
  false-sleep and failure traces, and long-run jitter/energy outcomes.
- [ ] Assemble the P5.4 correctness-qualified CPU/GPU crossover and rigid
  quality report with known limitations and reconsideration triggers.

Gate: sleeping improves indefinite rest without concealing an awake structural
failure; no body remains asleep through a required wake; active compaction is
capacity-safe and GPU-resident; performance claims retain equivalent quality.

Artifacts: sleep/wake schema, state traces, minimized false-sleep/missed-wake
failures, long-run playback, awake-fraction performance curves, complete
cross-host evidence, and compact P5.4 recommendation.

Commit boundaries: sleep contract and scalar/invariant tests; GPU sleep/wake
and compaction; cross-host/performance evidence; final review package. Stop for
the third mandatory human checkpoint before accepting sleep thresholds, final
rigid quality, or a CPU/GPU performance claim.

## Autonomous execution and human checkpoints

After this tactical is accepted, routine implementation, diagnostics,
reversible refactoring, candidate experiments, failure minimization, test
repair, documentation, evidence generation, and per-slice commits are
autonomous. The agent may proceed unattended through S5.1, S5.2, S5.3, and the
experimental/evidence portion of S5.4. It then stops with one compact package
for combined review of construction feel, awake stack quality, and any proposed
default changes.

After that decision, the agent may close the accepted S3/S4 gates and execute
all SAP/grid candidates and evidence in S5.5 unattended. It stops before
broadphase selection. After explicit selection, it may integrate the winner and
execute S5.6 plus sleep/wake candidate evidence in S5.7, then stops before
accepting sleep thresholds, rigid quality bands, or final crossover claims.

Human direction is required earlier if evidence demands any of the following:

- a different solver family, accumulation architecture, pair representation,
  or construction state owner;
- a numerical-tolerance increase, refreshed accepted baseline, changed oracle
  precedence, or Box2D/invariant conflict;
- a physical-default or accepted quality-band change rather than an experiment;
- a native-only required path, lost WebGPU/Android portability, CPU fallback,
  simulation-dependent readback, or silent capacity loss;
- an LBVH or materially different broadphase family not justified by the
  recorded SAP/grid evidence;
- a change to no-catch-up slow time, GPU residency, or the one-shared-host
  architecture; or
- expansion into arbitrary geometry, joints, liquid, CCD, or save/load.

Deployment is not autonomous: pushing `main` triggers production deployment.
The agent commits and validates each slice but does not push. Human deployed
feel review therefore requires the user to push the named clean commit.

## Validation baseline

Every slice runs the repository gates from `AGENTS.md`. Scenario, runtime,
rendering, input, launch, shader, or pixel changes also build the Android APK,
exercise the hardware-backed `jstorrent-tablet` AVD, attempt the physical
device when available, and visually inspect fresh artifacts.

Physics slices additionally run exact scalar/invariant traces, randomized
replay, deterministic minimization, clean pinned Box2D authority processes,
optimized Box2D performance where applicable, complete capacity checks, and
the scopes required by the performance contract. Fresh artifact publication
requires source revision/dirty state, scenario/projection hashes, adapter,
settings, run id, success sentinel, and atomic completion.

## Completion definition

S5 completes only when:

- the easy stack and bucket scene families are shared and reproducible;
- the selected pair-slot and stable-edge views are the exact product graph;
- users can place and erase circles and rectangles through the shared toolbar;
- awake settling and sleeping behavior have accepted objective bands;
- a portable broadphase is selected and integrated from complete evidence;
- 100, 1K, and 10K circle/rectangle bucket rows are correctness- and
  capacity-qualified, with larger applicable scale curves retained;
- the P5.4 package states honest CPU/GPU crossover results, failures, rejected
  alternatives, and reconsideration triggers; and
- all three mandatory human checkpoints are recorded in later commits.

Completion does not authorize liquid. P5.6 remains downstream of accepted P5.4
rigid quality and performance.
