# PhysBox 5 topic index

Topic documents are focused, durable records for continuing technical
concerns. They capture current decisions, evidence, alternatives, validation,
and reconsideration triggers without turning the README or bring-up plan into
a running log.

## Active topics

- [PhysBox 5: one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md)
  — authoritative engine direction; rigid-first GPU pipeline, solver lab,
  contacts, broadphase, process-isolated native oracle hierarchy, and later
  coupled-fluid acceptance.
- [Performance and scale contract](performance-and-scale.md) — workload and
  scene matrix, CPU/GPU comparison lanes, timing/capacity validity, provisional
  scale expectations, and milestone/regression gates.
- [Simulation time and overload](simulation-time-and-overload.md) — fixed-delta
  slow motion under load, zero-or-one interactive steps, permanent wall-time
  slip, explicit-step validation, and scheduler diagnostics.
- [Solver laboratory contracts](solver-laboratory-contracts.md) — versioned
  body/manifold/scenario/oracle/trace/metric/capacity/failure schemas,
  canonical JSON, hashes, provenance, and validity rules.
- [Scalar contact specification](scalar-contact-specification.md) — executable
  contact preparation, accumulated impulse, friction, bias, relaxation,
  restitution, trace ordering, and analytic/metamorphic baselines.
- [Box2D rigid authority](box2d-rigid-authority.md) — clean source pin, frozen
  deterministic build/world settings, shared process projection, direct public
  narrowphase, rejection policy, and microscope trigger.

## Conventions

- Use an unnumbered, descriptive kebab-case filename.
- State whether conclusions are decided, provisional, or exploratory.
- Date material changes and record the evidence that changed the conclusion.
- Link primary specifications, papers, pinned source revisions, code, and
  benchmark artifacts.
- Separate portable required behavior from optional accelerated behavior.
- Record alternatives and explicit triggers for reconsidering the decision.
- Update the topic when implementation or measurements change its conclusion.
- Create a sibling topic when a concern can evolve independently rather than
  broadening an unrelated document.
- Add every active topic to this index in the same commit that creates it.

Likely next topics include contact persistence and feature identifiers, GPU
accumulation strategies, GPU sleeping and island-wake strategy, broadphase
bake-off results, and oracle/failure-artifact schemas.
