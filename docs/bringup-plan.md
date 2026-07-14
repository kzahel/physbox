# Bring-up plan

The milestone order is a dependency graph. Later product breadth cannot hide a
weak solver, incomplete pair generation, or host synchronization.
Performance evidence follows the
[performance and scale contract](topics/performance-and-scale.md) from the
solver laboratory onward; P5.4 is the first complete rigid-engine claim, not
the first time scale is measured. Interactive scheduling follows the
[simulation time and overload contract](topics/simulation-time-and-overload.md):
fixed `dt`, at most one outer step per engine frame, and permanent wall-time
slip instead of catch-up.

## P5.0 — lean shared-host bootstrap (current)

- Fresh Git history and PhysBox 4 provenance.
- One GPU-resident bootstrap body, one compute step, and direct rendering.
- One `EngineHost::render_frame` path across desktop, web, Android, headless.
- Minimal launch contract and shared bootstrap scenario.
- Fresh artifacts, boundary tests, and exact-pushed-SHA deployment.
- No Rapier, liquid, product tools, settings, or old scene compatibility.

Exit: all locally available gates pass, Wrangler dry-run passes, hook is
installed, and the tree contains only the retained platform rim.

## P5.0a — oracle foundation and solver laboratory

Execution tracker:
[S1 rigid solver laboratory](tactical/001-rigid-solver-laboratory.md).

1. Replace the bootstrap eight-step catch-up accumulator with the shared
   slow-clock/explicit-step modes, diagnostics, and timing-sequence tests.
2. Define versioned body, authored-manifold, scenario, trace, metric, capacity,
   and failure schemas.
3. Add scalar Rust contact preparation and solve specification.
4. Add the scoped analytic fixture contact generator so multi-step fixtures
   can regenerate contacts; fixture geometries only, checked against Box2D
   public collision functions, replaced by P5.2 and never promoted.
5. Add a process-isolated native, pinned, single-threaded Box2D authority
   executable and direct public narrowphase harness; keep both outside the
   product dependency graph.
6. Add invariant/metamorphic generators and failure minimization.
7. Implement GPU Jacobi/gather and graph-colored Soft-TGS against precomputed
   manifolds; optionally add one XPBD/split comparator.
8. Benchmark the shared prefix-scan, radix-sort, unique, and compaction
   primitives on named adapters; their measured cost is a solver-selection
   input.
9. Record the numerical, cross-adapter, capacity, and 1K/10K/30K authored-contact
   performance matrix required by the scale contract, including at least one
   measurement on the named Android reference device.
10. Select the solver family and freeze its first checked baselines through
    one explicit human review.

Exit: exact/tightly bounded scalar pass comparisons, behavior matrix, no
readback dependency, no overflow, no interactive multi-step catch-up, and a
documented solver decision. Do not build a production broadphase before this
exit. PhysBox 3 browser automation, LiquidFun, and a Rapier2D comparator are
not part of this stage.

## P5.1 — minimal rigid state and fixed-floor contact

- Replace bootstrap state with generational SoA body storage and bounded GPU
  commands.
- Keep one analytic rectangle plus fixed floor and direct rendering.
- Exercise the selected solver with a precomputed floor manifold.
- Add reset and engine-owned neutral grab actions.
- Prove the same scenario and scene contract on every host.
- Record bytes per body and free-flight integration/rendering curves at 1K,
  10K, and 100K on the named reference adapters, including Android, before
  freezing the body layout.

## P5.2 — small-world collision and persistence

- Add circles and oriented rectangles.
- Use all-pairs AABB candidate generation.
- Implement analytic/SAT/clipped manifolds and direct Box2D comparison.
- Add sorted persistent contact keys, warm starting, friction, restitution,
  speculative separation, and explicit capacity diagnostics.
- Produce synchronized oracle/GPU failure playback.

## P5.3 — scalable broadphase bake-off

- Preserve brute force as pair-completeness oracle.
- Implement sorted sweep-and-prune and multi-cell grid candidates.
- Run the shared randomized/adversarial completeness corpus and applicable
  32-through-100K performance scene families.
- Choose from measured correctness, candidate amplification, memory, and GPU
  time. Add LBVH only if those measurements require it.

## P5.4 — rigid parity and performance gates

- Add sleeping/waking and compact asynchronous diagnostics.
- Accept landing, stack, pyramid, impact, friction, bounce, mass-ratio, and
  grab/release fixtures against Box2D and independent invariants.
- Publish correctness-qualified Box2D CPU/GPU crossover curves for solver-only,
  collision-only, end-to-end physics, and full-frame scopes at 32, 256, 1K,
  3K, 10K, 30K, and 100K where each scene family is applicable. Extend sparse
  and mostly-sleeping cases to 300K+ where useful.
- Track P50/P95/P99 GPU/CPU timing, dispatches, active work, amplification,
  memory high-water, every overflow, effective interactive time scale, and
  slipped wall time. A performance result is invalid when correctness or
  capacity fails; slow-time pacing cannot be reported as sustained 60 Hz
  throughput.

## P5.5 — first construction loop

- Restore place, erase, select, and grab for circles/rectangles.
- Add minimal authored save/load.
- Add one joint only after free-body contact remains green.

## P5.6 — LiquidFun-oracled coupled material

- Add bounded water only after P5.4 acceptance.
- Generate fluid fixtures from the same shared scenario projection.
- Add separate native oracle executables for the pinned PhysBox 3 particle
  sidecar plus Box2D v3 and for original LiquidFun/Box2D 2.3.
- Treat the native PhysBox 3 sidecar as the coupled-behavior authority and
  original LiquidFun as a secondary algorithm/scenario reference.
- Calibrate a small canonical corpus against fresh runs of the actual PhysBox
  3 web product; do not require that browser lane for routine correctness.
- Couple particle/body response inside the same GPU fixed step, including
  translation, off-center torque, pressure/boundary effects, and wake policy.
- Compare displacement, float/sink, wave impact, and moving boundaries with
  PhysBox 3/LiquidFun.
- Prove there is no simulation-dependent fluid impulse readback.

## Autonomous development loop

For each accepted slice: select or generate a shared scenario, project it to
the required process-isolated native oracle, run invariants and scalar checks,
run authority/GPU comparison, enforce metric bands, run capacity/performance
lanes, minimize any failure, and promote its reproducer to the corpus. Every
failure artifact records run/scenario ids, source and oracle revisions,
dirty/patch digests, adapter/browser where relevant, full contract, pass
counters, structured diff, and deterministic failure reason.

Automation may repair code to restore a baseline. It may not change the solver
family, oracle precedence or revision, physical defaults, tolerance bands, or
checked baseline without an explicit measured decision.
