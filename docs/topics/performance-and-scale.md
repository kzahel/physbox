# Performance and scale contract

Status: active contract with provisional targets, updated 2026-07-21.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Execution begins in the
[rigid solver laboratory](../tactical/001-rigid-solver-laboratory.md) and
continues through every stage of the [bring-up plan](../bringup-plan.md).

## Decision

Performance and correctness are coequal design axes from the first solver
experiment. GPU residency is justified by coherent future material coupling,
but PhysBox 5 must also demonstrate a measured scale advantage over a strong
CPU 2D engine. Scale cannot be postponed until the rigid pipeline is otherwise
complete: buffer layouts, accumulation strategy, dispatch topology,
broadphase, contact persistence, sleeping, and capacity policy all determine
the eventual performance envelope.

The benchmark result is a curve over a declared workload, not a maximum-body
badge. Every claim names the scene, active work, solver quality, adapter, host,
revision, and capacity state. A result is eligible for a performance claim only
after its correctness gates pass.

The initial planning shorthand is:

- **10K proves the engine works at useful scale.** It is not by itself a
  unique result.
- **30K should begin to demonstrate a GPU advantage** on realistic,
  contact-rich scenes on a named reference desktop adapter.
- **100K defines a qualified frontier.** It is plausible for sparse,
  mostly-sleeping, or bounded-neighbor scenes, not a promise for arbitrary
  dense stacks.
- **Phones are a first-class target with a smaller envelope.** The provisional
  planning band is 3K–10K awake contact-rich bodies on a named Android
  reference device — still far beyond a CPU phone engine and sufficient for
  the sandbox product. Mobile bandwidth, dispatch overhead, and thermal
  throttling interact badly with fixed-step pacing, so this band is measured
  early rather than extrapolated from desktop.

These are hypotheses to measure and revise. They are not portable absolute
acceptance thresholds yet.

## A scale result is a work vector

Body count alone hides the work that dominates collision and solving. Every
benchmark result records at least:

| Symbol | Meaning |
| --- | --- |
| `N_total` | Total live bodies |
| `N_awake` | Awake bodies that require integration or active solving |
| `N_shapes` | Live shapes, separated by circle and rectangle count |
| `P_candidate` | Canonical broadphase candidate pairs before narrowphase rejection |
| `P_overlap` | True AABB overlaps or the nearest available exact overlap count |
| `M` | Contact manifolds |
| `C` | Contact points or scalar constraints actually solved |
| `W_solver` | Solver work, including constraints multiplied by substeps/iterations or color passes |
| `D` | Compute dispatch count and declared global synchronization boundaries |
| `B_high` | GPU memory high-water and capacity high-water by buffer class |

Also record candidate amplification `P_candidate / max(P_overlap, 1)`, awake
fraction, pair churn, duplicate pairs before unique, maximum cell occupancy or
tree depth, and shape-size distribution where applicable. A statement such as
"100K bodies at 60 Hz" is incomplete without this work vector.

The asymptotic limit is explicit. A scene in which all `N` AABBs overlap
contains approximately `N * (N - 1) / 2` candidate pairs. No broadphase turns
that physical all-pairs workload into a linear real-time case. Such scenes test
bounded failure and overflow reporting rather than headline throughput.

## Evidence and initial expectations

The pinned Box2D authority is also a formidable CPU performance competitor.
Its official benchmark program uses four solver substeps and includes:

- a 100-row large pyramid with 5,050 dynamic boxes; and
- 400 ten-row pyramids with 22,000 dynamic boxes.

The checked-in Apple M2 Air Neon data at the pinned source revision reports
roughly 1.0 ms per timed step for the large pyramid and 4.1 ms per timed step
for many pyramids with four workers. These are best-run, physics-only results
from Box2D's benchmark method, not a PhysBox comparison and not a universal CPU
budget. The source of record is
`reference/box2d/shared/benchmarks.c` with results under
`reference/box2d/benchmark/m2air_neon/`; use
`npm run references:status` before citing them. Box2D publishes the same
[official benchmark suite](https://box2d.org/files/benchmark_ms_results.html)
across Apple M2 Neon and AMD 7950X SIMD configurations.

PhysX 5.6.1 reports that GPU rigid simulation can have very large advantages
with several thousand active actors, accelerates broadphase, contact
generation, shape/body management, and constraint solving, and requires
explicitly sized GPU buffers. Its default GPU memory settings are described as
sufficient for roughly 10,000 rigid bodies. This supports measuring a
crossover rather than expecting small scenes to favor the GPU, keeping the
complete hot pipeline resident, and treating capacity as correctness. See the
[PhysX GPU simulation guide](https://nvidia-omniverse.github.io/PhysX/physx/5.6.1/docs/GPURigidBodies.html).

The first expectations are therefore:

| Workload | Provisional expectation |
| --- | --- |
| 1K awake bodies with moderate contacts | CPU Box2D may win or remain competitive; use this as an overhead and crossover probe |
| 10K awake bodies with bounded, roughly linear contacts | Must be comfortably real-time; a GPU crossover is plausible but 60 Hz alone is not a unique claim |
| 30K awake bodies with bounded, roughly linear contacts | Plausible 60 Hz target on a reference desktop GPU and the first scale at which a material advantage should appear |
| 100K sparse or free-flight bodies | Plausible at 60 Hz and useful for integration, compaction, memory, and rendering, but not sufficient as the headline physics result |
| 100K mostly sleeping bodies | Should be practical if sleeping and active compaction are effective |
| 100K actively colliding circles | Strong-GPU stretch case; bounded-neighbor 30–60 Hz may be plausible and must be measured |
| 100K dense rectangle stacks | Not an initial portable-WebGPU promise; manifold points, propagation, and solver barriers may dominate |
| All-overlap or intentionally saturated cells | Capacity, diagnostics, and bounded-degradation test only |

"Real-time" initially means completing one fixed physics step inside a
16.67 ms 60 Hz frame on the named adapter. The eventual product physics budget
will be smaller so rendering and UI retain headroom. Do not freeze one absolute
physics-millisecond gate until measurements exist on a representative
integrated GPU, discrete GPU, browser implementation, and Android device.

## Benchmark scene families

Every family is an engine-owned, versioned shared scenario or a deterministic
parameterized generator. Native Box2D, GPU/headless, browser, and Android lanes
consume projections of that source; platform scripts and foreign adapters do
not recreate it.

| Family | Primary work exposed | Required variations | Claim role |
| --- | --- | --- | --- |
| Free flight | integration, active compaction, storage bandwidth, direct rendering | circles, rectangles, mixed; 100% awake and mostly sleeping | capacity/context only, never the sole headline |
| Sparse moving grid | broadphase construction, low amplification, coherent pair churn | uniform and randomized velocity; circle and mixed shapes | broadphase and end-to-end throughput |
| Packed circles | bounded-neighbor narrowphase and steady solver bandwidth | several densities, friction values, and awake fractions | contact-rich scale headline |
| Mixed tumbler | changing circle/rectangle contacts, persistent manifolds, friction | circle/rectangle ratios and angular speeds | realistic end-to-end headline |
| Rain onto floor | pair/manifold creation and destruction, allocation, warm-start churn | bursts, steady rain, different floor widths | transient throughput and high-water |
| Pyramids and stacks | propagation, convergence, manifold stability, sleeping | circle, rectangle, alternating; short and tall | correctness-qualified solver stress |
| Sleeping city | total capacity versus active work | 1%, 10%, and 100% awake | active compaction and sleep policy |
| Multiscale world | grid duplication, SAP overlap, large-static handling | one/few giant shapes plus many small bodies | broadphase-selection evidence |
| High-speed crossing | coherence loss, speculative contacts, pair-cache churn | opposing streams and randomized bursts | transient broadphase/persistence stress |
| Adversarial overlap | quadratic pairs and fixed-capacity exhaustion | controlled overlap fraction through all-overlap | bounded failure only |

Circles are deliberately included because they expose the largest plausible
body counts, but primary rigid-engine claims also require rectangles and a
mixed-shape scene. Rectangle manifolds may produce two points and exercise
orientation, clipping, angular response, and persistence; a large circle-only
result must not be presented as proof of equivalent rectangle capacity.

The standard count sweep is:

```text
32, 256, 1K, 3K, 10K, 30K, 100K
```

Free-flight and mostly-sleeping families may extend to 300K and beyond. A
family may stop earlier only when a declared capacity or time limit is reached;
the result records that boundary rather than omitting the failing scale.

Each applicable family varies at least:

- awake fraction: near-zero, 10%, and 100%;
- contact density: zero, sparse, moderate, packed, and adversarial;
- shape mix: circles, rectangles, and 50/50 mixed;
- size distribution: uniform and deliberately skewed;
- motion coherence: settled/coherent and high-churn; and
- solver quality: the accepted product setting plus explicit convergence
  sweeps in laboratory runs.

## Four benchmark scopes

Do not infer which stage won or regressed from full-frame FPS alone. Maintain
four scopes using the same data contracts:

1. **Solver-only:** authored/precomputed contacts; excludes broadphase and
   narrowphase and is valid from P5.0a.
2. **Collision-only:** body AABBs through canonical pairs and manifolds;
   excludes constraint solving and is required for the P5.3 bakeoff.
3. **End-to-end physics:** commands through final poses and sleep state;
   excludes unrelated rendering work.
4. **Full frame:** shared physics plus direct rendering and ordinary shared UI;
   proves that a compute-only gain survives in the product frame.

The solver laboratory must measure convergence against GPU cost at 1K, 10K,
and at least one 30K authored-contact workload before selecting a solver. It is
not acceptable to choose a formulation only from tiny correctness fixtures and
discover later that dispatches, coloring, gather bandwidth, or scratch growth
prevent scale.

## Throughput versus interactive pacing

The [simulation time and overload contract](simulation-time-and-overload.md)
allows an overloaded interactive world to run in slow motion, but that product
behavior is not a performance exemption.

Solver-only, collision-only, and end-to-end throughput benchmarks request an
explicit number of fixed steps independent of wall pacing and report time per
step. Full-frame interactive benchmarks use the zero-or-one-step slow clock and
add requested/effective time scale plus current and cumulative slipped wall
time. A smooth 30 FPS presentation that completes 30 outer 60 Hz steps per wall
second is approximately 0.5x simulation and must not be labeled sustained
60 Hz physics.

Every artifact names its pacing mode. Multi-step batches are permitted only
when an explicit validation or throughput request authored those steps; they
are never presented as interactive catch-up behavior.

## CPU comparison lanes

Box2D has two separate roles and therefore two separate configurations:

### Deterministic authority lane

- unmodified pinned Box2D v3 source;
- frozen single-worker/unthreaded configuration and world settings;
- complete provenance and richer state/contact capture; and
- correctness authority for accepted rigid behavior.

This lane is optimized for reproducible diagnosis. It is not the only CPU
number shown in a performance comparison.

### Optimized performance lane

- the same pinned Box2D source in a release build;
- SIMD enabled for the host;
- named worker counts, including one worker and a sensible multi-worker sweep;
- minimum result instrumentation and no debug draw; and
- the same shared scenario projection and fixed-step settings.

This is the honest CPU competitor. Comparing only against the deterministic
single-worker oracle would overstate the GPU advantage. Box2D's own benchmark
scenes may be retained as imported comparison cases, but PhysBox-owned scene
families remain the primary cross-engine corpus.

P5.0a implementation note, 2026-07-14: the optimized process uses the exact
clean authority source pin in a distinct Release build with SIMD enabled and
validation disabled. Named one- and four-worker lanes consume `ScenarioV1`
directly and use a fixed worker pool with stable worker indices. Warmup is
excluded, each sample starts from a fresh projected world outside the timed
window, and the artifact retains every wall-clock sample, Box2D contact/task
counters, memory bytes, final body hash, settings, and build provenance. The
scope is explicitly `end-to-end-physics-world-step`; it is not mislabeled as
solver-only time.

Rapier2D may later provide a non-gating optimized dissent lane if a measured
question justifies its cost. It is not required for the initial performance
contract and never replaces Box2D as the rigid authority or principal CPU
competitor.

## Fair comparison and equivalent quality

The CPU and GPU runs receive the same authored bodies, shape dimensions,
materials, gravity, fixed step, initial velocities, neutral actions, seed, and
measurement window. An adapter rejects an unsupported case instead of silently
approximating it.

Equal iteration counts are not automatically equal quality. Compare engines at
the declared product configuration and record penetration, stack-height error,
jitter, energy, sleep behavior, and scenario-specific structural outcomes.
Performance claims require both runs to remain inside their accepted
behavioral bands. If an additional GPU substep is required to match behavior,
its cost belongs in the comparison.

Every scalable scene has two related forms:

- an **oracle form**, usually 32 through 1K bodies, with detailed body/contact
  comparison and synchronized failure playback; and
- a **stress form**, extending through the standard count sweep, with
  structural and aggregate invariants plus sampled probes where full trace
  capture would perturb the measurement.

The stress form is not exempt from correctness. It may use less intrusive
evidence, but finite-state, capacity, no-missing-pair, energy, penetration,
settling, and scenario-completion checks still apply as relevant.

## Timing and measurement protocol

Record these clocks separately:

- GPU step and per-pass time from timestamp queries where available;
- CPU command construction, encoding, submission, and diagnostic-processing
  time;
- CPU Box2D wall-clock step time;
- host-visible end-to-end physics latency for bounded benchmark runs; and
- full shared-frame wall time and achieved cadence.

WebGPU's `timestamp-query` feature is optional. Request it when supported and
record the feature state. When unavailable, retain end-to-end measurements and
mark per-pass GPU time unavailable; never label CPU submission time as GPU
execution time. See the [WebGPU specification](https://gpuweb.github.io/gpuweb/#timestamp-query).
Browser implementations may also quantize timestamp queries (Chromium coarsens
them to roughly 100 µs). Record the known or suspected quantization granularity
with the result and do not interpret per-pass differences below that
granularity as real.

The initial protocol is:

1. Build release artifacts and record compiler/profile/features.
2. Create pipelines, allocate declared capacities, and run an explicit warmup
   that excludes shader compilation and first-use allocation from steady-state
   results.
3. Run a fixed-duration or fixed-step measurement window long enough to report
   P50, P95, and P99 without mixing setup or teardown.
4. Repeat runs and retain all samples; report the selected aggregation method
   rather than silently choosing the fastest result.
5. Interleave baseline and candidate runs on dedicated regression hardware
   where practical to reduce thermal and system-load bias.
6. Perform no per-step state mapping or blocking query resolution. Timestamp
   and compact counter results arrive through bounded delayed readback or are
   flushed only after the final benchmark step.
7. Record adapter, backend, device/driver, OS, browser/version where relevant,
   power mode, revision/dirty state, scenario hash, capacity contract, pacing
   mode, and all physics settings. Interactive results additionally record
   effective scale and slipped wall time.

Every result contains P50/P95/P99 total physics time, applicable per-pass GPU
time, CPU encode/submit time, full-frame time, dispatch count, the complete
work vector, and memory/capacity high-water. Average FPS alone is insufficient.

P5.0a implementation note, 2026-07-14: the authored-contact GPU benchmark mode
uses the exact Soft-Step operation schedule while disabling full trace writes,
resets both solver body buffers before (and outside) every timed step, and
performs pipeline warmup before sampling. It records CPU encode/submit and
host-completion samples independently. When `timestamp-query` is supported it
is explicitly requested, pass-boundary queries are resolved only after the
final sample, and the timestamp period is retained; otherwise GPU time remains
explicitly unavailable.

The same measurement path now covers engine-owned portable WGSL primitives:
a hierarchical exclusive prefix scan, stable four-bit-pass radix sort, ordered
unique, and predicate compaction. Each primitive validates its complete output
with one final readback after timing, exposes dispatch and allocated/high-water
capacity records, and has correctness coverage through 30K elements.

P5.1 S2.5 implementation checkpoint, 2026-07-15: the shared body benchmark
constructs the deterministic engine-owned free-flight scenario at 1K, 10K,
and 100K rectangles, then measures exactly one integration/hold/render step
per sample after pipeline warmup. Authored-state reset completes outside every
timed window; timestamp queries, when usable, span the first integration pass
through the final render pass and resolve only after all samples. CPU
encode/submit and host completion remain separate raw series. Complete pose,
motion, and generational metadata are mapped once after timing and compared
with the scalar one-step result at the existing `1e-5` bound.

The benchmark calls the same scenario generator, `GpuBodyWorld`, WGSL passes,
layout, capacities, and renderer used by the shared product host. Its only CPU
copy of authored body state is benchmark-owned reset/expected data; it is not
a live product mirror or a second mechanics path. Native and Android wait with
`Device::poll`, while the browser submits and maps a four-byte completion fence
because the wgpu WebGPU backend does not implement queue completion callbacks.
That fence follows, and is excluded from, the timestamp and encode/submit
window; browser host-completion time names the fence method explicitly.

Timestamp capability and usability are separate facts. A device that
advertises queries but returns equal zero pairs for every sample publishes
`advertised-unusable-zero`, preserves those rejected samples and the reported
period, leaves GPU time absent, and retains valid host-completion timing. Mixed
zero/nonzero samples are labeled quantized; reversed pairs fail the lane.
Chromium results retain a suspected 100 microsecond security quantization.
This prevents an advertised but broken clock from becoming a zero-cost GPU
claim.

P5.1 S2.5 clean evidence, 2026-07-15: the
[body scale package](../../evidence/p5.1-s2.5/README.md) retains 20 raw release
samples at 1K, 10K, and 100K bodies on native Metal, Chrome WebGPU,
`jstorrent-tablet` SwiftShader/Vulkan, and physical Pixel 7a Mali-G710/Vulkan.
The provisional 64-byte grouped SoA uses 6.4 MB for 100K bodies and every row
reports exact high-water with zero overflow. Native Metal GPU P50/P95 is
0.926/1.556 ms at 100K, while Chrome GPU P50/P95 is 1.503/1.615 ms. The Pixel's
unusable all-zero timestamp clock is excluded; bounded host completion at 100K
is 11.239/13.297 ms P50/P95. SwiftShader's 1452.551 ms 100K P50 is named
software portability evidence, not Android hardware performance.

Every accepted row validates complete final state after timing, with maximum
absolute error no larger than `1.1920929e-7` against the unchanged `1e-5`
scalar gate. The result exposes no current body-memory or native/browser
bandwidth limit that makes a fifth identity/material buffer credible before
P5.2. Human review subsequently accepted the limited 64-byte `BodyCoreV1`,
generational handle, existing command record, and explicit capacity-failure
semantics. Numeric capacities, scratch fields, and every future
shape/contact/joint/liquid allocation remain unfrozen and must produce their
own scale evidence.

S4.3 removes the measured unconditional full-body hold scan. Held interaction
now costs one sparse one-thread point-constraint dispatch per admitted step;
an active pointer adds one 12-thread GPU presentation dispatch and one
12-vertex tether draw. A begin still performs one deterministic serial GPU
scan of current analytic bodies. Begin is infrequent and P5.2a worlds are
deliberately small, so this is accepted interaction functionality rather than
a scale result. Scalable hit selection must reuse the later broadphase/spatial
query organization and remains a P5.3-era measurement; the serial begin scan
may not support a large-world picking claim. Body-benchmark work records are
version 2: the compatibility `holdWorkgroups` field is retained at zero and
`grabConstraintWorkgroups` reports the new sparse dispatch explicitly.

Android measurement uses a shell-executed native benchmark binary built with
the maintained NDK toolchain, rather than estimating compute cost from APK
startup or frame cadence. It calls the same solver and primitive benchmark
APIs, runs through the target's Vulkan adapter, and records raw GPU timestamp,
CPU encode/submit, host-completion, dispatch, and capacity samples. The
`jstorrent-tablet` AVD is the repeatable lane; a connected physical device uses
the identical binary and protocol. A locked or absent device remains a named
gap rather than being replaced silently by emulator data.

AVD tooling hardening, 2026-07-15: every maintained `jstorrent-tablet` launcher
now requests emulator `-gpu host` explicitly and requires the engine-reported
adapter to be non-CPU and not a known software rasterizer before publishing a
success sentinel or benchmark artifact. On the Apple Silicon development host
this selects the Apple M4 Pro through MoltenVK. The retained S1.5 and S2.5
SwiftShader rows predate this guard and remain immutable, correctly labeled
software-portability evidence; future maintained AVD evidence is a host-GPU
portability/regression lane and is not a proxy for physical Android speed.
The portable primitive pipelines explicitly skip wgpu's redundant generated
workgroup-memory zeroing because each shader initializes its complete
workgroup arrays before use and emulator MoltenVK rejects the generated
`gl_WorkGroupSize` translation. Complete primitive output validation remains
the correctness gate for that portability workaround.

The clean S1.5 raw matrix at `evidence/p5.0a-s1.5/matrix.json` records revision
`db84d635bc87c1f9d3c06f8ef58a86f2409596c0`, five warmup steps, 30 retained
samples, and 64 deterministic randomized seeds. It contains both candidates at
every authored-contact count from 1 through 32 plus 1K, 10K, and 30K; all four
portable primitives at 1K, 10K, and 30K; and optimized Box2D full-world rows at
32, 1K, 10K, and 30K with one and four workers. Every accepted row has finite
timing samples, complete capacity records, and zero overflow. The companion
AVD artifact measures both candidates at 1K contacts and all primitives at 10K
elements on Vulkan SwiftShader. The connected Pixel 7a was locked, so both its
benchmark and app validation runners refused execution and the matrix records
that physical-device gap explicitly. These measurements supported the accepted
S1.6 Jacobi-gather selection; they do not select a production primitive or
constitute a public end-to-end performance claim.

## Result validity

A timing result is invalid and excluded from comparisons when any of the
following is true:

- a correctness, structural, or numerical gate failed;
- a candidate pair, manifold, contact, constraint, body command, or required
  diagnostic was silently dropped;
- any declared capacity overflowed or saturated;
- the interactive step required a GPU-to-CPU result before it could continue;
- CPU and GPU lanes used different physical quality, scene semantics, fixed
  steps, or accepted solver settings;
- shader/pipeline compilation, allocation, scene construction, or teardown was
  mixed into a steady-state number without being labeled separately;
- the benchmark did not reach its declared steady-state or transient window;
- timestamp capability or the clock being reported is ambiguous;
- the pacing mode is missing or slow-time behavior is presented as real-time
  throughput;
- provenance, source scenario hash, or exact adapter identity is incomplete;
  or
- a result was copied from an earlier run and presented as current evidence.

Invalid runs remain useful failure artifacts. They never become a speed claim.

## Capacity and memory contract

Capacity is part of behavior. Track allocated and used bytes separately for at
least bodies/shapes, commands, AABB/cell records, candidate and unique pairs,
manifolds/contact points, contact-cache state, body-contact adjacency,
solver contributions/colors/batches, scans/sorts, active-body lists, indirect
arguments, traces, and diagnostics.

Report bytes per body, candidate pair, manifold, and contact point where a
meaningful amortized value exists. Record both allocation and high-water so an
apparently fast algorithm cannot hide impractical over-allocation.

The production path may use fixed capacity, infrequent explicit growth, or a
scene restart policy, but no accepted run may silently discard work. Every
count/scan/scatter stage checks its output capacity, raises a sticky structured
failure, and prevents the run from publishing a valid performance result.
PhysX's fixed-buffer behavior is a warning and organizational reference, not a
license to accept discarded pairs or constraints.

## Architectural consequences

The performance contract requires the engine to design for scale before the
production broadphase exists:

- use compact structure-of-arrays body and shape state with measured alignment
  and bytes per element;
- keep the hot step, counters, compaction, and indirect work sizing on the GPU;
- avoid per-body CPU command loops and all simulation-dependent readback;
- make awake-body compaction and sleeping measurable rather than bolt-on
  optimizations;
- retain persistent pairs/manifolds and warm-start state without unbounded
  hashing or allocation;
- expose candidate amplification and pair churn so a cheap solver cannot hide
  a pathological broadphase;
- treat dispatch count and global barriers as solver-selection inputs,
  especially for graph coloring and repeated substeps;
- share prefix-scan, radix-sort, unique, adjacency, and compaction
  infrastructure rather than create pass-local host orchestration, and
  benchmark those primitives as first-class artifacts from P5.0a onward —
  several run more than once per step, so their cost caps every downstream
  stage;
- isolate very large static shapes from a uniform-grid cell-size decision; and
- preserve the same logical algorithm on portable and optional accelerated
  capability tiers.

The P5.3 broadphase selection is scene-corpus driven. A uniform grid may win
for similarly sized dense bodies, sweep-and-prune may win for coherent worlds
with axis separation, and LBVH may become justified by sparse or multiscale
worlds. No algorithm wins by a single favorable scene, and all candidates must
first emit the complete canonical pair set.

Candidate bakeoff checkpoint, 2026-07-21: clean revision `2f51e15` qualifies
full-sort one-axis SAP and the multi-cell grid on Apple M4 Pro Metal and Chrome
150 Browser WebGPU through the standard 100K frontier. Both replay exactly,
match the bounded brute-force authority with zero missing pairs, reject
capacity overflow atomically, and execute through portable WebGPU usage scopes.
SAP wins every native 10K, 30K, and 100K family while allocating 82.80 MiB at
100K versus grid's 323.97 MiB under the shared evidence policy. The grid wins
some fixed-overhead small rows and one 100K browser giant-static row by 2.2%,
but does not win the complete cost. Human review therefore selects SAP and
rejects grid for product integration in
[S21](../tactical/021-scalable-broadphase.md). No end-to-end product scale,
CPU crossover, or 60 Hz physics claim follows from this isolated candidate-
stage evidence, and the product remains all-pairs until S21.5 lands.

No LBVH arm is admitted from this matrix. Reopen that question only for a
measured integrated-SAP miss or a specific sparse multiscale/active-window
workload not resolved by the current corpus.

## Milestone integration

- **P5.0a solver laboratory:** establish the benchmark schema, work vector,
  timestamp/counter path, capacity failures, and 1K/10K/30K authored-contact
  convergence-versus-cost curves; benchmark the shared scan/sort/compaction
  primitives; name the desktop and Android reference adapters and record at
  least one Android solver-laboratory measurement.
- **P5.1 body state:** measure bytes per body and free-flight integration plus
  rendering at 1K, 10K, and 100K before layouts become expensive to change.
- **P5.2 collision/persistence:** keep all-pairs deliberately small, but record
  narrowphase/manifold/cache cost per candidate and contact.
- **P5.3 broadphase:** run the complete scene-family and count corpus against
  brute force, SAP, grid, and any justified LBVH candidate; select from
  completeness, amplification, time, memory, and robustness.
- **P5.4 rigid parity:** publish the first correctness-qualified Box2D CPU/GPU
  crossover curves and full 32 through 100K performance envelope.
- **P5.6 coupled material:** extend the work vector with particles,
  particle/body contacts, density/solver work, and fluid/rigid exchange without
  changing the timing or validity rules.

Performance evidence is therefore required at every stage even though P5.4 is
the first complete rigid-engine claim.

S3.1 contract checkpoint, 2026-07-15: the collision byte surface is fixed for
measurement but remains provisional for S3.6 review: 32-byte AABB, 16-byte
candidate, 48-byte manifold, 48-byte point, 32-byte persistent key, 16-byte
cache value, 16-byte capacity counter, and 128-byte complete work counters.
Capacity artifacts distinguish attempts, atomic committed count, high-water,
overflow, and acceptance. S3.2 and later must report those quantities rather
than body count alone.

S3.2 candidate checkpoint, 2026-07-15: the complete baseline uses eight GPU
dispatches and reports 32-byte AABBs, 16-byte candidates, four-byte flags and
offsets, 16-byte pair scratch, four-byte scan blocks, and the 128-byte work
record separately. The 324-case correctness matrix had exact scalar/GPU pair
agreement and zero accepted overflow. This stage deliberately allocates
quadratic pair capacity and publishes no throughput or crossover claim; its
work and memory rows are the small-world reference against which P5.3
broadphase amplification and savings will be measured.

S3.3 narrowphase checkpoint, 2026-07-15: the bounded graph adds seven GPU
dispatches after the eight-dispatch candidate baseline and allocates 144 bytes
of narrowphase scratch per candidate, 48 bytes per committed manifold, 48
bytes per committed point, four-byte manifold flags and offsets, four-byte
scan blocks, and grouped 16-byte capacity counters. The 324-case scalar/GPU
matrix and 100-case clean Box2D authority matrix passed before measurements
were accepted. This checkpoint records work and memory shape only; it is not
an end-to-end throughput or CPU/GPU crossover claim because the product frame
does not consume generated contacts until S3.5.

S3.4 persistence checkpoint, 2026-07-15: the correctness baseline stores a
48-byte persistent entry and a 64-byte current sort record. Its transparent
eight-word, four-bit radix schedule uses 64 passes and brings the complete
candidate/narrowphase/persistence graph to 212 dispatches per outer step. The
100-case, 212-step scalar/GPU matrix had exact cache agreement, deterministic
replay and input-permutation equivalence, and zero accepted overflow. This is
a bounded work/memory measurement, not an accepted production-performance or
CPU/GPU crossover claim. S3.5 must time the complete generated-contact frame;
S3.6 must decide whether the full key should be frozen, packed, or paired with
a different lookup organization before P5.3.

S3.5 generated-contact checkpoint, 2026-07-15: the complete Metal graph uses
282 dispatches per outer step. At 32/256/1024 bodies with 16/128/512 isolated
contacts, all-pairs amplification is 31x/255x/1023x, end-to-end P50 is
2.656/7.007/23.675 ms, and allocation is 0.786/7.018/37.451 MB. The fastest
separately scoped optimized Box2D rows are 0.00717/0.0527/0.1526 ms. This GPU
baseline has no CPU crossover and is about 155x slower at 1K. P5.3 can reduce
quadratic candidate work, but not the 64-pass full-key persistence schedule or
the deterministic body-by-point incidence scan. S3.6 must review those two
costs before the representation is frozen; these measurements are not a P5.4
crossover claim.

S3.6 pinned-source checkpoint, 2026-07-15: the PhysX GPU organization audit
shows that PhysBox, PhysX, and the portable wgmath reference all use four-bit
radix digits. The S3.4 pass count is dominated by the full 256-bit semantic
key, so changing digit width is not the lead experiment. The proposed S3.5a
matrix instead isolates stable pair-slot persistence and compact body-edge
index sorting plus offsets/segmented gather, with the existing full-key sort
and body-by-point scan retained as correctness baselines. It must report
subphase dispatches, sorted key/index bytes, total allocation/high-water, and
P50/P95/P99 at generated 32/256/1K plus isolated contact-rich 1K/10K rows.
This study supplies no candidate timing and accepts no performance claim.

Human review authorized that S3.5a matrix on 2026-07-15 without selecting a
winner or accepting a performance threshold. The generated 32/256/1K rows and
isolated contact-rich 1K/10K rows must compare baseline and candidate on the
same revision and adapter, retain all raw samples, and remain invalid when
correctness, exact replay, capacity, or physical-policy gates fail.

S3.5a pair-slot implementation checkpoint, 2026-07-15: the isolated candidate
replaces the 197-dispatch full point-key persistence stage with 10 portable
dispatches, bringing candidate/narrowphase/persistence from 212 to 25. Its
96-byte pair record holds up to two complete features and caches; two cache
buffers, per-slot generations, deterministic new/free scans, and their
high-water records remain explicit rather than hidden. Native adversarial
tests pass exact replay, scalar equivalence, permutation, churn, generation,
forced-fingerprint-collision, and atomic-overflow gates. No latency or memory
winner is claimed until the candidate and baseline run through the same
generated and contact-rich timing matrix.

S3.5a body-edge implementation checkpoint, 2026-07-15: the generated solver's
candidate topology replaces the canonical per-body scan over every point slot
with a compact index build, stable body-key radix grouping, range construction,
and the same deterministic body-owned gather. Topology dispatches are 6 for
the small corpus, 9 at 32 bodies, 12 at 256/1K, and 15 at 10K, versus 4 for the
scan baseline. At the governed 32-body capacity the baseline topology is 8,320
bytes; the candidate is 32,512 bytes for two edge-index buffers, radix blocks,
and body ranges. The candidate therefore has an explicit dispatch and memory
cost before it can win by avoiding `bodies * point_capacity` scatter tests.
Permanent exact-output, scheduling-permutation, high-contention, multi-step,
benchmark-completion, and atomic-overflow gates pass. Timing and contact-rich
scale evidence, not this static work shape, will decide the lane.

S3.5a evidence-automation checkpoint, 2026-07-15: the native bakeoff runner
now emits fresh atomic provenance and raw-sample-derived P50/P95/P99 for exact
point-key versus stable pair-slot persistence, complete generated baseline
versus stable-edge graphs, and 1K/10K authored-contact baseline versus
stable-edge rows. The authored rows use the shared deterministic chain source
and never execute the quadratic all-pairs stage. Representative whole-pass
samples are kept separate from a diagnostic per-operation timestamp lane that
attributes index construction, offset construction, gather, and remaining
solve time. Exact result digests, replay, randomized permutation, allocation,
capacity, dispatch, and atomic rejection qualify each row. No timing result is
accepted until the clean same-revision evidence commit.

S3.5a clean measurement, 2026-07-15: on Apple M4 Pro Metal, stable pair-slot
persistence is exact and reduces persistence P50 from 3.859/4.801/14.319 ms to
0.0565/0.0242/0.0344 ms at 32/256/1K bodies. Comparable collision plus
persistence allocation falls from 0.522/4.962/29.279 MB to
0.275/2.955/21.263 MB. Stable body-edge views are also exact; their complete
generated-graph P50 is 4.417/5.591/16.939 ms versus
4.578/7.172/23.427 ms for the canonical scan. The more decision-relevant
authored-contact rows, which exclude all-pairs, improve from 0.489 to 0.319 ms
at 1K and 3.569 to 1.565 ms at 10K, while total allocation rises about 6% and
topology bytes approximately double. The isolated pair-slot and complete
stable-edge results are not one integrated timing and may not be added into a
CPU crossover claim. The current full-key product baseline remains
23.422 ms at 1K versus the fastest 0.1547 ms optimized Box2D row; representation
selection and any integrated-performance requirement remain at S3.6.

S3.6 decision, 2026-07-15: human review selected stable pair slots and stable
body-edge views for product integration because they are exact and win the
scoped work they replace. The approximately 6% authored-contact total
allocation overhead is accepted for this direction because it eliminates the
quadratic incidence scan, while every byte and capacity remains explicit.
Before closing S3, the combined product graph must be timed against both exact
baselines and preserve the candidate gains. This selects a representation
direction, not an accepted regression band, final stack-quality band, or CPU
crossover claim.

Combined-evidence automation checkpoint, 2026-07-15: the authority runner now
measures the actual selected pair-slot-plus-stable-edge graph at 32/256/1K,
compares each result to the full-key/canonical oracle, and re-runs exact
canonical-versus-selected authored-contact rows at 1K/10K. Raw timing samples,
dispatches, allocation, topology identity, capacity, replay, and result digests
are emitted together. Representation acceptance is distinct from solver
quality acceptance so known minimized policy or settling failures remain
blocking inputs to their own gate rather than contaminating or disappearing
inside a performance claim.

S3.6 combined clean measurement, 2026-07-15: on Apple M4 Pro Metal, the actual
selected pair-slot-plus-stable-edge product graph is exact against the retained
oracles and measures `0.383`/`0.985`/`3.159 ms` GPU P50 at 32/256/1,024 bodies
with 16/128/512 generated contacts. The prior full-key/canonical product was
`2.731`/`7.163`/`23.422 ms`; the integrated 1K graph improves about 7.4x but is
still about 20.7x slower than the fastest equivalent optimized Box2D row at
`0.1524 ms`. Authored-contact stable-edge P50 is `0.297`/`2.555 ms` at 1K/10K
contacts versus `0.456`/`3.736 ms` for canonical gather, a 1.54x/1.46x gain.
The 100-body circle/rectangle bucket rows are `0.589`/`0.745 ms` complete GPU
P50 and remain explicitly quadratic all-pairs small-world measurements. Raw
30-sample distributions, including startup-tail behavior, are retained. These
results close the representation-performance gate, not the solver-quality,
broadphase-scale, liquid, or CPU-crossover gates.

S5.4 awake candidate cost, 2026-07-16: clean Apple M4 Pro Metal measurements
retain 20 complete samples for nine reversible solver executions at
32/256/1,024 bodies. The selected 4x1 graph is
`0.635`/`1.867`/`3.532 ms` P50 with 100/103/103 dispatches, versus the fastest
optimized Box2D rows at `0.0150`/`0.0785`/`0.170 ms`. The selected 1K row uses
about 21% of a 60 Hz frame but remains 20.8x slower than Box2D. The other
quality-relevant 1K rows span `3.244 ms` for 8x1 through `5.981 ms` for
combined 8x2, with 167 through 231 dispatches. All timing rows are
capacity-clean and correctness-hashed, but none is physically qualified
because every candidate loses the awake 8-body stack. These measurements do
not establish a GPU crossover and must not be extrapolated to liquid-heavy or
scalable-broadphase workloads.

S5.4a cost contract, 2026-07-16: the approved mass-split/block follow-up must
report the complete current, scalar-mass-split, manifold-block, and combined
block-mass-split graph at 1/2/4/8 iterations. Retain 32/256/1K raw samples,
dispatches, allocations, count-topology bytes, and correctness/capacity hashes.
The fixed 16.67 ms 1K desktop boundary is only a conditional-comparator
feasibility trigger, not an accepted quality target or public performance
claim. If a candidate preserves structure but exceeds it, retain the result
and trigger colored/in-place comparison rather than weakening the scene or
timing scope. No mass-split result may be extrapolated to liquid.

S5.4a complete cost, 2026-07-16: clean Apple M4 Pro Metal evidence retains 20
samples for every one of 60 GPU rows. The complete 1K point/scalar/block lanes
span `3.343` to `5.158 ms`; their allocations span `29.149` to `29.208 MiB`,
but none preserves the seven-scene corpus. Colored in-place i1/i2/i4/i8 costs
`12.202`/`14.741`/`19.308`/`28.042 ms` with
824/1,336/2,360/4,408 dispatches and `29.454` through `30.329 MiB`. I4 and i8
are the only candidates that preserve all seven structures, and both miss the
16.67 ms feasibility boundary.

The fastest equivalent optimized Box2D 1K row is `0.185 ms`, making colored
i4 104.3x slower and i8 151.4x slower on this fixture. Actual final color use
is four for the 16-stack and twelve for the 4-row pyramid, while the comparator
intentionally charges every bounded pass; a future compact/fused colored lane
must report the complete cost rather than subtracting it analytically. This is
a generated rigid-contact small-world measurement with 512 contacts at 1K. It
does not establish a crossover, qualify scalable broadphase, or predict
liquid-heavy performance.

S5.4b cost authorization, 2026-07-16: measure a scheduling-only compact
point-color control before combining manifold coloring and block math. Retain
separate GPU time for color construction, compaction, indirect empty passes,
and solve; report actual and allocated colors, occupancy, complete dispatches,
memory, and raw 32/256/1K samples. Add 32/64-stack, both 100-body bucket, and
deterministic churn diagnostics. The 16.67 ms boundary is feasibility only and
does not establish the headroom required by later liquid-heavy scenes.

S5.4b scheduling instrumentation checkpoint, 2026-07-16: the V4 colored
execution evidence now records actual color occupancy, stable compacted point
indices, all 32 indirect workgroup triples, active and empty color passes,
scheduled point invocations, and the separately capacity-accounted count,
offset, cursor, index, and indirect-argument storage. Overflow emits zero work
for every indirect color pass. Timing claims remain deferred to the complete
32/256/1K factorial; this checkpoint establishes byte-exact attribution and
measurable work accounting only.

S5.4b timing-instrumentation checkpoint, 2026-07-16: color assignment and
stable range compaction are distinct compute passes so their GPU timestamps no
longer require analytical subtraction. The qualified generated-contact
benchmark preserves its ordinary complete-step samples and adds a separate
diagnostic sample series for construction, compaction, active-color solve,
empty indirect solve, and all other solver operations. Those instrumented
subphases deliberately use per-operation pass boundaries and therefore
explain cost; only the ordinary graph establishes complete performance.

S5.4b evidence-automation checkpoint, 2026-07-16: the clean runner requires
all eight authorized controls/candidates at 32/256/1,024 bodies and retains
ordinary complete-step samples separately from instrumented solver subphases.
It also publishes actual/allocated color work for every quality and diagnostic
scene, deterministic construction/erase recoloring counts, optimized Box2D
rows, and explicit incremental-coloring/workgroup-island trigger results. The
runner cannot change the selected execution or acceptance tolerances.

S5.4b complete cost, 2026-07-16: clean Apple M4 Pro Metal evidence retains 20
ordinary and 20 diagnostic samples for all 24 GPU rows. The three structurally
qualified manifold lanes cost `17.685 ms` forward i2, `21.825 ms` forward i4,
and `21.853 ms` alternating i4 at 1K. The fastest equivalent optimized Box2D
row is `0.170 ms`, so the leading complete GPU lane remains 104.1x slower and
slightly misses the fixed 16.67 ms feasibility boundary. The point compact
scheduling control regresses from fixed-grid `19.228 ms` to `25.448 ms`; stable
compaction does not remove the bounded empty-dispatch floor.

For forward i2 at 1K, diagnostic phase P50 is `8.742 ms` construction,
`7.998 ms` compaction, `0.393 ms` active color solve, and `4.080 ms` empty color
solve. Those grouped diagnostic phases explain cost but do not replace the
ordinary complete graph measurement. The construction/compaction and empty
dispatch triggers both fire, authorizing the already bounded incremental-color
and workgroup-per-island comparators. This rigid small-world result neither
accepts a product timing band nor predicts liquid-heavy crossover.

S5.4b incremental-maintenance checkpoint, 2026-07-16: the generated-contact
benchmark and output contracts now accept an explicit versioned scheduling
policy while every existing entry point defaults to full rebuild plus
per-color indirect dispatch. The incremental lane reports retained/new/
recolored/removed units and capacity-accounted persistent identities; complete
timing and diagnostic phase attribution use the same benchmark path as the
full-rebuild control. Native exactness, replay, first-step allocation, stable
retention, and atomic color overflow pass. No performance claim is made from
the implementation check: the renewed 32/256/1K factorial must measure it,
and the workgroup-per-island factor is measured separately below.

S5.4b workgroup-island checkpoint, 2026-07-16: the second scheduling factor
replaces every 32-entry per-color indirect solver-stage sequence with one
indirect workgroup per dynamic contact island. The GPU also performs one
deterministic island-construction/`(island,color)` compaction dispatch. Fixed
bodies do not merge islands. Complete storage accounting includes dynamic
parent and island maps, roots, body/manifold occupancies, three
`body_capacity * 32` bin views, compacted manifold indices, and one additional
indirect triple. Native exactness, independent-island race coverage, combined
incremental/island replay, benchmark phase attribution, and atomic overflow
pass. No speedup is claimed from dispatch reduction alone: the renewed
32/256/1K ordinary and diagnostic timing matrix must include construction,
memory, and complete-step cost on the same adapter.

S5.4b renewed scheduling cost, 2026-07-16: clean Apple M4 Pro Metal evidence
retains 20 ordinary and 20 diagnostic samples for 12 GPU rows. All four
forward manifold-block i2 scheduling policies pass the identical seven-scene
quality gate with exact replay and clean capacity. Full rebuild plus per-color
indirect is the 1K leader at `17.623 ms`, 1,337 dispatches, and 31.15 MB.
Incremental/per-color costs `30.821 ms` and 34.29 MB; full/island costs
`30.946 ms` and 31.38 MB with 98 dispatches; incremental/island costs
`45.741 ms` and 34.52 MB.

The fastest equivalent optimized Box2D 1K row is `0.164 ms`, making the
recommended laboratory lane 107.6x slower. It misses the 16.67 ms boundary by
`0.956 ms` (5.7%). The workgroup-island lane wins at 32 bodies (`1.326` versus
`5.990 ms`), is already slightly behind at 256 (`8.741` versus `8.527 ms`),
and loses materially at 1K because its serial island/bin compaction costs
`26.221 ms` diagnostic P50. Incremental maintenance correctly retains stable
colors but raises construction from `8.741` to `18.999 ms` while leaving the
other scheduled work. Neither comparator qualifies as a current production
optimization. These small-world rigid measurements do not establish a liquid
crossover and do not weaken the approximately 2x public scale-advantage goal.

Human scaling decision, 2026-07-16: accepting the full-rebuild/per-color solver
baseline does not accept its performance curve. From 256 to 1,024 bodies the
GPU leader grows from `8.527` to `17.623 ms`, about `11.84 us` per added
body-step. The best-per-count Box2D curve grows from `0.078` to `0.164 ms`,
about `0.112 us` per added body-step; holding worker count fixed changes the
exact ratio but not the conclusion. The present implementation has no credible
large-N crossover while serial construction and compaction scale this way.

Parallel scheduler construction and compaction are therefore a required named
workstream. It must report ordinary complete-step slopes, work vectors, and
same-adapter Box2D comparisons alongside diagnostic attribution. A smaller
dispatch floor or faster diagnostic pass alone cannot establish improvement.
S5.5 broadphase remains separately necessary for candidate-generation scale,
and may proceed in parallel, but neither lane satisfies the public crossover
contract without correctness-qualified end-to-end evidence.

Product-baseline checkpoint, 2026-07-16: the accepted forward manifold-block
i2/full-rebuild/per-color lane is now the selected product execution. Its
published 17.623 ms/1K row remains the controlling pre-optimization evidence,
not an accepted performance band. S5.4c compares every scheduler candidate
against this exact serial graph and must improve ordinary complete-step slope;
changing only diagnostic attribution, empty-dispatch count, or color order
cannot establish a crossover.

Scheduler-contract checkpoint, 2026-07-16: work-vector v1 and the
`physbox-scheduler-benchmark` runner now make that comparison executable. Each
row retains raw ordinary complete-step or authored-solver samples, separately
instrumented construction/compaction/solve samples, capacity-slot and live
work, exact assignment/order digests, replay and capacity gates, and adjacent
P50 slopes per added body and contact slot. The governed corpus is generated
32/256/1K plus authored 1K/10K contacts; the authored scope deliberately
removes broadphase selection from the scheduler scaling question. The first
dirty smoke is not an accepted baseline or performance claim.

Byte-exact compaction checkpoint, 2026-07-16: the first S5.4c candidate keeps
color assignment and solve order unchanged while replacing serial count,
prefix, and scatter with four bounded GPU passes. Its work vector declares two
point-slot traversals and the complete block/color prefix range; extra scratch
capacity is explicit. A dirty one-sample Metal smoke moved generated 1K
ordinary cost from `17.535` to `12.368 ms` and diagnostic compaction from
`7.994` to `0.098 ms`, with matching assignment/order digests. These numbers
are directional only. Promotion or a parallel-coloring trigger requires fresh
same-adapter multi-sample ordinary slopes and the full correctness corpus.

Clean compaction measurement, 2026-07-16: paired 21-sample Metal runs at clean
revision `42c5bff` qualify the byte-exact candidate. Generated ordinary P50 is
`5.960`, `8.234`, and `12.347 ms` at 32/256/1K versus the serial control's
`6.003`, `9.364`, and `17.575 ms`. The 256-to-1K marginal cost falls by 49.9%
from `10.691` to `5.355 us/body`. Authored 10K improves 24.6%; authored 1K
regresses 13.9%, exposing the three-dispatch small-scope floor. Fresh optimized
four-worker Box2D is `0.153 ms` at 1K and has a `0.090 us/body` 256-to-1K
slope, so the candidate remains 80.9x slower at 1K with a 59.7x worse marginal
curve. Separately instrumented serial assignment remains `9.618 ms` at 1K;
parallel compaction is `0.098 ms`. This satisfies the order-preserving
optimization gate and fires, rather than resolves, the required parallel-color
construction work. It does not establish a crossover or change the product
selection.

Parallel-coloring implementation smoke, 2026-07-16: the bounded speculative
candidate replaces the one serial assignment dispatch with 98 portable GPU
dispatches and reuses the four-pass parallel compactor. Work-vector v2 declares
all fixed winner-table and point-slot visits plus convergence counters. A dirty
one-sample Metal run reports about `9.22 ms` for generated 1K and `7.55 ms` for
authored 10K, with separately instrumented construction about `0.40` and
`0.67 ms`. Small generated 32 rises to about `8.89 ms`, exposing the fixed
dispatch floor. These values are directional implementation smoke only; clean
multi-sample ordinary slopes, the complete quality corpus, and required-host
evidence decide whether the comparator is viable. No crossover or product
selection is claimed.

Clean parallel-scheduler measurement, 2026-07-16: five warmups and 20 retained
same-adapter Metal samples qualify the bounded candidate against both controls.
Generated ordinary P50 is `6.069`, `7.138`, and `8.781 ms` at 32/256/1K versus
serial `5.787`, `9.377`, and `16.794 ms`. The fixed 98-dispatch construction
graph therefore costs 4.9% at 32 bodies but cuts generated 1K by 47.7%; the
256-to-1K marginal cost falls from `9.658` to `2.140 us/body`. Byte-exact
parallel compaction remains between them at `13.206 ms` and `6.525 us/body`.

Broadphase-independent authored solver P50 is `6.238 ms` at 1K and `7.535 ms`
at 10K contacts for parallel coloring, versus serial `6.618` and `16.181 ms`.
Parallel compaction narrowly leads the 1K authored row at `6.080 ms`, exposing
the same fixed small-scope floor, but loses at 10K with `11.550 ms`. Separately
instrumented candidate construction is `0.407 ms` at generated 1K and
`0.758 ms` at authored 10K; ordinary complete-step timing remains the
controlling result.

Fresh optimized Box2D is `0.173 ms` at 1K. The GPU leader is therefore still
50.8x slower, and its best 256-to-1K marginal curve is about 19.8x worse than
best-per-count Box2D. Parallel construction repairs the prior load-bearing
serial curve enough to fit the measured 60 Hz boundary, but it does not create
a credible CPU crossover, predict liquid-heavy behavior, or satisfy the
approximately 2x public scale-advantage contract. Promotion remains a
changed-order human decision, not a performance-band acceptance.

Scheduler acceptance checkpoint, 2026-07-17: human review accepted bounded
parallel construction as the product scheduling architecture. This promotes
the measured `8.781 ms` generated-1K and `7.535 ms` authored-10K leader only as
the next implementation baseline. It does not accept a regression threshold,
Box2D crossover, 2x scale claim, liquid projection, or representative-hardware
band. The serial schedule remains a direct control, and equivalent-quality
timing must be remeasured after any settling-policy change.

S6.1 qualification, 2026-07-17: clean product/control reports account for
1,434 dispatches and 1,573,916 bytes for selected bounded parallel scheduling,
versus 1,334 and 1,523,728 for the serial control on the shared validation
source. Native, browser, and hardware-AVD paths agree. These work-vector values
qualify integration only; the S5.4c timing and unresolved scale contract remain
controlling until equivalent-quality settling is remeasured.

Physical mobile pacing repair, 2026-07-18: the first unlocked selected-product
run on a Pixel 7a exposed a small-scene dispatch regression hidden by Metal and
the host-backed AVD. With report markers off, the two-body bootstrap presented
at `13.51 FPS` (`74.03 ms` mean, `94.83 ms` P95); markers on measured `13.31
FPS`, so serialization/logging was not the controlling cost. The shared
free-flight control on the same device held `60.07 FPS` and `16.72 ms` P95.

The former hard/direct product chose the previously measured
workgroup-per-island reserve only for the 64-body interactive capacity tier,
reducing bootstrap from 1,434 to 195 dispatches. Larger hard/direct tiers keep
the S6.1 scalable per-color schedule because the island implementation
regresses the same-Metal generated 1K P50 from `8.781` to `21.884 ms`. The
soft-coupled default uses its own authority-source schedule and reports 68
bootstrap dispatches. `android/validate-pacing.sh` is the maintained
physical regression lane: it requires a passing free-flight device control,
then independently gates marker-off bootstrap presentation and marker-derived
fixed-step pacing. The Pixel 7a result is `60.06 FPS`, `16.75 ms` presentation
P95, `59.38` steps/s, `0.990` effective scale, and `0.067 s` slipped during the
six-second all-marker observation for the former hard default. This is a
named-device small-world gate,
not a 3K-10K mobile scale claim or a heterogeneous-CI threshold.

S9 cost disposition, 2026-07-18: the
[schedule-differential review](../../evidence/p5.2b-s9.6-schedule-differential-review/README.md)
has no material survivor, so none enters the declared equivalent-quality
Metal 256/1K timing or cross-host qualification matrix. E3's 16 island-launch
reduction and E5/E6's three-launch reductions are implementation work-vector
facts only: E3 and E5 fail quality, while E6 is byte-inert and cannot rank a
settling formulation. No S9 timing, browser, AVD, or physical-Android result
is inferred for a discarded arm. The accepted scheduler rows and unresolved
CPU-crossover/scale contract remain controlling.

S12 static work-vector disposition, 2026-07-19: the authority-structure
[feasibility package](../../evidence/p5.2b-s12.5-gpu-feasibility-decision/README.md)
projects command and storage counts only; it contains no S12 GPU timing. On
the retained paired-contact 32/256/1K vectors, the exact X1-X4 policy would
visit `384/3,072/12,288` contact point-stages instead of
`640/5,120/20,480`. Source-faithful color construction, one-biased/one-relax
composition, and 24 rather than 32 large-world lanes project complete encoded
dispatches from `195/1,437/1,437` to `79/633/633`. Conservatively reusing the
selected topology allocation adds zero storage; removed 256-byte parameter
records reduce projected allocation by `29,696/205,824/205,824` bytes. These
counts do not imply faster execution: a single-invocation exact color builder,
serial overflow lane, midpoint identity projection, and scalar point-
sequential kernel require measured portable cost and watchdog evidence before
any timing, crossover, mobile, or product claim.

S13 measured exact-probe cost, 2026-07-19: the clean
[32/256/1K package](../../evidence/p5.2b-s13.5-gpu-work-time/README.md)
replaces the S12 projection for this exact unpromoted candidate. On Apple M4
Pro Metal, five warmups and 20 alternating retained samples per configuration
measure the selected hard/direct control and exact candidate on body-disjoint
repetitions of the governed two-point contact shape. Candidate GPU
complete-step median/p95 is `1.110/1.258`, `2.229/3.273`, and
`4.606/5.820 ms`; selected is `1.540/1.732`, `5.598/10.693`, and
`7.245/9.262 ms`. CPU encode/submit is reported independently in the raw
manifest.

Candidate total allocation is `62,048/302,860/893,452` bytes, deltas of
`-40,460/-319,244/-509,708` from selected, while its versioned initial-
transform slice is exactly 12 bytes per body. All result and diagnostic lanes
replay, capacities stay clean, timestamps are supported, and no hard execution
constraint appears. These results do not qualify or promote the candidate:
both governed response sources fail S13.4. They establish only that performance
and bounded GPU execution are not the reason the exact route is rejected. They
are named-adapter authored-solver measurements, not a CPU crossover,
representative-hardware band, liquid projection, or public scale claim.

## Configurable physics work vectors

The 2026-07-21 product direction makes outer fixed rate, solver substeps,
family-specific iterations, and contact tuning configurable. A benchmark must
therefore bind its result to the complete applied physics configuration. Body
count and scene name are insufficient.

Every shipped preset reports:

- outer-step Hz and `dt`;
- contact-generation rate;
- substeps, internal substep `dt`, biased iterations, and relaxation passes;
- requested and effective contact Hz plus damping/static multiplier;
- work, dispatches, active contacts, allocation, capacity, and overflow;
- GPU time and CPU encode/submit per outer step;
- GPU time and CPU encode/submit per simulated second; and
- interactive presentation cadence, effective scale, and slipped wall time.

Comparisons such as 60/8 versus 120/4 must state that both have a `1/480 s`
internal interval while performing different outer contact-generation and
presentation work. A higher-quality preset may cost more, but slow-time pacing
cannot turn that cost into a sustained-rate claim. Default-soft and
hard-comparison curves must be freshly paired on the same revision and named
adapter for the S19 ordinary-default change.

S19.8 now supplies that fresh pairing in the
[Metal complete-step package](../../evidence/p5.2b-s19.8-metal-complete-step/README.md).
For hard/soft at 32/256/1K manifolds, GPU P50 is `1.005/0.772`,
`6.212/2.145`, and `6.420/3.384 ms`; GPU P95 is `1.050/0.829`,
`6.262/2.174`, and `6.512/3.478 ms`. The package records whole-host and CPU
encode/submit distributions, dispatch/allocation vectors, exact replay,
capacity state, and cost per simulated second. On the physical Pixel 7a, the
soft default presents `60.01 FPS` at `16.91 ms` P95 and advances `55.77`
steps/s with effective scale `0.930`; `0.433 s` slips during the six-second
sample instead of being caught up. This replaces the former-default mobile
number for current small-world gating, not for a 3K-10K scale claim.

## Regression and claim gates

Before representative hardware is selected, store named measurements and
compare candidate versus base revision in the same environment. Do not invent
a universal regression percentage from heterogeneous CI machines. Establish
three eventual lanes:

- a short smoke matrix for changes that touch physics layouts, shaders,
  passes, capacities, or scenario generation;
- a scheduled full count/scene sweep on stable named hardware; and
- release evidence across the representative native, browser, integrated,
  discrete, and Android adapters.

Once sufficient measurements exist, freeze per-scene noise bands and relative
regression thresholds. An agent may repair an unexplained regression; it may
not refresh the baseline, weaken solver quality, remove a hard scene, change
capacity, or widen the noise band simply to make the result pass.

The provisional uniqueness objective is stronger than "10K at 60 Hz." Before
making a public scale-advantage claim, require correctness-qualified evidence
on at least two nontrivial contact-rich scene families showing either a clear
throughput advantage over optimized Box2D (approximately 2x is the initial
research target) or a materially larger accepted body count inside the same
physics budget. Also report the crossover point, sparse/free-flight results,
CPU time consumed, and any scene where Box2D still wins. This objective may be
revised from measurements, but not silently weakened after an unfavorable run.

## Open decisions and reconsideration triggers

The first implementation work must resolve:

- the named integrated, discrete, browser, and Android reference adapters;
  the Android device is named during P5.0a and measured before the solver
  decision rather than deferred to P5.4;
- the product physics-time envelopes for the selected 60 Hz default and each
  shipped quality/rate preset;
- warmup length, measurement-window length, repetition count, and noise model;
- the first optimized Box2D worker-count policy for cross-engine reports;
- compact asynchronous timestamp/counter transport on hosts without identical
  query capabilities;
- storage format and retention policy for machine-specific performance
  artifacts; and
- the measured crossover and saturation points for each solver and broadphase
  candidate.

Reconsider the benchmark matrix or planning targets when a new product scene
changes the work distribution, an accepted feature adds a materially different
constraint class, representative adapters expose a missing capability, or
measurements show that a count tier no longer distinguishes architectures.
Record the evidence and date in this topic. Do not redefine a workload merely
because its current result is inconvenient.
