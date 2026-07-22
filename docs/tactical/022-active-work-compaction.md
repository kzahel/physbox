# S22: active-work compaction

Status: complete 2026-07-22. A Pixel 9 subjective run established the trigger:
the 256-slot world sustains presentation at 60 Hz with 256 awake rectangles,
while selecting the 1K reserve prevents 60 Hz with only one placed rectangle.
At revision `a436801587f3`, the 256- and 1K-slot bootstrap graphs both encode 497
solver dispatches per step, but collision, topology, solve, integration, and
render work still traverse reserve-sized buffers. Reserved capacity is
therefore incorrectly acting as an execution-size setting.

Governing topics:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md),
[performance and scale](../topics/performance-and-scale.md), and
[simulation time and overload](../topics/simulation-time-and-overload.md).
This work inserts the active-work stage required before sleeping in the
[bring-up plan](../bringup-plan.md).

## Goal

Make reserved body/contact capacity control allocation and fail-closed
overflow bounds, while GPU-resident live body, candidate, manifold, and point
counts control hot execution. With the same tiny live scene, 256, 1K, and 10K
capacity presets should have approximately equal step cost. Cost should grow
with live/awake work and contacts rather than unused slots.

The active body order is deterministic ascending slot order. Placement and
erase update it on the GPU in the same ordered construction transaction that
owns body liveness. No simulation-dependent count is read back to the CPU to
admit or size the next interactive step.

## Exclusions and ownership

- Sleeping and waking policy are not part of S22. S22 supplies the live-body
  representation that a later awake-body filter will consume.
- S22 does not change fixed `dt`, substeps, iterations, solver coefficients,
  contact identity, broadphase selection, or the soft solver default.
- Capacity remains fixed for a world lifetime; dynamic buffer growth is not
  introduced.
- No CPU physics, CPU broadphase, host-local body list, readback-driven
  dispatch, automatic backend fallback, or alternate browser/Android path is
  allowed.
- `BodyCoreV1` and persistent body identities remain unchanged. Active lists,
  counters, and indirect arguments are capacity-accounted scratch.
- Exact replay, canonical pair order, atomic overflow rejection, and the
  zero-or-one interactive step/no-catch-up contract remain gates.

## Ordered delivery slices

### S22.1 — frozen sparse-capacity probe

- [x] Add one focused benchmark/probe that runs the same small live scene at
  64, 256, 1K, and 10K reserve, records GPU collision/persistence/end-to-end
  samples when timestamps exist, host completion, dispatches, live work,
  allocation, replay, and overflow.
- [x] Record a clean pre-change native artifact and the physical Pixel 9
  256/1K observation without presenting FPS alone as a throughput claim.

Gate: the probe must expose the current reserve-dependent cost and must reject
stale, overflowed, non-replaying, or differently configured rows.

### S22.2 — deterministic live-body set

- [x] Add a capacity-accounted GPU active-slot list, live/dynamic/fixed counts,
  and indirect compute/render arguments initialized from authored bodies.
- [x] Update the set transactionally on successful place/erase and restore it
  on authored reset; failed construction intents leave it byte-unchanged.
- [x] Cover sparse authored slots, slot reuse/generation advance, fixed-body
  protection, capacity-full, deep-overlap, erase miss, reset, and exact replay.

Gate: readback validation proves sorted unique live slots agree exactly with
body metadata and construction-state counts after every lifecycle case.

### S22.3 — live-count broadphase and presentation

- [x] Build SAP proxies densely from the active-slot list, size proxy radix and
  enumeration through GPU-written indirect arguments, and remove the
  reserve-sized product proxy copy.
- [x] Integrate and render only active slots using indirect work/draw arguments.
- [x] Preserve the isolated snapshot SAP lane and canonical generational pair
  output unchanged.

Gate: product SAP equals brute force across the maintained corpus and dead-slot
reuse; sparse 256/1K/10K rows report identical live proxy/sort work.

### S22.4 — live-count narrowphase and persistence

- [x] Size narrowphase generate/scan/scatter from committed candidates without
  clearing or scanning unused candidate capacity.
- [x] Size persistence preparation/matching/retirement from committed current
  and previous pair/point lists while retaining fail-closed capacity behavior.
- [x] Publish GPU point-count indirect arguments for the solver.

Gate: zero-candidate and one-contact rows do bounded constant work across
reserve presets; contact lifecycle, feature identity, cache expiry, replay,
and overflow tests remain exact.

### S22.5 — live-count solve and integration

- [x] Dispatch body passes over active slots and contact passes over committed
  solver points.
- [x] Remove reserve-sized topology/color scans from the hot path, including
  the `body_capacity * 32` color-winner clear; retain explicit allocation
  capacities and stale-state safety for reused slots.
- [x] Keep every solver profile on the shared operation graph and preserve
  selected soft-product results.

Gate: structural/energy/penetration validation and exact replay remain within
their existing bands; sparse 256/1K/10K step timing is approximately flat.

### S22.6 — cross-host qualification and closure

- [x] Run `npm run test:fast` once for the ordinary lane.
- [x] Run production clippy with warnings denied.
- [x] Produce one fresh headless smoke artifact.
- [x] Run the ARM64 Android check, build a release APK, and validate capacity
  256/1K/10K on the unlocked physical Pixel.
- [x] Confirm the ordinary browser WebGPU result and a focused browser sparse-
  capacity probe; do not duplicate solver-menu coverage unless menu behavior
  changes.
- [x] Update the performance topic, capability ledger, bring-up plan, and this
  tactical with measured before/after evidence.

Gate: with the same one-rectangle scene, selecting 1K or 10K does not cause a
material pacing regression relative to 256 on the physical Pixel. A dense row
must still demonstrate that real live work grows and every row must identify
capacity, live bodies, contacts, pacing mode, adapter, and overflow state.

Checkpoint evidence, 2026-07-21: commit `b7effce` passes exact generated-series
replay and the everyday lane. Chrome debug bootstrap checks report effective
scale `0.973` at 1K with no slip and `0.923` at 10K with one slipped step. A
marker-heavy connected Pixel 7a comparison reports `0.591/0.604/0.550` at
256/1K/10K; it is a relative reserve-size gate, not an absolute rate claim.
The original subjective Pixel 9 observation remains the pre-change trigger.

Closure evidence, 2026-07-22: the durable command is
`npm run benchmark:sparse-capacity -- --output PATH --warmup 8 --samples 31`.
It rejects dirty source, stale output, replay/capacity failure, a changed
adapter or configuration, and anything other than the frozen 64/256/1K/10K
sparse rows plus the dense 64-slot control. Clean AMD Radeon 890M RADV Vulkan
artifacts compare probe commit `4a940d7` before persistence/topology closure
with implementation commit `f395fe4`:

| reserve | persistence P50, pre -> final (ms) | GPU end-to-end P50, pre -> final (ms) | final host P50 (ms) | final body/mask/winner work |
| ---: | ---: | ---: | ---: | ---: |
| 64 | 0.017 -> 0.069 | 4.125 -> 4.225 | 7.324 | 2 / 2 / 2,048 |
| 256 | 0.019 -> 0.071 | 3.961 -> 3.801 | 6.481 | 2 / 2 / 2,048 |
| 1,024 | 0.038 -> 0.023 | 4.135 -> 3.794 | 6.110 | 2 / 2 / 2,048 |
| 10,000 | 0.153 -> 0.074 | 4.946 -> 3.987 | 6.976 | 2 / 2 / 2,048 |

All rows contain two live bodies and one committed point, replay exactly, and
remain capacity-clean. Across all four reserves final GPU end-to-end max/min
is `1.114`; across 256/1K/10K it is `1.051`. Final host-completion max/min is
`1.199` across all rows and `1.142` across 256/1K/10K. Persistence P50 has
sub-0.074 ms timer/runtime noise but no longer grows with the 10K reserve.
Accounted allocation still grows honestly from 974,300 to 95,873,384 bytes.
The 64-body dense control reports 312 pair attempts, 32 candidates, 42 contact
points, 64 body slots, and 65,536 winner visits, proving that actual live work
still expands.

The final graph passes `npm run test:fast`, production clippy, the ARM64
Android check, a release-native dual-ABI build, and Gradle `assembleRelease`.
Clean revision `f395fe4` produced successful fresh headless and Browser WebGPU
validation. With no physical device attached for the closing run, the allowed
AVD fallback passed on `jstorrent-tablet` using the host Radeon Vulkan adapter;
the screenshot was visually complete and the script confirmed emulator
shutdown. The physical Pixel 7a capacity comparison from the preceding S22
checkpoint remains the physical-device evidence; the final persistence and
topology delta was requalified on the AVD rather than misreported as a new
physical run.

## Completion condition

S22 is complete only when all hot product stages are driven by GPU-resident
live/current work, the sparse-capacity probe is approximately flat through
10K, dense cost still scales honestly, correctness/replay/capacity gates pass,
and browser plus physical Android evidence is recorded. Merely hiding dead
bodies in a shader, selecting a cheaper fixed schedule, or improving one
capacity preset does not complete the workstream.

That condition is met at `f395fe4`: allocation capacity remains explicit and
fail-closed, while committed active bodies, candidates, pair slots, points,
and colors size hot work without a CPU count readback. Sleeping can now consume
the deterministic live set as its awake-work foundation.
