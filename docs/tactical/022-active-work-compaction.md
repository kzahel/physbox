# S22: active-work compaction

Status: active 2026-07-21. A Pixel 9 subjective run established the trigger:
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

- [ ] Add one focused benchmark/probe that runs the same small live scene at
  64, 256, 1K, and 10K reserve, records GPU collision/persistence/end-to-end
  samples when timestamps exist, host completion, dispatches, live work,
  allocation, replay, and overflow.
- [ ] Record a clean pre-change native artifact and the physical Pixel 9
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
- [ ] Preserve the isolated snapshot SAP lane and canonical generational pair
  output unchanged.

Gate: product SAP equals brute force across the maintained corpus and dead-slot
reuse; sparse 256/1K/10K rows report identical live proxy/sort work.

### S22.4 — live-count narrowphase and persistence

- [x] Size narrowphase generate/scan/scatter from committed candidates without
  clearing or scanning unused candidate capacity.
- [ ] Size persistence preparation/matching/retirement from committed current
  and previous pair/point lists while retaining fail-closed capacity behavior.
- [x] Publish GPU point-count indirect arguments for the solver.

Gate: zero-candidate and one-contact rows do bounded constant work across
reserve presets; contact lifecycle, feature identity, cache expiry, replay,
and overflow tests remain exact.

### S22.5 — live-count solve and integration

- [x] Dispatch body passes over active slots and contact passes over committed
  solver points.
- [ ] Remove reserve-sized topology/color scans from the hot path, including
  the `body_capacity * 32` color-winner clear; retain explicit allocation
  capacities and stale-state safety for reused slots.
- [x] Keep every solver profile on the shared operation graph and preserve
  selected soft-product results.

Gate: structural/energy/penetration validation and exact replay remain within
their existing bands; sparse 256/1K/10K step timing is approximately flat.

### S22.6 — cross-host qualification and closure

- [ ] Run `npm run test:fast` once for the ordinary lane.
- [ ] Run production clippy with warnings denied.
- [ ] Produce one fresh headless smoke artifact.
- [ ] Run the ARM64 Android check, build a release APK, and validate capacity
  256/1K/10K on the unlocked physical Pixel.
- [ ] Confirm the ordinary browser WebGPU result and a focused browser sparse-
  capacity probe; do not duplicate solver-menu coverage unless menu behavior
  changes.
- [ ] Update the performance topic, capability ledger, bring-up plan, and this
  tactical with measured before/after evidence.

Gate: with the same one-rectangle scene, selecting 1K or 10K does not cause a
material pacing regression relative to 256 on the physical Pixel. A dense row
must still demonstrate that real live work grows and every row must identify
capacity, live bodies, contacts, pacing mode, adapter, and overflow state.

## Completion condition

S22 is complete only when all hot product stages are driven by GPU-resident
live/current work, the sparse-capacity probe is approximately flat through
10K, dense cost still scales honestly, correctness/replay/capacity gates pass,
and browser plus physical Android evidence is recorded. Merely hiding dead
bodies in a shader, selecting a cheaper fixed schedule, or improving one
capacity preset does not complete the workstream.
