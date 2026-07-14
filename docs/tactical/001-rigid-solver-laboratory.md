# S1: Oracle foundation and rigid solver laboratory

Status: active, 2026-07-14.
Direction: [one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Bring-up stage: [P5.0a](../bringup-plan.md#p50a--oracle-foundation-and-solver-laboratory).

## Goal

Execute P5.0a as a sequence of independently gated implementation slices. The
work starts with shared test/result schemas, builds a scalar contact
specification and Box2D authority lane, evaluates portable GPU candidates, and
ends with the evidence package needed to choose the first rigid solver.

## Governing contracts

This tactical tracks execution; it does not restate or revise the direction's:

- [solver-laboratory purpose and selection criteria](../topics/physbox5-unified-gpu-physics.md#solver-laboratory-before-the-product-pipeline);
- [portable accumulation constraints](../topics/physbox5-unified-gpu-physics.md#portable-and-accelerated-accumulation);
- [layered oracle model](../topics/physbox5-unified-gpu-physics.md#layered-rigid-oracle-model);
- [oracle execution and precedence](../topics/physbox5-unified-gpu-physics.md#oracle-execution-topology-and-precedence); and
- [failure-artifact](../topics/physbox5-unified-gpu-physics.md#oracle-driven-development-and-failure-artifacts)
  requirements;
- [performance and scale](../topics/performance-and-scale.md), including its
  workload vector, solver-only scope, timing validity, and CPU comparison
  lanes; and
- [simulation time and overload](../topics/simulation-time-and-overload.md),
  including zero-or-one interactive steps, permanent wall-time slip, and
  explicit-step validation.

P5.0a operates on authored manifolds plus the deliberately scoped analytic
fixture contact generator. Production broadphase, general
narrowphase, product tools, liquid, and scene breadth remain later stages.
Original LiquidFun, the PhysBox 3 sidecar and web product, and an optional
Rapier2D comparator are outside this tactical.

## Delivery slices

### S1.1 — schemas and invariants

- [x] Replace the bootstrap eight-step catch-up loop with engine-owned
  interactive slow-clock and explicit-step modes.
- [x] Define requested/effective scale, active wall time, overload slip,
  lifecycle discard, completed-step, and pacing-mode report fields.
- [x] Add deterministic 120/60/50/30/15 Hz, long-stall, pause/single-step, and
  suspend/resume timing sequences that reject multi-step interactive frames or
  retained whole-step debt.
- [x] Define versioned body, authored-manifold, contact-cache, scenario,
  metric, trace, capacity, and failure-result schemas.
- [x] Define the versioned native-oracle request/result protocol, including
  canonical JSON serialization, unsupported-feature rejection, source scenario
  hash, settings, provenance, atomic result creation, and
  `PHYSBOX5_ORACLE_SUCCESS`.
- [x] Add finite-state, equal-and-opposite, symmetry, projected-impulse,
  capacity, and repeatability invariants.
- [x] Add deterministic authored fixtures, randomized generation, and replayable
  reproducer serialization.
- [x] Add negative fixtures for a bad source/projection hash, wrong oracle pin,
  unsupported feature, timeout, stale or partial result, and missing success
  sentinel.

Gate: schema round trips, generators replay exactly, deliberately broken
implementations trip every invariant, and every transport/provenance negative
fails deterministically without accepting an old result. Interactive timing
never executes more than one outer step, while explicit-step fixtures complete
the exact requested count independent of wall cadence.

### S1.2 — scalar executable specification

- [x] Implement contact preparation, effective mass, accumulated normal and
  tangent impulses, restitution, bias, and relaxation in scalar Rust.
- [x] Emit pass-level traces that identify the first divergent value.
- [x] Check analytic one-contact cases and metamorphic variants.
- [x] Implement the scoped analytic fixture contact generator (circle/floor,
  rectangle/floor, axis-aligned box-on-box) so settling, sliding, and sleep
  fixtures can regenerate contacts across fixed steps; it must not grow SAT
  clipping, general feature persistence, or broadphase behavior.
- [x] Establish tightly bounded scalar baseline fixtures.

Gate: analytic expectations and all invariant fixtures pass without Box2D.

### S1.3 — Box2D authority and microscope lanes

- [ ] Validate the unmodified source pin with
  `npm run references:status`, then pin the single-threaded Box2D authority
  build and options.
- [ ] Build a process-isolated native authority executable that links the
  unmodified library, consumes the shared scenario projection, and emits the
  canonical result schema.
- [ ] Keep schema parsing and emission in a thin Rust CLI over direct Box2D C
  FFI; no engine or app crate may depend on the adapter.
- [ ] Reject unsupported projection features without approximating them or
  introducing a second authored format.
- [ ] Compare public circle/polygon collision manifolds directly, including
  the fixture contact generator's manifolds on its supported geometries.
- [ ] Add the separately identified, patch-digested microscope lane if scalar
  and public Box2D output cannot localize a failure.
- [ ] Produce synchronized scalar/Box2D playback for behavioral fixtures.

Gate: the native authority result is reproducible from recorded provenance and
one shared projection; it links no product runtime, and microscope output is
never used as acceptance truth.

### S1.4 — portable GPU candidates

- [ ] Implement contact-centric Jacobi with equal-and-opposite contribution
  records and per-body gather.
- [ ] Implement graph-colored or disjoint-batch Soft-TGS without CPU-driven
  convergence or silently dropped conflicts.
- [ ] Optionally implement one rigid XPBD or split position/velocity comparator
  only if the first two leave a material unanswered question.
- [ ] Compare every declared pass with the scalar trace and run the same
  authored-manifold corpus on native, browser, Android, and headless.

Gate: pass-level tolerances, invariants, capacity, and cross-adapter behavior
pass with no simulation-dependent readback.

### S1.5 — failure automation and performance matrix

- [ ] Minimize randomized failures and promote reproducers into the corpus.
- [ ] Generate the correctness, convergence, capacity, and timing matrix
  required by the performance and scale contract.
- [ ] Benchmark solver-only fixtures after warmup at 1–32 and 1K, 10K, and at
  least one 30K authored-contact workload on named adapters and revisions.
- [ ] Record bodies, contacts, solver work, dispatches, GPU and CPU timing,
  scratch/allocation high-water, and every capacity counter for each solver
  candidate at equivalent accepted quality.
- [ ] Run the separate optimized Box2D performance lane where the authored
  fixture has a valid shared projection; do not use the deterministic authority
  lane as the sole CPU comparison.
- [ ] Benchmark the shared prefix-scan, radix-sort, unique, and compaction
  primitives at 1K, 10K, and 30K element scales on the same named adapters and
  include their curves in the comparison report.
- [ ] Name the Android reference device and record at least one solver-only
  and primitive measurement on it; if no device is available, record that gap
  explicitly rather than omitting the lane.
- [ ] Generate one comparison report and synchronized playback set.

Gate: failures are actionable without rereading logs, every accepted run has
zero overflow, performance results name their source revision and adapter, and
the solver decision includes convergence-versus-cost curves — including shared
primitive cost and the Android measurement or its recorded absence — that
expose any dispatch, bandwidth, scratch, or capacity limit at useful scale.

### S1.6 — solver decision

- [ ] Review the objective matrix and synchronized playback.
- [ ] Select the initial solver family, portable accumulation strategy,
  substep/iteration policy, and checked baseline parameters.
- [ ] Record rejected alternatives and explicit reconsideration triggers in
  the authoritative topic.
- [ ] Update the bring-up plan and mark this tactical complete.

This is a manual checkpoint. Automation may prepare evidence and repair an
implementation; it may not silently choose or switch the solver family.

## Completion definition

S1 is complete when every slice gate passes and the reviewed comparison package
selects one portable solver foundation with recorded parameters, rejected
alternatives, and reconsideration triggers. Completion authorizes P5.1 body
state and fixed-floor work; it does not claim full rigid-foundation acceptance
or authorize liquid work.
