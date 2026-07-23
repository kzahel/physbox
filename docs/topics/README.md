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
- [Build, validation, and evidence health](build-validation-and-evidence-health.md)
  — 2026-07-20 repository audit of Cargo target growth, gate latency, local
  build and evidence storage, failure-recovery cost, and proposed remediation
  and retention boundaries.
- [Product capability status](product-capability-status.md) — compact current
  ledger of implemented, limited, missing, and deferred engine features plus
  the next execution order.
- [Rigid quality targets](rigid-quality-targets.md) — current awake stack
  hierarchy: 2/4/8 regression foundation, 12 blocking, 14 stretch, 16 as the
  matched-Box2D frontier diagnostic, and the rule that sleep cannot earn an
  awake pass.
- [Simulation time and overload](simulation-time-and-overload.md) — fixed-delta
  slow motion under load, zero-or-one interactive steps, permanent wall-time
  slip, explicit-step validation, and scheduler diagnostics.
- [Physics configuration and solver profiles](physics-configuration-and-solver-profiles.md)
  — selected soft-coupled product direction, configurable fixed rate,
  substeps, iterations, contact response, named presets, shared-host UX, and
  configuration-specific validation and cost.
- [Solver laboratory contracts](solver-laboratory-contracts.md) — versioned
  body/manifold/scenario/oracle/trace/metric/capacity/failure schemas,
  canonical JSON, hashes, provenance, and validity rules.
- [Scalar contact specification](scalar-contact-specification.md) — executable
  contact preparation, accumulated impulse, friction, bias, relaxation,
  restitution, trace ordering, and analytic/metamorphic baselines.
- [Box2D rigid authority](box2d-rigid-authority.md) — clean source pin, frozen
  deterministic build/world settings, shared process projection, direct public
  narrowphase, rejection policy, and microscope trigger.
- [Interactive Box2D comparison](interactive-box2d-comparison.md) —
  implemented default-enabled compile-removable shadow-world review in the
  existing shared UI, stable-identity action mirroring, cross-host packaging,
  authority equivalence, and strict non-fallback/performance boundaries;
  physical Android execution is the remaining qualification gate.
- [Collision and contact persistence](collision-and-contact-persistence.md) —
  analytic shape encoding, canonical pairs/features, persistent keys, GPU
  record bytes, atomic capacity semantics, and P5.2 reconsideration triggers.
- [Contact solver parallelism and scheduling](contact-solver-parallelism.md) —
  why independent-parallel (Jacobi) solving fails deep stacks, the measured
  S5.4/S5.4a alternatives, the selected colored/manifold direction, the
  portable WebGPU dispatch floor, the closed negative S9 schedule study,
  fallbacks, prior art, and pacing boundaries.
- [GPU sleeping and waking](gpu-sleeping-and-waking.md) — master record for
  the sleeping/waking feature: per-body timers with bounded neighbor wake
  propagation, versioned sleep state, wake-trigger semantics, the
  `sleeping_enabled` toggle, GPU state visualization and policy experimentation
  surface, the mask rule against concealing awake losses, and the S23 execution
  overview.
- [GPU sleep-island connectivity research](gpu-sleep-island-connectivity.md) —
  exploratory source study of Box2D's persistent islands, the measured K=4
  baseline, static and persistent GPU connected-component alternatives, and
  the performance/behavior gates for any exact-island prototype.
- [Awake settling and rest stability](awake-settling-and-rest-stability.md) —
  resting-contact identity, warm-start continuity, soft-contact damping,
  realistic drop fixtures, awake quality gates, and the required visible
  review before physical-default promotion, broadphase selection, or sleeping.
- [Box3D planar stability reference](box3d-planar-stability-reference.md) —
  completed bounded sleep-off study of Box3D's planar locks, persistent
  contacts, and graph order; its forwarded static-late PhysBox arm was
  measured and discarded by S9.

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

Likely next topics include broadphase bake-off results and the coupled
liquid/rigid constraint surface.
