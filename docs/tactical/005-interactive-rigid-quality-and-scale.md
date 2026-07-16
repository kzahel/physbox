# S5: Interactive rigid quality, construction, and scale

Status: active, 2026-07-16. S5.4b compact-coloring evidence is complete, the
bounded incremental-color and workgroup-per-island comparators are implemented,
and the renewed evidence package remains;
product solver promotion, quality-band acceptance, and S5.5 remain blocked on
the renewed human checkpoint.
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
- [x] Allocate and retire body slots on the GPU, increment generations, and
  invalidate stale pair, manifold, cache, grab, command, and render identity.
- [x] Expose `Grab`, `Circle`, `Rectangle`, and `Erase` in the existing shared
  glyph toolbar with deterministic UI/tool/gesture priority.
- [x] Make an empty placement request finite and deterministic; reject deep
  coincident placement or full capacity explicitly rather than ghosting,
  disabling collision, silently moving the body, or losing work.
- [x] Exercise place, fall, collide, stack, grab, erase, reuse, restart, scene
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

Evidence-automation checkpoint, 2026-07-16: shared Rust now owns versioned
exact scripts for the full interaction sequence, warm-contact erase/reuse,
deep-overlap rejection, and 64-slot capacity exhaustion. The construction
runner records every frame, all four GPU body lanes, graph counts, cumulative
results, exact replay, and finite captures. Browser and Android validation can
request one raw circle-placement interaction and must observe its later-frame
`async-gpu` result; compact Android markers include only bounded construction
result counters and still declare that the diagnostic does not block. Review
captures replay multi-step exact requests at interactive one-step submission
cadence and require byte-exact body slots against the batched exact trace.
Fresh clean-revision native/web/AVD/available-device artifacts remain the next
commit.

S5.3 completion, 2026-07-16: the clean
[construction evidence package](../../evidence/p5.2b-s5.3/README.md) records
27 shared exact interaction frames and byte-exact replay, all four GPU body
lanes, warm erase/reuse/recontact graph state, an atomic 64-slot capacity
failure, and explicit deep-overlap rejection. The complete sequence runs once
in the shared native/headless host; browser and Android each inject one raw
placement and consume the engine result so adapters do not duplicate the
fixture. Chrome and the hardware-backed `jstorrent-tablet` AVD pass clean
revision `4d44af0`; no physical Android device was connected. No product CPU
occupancy mirror, blocking advancement readback, physics-default change, or
tolerance change was introduced. Construction feel remains provisional for
the mandatory S5.4 review.

### S5.4 — awake settling and tuning bakeoff

- [x] Run 2/4/8 rectangle stacks and small pyramids with sleeping disabled in
  both GPU and Box2D lanes to separate solver residuals from sleep policy.
- [x] Attribute visible jitter to contact churn, warm-start transfer,
  two-point torque balance, convergence, relaxation, speculative distance,
  depenetration, or another measured source; preserve the first divergence.
- [x] Evaluate reversible tuning candidates inside the accepted solver family,
  including substep/iteration count, relaxation, warm-start policy, and the
  already governed bias/contact policy. Include their complete GPU cost.
- [x] Minimize every structural failure and add it as a permanent reproducer.
- [x] Publish synchronized playback and an objective recommendation without
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

Measurement-instrumentation checkpoint, 2026-07-16: shared Rust now authors
deterministic 2/3/4-row rectangle pyramids in addition to the 2/4/8 stacks.
The generated GPU lane exposes only valid final-pass contact identities,
anchors, normals, effective masses, accumulated impulses, and equal/opposite
contributions; invalid capacity slots are explicitly scrubbed. Versioned lab
metrics aggregate awake residual windows, contact churn and impulse variation,
two-point impulse/torque balance, exact GPU/authority body deltas, and the
first measured divergence. The Box2D executable has a separate
`world-awake` process lane that disables sleep and records that worker policy
in provenance; the prior frozen `world` lane remains unchanged. These are
measurement facilities only: the selected GPU operation schedule, physical
defaults, accepted tolerances, and Box2D authority are unchanged.

Candidate-execution checkpoint, 2026-07-16: the existing generated-contact
graph now has a versioned native-laboratory execution policy for reversible
warm-start and relaxation toggles. Experimental scenarios may use one through
16 substeps and one through eight iterations at the governed 60 Hz outer step;
each iteration repeats the existing normal/tangent Jacobi stages before and,
when enabled, after position integration. The complete graph benchmark reports
the resulting dispatch and GPU cost. Ordinary product, browser, Android, and
validation constructors still accept only the selected four-substep,
one-iteration, warm-started, relaxed schedule. Candidate output records the
exact solver settings and policy, and replay tests prove that candidate use
does not promote it to a product default.

Candidate-automation checkpoint, 2026-07-16: one fresh-artifact runner now
screens the selected execution, 8-substep, 2/4-iteration, combined 8x2,
no-warm-start, no-relaxation, lower-bias, and governed bounded-contact variants
on all 2/4/8 stacks and 2/3/4-row pyramids. It compares every checkpoint with
the process-isolated awake Box2D lane, retains raw contacts/cache/work/capacity
state, reports complete 32/256/1K GPU cost and optimized Box2D cost, renders
synchronized playback, and replays every first structural failure into an
exact scenario artifact. Candidate output remains explicitly experimental;
the compact atomic manifest retains the complete data without pretty-print
inflation. The clean evidence package and recommendation are the next commit.

S5.4 completion, 2026-07-16: the clean
[awake settling package](../../evidence/p5.2b-s5.4/README.md) compares nine
reversible executions on all six awake sources through 1,200 exact steps. All
54 GPU lanes replay exactly and remain capacity-clean; 72 process-isolated
clean pinned Box2D checkpoints replay exactly with sleep disabled. Box2D keeps
every structure upright with settled-window linear/angular maxima below
`2.72e-6`/`1.78e-6`, while every GPU candidate loses the 8-body stack. More
substeps or iterations, warm-start/relaxation removal, and lower bias are
non-monotonic. The governed bounded-contact lane is byte-identical to the
selected execution on this corpus, excluding that policy change as an
explanation.

Twenty first failures are preserved as exact permanent reproducers. The
20-sample selected complete-graph P50 is `0.635`/`1.867`/`3.532 ms` at
32/256/1,024 bodies versus fastest optimized Box2D at
`0.0150`/`0.0785`/`0.170 ms`; the 1K GPU row fits 60 Hz but is 20.8x slower and
is not quality-qualified. Native/headless, browser, and hardware-backed
`jstorrent-tablet` AVD artifacts are fresh; no physical device was attached.
No candidate, quality band, tolerance, oracle rule, or product constructor was
promoted. The recommended next laboratory question is manifold-coupled
two-point normal solving, with a colored/in-place comparison if preserving the
accepted equal-and-opposite accumulation proves impossible. That is a
mandatory human architecture decision before S5.5.

### S5.4a — mass-splitting and manifold-block follow-up

Human authorization, 2026-07-16: reviewer analysis identified the unscaled
parallel point-Jacobi iteration as the leading explanation for the depth-graded
stack failure. Every point computes a standalone effective mass from the same
stage snapshot and the body gather applies all incident contributions at full
physical inverse mass/inertia. More iterations then amplify rather than damp
the unstable chain mode. Human review authorized the following laboratory
sequence; it did not authorize a product-default or quality-band change.

- [x] Add a frozen-contact scalar diagnostic that reports the measured
  contraction/amplification of the normal iteration for 2/4/8/16 stacks. Do
  not present a nonlinear projected solve as a formal spectral proof.
- [x] Implement scalar mass splitting exactly as Tonge, Benevolenski, and
  Voroshilov define it: multiply each dynamic body's inverse mass and inverse
  inertia by its valid contact-point incidence count only in effective-mass
  preparation, while applying impulses with the full physical mass.
- [x] Implement a coupled active-set 2x2 normal solve for each two-point
  manifold, degenerating exactly to the scalar point solve for zero/one valid
  point. Keep tangent/friction staging unchanged so its effect is attributable.
- [x] Implement block mass splitting by using each body's distinct live
  manifold/body-pair count in the block effective-mass matrix, again applying
  output impulses with the full physical mass.
- [x] Implement laboratory-only GPU scalar mass splitting from the accepted
  point-incidence topology without changing product construction or defaults.
- [x] Implement laboratory-only GPU manifold-block and block-mass-split lanes
  on stable pair slots, preserving equal-and-opposite records and body gather.
- [x] Evaluate the factorial matrix `current`, `mass-split`, `manifold-block`,
  `block-mass-split`, and the now-triggered `colored-in-place` comparator at
  1/2/4/8 iterations on the unchanged six S5.4 sources plus the 16-body stack.
  Preserve exact first failures and complete 32/256/1K GPU cost for every
  governed candidate.
- [x] Add the generated-contact colored/in-place comparator only if no
  mass-split/block lane preserves all existing S5.4 structures through 1,200
  steps with monotonic iteration behavior and a complete 1K step below the
  fixed 16.67 ms feasibility boundary on the named desktop adapter.
- [x] Publish a renewed review package and stop before promoting a solver,
  accepting residual-motion bands, changing physical defaults/tolerances, or
  beginning S5.5.

The scalar mass-split count is contact-point incidence, including both points
of a two-point manifold. The block count is distinct live manifolds/body
pairs, not point incidence. Fixed bodies retain zero inverse mass/inertia.
Counts are generated from the same accepted contact graph and must reject the
complete candidate if their topology/capacity is invalid. Equal-and-opposite
records, stable pair slots, stable body-edge ranges, device residency, and the
single product graph remain unchanged.

Quality gates: every lane is deterministic, finite, capacity-clean, and exact
under replay. The unchanged structural checks own pass/failure. A credible
convergence result must not merely delay failure: increasing iterations must
not introduce an earlier first failure or increase the settled-window peak
energy/residual-speed trend across the required corpus. Sleeping remains off,
and a quiet toppled structure remains a failure. No new numerical acceptance
tolerance is introduced here.

Evidence gates: retain raw per-iteration impulses, two-point matrix/active-set
choice, incident/manifold counts, body results, work, memory, dispatches, and
timing samples. Compare clean awake Box2D at the same checkpoints. Re-run the
current product baseline on native/headless/browser and the hardware-backed
`jstorrent-tablet` AVD; attempt physical Android when attached. A potential
candidate must execute through a shared validation lane on browser and AVD
before it can be recommended, without making it a product constructor default.

Commit boundaries:

1. this authorization, definitions, gates, and source attribution;
2. scalar mass-split/block specification and frozen-chain diagnostics;
3. laboratory-only GPU scalar mass splitting with exact count/topology tests;
4. laboratory-only manifold block and combined block-mass-split candidates;
5. factorial runner, convergence checks, minimization, and complete cost;
6. conditional generated-contact colored/in-place comparator only if its
   trigger becomes true; and
7. clean evidence, governing status, and renewed mandatory human checkpoint.

Source basis: Richard Tonge, Feodor Benevolenski, and Andrey Voroshilov,
“Mass Splitting for Jitter-Free Parallel Rigid Body Simulation,” ACM TOG 31(4),
2012, DOI `10.1145/2185520.2185601`. Implement the declared equations
independently; do not copy source code from an external engine.

Scalar checkpoint, 2026-07-16: the executable frozen-normal specification now
reports point/manifold incidence, effective 2x2 matrices, active-set choices,
impulses, body results, complementarity residual, closing speed, and energy.
The 16-stack rocking probe applies one recorded gravity substep plus alternating
`+/-0.01 rad/s` angular velocity. In that deliberately excited frozen system,
point Jacobi changes from a `0.705` first-iteration residual ratio to ratios
above one and finishes iteration eight at `0.787`; scalar mass splitting keeps
every ratio below one and finishes at `0.0369`. Unperturbed symmetric gravity
contracts under both methods, so this probe is diagnostic evidence for the
rocking/chain mode, not a formal spectral proof or an awake acceptance result.

GPU scalar checkpoint, 2026-07-16: solver-execution schema v2 names the normal
candidate explicitly while retaining point Jacobi as `SELECTED`. The
laboratory-only scalar-mass-split lane reads exact point incidence from either
the canonical body scan or stable body-edge ranges during preparation, scales
normal and tangent effective mass, and leaves equal-and-opposite emission plus
full-physical-mass gather unchanged. Both topology views produce byte-identical
body, cache, and pass traces for the two-stack check; no dispatch, buffer, or
product-constructor path was added. Manifold modes remain explicitly rejected
at this checkpoint pending their separate implementation slice below.

GPU block checkpoint, 2026-07-16: the generated-contact laboratory now runs
one coupled active-set normal solve from each stable pair slot's local point
zero and emits both points through the unchanged equal-and-opposite record and
full-physical-mass body gather. The one-point form is byte-identical to point
Jacobi, tangent/friction staging is unchanged, and canonical versus stable
gather produces identical bodies, cache, and block traces. Each trace retains
the symmetric matrix coefficients, active-set choice, final impulses, and the
normal effective-mass incidence applied to each body; an invalid active-set
result rejects the complete artifact. Pure manifold block adds no topology
memory or dispatch. Combined block mass splitting counts distinct valid live
manifolds in one candidate-only dispatch and adds exactly four topology bytes
per body. Point Jacobi remains `SELECTED`; no constructor, tolerance, physical
default, or acceptance band changed.

Factorial automation checkpoint, 2026-07-16: the awake evidence runner now
defines exactly sixteen candidates—point Jacobi, scalar mass split, manifold
block, and block mass split at 1/2/4/8 iterations—and evaluates the unchanged
six S5.4 sources plus the 16-body stack. It retains the frozen eight-iteration
normal traces, exact GPU checkpoints, process-isolated awake Box2D samples,
first-failure reproducers, complete 32/256/1K timing rows, topology capacity,
dispatch, and raw timing evidence. A versioned decision record checks earlier
structural failure and strict increases in settled peak linear speed, angular
speed, or energy between adjacent iteration counts. It triggers the colored
generated-contact comparator only when no experimental family member also
preserves all six existing structures, replays exactly and capacity-clean, and
fits a complete 1K step below 16.67 ms. This checkpoint implements the runner;
the clean measured package and any conditionally required comparator remain
separate commits.

Every-step evidence correction, 2026-07-16: convergence decision version 2 now
defines structural preservation by evaluating every one of the 1,200 requested
GPU steps. The bounded native runner copies post-step poses and
construction-capacity counters into one
validation-only history buffer, maps it only after the final step, and scans
the exact ordered history without assuming that failure is monotonic. A lane
that topples or separates transiently and later recovers therefore retains its
first failed step and no longer qualifies as preserving the corpus. Settled
speed and energy trends are compared only when both adjacent iteration lanes
preserve structure throughout, avoiding a misleading comparison between a
quiet toppled structure and an active upright one. This changes evidence
reporting only; it does not change simulation execution or the colored-family
trigger found by the preliminary probe.

Colored comparator checkpoint, 2026-07-16: corrected every-step decision
version 2 confirmed the conditional trigger, so solver-execution schema v3 now
separates normal formulation from accumulation architecture. The comparator
colors the current generated point graph deterministically on the GPU, then
applies each disjoint color directly to the shared motion state. It retains the
same generated narrowphase points, stable pair slots, stable body-edge view,
warm-start cache, preparation, physical parameters, friction, restitution,
relaxation, and body storage as the Jacobi/gather lanes. It neither constructs
a second world nor changes `SELECTED`, which remains point Jacobi with
Jacobi/gather accumulation.

The portable comparator deliberately encodes all 32 bounded color passes for
each warm, normal, tangent, relaxation, and restitution stage; unused colors
perform no work. One deterministic GPU invocation rebuilds colors after each
generated contact graph, and no CPU count or result advances the solve. Color
evidence records every live point assignment, the used count, the fixed
32-color bound, topology bytes, and dispatches. A 33rd incident contact point
atomically rejects the candidate with an explicit `solver-colors` capacity and
failure artifact. Disjointness, exact replay, and that overflow path are
permanent tests. The complete 1/2/4/8 quality and timing matrix remains the
next evidence slice; the high fixed dispatch count is intentionally retained
rather than hidden before measurement.

S5.4a completion, 2026-07-16: the clean
[parallel-normal factorial package](../../evidence/p5.2b-s5.4a/README.md)
records 20 candidates, seven awake scenes, 84 clean process-isolated Box2D
authority invocations, 48 exact first failures, 60 complete GPU timing rows,
six optimized Box2D rows, synchronized playback, and fresh selected-product
headless/browser/AVD artifacts. All candidates replay exactly and remain
capacity-clean. No family passes the strict monotonic gate and no candidate
satisfies every conditional exit.

Colored in-place is the structural leader: i4 and i8 preserve all seven
structures, whereas no Jacobi/gather formulation preserves the 16-stack.
The fixed-pass colored i4 implementation costs `19.308 ms` and 2,360
dispatches at 1K, versus `0.185 ms` for optimized Box2D, so it is not a
production candidate. The recommended next human-authorized experiment is a
GPU-owned compact manifold-color schedule with coupled two-point block normals
and fused compatible stages. It must preserve the one-world contracts, avoid
CPU readback, pass the same quality/capacity/cost gates, and execute through
shared browser and AVD validation before promotion. Point Jacobi/Jacobi-gather
remains `SELECTED`; S5.5 remains blocked at this renewed human checkpoint.

### S5.4b — compact colored scheduling and manifold blocks

Human authorization, 2026-07-16: proceed with the bounded factorial in
[contact solver parallelism](../topics/contact-solver-parallelism.md). This
authorizes laboratory scheduling and solver candidates only. Point
Jacobi/Jacobi-gather remains `SELECTED`; no product default, residual band,
tolerance, pacing rate, broadphase, liquid path, or CPU fallback is authorized.

- [x] Add a compact/fused scheduling-only point-colored control that is
  byte-identical to the existing point-colored result at equal iterations.
  The V4 laboratory execution contract distinguishes fixed-full-grid from
  compact-indirect scheduling while retaining point units and forward order.
  Native Metal replay of the eight-body/eight-step/i2 stack is byte-identical
  across bodies, cache, prepared contacts, contribution impulses, and block
  evidence. Stable per-color compaction reports occupancy, point indices,
  indirect arguments, empty launches, and scheduled invocations; the
  33-incident-point overflow fixture emits zero-work arguments for every color
  and preserves the atomic failure. No solve stages were fused: every possible
  cross-stage fusion changes the established color/stage order, so none meets
  this slice's byte-exact attribution gate.
- [x] Add manifold-granularity coloring with the active-set 2x2 normal block
  and explicit one-point degeneration.
  One compact color item now owns a complete generated manifold. Two-point
  normal stages run the existing validated active-set block equations; warm
  start, tangent, and restitution points execute deterministically inside the
  same disjoint item. Native Metal stack replay is exact, every coupled matrix
  and active set is preserved in evidence, a one-point restitution manifold is
  byte-identical to compact scalar coloring, and 33 manifolds incident on one
  body reject atomically with zero indirect solve work.
- [x] Compare stable forward with deterministic alternating forward/reverse
  color sweeps at i1/i2/i4.
  Alternating parity is the global `substep * iterations + iteration` ordinal;
  normal and tangent retain one direction, relaxation mirrors the matching
  iteration, and warm/restitution use the first/last substep parity. Exact
  native i1/i2/i4 replays preserve assignments and capacity, differ from the
  forward lane on the stack corpus, and the one-color degeneration remains
  byte-identical. Full 1,200-step quality and cost judgments remain in the
  evidence slice below.
- [x] Separate color construction, stable compaction, active-color solve, and
  zero-work indirect timing without changing the ordinary measured graph.
  The benchmark retains complete-step timestamps and adds per-operation
  phase classification. Consecutive operations of one phase share a bounded
  timestamped pass, and samples partition safely at WGPU's 4,096-query limit;
  this avoids the fixed-grid i4 control's former 4,660-query rejection and
  2,330-pass diagnostic explosion. The clean matrix, not this mechanical
  checkpoint, owns every cost conclusion.
- [x] Retain the seven accepted sources and add 32/64-stack depth diagnostics,
  both 100-body buckets, and deterministic construction/erase churn.
  The atomic runner and shared diagnostic sources are implemented: 32/64 are
  accepted by the stack generator only as named diagnostics, and five churn
  snapshots record construction, erase, generation reuse, full rebuild units,
  surviving units, and surviving color changes. The first full attempt closed
  all seven accepted sources and stack-32, then exposed the declared 32-color
  limit at stack-64 post-step 357. Diagnostic histories now preserve that
  transient `FailureV1` and continue, while strict history callers still
  reject it. The clean package completes every diagnostic and preserves all
  resulting failures.
- [x] Publish complete color occupancy, compaction/recoloring cost, dispatch,
  memory, 32/256/1K timing, exact replay, capacity, and first-failure evidence.
  The clean S5.4b package retains 24 GPU rows, six optimized Box2D rows, 19
  permanent reproducers, 124 authority runs, all color work/capacity records,
  and raw ordinary plus diagnostic timing samples.
- [ ] Run a potential candidate through native/headless/browser and the
  hardware-backed `jstorrent-tablet` AVD; attempt physical Android if attached.
  The shared opt-in `solver-manifold-i2` scene and scene-contract-v4 execution
  identity are implemented. Native exact-eight-step and Chrome BrowserWebGPU
  smokes pass without changing the selected product execution. Keep this item
  open until clean-revision headless/browser/AVD artifacts are copied into the
  renewed package and a physical device is attempted if attached.
- [x] Add incremental coloring only if rebuild cost is material, or a
  workgroup-per-island comparator only if dispatch boundaries remain material.
  Both conditions are measured true. The leading qualified forward-i2 lane is
  `17.685 ms` at 1K; construction plus compaction is `16.740 ms` diagnostic
  P50 versus `0.393 ms` active solve, and empty color work is `4.080 ms`.
  Every surviving churn unit keeps its color while all units are rebuilt.
  The first comparator is now a versioned scheduling policy that retains
  generation-safe stable-pair manifold colors and reports retained, new,
  recolored, and removed work. Native forward-i2 exactness/replay and the
  33-manifold atomic overflow pass. The second comparator constructs dynamic
  islands on GPU, compacts representatives by island/color, and uses one
  indirect workgroup per island with barriers between colors. Fixed bodies do
  not connect islands or receive raced writes. Connected-stack and two-island
  shared-floor results match per-color execution byte for byte; the combined
  incremental/island policy replays exactly and atomic overflow remains
  zero-work. Run the shared matrix before any cost conclusion.
- [ ] Publish a renewed review package and stop before solver promotion,
  accepting a residual band, retargeting pacing, or beginning S5.5.

Commit boundaries:

1. authorization, corrected evidence claims, factorial, exclusions, and gates;
2. compact/fused point-color control and byte-equivalence evidence;
3. manifold-colored active-set block execution;
4. forward versus alternating order;
5. depth/degree/churn automation and complete cost;
6. a conditional incremental-color or workgroup-island comparator only if its
   recorded trigger fires; and
7. clean evidence, governing status, and renewed human checkpoint.

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
