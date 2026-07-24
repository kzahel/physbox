# S37 — Deferred rigid interaction API

Status: active 2026-07-24. Human review approved the Box2D-shaped deferred
query, point-constraint, selected-body destruction, and non-lossy asynchronous
result boundary for end-to-end implementation.

Governing architecture:
[reusable GPU physics library boundary](../topics/reusable-gpu-library-boundary.md).
Authoritative engine direction:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Pacing:
[simulation time and overload](../topics/simulation-time-and-overload.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Let an independent default-feature `physbox-sim` consumer compose dragging
and pointed body destruction from neutral physics primitives. Preserve
caller-owned WGPU device, encoder, submission, and rendering while keeping
selection, retained constraints, body retirement, and authoritative live
state on the GPU.

The proving consumer queues a point query and point-constraint creation in one
step, updates and destroys that constraint, queues a point query and selected
body destruction in one step, observes terminal results asynchronously, and
validates final physics state through `GpuBodyObservation`.

## Exclusions

- Do not export grab, erase-at, pointer, controller, hand, Quest, OpenXR,
  renderer, camera, workbench, scene, preset, comparator, or backend-wrapper
  types.
- Do not expose `physbox-contracts`, `engine-internal`, private WGSL, private
  buffer layouts, or direct views of solver storage.
- Do not add synchronous point queries, per-frame pose readback, a CPU body
  mirror, an internal queue submission, or a frame-blocking map/wait.
- Do not add arbitrary joints, multi-touch policy, particles, water
  interaction, shape casts, ray casts, contact callbacks, or a generic
  multi-backend trait.
- Do not change rigid equations, ordinary solver defaults, authored scenes,
  pacing, presentation, Box2D comparator behavior, or product tool semantics.
- Do not require the sibling Quest repository in PhysBox 5 CI.

## Ownership and semantic constraints

- Public commands are general point queries, point constraints, target
  updates, constraint destruction, and selected-body destruction.
- `BodySelectionId`, `PointConstraintId`, and `CommandId` are opaque
  generational or monotonic logical handles. They reveal no GPU offset.
- A deferred selection may feed a later command queued for the same step.
  Constraint creation synchronously reserves its handle even when selection
  execution is pending.
- GPU commands execute in accepted order. Pending target/end commands either
  apply after successful creation or terminate as cancelled after dependency
  failure.
- Queue-full rejection is synchronous and returns no command ID. Every
  accepted mutation has exactly one terminal result. Application result
  backpressure may be bounded; internal lifecycle reconciliation is non-lossy.
- Dynamic bodies are eligible, including sleepers. Overlap choice is nearest
  origin then `BodyId`. Constraint success wakes the selected body.
- Point-constraint tuning uses hertz, damping ratio, maximum acceleration, and
  linear/angular speed limits. Defaults preserve the existing private grab
  mechanics.
- `World::step` records only. Result copy, submission, map, poll, and result
  reconciliation remain explicit and asynchronous.
- Point destruction must eventually correct the CPU handle catalog and
  counters, reject stale body IDs, and permit slot reuse without leaking
  authoring capacity.
- Cancellation before `step` emits deterministic terminal results. Recorded
  commands cannot be revoked from the caller's encoder and must reconcile
  before further admission. Drop releases reservations without requiring
  observable post-drop results.

## Ordered delivery slices

### S37.1 — Durable contract and public value types

- [x] Amend the reusable-library topic with the decided API and semantics.
- [x] Add this tactical to the status index in creation order.
- [x] Add documented public command, selection, constraint, tuning, result,
      capacity-error, and counter types.
- [x] Add compile-time/API tests proving no first-party or rendering
      vocabulary enters the public surface.

Gate: public definitions express bounded deferred mechanics without exposing
private contracts or application intent names.

### S37.2 — Bounded command admission and same-step dependency graph

- [x] Reserve generational selection and point-constraint handles
      synchronously.
- [x] Admit query/create/target/destroy commands in a bounded ordered batch.
- [x] Support query-to-constraint and query-to-body-destroy chaining in the
      same step.
- [x] Define pending target/end behavior, bounded target admission,
      pre-step cancellation, high-water marks, and synchronous rejection
      accounting.
- [x] Preserve all existing `World` construction/destruction behavior.

Gate: focused CPU tests cover capacity, generation reuse, stale handles,
same-step dependency admission, pending operations, reset, and counters.

### S37.3 — GPU execution and non-lossy lifecycle reconciliation

- [x] Execute deterministic dynamic-body point selection on the GPU.
- [x] Create, tune, update, solve, and destroy one retained point constraint.
- [x] Destroy the exact body selected by a deferred query and reconcile its
      generational identity into the public body catalog.
- [x] Write one bounded terminal result for every accepted mutation.
- [x] Keep internal reconciliation independent of lossy application event
      retention by backpressuring admission until the complete batch
      reconciles.
- [x] Keep `step`, observation, and ordinary interactive advancement free of
      mapping, waiting, and private submission.

Gate: GPU tests prove hit/miss, ordered target/release, stale selection and
constraint handling, body retirement, live counters, slot reuse, and no lost
accepted result at capacity.

### S37.4 — Asynchronous result API and external proof

- [x] Add explicit caller-recorded result copy and nonblocking completion
      polling.
- [x] Reconcile completed body destruction and handle reservations exactly
      once.
- [x] Extend the default-feature external consumer to perform
      query/drag/release/erase without a renderer.
- [x] Validate motion and final live state through `GpuBodyObservation`.
- [x] Prove the consumer has no engine, contracts, renderer, workbench,
      comparator, or internal-feature dependency.

Gate: the external proof compiles and runs using only public default
`physbox-sim`, caller-owned WGPU work, asynchronous results, and canonical
physics observation.

### S37.5 — First-party integration and regression qualification

- [x] Keep the existing first-party grab/erase behavior and UI contract
      unchanged, or migrate it only through a behavior-equivalent neutral
      adapter.
- [x] Pass focused simulation, engine, dependency-negative, and external
      consumer tests.
- [ ] Pass `npm run test:fast` and production warnings-denied clippy.
- [ ] Pass a fresh headless validation and `npm run test:web:lab`.
- [ ] Pass Box2D comparator feature-on, feature-off, and direct comparator
      tests.
- [ ] Pass focused performance smoke and record any dispatch/cost change.
- [ ] Pass ARM64 Android check, release APK, and one physical Android
      validation.
- [ ] Compile the sibling Quest adapter against the final public API and run a
      physical Quest behavior check when its consumer slice is available.

Gate: proportional maintained-host evidence passes without a new CPU
authority, renderer coupling, physics-default change, hidden submission, or
interactive readback dependency.

Implementation checkpoint, 2026-07-24: the default public crate now reserves
same-step selections and one point constraint, executes the ordered commands
on the GPU, writes a fixed result slot for every accepted command, and blocks
further admission until the complete asynchronous batch reconciles. Query
misses cancel dependent pending operations; successful body destruction
retires the exact generation, updates the `World` catalog, rejects the stale
handle, and permits next-generation slot reuse. Default constraint tuning
reproduces the existing private spring/damping/limit coefficients.

Focused `physbox-sim` tests pass 41/41; the default-feature public-world
integration passes 2/2; the surface-free external consumer passes hit, miss,
drag, release, erase, cancellation, full 64-result delivery, observation, and
slot reuse; the platform-boundary negatives pass; all-feature
warnings-denied `physbox-sim` clippy passes; and the all-feature first-party
engine plus external consumer compile. Cross-host qualification remains.

## Completion condition

S37 is complete when a separate-style consumer can compose drag/release and
pointed rigid destruction from the public neutral primitives; same-step
deferred dependencies and tuning have pinned semantics; every accepted
mutation has a terminal asynchronous result; body/constraint lifecycle
reconciles without loss or blocking; slots reuse safely; the default-feature
API remains physics-only; and proportional native, browser, comparator,
performance, Android, and Quest gates pass.
