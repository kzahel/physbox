# S3: Small-world collision and contact persistence

Status: active quality follow-up, 2026-07-16; combined representation gate
passed and human review authorized the S5.4a mass-splitting/manifold-block
experiment before closure.
Direction: [one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Bring-up stage: [P5.2](../bringup-plan.md#p52--small-world-collision-and-persistence-active).
Predecessor: [accepted P5.1 body core](../../evidence/p5.1-s2.6/README.md).

## Goal

Replace the temporary precomputed floor manifold with the first product GPU
collision and outer-step contact-persistence path. Add circles beside oriented
rectangles, generate complete all-pairs candidates for deliberately small
worlds, produce stable manifolds checked directly against public Box2D
collision functions, transfer cached impulses by sorted persistent keys, and
feed the accepted Jacobi/gather solver without simulation-dependent readback.

Finish with correctness, behavior, capacity, cross-host, and scoped performance
evidence for human review before the contact representation is frozen or P5.3
begins.

## Accepted foundations

S3 executes these decisions and may not silently revise them:

- `BodyCoreV1` is four independent 16-byte lanes, 64 bytes per body slot, with
  a generational `BodyId { slot, generation }`. It is the stable dynamic core,
  not a universal body-plus-shape-plus-joint allocation.
- The existing 48-byte command record and atomic diagnosed capacity-failure
  semantics are accepted. Numeric capacities, scratch meanings, and the
  full-capacity grab-hold scan remain unfrozen.
- Contact-centric Jacobi with equal-and-opposite records and GPU per-body
  gather remains the selected solver family and accumulation architecture,
  with the accepted S1.6 fixed-step and solver parameters.
- Interactive frames use the no-catch-up slow clock; validation and throughput
  measurements use explicit exact stepping.
- Invariants and the scalar specification own pass correctness. Clean pinned,
  process-isolated Box2D v3 owns rigid manifold and behavior acceptance.
- Every product stage remains portable WebGPU and GPU resident. No CPU product
  collision path, fallback, required native feature, or simulation-dependent
  readback is allowed.

## Scope and exclusions

S3 includes one centered analytic shape per body: circles and oriented
rectangles. It includes body AABBs, complete all-pairs candidate generation,
circle/circle, circle/rectangle, and rectangle/rectangle narrowphase, stable
feature identifiers, sorted outer-step contact persistence, warm-start
transfer, friction, restitution, speculative separation, and the accepted
solver operating on generated contacts.

All-pairs is intentionally a small-world correctness baseline. It remains the
pair-completeness oracle after S3 but is not presented as the scalable product
broadphase.

S3 excludes scalable SAP/grid/LBVH selection, capsules, triangles, general
polygon or compound-shape storage, per-shape materials, joints, sleeping,
construction tools, save/load, liquid, CCD, terrain, and a CPU product backend.
Future general shapes use separate versioned shape/geometry buffers rather
than silently reinterpreting `BodyCoreV1`. Future joints use separate
constraint records referencing generational body handles.

## Ownership and shared execution

The engine owns analytic shape interpretation, AABB generation, pair and
manifold pipelines, persistence, solver integration, capacities, rendering,
and scenario outcomes. `physbox-contracts` owns versioned neutral evidence
records. `physbox-lab` owns scalar/invariant comparison, oracle orchestration,
minimization, and synchronized playback. `physbox-launch` owns scenario names
and arguments.

Native, browser, headless, and Android adapters transport lifecycle, raw input,
arguments, and artifacts only. Box2D remains an isolated test process consuming
the engine-owned neutral projection. No platform or oracle authors a second
fixture or numerical expectation.

The product dataflow is one GPU graph:

```text
BodyCoreV1
  -> body AABBs
  -> complete canonical all-pairs candidates
  -> analytic/SAT/clipped manifolds with stable features
  -> sorted current keys + previous cache merge
  -> prepared constraints and warm impulses
  -> accepted Jacobi contributions and body gather
  -> updated BodyCoreV1 pose/motion
  -> direct render
```

Counts, overflow flags, indirect work, contact caches, and solver state stay on
the GPU. Bounded validation may map complete results only after its final step.

## Stop conditions

Stop early for human direction if progress requires changing `BodyCoreV1`, the
solver family or accumulation architecture, accepted physical defaults,
numerical tolerances, oracle authority or pin, the no-catch-up clock, platform
portability, or equal-and-opposite current-state response. Also stop for a
required CPU fallback, native-only path, simulation-dependent readback, silent
pair/manifold/contact loss, or a genuine Box2D-versus-invariant conflict.

Stop if evidence shows that choosing the feature-ID convention, contact-key
shape, cache organization, manifold point grouping, or capacity architecture
requires a material difficult-to-reverse fork not resolved by the recorded
comparisons. Do not add general polygons, joints, a scalable broadphase, or
liquid to answer an S3 failure.

Ordinary shader organization, reversible buffers, exact capacity sizing,
fixture additions inside scope, test failures, minimization work, and an
unavailable physical device do not require interruption. Preserve unavailable
hardware as a named gap.

## Delivery slices

### S3.1 — analytic shape and persistence contracts

- [x] Define the versioned circle encoding within the accepted analytic shape
  payload and reserve a reviewed shape flag without changing body lane strides.
- [x] Define canonical body AABB, candidate pair, manifold, contact point,
  geometric feature, persistent key, cache value, work, capacity, trace, and
  failure records with complete byte accounting.
- [x] Add shared circle/rectangle scenario sources and neutral Box2D projection;
  reject unsupported shapes or actions instead of approximating them.
- [x] Add structural and negative tests for canonical body ordering, generation
  participation, finite values, feature ordering, schema/hash stability, stale
  identity, and inconsistent capacity claims.

Completion, 2026-07-15: collision layout version 1, the provisional geometric
feature convention, atomic capacity artifacts, four shared sources and neutral
projections, exact byte claims, canonical hashes, and permanent negative cases
are recorded in the [S3.1 package](../../evidence/p5.2-s3.1/README.md). The
accepted body lanes remain 64 bytes and commands remain 48 bytes. Rectangle
bit 8 is unchanged; circle bit 9 uses the existing analytic payload. No GPU
collision behavior is included.

Likely boundaries: `physbox-contracts` owns serializable records;
`physbox-engine` owns analytic body encoding and GPU layouts; `physbox-lab`
owns comparison inputs. No general shape table is introduced in this slice.

Artifacts: schema round trips, byte tables, canonical hashes, neutral circle
and mixed-shape projections, and permanent invalid/stale fixtures.

Gate: `BodyCoreV1` and existing command bytes are unchanged; every new record
has a version, deterministic ordering, source/projection hash, explicit
capacity, and negative validation; every scenario has one shared Rust source.

Commit boundary: contracts, scenario sources, negative tests, byte evidence,
and the S3.1 tactical status land together without GPU collision behavior.

### S3.2 — complete GPU all-pairs candidates

- [x] Generate circle and oriented-rectangle world AABBs from current GPU pose
  and analytic shape data.
- [x] Enumerate every unordered body pair, reject fixed/fixed and non-overlap
  cases, and emit sorted canonical candidates without CPU count readback.
- [x] Compare complete GPU candidate sets against brute-force scalar AABB
  enumeration across deterministic randomized and adversarial small worlds.
- [x] Diagnose attempted, emitted, rejected, high-water, and overflow counts;
  overflow invalidates the complete step and never truncates a pair prefix.

Completion, 2026-07-15: the [S3.2 evidence package](../../evidence/p5.2-s3.2/README.md)
records exact scalar/GPU agreement across four governed sources and 64 seeds
times five randomized/adversarial patterns (324 cases), with zero missing,
unexpected, or duplicate pairs. Maximum AABB absolute error was
`2.3841858e-7` under the unchanged `2e-5` comparison bound. An attempted
119-pair output at capacity one committed zero and produced a structured
failure; the injected missing-pair harness minimized deterministically to two
bodies. The eight-dispatch scan/scatter graph never maps a count to schedule
work. Its explicit all-pairs scratch remains a quadratic small-world baseline,
not the P5.3 broadphase selection.

Likely boundary: a focused engine collision module and portable WGSL passes
consume `BodyCoreV1`; the lab reuses their versioned outputs after bounded
completion rather than implementing an alternate product broadphase.

Artifacts: exact candidate-set diffs, randomized seeds, minimized missing-pair
reproducers, dispatch/work records, and capacity-negative artifacts.

Gate: zero false negatives and canonical duplicates across the accepted
corpus, exact brute-force agreement where declared, finite AABBs, explicit
quadratic work accounting, zero accepted overflow, and no simulation-dependent
readback.

Commit boundary: AABB/candidate pipelines, brute-force comparison, failure
minimization, capacity evidence, and tactical checkboxes land together.

### S3.3 — Box2D-checked GPU narrowphase

- [x] Implement circle/circle and circle/rectangle analytic manifolds and
  oriented rectangle/rectangle SAT reference selection plus incident-edge
  clipping, with no more than two canonical points.
- [x] Assign stable geometric feature identifiers and test nearly tied axes,
  swapped body order, translation/rotation equivariance, and contact creation
  and removal boundaries.
- [x] Compare direct results with public `b2CollideCircles`,
  `b2CollidePolygonAndCircle`, and `b2CollidePolygons` from the clean pinned
  Box2D authority.
- [x] Randomize and deterministically minimize topology, normal, point,
  separation, and feature disagreements into permanent reproducers.

Completion, 2026-07-15: the [S3.3 evidence package](../../evidence/p5.2-s3.3/README.md)
records exact scalar/GPU topology and feature agreement across 324 cases and
100 fresh clean-pinned process-isolated Box2D public-collision runs. Maximum
GPU numerical error was `1.2516975e-6`; maximum authority geometry error was
`3.2782555e-7`, both below the unchanged `2e-5` bound. Atomic grouped overflow
committed no manifolds or points, and injected minimization reached two bodies.
The provisional deep-coincidence coherence exception is measured, documented,
and remains an explicit S3.6 review item rather than a hidden tolerance change.

Likely boundary: engine WGSL produces the product manifold contract; the
existing process-isolated Box2D adapter exposes only public collision results;
the shared lab owns correspondence, tolerances, and diffs.

Artifacts: direct authority request/results, invariant matrix, randomized
corpus, first-divergence traces, minimized reproducers, and compact manifold
playback.

Gate: identical accepted topology and feature convention plus bounded geometry
against Box2D, all independent metamorphic invariants pass, every disagreement
is explained or preserved, and no tolerance or source pin changes silently.

Commit boundary: all three narrowphase paths, direct authority comparison,
minimized failures, evidence, and governing status land together.

### S3.4 — sorted persistence and warm-start transfer

- [x] Form persistent keys from canonical generational body handles and stable
  features, radix-sort current contacts, and merge them with the previous GPU
  cache entirely on device.
- [x] Transfer accumulated normal and tangent impulses to exact matches; start
  new points cold and expire missing or generation-stale points deterministically.
- [x] Preserve manifold point grouping and contact ordering through pair churn,
  body reset/reuse, feature transitions, and deliberately permuted input order.
- [x] Emit cache high-water, matched/cold/expired counts, bytes per point, sort
  and merge work, overflow, and first-divergence traces.

Likely boundary: persistence uses shared scan/radix infrastructure and feeds
the existing prepared-contact contract. The CPU neither owns the cache nor
reads its count to schedule the next interactive step.

Artifacts: multi-step cache traces, deterministic replay hashes, churn and
stale-generation fixtures, memory/work tables, and minimized persistence
failures.

Gate: exact key stability and deterministic replay, correct warm/cold/expired
classification, no impulse inheritance across reused generations, zero
accepted overflow, no per-step mapping, and complete scalar/pass comparison.

Commit boundary: GPU sort/merge/cache mechanics, replay and negative tests,
capacity/performance evidence, and tactical status land together.

Completion, 2026-07-15: the [S3.4 evidence package](../../evidence/p5.2-s3.4/README.md)
records exact scalar/GPU persistence across 100 cases and 212 multi-step
states, including byte-exact replay, deliberate input reversal, feature churn,
separation/recontact, and stale body generations. Atomic overflow committed no
cache entries, and injected failure minimization reached two bodies. The
48-byte persistent record and 64-byte current sort record are explicit. The
direct eight-word radix baseline costs 64 passes and 212 total graph dispatches
per outer step; that measured cost remains provisional for S3.6 rather than an
accepted production-performance claim.

### S3.5 — generated-contact solve and cross-host evidence

Implementation checkpoint, 2026-07-15: the shared product graph now feeds
all-pairs candidates, generated manifolds, sorted warm-start persistence, and
the accepted Jacobi/body-gather solver directly from the GPU body store. The
temporary authored floor manifold is no longer used by bootstrap, rigid-floor,
or reset/grab; the new `rigid-stack` launch exercises mixed circles and
rectangles. Exact validation splits long requests into bounded submissions
without changing the requested step count or mapping intermediate simulation
state. Canonical per-body incidence construction removed an eight-body
deep-coincidence replay race while retaining the accepted accumulation
architecture. Native, browser, headless, and the host-GPU `jstorrent-tablet`
AVD execute the graph; no physical Android device is currently connected.
Correctness-qualified authority, failure, and performance publication remains
the rest of this slice.

Evidence checkpoint, 2026-07-15: the
[S3.5 package](../../evidence/p5.2-s3.5/README.md) publishes nine exact
GPU/Box2D tracks, 128 randomized cases, atomic overflow, native/browser/AVD
captures, 32/256/1K complete-step timings, optimized Box2D rows, and
synchronized playback. Execution mechanics pass, but the slice gate is not
accepted: the unreviewed speculative margin, minimized deep-overlap energy
failure, material small-stack endpoint difference, and noncompetitive
64-pass/282-dispatch baseline are explicit S3.6 decisions. No physical default,
quality band, tolerance, or representation was silently accepted.

- [x] Replace the temporary precomputed floor manifold in the product path
  with generated and persisted contacts feeding the accepted Jacobi/gather
  solver.
- [x] Exercise circle/rectangle falls, bilateral impacts, off-center rotation,
  flat/corner/edge landings, small stacks, friction slides, restitution, pair
  churn, reset, and grab/release through exact shared scenarios.
- [x] Compare isolated passes with the scalar specification and bounded worlds
  with deterministic and optimized Box2D lanes at equivalent quality.
- [x] Capture native, browser, headless, host-GPU `jstorrent-tablet`, and
  available physical Android outcomes with complete provenance and synchronized
  failure playback.
- [x] Measure collision-only and end-to-end scopes at 32, 256, and applicable
  1K small-world bodies, reporting candidate amplification, manifolds/points,
  cache churn, solver work, dispatches, memory, timing, and every capacity.

Artifacts: fresh atomic `evidence/p5.2-s3.5` package, authority/GPU traces,
synchronized playback, cross-host matrix, scoped CPU/GPU timing, capacity
matrix, randomized corpus, and minimized permanent failures.

Gate: finite state, stable contacts, projected friction/normal impulses,
bounded penetration and energy, accepted scalar and Box2D bands, complete
replay, zero overflow, identical structural host outcomes, and no readback
dependency. Performance rows are invalid when any correctness or capacity gate
fails and are not presented as the P5.4 crossover claim.

Commit boundary: product collision/persistence integration and its complete
correctness-qualified evidence land together; no P5.3 broadphase candidate is
bundled into the slice.

### S3.6 — contact representation and P5.3 readiness review

- [x] Publish a compact report covering analytic shape encoding, manifold and
  feature conventions, persistent key/cache layout, bytes and capacities,
  behavior/authority results, cross-host status, scoped performance, minimized
  failures, rejected alternatives, and reconsideration triggers.
- [x] State whether evidence supports freezing the S3 contact surface and
  advancing to the P5.3 broadphase bakeoff without selecting that outcome
  silently.
- [x] Stop for human review before freezing contact ABI, beginning SAP/grid/LBVH
  work, adding polygons/joints, or widening accepted numerical bands.

Gate: a reviewer can assess collision correctness, persistence stability,
memory/dispatch cost, downstream broadphase compatibility, and known failures
without reading the full implementation or raw logs.

First manual checkpoint, 2026-07-15: the
[S3.6 review](../../evidence/p5.2-s3.6/README.md) recommends against freezing
or advancing. It requests review of proposed `0.02` speculative distance and
`3.0 m/s` depenetration speed, a compact/pair-grouped persistence bakeoff, and
a stable adjacency bakeoff. No proposal is accepted until the human decision
is recorded in a later commit.

Pinned-source addendum, 2026-07-15: the
[PhysX GPU organization study](../../evidence/p5.2-s3.6/physx-gpu-organization-study.md)
finds that key width, not radix digit width, drives the 64-pass baseline. It
strengthens stable pair-slot persistence and compact body-edge index views with
segmented gather as the lead bakeoff lanes. PhysX remains a source reference,
not an oracle or dependency. At publication this changed no accepted scope;
the subsequent human authorization recorded below activates the bakeoff.

Commit boundary: the S3.6 review package and manual-checkpoint status land
alone. A human decision is a later documentation commit.

### S3.5a — approved contact-policy and representation remediation

Human authorization, 2026-07-15: execute this measured bakeoff and regenerate
S3.5/S3.6 evidence. This authorization selects neither a production contact
representation nor final physical defaults.

- [x] Add explicit `0.02` speculative distance and `3.0 m/s` maximum
  depenetration speed as candidate policy, with the two minimized S3.5
  failures and clean Box2D/structural curves as mandatory comparisons.
- [x] Keep the exact 256-bit point-key sort as a correctness baseline and
  compare stable pair-slot, pair-grouped two-point persistence as the lead
  lane. Keep compact fingerprint plus full equality verification as a
  fallback/comparator; a fingerprint alone never transfers impulses.
- [x] Keep the canonical body-by-point incidence scan as a correctness
  baseline and compare compact stable body-edge indices, offsets, and
  deterministic segmented gather as the lead lane.
- [x] Express pair, body-endpoint, and later particle orderings as compact
  index/remap views over canonical payloads. Do not change `BodyCoreV1`, create
  a liquid ABI, begin liquid, or create a second engine.
- [x] Measure persistence, index sorting, offset construction, gather, and
  solve separately, including dispatches, key/index bytes, allocation and
  high-water, P50/P95/P99, exact replay, permutation, churn, recontact,
  generation reuse, collision/full-verify negatives, and atomic overflow.
- [x] Retain generated 32/256/1K rows and add isolated contact-rich authored
  1K and 10K representation rows that do not pay the quadratic all-pairs
  baseline. Regenerate native, browser, maintained host-GPU AVD, available
  physical Android, optimized Box2D, failure, and synchronized playback
  evidence.

Contact-policy completion, 2026-07-15: the optional versioned scenario policy
now drives conservative GPU/scalar AABB expansion, actual-separation
narrowphase admission, and a per-contact depenetration-bias clamp in the scalar
specification and both GPU accumulation lanes. Policy-free authority and
authored scenarios retain their prior canonical encoding and exact-overlap
behavior. Permanent scalar/GPU/end-to-end cases keep both minimized S3.5
reproducers: the `0.02`/`3.0 m/s` candidate creates two speculative contacts and
reduces the crossing case from `0.00507` to `0.000508` final penetration, while
reducing the six-body coincident maximum speed from `126.27` to `27.41 m/s`.
The two explicit policy uniform lanes add 32 bytes to the maintained floor
scene (`23668` to `23700` capacity bytes) without changing its 282-dispatch
schedule. Those residuals remain evidence for review; this completion does not
accept the candidate as the product default.

Pair-persistence implementation checkpoint, 2026-07-15: an isolated portable
GPU comparator now groups at most two points under each canonical generational
body pair, retains stable logical slots for active pairs, and recycles lost
slots deterministically with an incremented slot generation. A compact feature
fingerprint is only a shortlist: complete local feature equality and complete
pair identity are required before either impulse transfers. Permanent cases
cover scalar equivalence, exact replay, reversed scheduling, churn/recontact,
body-generation reuse, forced fingerprint collision, and whole-stage overflow.
The comparator reduces candidate/narrowphase/persistence work from the
64-pass baseline's 212 dispatches to 25 (10 persistence dispatches) without
mapping until the final bounded step. This checkpoint implements a bakeoff
lane; it does not select the representation, replace the product path, or make
a timing claim before the shared scale matrix.

Body-edge implementation checkpoint, 2026-07-15: the real generated-contact
solver can now select an isolated stable body-edge topology while the product
continues to default to the canonical body-by-point scan. The candidate emits
two eight-byte `{ body, contribution }` index records per point, stably
radix-groups only the body key, derives contiguous per-body ranges, and gathers
the unchanged equal-and-opposite contribution payloads in canonical
point/side order. It uses one to four radix passes through the governed scale
range: topology construction is 6 dispatches for the small behavior corpus, 9
at 32 bodies, 12 at 256/1K, and 15 at 10K, versus the baseline's fixed 4. The
32-body fixed-capacity row records 1,984 edges and 32,512 topology bytes versus
8,320 baseline topology bytes. All eight behavior families, 30-step pair
churn, eight-body high contention, reversed build scheduling, qualified
benchmark completion, and upstream atomic overflow are byte-exact against the
baseline. This implements the second bakeoff lane but makes no latency or
memory winner claim before contact-rich measurement.

Evidence-automation checkpoint, 2026-07-15: one fresh, atomic native command
now measures the exact point-key and stable pair-slot persistence lanes on the
retained 32/256/1K generated scenarios, compares both gather topologies in the
complete generated graph, and runs canonical-scenario 1K/10K authored-contact
rows without the all-pairs stage. Representative timestamp samples remain
distinct from an explicitly instrumented diagnostic lane that attributes
index construction, offset construction, gather, and the remaining solve
operations. Every large authored row performs two exact result-digest
replays, while permanent tests compare the candidate digest directly with the
canonical topology. The command also preserves randomized
replay/permutation and atomic-overflow evidence, records complete source dirty
state, publishes only by fresh directory rename, and always records
`selectionMade=false`. This completes the compact index-view organization and
evidence mechanics; the timing and cross-host checkboxes remain open until a
clean same-revision package is committed.

Clean evidence completion, 2026-07-15: the
[S3.5a package](../../evidence/p5.2-s3.5a/README.md) records 30-sample Metal
rows at clean revision `27a0e8c`. Stable pair slots preserve complete
pair/feature verification while reducing isolated persistence P50 by
68x/198x/416x and comparable allocation by 47%/40%/27% at 32/256/1K bodies.
Stable body edges preserve exact output while improving complete generated
P50 by 1.04x/1.28x/1.38x and isolated contact-rich P50 by 1.54x/2.28x at
1K/10K contacts. Their fixed-capacity topology memory is about four times the
generated scan baseline and about twice the contact-rich topology bytes. All
64 bakeoff randomized cases and 128 generated authority cases replay exactly,
overflow is atomic, native/browser/hardware-accelerated AVD pass the same
scene contract, and the attempted physical lane had no connected device. The
pair and edge gains remain separately scoped; no combined candidate or CPU
crossover claim is inferred. S3.6 remains the human selection checkpoint.

Renewed manual checkpoint, 2026-07-15: the revised
[S3.6 review](../../evidence/p5.2-s3.6/README.md) recommends selecting stable
generational pair slots plus stable body-edge index views, and accepting the
`0.02` speculative-distance / `3.0 m/s` per-contact depenetration candidate as
provisional P5.2 defaults. It requires one post-selection combined-graph
integration and structural-quality gate before closing S3 or making a CPU
performance claim. The current full-key/body-scan baseline and physical
defaults remain selected until human acceptance is recorded.

Human decision, 2026-07-15: accept stable generational pair slots and stable
body-edge index views as the P5.2 production direction, subject to one combined
product-graph integration gate. Accept `0.02` speculative distance and
`3.0 m/s` per-contact maximum depenetration velocity as provisional P5.2
defaults. Keep the full semantic key and canonical body scan as correctness
oracles, keep deep coincident placement diagnosed rather than ordinary, and
accept no final stack-quality band or CPU crossover claim. This authorizes the
post-selection integration slice; it does not close S3 or authorize P5.3.

Sequencing addendum, 2026-07-15: human review prioritized S4 camera, shared UI,
diagnostics, and anchored-grab slices before the combined S3 closure work so
interactive physics feedback becomes useful. Minimal place/erase still waits
for combined pair-slot integration because body retirement and generational
slot reuse must prove cache invalidation. P5.3 remains blocked until both S3
and S4 gates close.

Execution addendum, 2026-07-15: the accepted next sequence is tracked by
[S5 interactive rigid quality, construction, and scale](005-interactive-rigid-quality-and-scale.md).
It adds an easy rectangle stack and objective settling contract before the
combined graph, then uses successful generational invalidation to unblock the
shared circle/rectangle construction loop. This does not weaken the S3 closure
gate or authorize P5.3 early.

Gate: the speculative and deep-overlap reproducers pass without widening the
existing numerical tolerance; every candidate exactly preserves accepted
semantic identity and deterministic replay; every overflow rejects the whole
stage; no step depends on readback; subphase and complete-step matrices remain
correctness-qualified; and the final S3.6 package exposes measured winners,
failures, rejected alternatives, and reconsideration triggers.

Commit boundaries: land this acceptance record alone. Then land the combined
product graph and its exactness/capacity tests, structural-quality evidence,
performance/cross-host evidence, and S3 closure in separately reviewable
commits. Stop for human review before widening a quality band or advancing to
P5.3 with a failed combined gate.

Post-selection pair-slot checkpoint, 2026-07-15: stable generational pair
slots now replace the 64-pass full-key sort in the real generated-contact
product graph. A dedicated pair-local bridge preserves complete feature
verification, the narrowphase's sparse two-point slot convention, and the
full-key oracle's exact contribution order. All behavior families match the
explicit full-key lane at every checked step under reversed scheduling and a
forced fingerprint collision; isolated generation reuse, churn/recontact, and
atomic overflow remain permanent negatives. The graph now performs 25
candidate/narrowphase/persistence dispatches instead of 212. The canonical
body scan remains selected until the next separately committed stable-edge
integration, so this checkpoint does not satisfy the combined closure gate.

Post-selection stable-edge checkpoint, 2026-07-15: the real product graph now
combines stable pair slots with stable sorted body-edge ranges. The canonical
body scan remains an explicit validation lane, and the combined product output
is exact against full-key/canonical and pair-slot/canonical oracles across all
behavior families, intermediate steps, permuted construction, churn, high
contention, and atomic overflow. The maintained two-body floor graph uses 97
dispatches and 19,640 bytes; a 32-body generated fixture uses 100 dispatches.
This is the selected combined representation, but it is not yet an S3 closure:
fresh combined timing, structural, platform, failure, and CPU-comparison
evidence remains mandatory.

The combined evidence runner now records selected topology provenance in each
behavior row, checks 32/256/1K generated rows against the full-key/canonical
oracle, retains raw samples, and includes exact 1K/10K authored-contact
canonical-versus-stable-edge rows. It reports representation acceptance and
solver-quality acceptance independently; generated packages therefore cannot
turn a preserved physical failure into an accepted representation claim.

Combined evidence completion, 2026-07-15: the clean
[combined representation package](../../evidence/p5.2-s3.6-combined/README.md)
passes the selected representation gate at revision `1e40c55`. All 9 directed
and 128 randomized replays are exact, the dense capacity negative rejects
atomically, 1K/10K authored-contact output matches canonical gather, and clean
native/browser/hardware-backed AVD artifacts name the selected topology. The
actual integrated 32/256/1K GPU P50 is `0.383`/`0.985`/`3.159 ms`; the 1K row
is about 7.4x faster than the prior `23.422 ms` full-key product baseline but
about 20.7x slower than the fastest equivalent optimized Box2D row. Stable
edges are 1.54x/1.46x faster than canonical gathering at 1K/10K authored
contacts. No CPU crossover is claimed.

This satisfies the post-selection combined representation requirement and
unblocks generational construction work. It does not set a final contact ABI,
accept stack quality, or mark the whole solver implementation accepted: the
two minimized physical-policy failures and the 4/8-body awake stack failures
remain explicit S5.4 inputs. S3 therefore stays active until the mandatory
S5.4 human review accepts a quality path; P5.3 also remains blocked by that
review and completion of the construction checkpoint.

S5.4 update, 2026-07-16: construction is complete and the
[awake settling package](../../evidence/p5.2b-s5.4/README.md) confirms all
nine reversible executions lose the 8-body stack while clean awake Box2D
preserves it. The representation exactness decision still stands, but no
contact-quality band or solver default is accepted. S3 remains at the human
checkpoint for the recommended manifold-coupled two-point laboratory question;
P5.3 remains blocked until that direction is reviewed.

Human authorization now advances that recommendation into the bounded S5.4a
factorial experiment. The stable pair/body-edge representation remains
accepted and unchanged; the outcome returns to human review before any solver
promotion or S3 closure.

## Validation baseline

Every shared slice runs the repository gates from `AGENTS.md`:

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

Runtime, shader, rendering, scenario, launch, or pixel changes also build the
Android APK, run the maintained host-GPU AVD lane, attempt the physical-device
lane when available, and visually inspect fresh artifacts. Narrowphase slices
add direct clean-pinned Box2D public-collision checks. Persistence and product
integration slices add exact scalar/pass traces, randomized replay and
minimization, clean oracle provenance, and the scoped performance/capacity
matrix declared above.

## Completion definition

S3 reaches `manual checkpoint` when S3.1 through S3.5 pass, the S3.6 review
package is committed, all known failures are minimized and linked, and no
contact-layout or quality decision is hidden in implementation history.
Completion requires later human acceptance of a portable contact and
persistence representation with complete pair/manifold correctness, explicit
capacity, no simulation-dependent readback, accepted Box2D behavior, and
complete cross-host evidence. Only that acceptance may close P5.2 and authorize
P5.3 broadphase selection work.
