# S26 exact contact-component observation

Status: complete at the required human checkpoint, 2026-07-22. Stopping point
1 from
[GPU sleep-island connectivity research](../topics/gpu-sleep-island-connectivity.md)
passes exactness, replay, capacity, and cost observation. The implementation is
laboratory-only and has no wake, admission, solve, render, or product-default
consumer. Work is stopped before component reductions or a behavior switch.

## Goal

Measure whether exact connected components of the current dynamic-body contact
graph can be rebuilt correctly, deterministically, and cheaply enough to
justify a later sleep-island decision. The stopping result is evidence, not a
product candidate.

## Exclusions and ownership

- Keep S23's K=4 wake and admission behavior byte-for-byte authoritative.
- Do not reduce wake causes or readiness by component, add an exact-island
  behavior switch, change solver/default configuration, or resume S23.7.
- Reuse the engine-owned generated-contact projection. The probe may observe
  one edge per committed dynamic-dynamic manifold, but it may not create a
  second scene, contact generator, or platform implementation.
- Product graphs allocate and dispatch no component-probe resources. Only an
  explicit validation/laboratory construction enables the probe.
- CPU traversal is test/evidence tooling only and may read back after a bounded
  run; no interactive frame depends on GPU results returning to the CPU.

## Frozen observation contract

- Vertices are current live non-fixed body slots.
- An edge is one current committed dynamic-dynamic manifold. Scalar point 1 of
  a two-point manifold is not a second edge, and a fixed floor never connects
  independent piles.
- The exact diagnostic label is the minimum live body slot in the component.
- GPU construction uses monotonic minimum-root hooking plus pointer shortcut
  passes under a capacity-derived finite phase bound. It contains no unbounded
  in-shader spin or host convergence loop.
- An independent final GPU check reports invalid roots and any observed edge
  whose endpoint labels differ. Either count invalidates the sample; capacity
  or convergence failure may not silently produce partial labels.
- Report dynamic vertices, unique edges, component count, largest component,
  executed and last-changing phases, hook/shortcut work, dispatches, parent and
  scratch bytes, and component-stage GPU P50/P95/P99.

## Ordered delivery slices

### S26.1 — contract and scalar proof

- [x] Freeze the graph, label, exclusion, failure, and evidence contracts.
- [x] Add exhaustive small-graph and adversarial chain/star/pile-model tests
  for the fixed phase bound and deterministic minimum labels.
- [x] Record the bound beside the implementation and fail closed if capacity
  arithmetic cannot represent it.

Gate: exhaustive connected and disconnected graphs through the practical
small-graph limit match a CPU breadth/depth traversal exactly.

### S26.2 — opt-in GPU observer

- [x] Add an opt-in generated-contact stage that initializes parents, performs
  bounded hook/shortcut passes, flattens labels, accumulates component sizes,
  and validates roots/edges.
- [x] Expose compact diagnostic readback and timestamp scope without routing
  any value into sleep, solve, or render decisions.
- [x] Prove through construction tests that the ordinary product graph has no
  observer allocation or dispatch.

Gate: exact replay, zero invalid roots, and zero cross-label contact edges.

### S26.3 — CPU differential and fixtures

- [x] Compare GPU labels with an independent CPU traversal after bounded runs.
- [x] Cover isolated bodies, chains, two piles sharing a fixed floor, a dense
  pile, an arch/bridge, component split/removal, and contact churn.
- [x] Report the first divergent body or edge and fail the command on any
  mismatch, overflow, stale generation, or nondeterminism.

Gate: every admitted fixture and repeated run matches exact minimum labels.

### S26.4 — cost evidence and stopping report

- [x] Measure P50/P95/P99 observer GPU time, full physics time, dispatch count,
  allocated bytes, phase usage, and atomic-work proxies at representative
  256/1K/10K capacities and applicable pile/sleeping-city rows.
- [x] Run proportional engine, clippy, browser, headless, ARM64, release APK,
  and available Android hardware validation required by the touched boundary.
- [x] Record correctness, cost, limitations, and a recommendation, then stop
  before component reductions or a behavior switch.

Gate: evidence is provenance-complete and separates reserved capacity from
live work. A correct but expensive result remains a valid observation outcome.

## Completion condition

S26 completes when the observation-only GPU labels match the independent CPU
reference on the admitted fixtures, timing and resource costs are recorded,
ordinary product behavior remains unchanged, and the repository returns to a
human decision. Completion does not authorize stopping points 2–5 from the
research topic.

## Completed observation

Implementation revision `add83160c334dad6b8210a6adc38ffb2b6b48d21`
owns the clean release measurement. The observer is a separate laboratory
object beside `GpuGeneratedContactGraph`; creating the ordinary graph does not
allocate its parent/size/stat/parameter buffers or encode its passes. It reads
the real post-persistence solver-point projection, selects local point zero as
the unique manifold edge, checks endpoint generations, excludes fixed bodies,
and never exposes a label to a product decision.

The recurrence executes `2 * ceil(log2(body_capacity)) + 1` fixed
hook/shortcut phases. Every hook lowers a root with `atomicMin`; every shortcut
replaces a parent with its grandparent. Exhaustive scalar comparison covers all
graphs through six vertices, and adversarial ordered/reversed chains and stars
extend through 1,024 vertices. The final GPU passes independently reject a
non-root label, a stale/malformed edge, or any dynamic edge crossing two
labels. Atomic attempt/success/write counts are contention proxies and may
vary with valid thread order; labels and all structural counts replay exactly.

The durable command is:

```sh
npm run benchmark:contact-components -- \
  --output /tmp/physbox5-s26-contact-components-add8316.json \
  --warmup 3 --samples 20
```

It emits `PHYSBOX5_VALIDATION_SUCCESS`, records clean revision and adapter
provenance, and has SHA-256
`4be049c00eac8a45743437c03a2b2696cb48bcc5904840fdd5b20498d2e4b373`.
All ten rows match the independent CPU traversal in every sample with zero
invalid roots, invalid edges, or cross-label edges.

| Row | Reserve | Edges | Components / largest | Fixed / last-changing phases | Observer dispatches | Observer bytes | GPU P50 / P95 / P99 | P50 share of physics |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| isolated, common floor | 64 | 0 | 2 / 1 | 13 / 0 | 29 | 4,160 | 0.110 / 0.240 / 0.240 ms | 15.78% |
| eight-body chain | 64 | 7 | 1 / 8 | 13 / 3 | 29 | 4,160 | 0.219 / 0.246 / 0.246 ms | 15.40% |
| two piles, common floor | 64 | 2 | 2 / 2 | 13 / 1 | 29 | 4,160 | 0.215 / 0.319 / 0.319 ms | 16.82% |
| dynamic bridge | 64 | 4 | 1 / 5 | 13 / 2 | 29 | 4,160 | 0.126 / 0.220 / 0.220 ms | 11.58% |
| pair churn, contact | 64 | 1 | 1 / 2 | 13 / 1 | 29 | 4,160 | 0.203 / 1.055 / 1.055 ms | 18.03% |
| pair churn, removed | 64 | 0 | 2 / 1 | 13 / 0 | 29 | 4,160 | 0.238 / 0.830 / 0.830 ms | 14.44% |
| packed pile 100 | 256 | 342 | 1 / 100 | 17 / 4 | 37 | 6,720 | 0.299 / 0.560 / 0.560 ms | 4.72% |
| packed pile 1,000 | 1,024 | 3,554 | 1 / 1,000 | 21 / 6 | 45 | 13,888 | 0.577 / 0.609 / 0.609 ms | 3.44% |
| sleeping city 1,000 | 1,024 | 500 | 500 / 2 | 21 / 1 | 45 | 13,888 | 0.361 / 0.389 / 0.389 ms | 5.73% |
| sleeping city 200, 10K reserve | 10,000 | 100 | 100 / 2 | 29 / 1 | 61 | 87,744 | 0.489 / 0.844 / 0.844 ms | 11.04% |

Twenty samples make nearest-rank P95 and P99 select the same maximum sample.
The dense rows are deterministic initially touching packed piles, not a
time-evolved pour. That isolates graph correctness and contention from solver
settling, but a long poured-pile decision trace remains work for a later
component-reduction comparison rather than evidence claimed by S26.
The 100K broadphase-only lane is not an integrated exact-contact graph and is
therefore not presented as an applicable component row; the maintained
integrated capacity presets end at 10K.

## Disposition and gates

The observation succeeds, but a static rebuild is not yet a product decision.
Its 29–61 dispatches are 3.2–6.8 times the current nine-dispatch K=4
sleep-enabled delta before any component wake/readiness reductions are added.
Its dense 1K cost is modest at 0.577 ms and only 3.44% of that row's full
physics interval, while small/sparse rows expose a material 11–18% share. The
fixed phase schedule also executes 13–29 phases although every measured graph
last changes by phase 6. These results support one later component-reduction
observation beside K=4 if human review wants behavioral decision evidence;
they do not support skipping directly to an exact-island switch. Persistent
dirty-split complexity is still premature until reductions show that exact
cohorts materially change decisions.

Proportional validation passes: `npm run test:fast` in 23.01 seconds including
real Browser WebGPU, production and directly affected warnings-denied clippy,
the complete component unit/GPU fixture tests, solver-lab Browser WebGPU, a
fresh headless bootstrap, ARM64 solver-lab release-native build, debug APK,
and release APK. No physical device appeared in the SDK's `adb` list despite
the expected attachment, so the authorized hardware-backed
`jstorrent-tablet` AVD fallback passed bootstrap on host RADV/Vulkan and emitted
`PHYSBOX5_ANDROID_AVD_SHUTDOWN_SUCCESS`; a final device list was empty.

Additional artifact SHA-256 values:

- headless PNG: `fdbbdbbc218c6bad4e7af76c0013f8fc28c61e6e0919b11dd856889cedc99c2a`;
- headless report: `b51ce5173d31ef64c801cf544fc9c81fcb742b276b4e294b3204ce9600d80d7f`;
- AVD PNG: `c0495c16805721e07b85c7a32c58954eda92fffcbda59959e871950c2937e98e`;
  and
- AVD provenance: `5f5d1d06b4b49ffbe668d2540704f92e7064ebef799f0bf386fa2f53a172b688`.
