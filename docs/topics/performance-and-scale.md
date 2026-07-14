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
