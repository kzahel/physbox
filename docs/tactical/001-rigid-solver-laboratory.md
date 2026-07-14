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
- [layered oracle model](../topics/physbox5-unified-gpu-physics.md#layered-rigid-oracle-model); or
- [failure-artifact](../topics/physbox5-unified-gpu-physics.md#oracle-driven-development-and-failure-artifacts)
  and [performance](../topics/physbox5-unified-gpu-physics.md#performance-and-capacity-gates)
  requirements.

P5.0a operates on authored manifolds. Production broadphase, general
narrowphase, product tools, liquid, and scene breadth remain later stages.

## Delivery slices

### S1.1 — schemas and invariants

- [ ] Define versioned body, authored-manifold, contact-cache, scenario,
  metric, trace, capacity, and failure-result schemas.
- [ ] Add finite-state, equal-and-opposite, symmetry, projected-impulse,
  capacity, and repeatability invariants.
- [ ] Add deterministic authored fixtures, randomized generation, and replayable
  reproducer serialization.

Gate: schema round trips, generators replay exactly, and deliberately broken
implementations trip every invariant through negative fixtures.

### S1.2 — scalar executable specification

- [ ] Implement contact preparation, effective mass, accumulated normal and
  tangent impulses, restitution, bias, and relaxation in scalar Rust.
- [ ] Emit pass-level traces that identify the first divergent value.
- [ ] Check analytic one-contact cases and metamorphic variants.
- [ ] Establish tightly bounded scalar baseline fixtures.

Gate: analytic expectations and all invariant fixtures pass without Box2D.

### S1.3 — Box2D authority and microscope lanes

- [ ] Pin the unmodified single-threaded Box2D authority build and options.
- [ ] Project the shared scenario schema without a second authored format.
- [ ] Compare public circle/polygon collision manifolds directly.
- [ ] Add the separately identified, patch-digested microscope lane if scalar
  and public Box2D output cannot localize a failure.
- [ ] Produce synchronized scalar/Box2D playback for behavioral fixtures.

Gate: authority results are reproducible from recorded provenance; microscope
output is never used as acceptance truth.

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
  required by the direction's performance contract.
- [ ] Benchmark solver-only fixtures after warmup at micro and representative
  contact counts on named adapters and revisions.
- [ ] Generate one comparison report and synchronized playback set.

Gate: failures are actionable without rereading logs, every accepted run has
zero overflow, and performance results name their source revision and adapter.

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
