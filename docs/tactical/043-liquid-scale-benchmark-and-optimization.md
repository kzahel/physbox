# S43 — Liquid scale benchmark and optimization

Status: active, 2026-08-10. Human direction authorizes continuous execution,
proportional validation, and coherent commits through the final evidence and
disposition. The work starts from clean `main` at
`6696c1a97c19e8ff53ea48496a4d3d096aaaf1e9`.

Governing performance contract:
[performance and scale](../topics/performance-and-scale.md).
Current diagnosis:
[liquid dispatch and capacity performance](../topics/liquid-dispatch-and-capacity-performance.md).
Coupled mechanics:
[coupled liquid/rigid constraint surface](../topics/coupled-liquid-rigid-constraint-surface.md).
Pacing:
[simulation time and overload](../topics/simulation-time-and-overload.md).
Reusable consumer boundary:
[web showcase and public water consumption](../topics/web-showcase-and-public-water-consumption.md).

## Goal

Create the first correctness-qualified, cross-host liquid throughput surface
for PhysBox and use it to reduce the dominant structural costs without
weakening the selected mechanics. Measure raw water, a useful 20-dynamic-solid
demo, and a 100-dynamic-solid stress demo over meaningful particle counts on
the same shared Rust/WGPU/WGSL implementation. Run the benchmark through
native desktop, Browser WebGPU, and standalone Android-native Vulkan, using a
physical Pixel 7a for the paired Android/Browser headline whenever available.

The target hierarchy is:

- 20 dynamic solids plus 8K active water: ordinary demo gate;
- 20 dynamic solids plus 16K active water: strong demo target;
- 100 dynamic solids plus 8K active water: headline target;
- 100 dynamic solids plus 16K active water: stretch target; and
- 32K water: bounded capacity/frontier evidence unless a complete qualified
  row actually sustains its declared rate.

A sustained 60 Hz interactive result requires at least 30 seconds of measured
operation, effective simulation scale at or above `0.95`, zero capacity or
correctness rejection, and complete timing/work provenance. Physics P95 at or
below 12 ms is the initial headroom objective, not a result that may be forced
by changing quality.

## Exclusions

- Do not change the 60 Hz outer default, two 120 Hz particle substeps, pressure
  iterations, coupling strength, sleeping-on/exact default, fluid-solid
  quality default, or no-catch-up pacing merely to improve a result.
- Do not introduce a CPU particle or rigid product path, simulation-dependent
  readback, native-only mechanics, browser-specific quality, or automatic
  backend fallback.
- Do not include the Box2D shadow in any timing row. Benchmark builds compile
  it out or prove it is absent at runtime.
- Do not call allocation, construction, a first frame, a stalled frame, or a
  slow-time presentation result sustained throughput.
- Do not relabel the public showcase's rejected 32K authored preset as passing
  without a fresh bounded result under this contract.
- Do not expose private solver layouts or benchmark policy through the public
  `physbox-sim` consumer API.
- Do not add smooth water, materials, tools, scene persistence, interpolation,
  or unrelated presentation work.
- Do not publish a performance claim or deploy a changed showcase without a
  separate explicit request.

## Ownership constraints

- Shared Rust owns parameterized benchmark definitions, scenario hashes,
  correctness/work records, timing schemas, and result validation.
- `physbox-sim` owns reusable physics execution and may expose only neutral
  benchmark plumbing needed by first-party tools; the stable consumer surface
  remains render- and benchmark-policy-neutral.
- Native, Browser, and Android tools transport parameters and publish raw
  artifacts. They may not recreate scenarios, choose quality, or interpret
  pass semantics independently.
- Physics-only timing records explicit requested steps through the same world
  graph without rendering. Full-frame timing uses the ordinary shared product
  or public-consumer frame path and reports effective scale and slip.
- GPU timestamp queries are optional capability. Unsupported or unusable
  clocks leave GPU timing absent while preserving host-completion evidence.
- No per-step mapping, wait, or compact diagnostic may be required to encode
  the next step. Bounded validation readback occurs only after timed work.

## Benchmark contract

Every scenario reports dynamic and fixed solids separately. The 20-solid lane
uses 20 dynamic circle/rectangle bodies plus fixed container boundaries and
uses the product minimum 64-slot rigid capacity. The 100-solid lane uses 100
dynamic bodies plus the same boundary semantics and should naturally fit a
128-slot capacity. This deliberately brackets the current 64-slot rigid
scheduling boundary.

The count sweep is `1K`, `4K`, `8K`, `16K`, and `32K` active water. Raw-water
rows keep density and expected neighbor count approximately constant while
world area grows, rather than packing more particles into the same volume.
Coupled demo rows use the same material, shape mix, authored density, and
mechanically meaningful fluid/solid interaction at each tier. Each applicable
lane also carries a rigid-only control and a fixed-reserve live-particle sweep
so active work can be separated from allocation capacity.

Every accepted artifact records at least:

- source revision/dirty state, scenario ID/hash, build profile/features, host,
  adapter/backend/driver, browser version where applicable, and timestamp
  capability/usability;
- outer rate/delta, solver substeps/iterations, fluid-solid quality, sleeping
  configuration, render extent/scale, warmup, samples, and percentile method;
- dynamic/fixed/live/awake body counts, body capacity, candidates, manifolds,
  points, particle live/capacity counts, grid insertions/occupancy, neighbor
  checks/acceptances, body contacts/contributions, and wake roots;
- rigid, liquid, physics-total, render, full-frame, CPU encode/submit, and host
  completion P50/P95/P99 where available;
- dispatches, workgroups or equivalent work counters, allocated/high-water
  bytes, every overflow, final finite/capacity state, completed steps,
  effective simulation scale, and slipped wall time; and
- an explicit distinction between raw explicit-step throughput and interactive
  sustained-rate evidence.

## Ordered delivery

### S43.1 — shared benchmark definitions and schema

- [x] Add versioned parameterized raw-water, 20-solid, and 100-solid scenario
      definitions owned by shared Rust.
- [x] Generate the supported particle tiers with approximately constant
      authored density and exact requested active counts.
- [x] Define versioned configuration, work-vector, timing-series, percentile,
      capacity, correctness, provenance, and artifact contracts.
- [x] Add canonical hashes, validation negatives, deterministic construction,
      and tests proving the 64-slot/128-slot capacity bracket.
- [x] Keep benchmark vocabulary out of browser glue and application-owned
      showcase scene definitions.

Gate: every host can consume one shared definition; malformed counts, capacity
or quality changes, incomplete provenance, non-finite timing, overflow, and
incorrect final particle populations reject the row.

### S43.2 — physics-only native benchmark

- [x] Add a comparator-free release benchmark API around the actual
      `GpuBodyWorld`/`World` graph with explicit warmup and retained samples.
- [x] Time physics only with one outer step per sample, pipeline creation and
      reset outside retained windows, and no rendering.
- [x] Record total GPU timestamps when supported plus grouped particle-stage
      timestamps for rigid, integration/boundary, grid, density/correction,
      body reaction, velocity, and compaction work.
- [x] Record CPU encode/submit and host completion independently.
- [x] Map and validate final state once after timing; reject stale, incomplete,
      comparator-enabled, overflowed, or mechanically invalid rows.
- [x] Provide an atomic JSON writer and a bounded CLI for selected/full
      matrices.

Gate: a clean native release baseline publishes raw retained samples and a
complete valid work vector for raw water and both solid lanes.

### S43.3 — Browser WebGPU and Android-native runners

- [x] Expose the same benchmark API to a separately built, smoke-gated Browser
      observer without adding semantic policy to ordinary product JavaScript.
- [x] Build a Browser runner that performs fresh-world trials, retains raw
      samples, records timestamp quantization, and writes the shared artifact.
- [x] Add a standalone Android ARM64 native benchmark binary using the same
      Vulkan adapter path and JSON schema as native desktop.
- [x] Add device and AVD transport scripts with adapter guards, atomic pull,
      clear success sentinels, and mandatory AVD shutdown.
- [ ] Prove native desktop, Browser WebGPU, and Android runners select the same
      scenario hash, quality, work contract, and comparator-off graph.

Implementation note, 2026-08-10: native desktop and Browser WebGPU completed
the same dirty-tree `20 solids + 4K` smoke with identical source-definition,
configured-scenario, configuration, capacity, and dispatch contracts. Chrome
151 on macOS advertised timestamps but returned backward frame markers; the
artifact rejects that clock as `advertised-unusable-order` and retains host
completion samples. The ARM64 Android runner compiles and its transport was
exercised on the available `jstorrent-tablet` AVD. The emulator lost its
Vulkan device during first pipeline creation, so no Android artifact was
accepted; teardown emitted `PHYSBOX5_ANDROID_AVD_SHUTDOWN_SUCCESS`. No physical
Android device was attached. The cross-host proof item and S43.3 gate remain
open until an Android artifact exists.

Gate: one selected 4K row passes on native desktop, Browser WebGPU, Android
ARM64 build, and a physical Android device when available; a hardware-backed
AVD is portability evidence only.

### S43.4 — clean baseline matrix and first disposition

- [ ] Capture raw-water `1K/4K/8K/16K/32K` physics-only rows.
- [ ] Capture coupled `20/100 × 4K/8K/16K` physics-only rows plus rigid-only
      controls; add 32K only while the bounded runner remains responsive.
- [ ] Capture full-frame 20/100-solid 8K rows and the 16K targets on native and
      Browser WebGPU.
- [ ] Run paired Android-native and Browser WebGPU rows on the same physical
      Pixel 7a when available.
- [ ] Publish a stage-dominance and scaling report without subtracting
      unrelated controls or treating mean-only measurements as percentiles.
- [ ] Confirm, revise, or reject the preliminary structural optimization order
      from the measured work and time.

Gate: the baseline explains the 20-to-100 and 64-to-128 transitions with raw
samples; no optimization starts from an unmeasured attribution assumption.

### S43.5 — live-work rigid schedule routing

- [ ] Qualify workgroup-per-island execution through the bounded 128-slot,
      100-dynamic-body benchmark topology without changing solver equations or
      color/order semantics.
- [ ] Route from GPU-visible live/topology work rather than reserved capacity,
      with exactly one selected route and a fail-closed scalable fallback.
- [ ] Prove runtime growth/churn can cross the route bound without CPU
      observation, duplicate solving, dropped work, or watchdog exposure.
- [ ] Compare native, Browser, and physical Android 20/100 rigid-only and 8K
      coupled rows on the same revision.

Gate: the selected route is mechanically equivalent, capacity-clean, portable,
and materially reduces the diagnosed 128-slot mobile rigid cost; otherwise it
is rejected with evidence and the general schedule remains selected.

### S43.6 — particle pass and live-work execution

- [ ] Measure encoding consecutive particle dispatches in fewer WebGPU compute
      passes while retaining required visibility and identical final state.
- [ ] Move safe per-particle kernels from capacity-sized direct dispatch to the
      GPU compact live list and indirect work count.
- [ ] Make zero-live and fixed-reserve `1/1K/4K/8K` rows expose bounded
      meaningful work without CPU particle counts.
- [ ] Retain final-only validation, command/reset behavior, kill/escape,
      rendering observations, and capacity failure semantics.

Gate: a retained candidate lowers CPU encode or GPU/host cost on Browser or
physical Android without changing mechanics; neutral or regressing candidates
are reverted or left unselected with their evidence.

### S43.7 — particle/body spatial candidates

- [ ] Add a compact GPU-resident active body set for particle coupling.
- [ ] Build complete particle/body candidates from conservative body AABBs and
      the existing or a measured replacement spatial structure.
- [ ] Replace full reserved-body loops in boundary projection, boundary
      density, pressure, and surface-relative velocity with nearby candidates.
- [ ] Prove zero missed analytic contacts against the current full-scan
      authority over randomized, adversarial, phase-shifted, 20-solid, and
      100-solid sources.
- [ ] Record candidate amplification, cell occupancy, memory, build/query
      dispatches, and overflow; giant or multiscale bodies fail closed rather
      than disappearing.

Gate: the candidate preserves every accepted contact and coupled result while
materially reducing the 100-solid body-test work and complete-step cost on a
required host.

### S43.8 — body-contribution accumulation

- [ ] Compare the current floating-point tiled gather with at least one
      compact keyed reduction or bounded fixed-point atomic accumulator.
- [ ] For fixed point, document scale, worst-case translation/torque range,
      quantization, contention, generation validation, overflow detection,
      and wake behavior before performance ranking.
- [ ] Preserve same-step equal-and-opposite position/velocity/torque response
      and current-body feedback before later particle iterations.
- [ ] Run scalar differentials, permanent wet-overlap corpus, PhysBox 3 review
      metrics, and 20/100-solid timing rows.

Gate: an accumulator is selected only when capacity/range cannot silently
overflow, numerical/behavior bands pass, and complete-step cost materially
improves on Browser or physical Android.

### S43.9 — conservative grid-rebuild reuse and bounded fusion

- [ ] Attribute the current six compact grid rebuilds and 24 rebuild
      dispatches independently after earlier optimizations.
- [ ] Compare bounded reuse per pressure iteration or particle substep with a
      conservative query halo derived from maximum possible displacement.
- [ ] Compare only mathematically safe kernel fusion where no cross-particle or
      particle/body visibility boundary is removed.
- [ ] Prove neighbor completeness, density/correction bands, escape, coupling,
      wake, and wet-overlap behavior across all phases and target tiers.

Gate: fewer rebuilds or fused kernels are selected only if quality and
completeness remain accepted and end-to-end cost improves. Solver substeps or
pressure iterations remain unchanged unless separately authorized after this
structural program.

### S43.10 — target qualification and closure

- [ ] Re-run the complete accepted native matrix with raw samples and clean
      provenance.
- [ ] Re-run selected Browser WebGPU 20/100-solid 8K/16K rows and interactive
      30-second pacing gates.
- [ ] Run `npm run test:fast`, production warnings-denied clippy, focused
      `physbox-sim`/benchmark tests, and one fresh headless coupled artifact.
- [ ] Run the ARM64 Android check, release APK where shared product code
      changed, standalone native benchmark, and physical Pixel native/browser
      matrix; use and shut down an AVD only for an otherwise unavailable
      portability question.
- [ ] Update the governing topics, capability ledger, README where warranted,
      tactical index, and this document with measured results and retained
      limitations.
- [ ] Record each target as passed, missed, or unmeasured. Do not weaken a
      target because an optimization underperforms.

Gate: every selected optimization has a same-revision control, required host
evidence, complete work/capacity records, and no unexplained quality change.

## Commit and stop policy

Commit the approved plan before implementation. Thereafter commit each
coherent contract, runner, evidence, optimization, qualification, and
documentation slice after its focused gates pass. Keep the worktree clean
between slices and preserve unrelated user changes if any appear.

A hard invariant, authority, capacity, or completeness rejection stops later
expensive work for that candidate but does not stop the tactical: record the
negative disposition and proceed to the next independent structural candidate.
Unavailable physical hardware is recorded explicitly after safe device checks;
it does not permit a software or AVD result to become a physical speed claim.

## Completion condition

S43 is complete when the shared 0/20/100-solid benchmark exists on native,
Browser WebGPU, and Android-native paths; clean baseline and final matrices
record raw samples and complete work/capacity/provenance; every admitted
structural optimization has a measured accepted or rejected disposition; all
selected mechanics and pacing gates pass; the 20/100-solid target hierarchy is
reported honestly; required maintained-host gates pass; and the repository is
left clean with every S43 change committed.
