# Performance and scale contract

Status: active contract with provisional targets, 2026-07-14.

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
P5.2. It does expose the unconditional full-body grab-hold scan as a future
sparsification candidate. These observations propose, but do not freeze, the
layout and command architecture; S2.6 remains a human checkpoint.

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
- the product physics-time envelope within a 60 Hz frame;
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
