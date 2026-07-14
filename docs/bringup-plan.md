# Bring-up plan

The milestone order is a dependency graph. Later product breadth cannot hide a
weak solver, incomplete pair generation, or host synchronization.

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

1. Define versioned body, authored-manifold, scenario, trace, metric, capacity,
   and failure schemas.
2. Add scalar Rust contact preparation and solve specification.
3. Add pinned, single-threaded Box2D authority lane and direct public
   narrowphase harness; keep it outside the product dependency graph.
4. Add invariant/metamorphic generators and failure minimization.
5. Implement GPU Jacobi/gather and graph-colored Soft-TGS against precomputed
   manifolds; optionally add one XPBD/split comparator.
6. Record numerical, cross-adapter, capacity, and performance matrix.
7. Select the solver family and freeze its first checked baselines through one
   explicit human review.

Exit: exact/tightly bounded scalar pass comparisons, behavior matrix, no
readback dependency, no overflow, and a documented solver decision. Do not
build a production broadphase before this exit.

## P5.1 — minimal rigid state and fixed-floor contact

- Replace bootstrap state with generational SoA body storage and bounded GPU
  commands.
- Keep one analytic rectangle plus fixed floor and direct rendering.
- Exercise the selected solver with a precomputed floor manifold.
- Add reset and engine-owned neutral grab actions.
- Prove the same scenario and scene contract on every host.

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
- Run randomized/adversarial completeness and scale corpus.
- Choose from measured correctness, candidate amplification, memory, and GPU
  time. Add LBVH only if those measurements require it.

## P5.4 — rigid parity and performance gates

- Add sleeping/waking and compact asynchronous diagnostics.
- Accept landing, stack, pyramid, impact, friction, bounce, mass-ratio, and
  grab/release fixtures against Box2D and independent invariants.
- Benchmark solver-only, collision-only, and end-to-end at 1–32, 1K, 4K, and
  16K bodies where adapters support them.
- Track P50/P95/P99 GPU/CPU timing, dispatches, active work, amplification,
  memory high-water, and every overflow. A performance result is invalid when
  correctness or capacity fails.

## P5.5 — first construction loop

- Restore place, erase, select, and grab for circles/rectangles.
- Add minimal authored save/load.
- Add one joint only after free-body contact remains green.

## P5.6 — LiquidFun-oracled coupled material

- Add bounded water only after P5.4 acceptance.
- Generate fluid fixtures from the same shared scenario projection.
- Couple particle/body response inside the same GPU fixed step, including
  translation, off-center torque, pressure/boundary effects, and wake policy.
- Compare displacement, float/sink, wave impact, and moving boundaries with
  PhysBox 3/LiquidFun.
- Prove there is no simulation-dependent fluid impulse readback.

## Autonomous development loop

For each accepted slice: select or generate a shared scenario, run invariants
and scalar checks, run authority/GPU comparison, enforce metric bands, run
capacity/performance lanes, minimize any failure, and promote its reproducer to
the corpus. Every failure artifact records run/scenario ids, source and oracle
revisions, dirty/patch digests, adapter/browser, full contract, pass counters,
structured diff, and deterministic failure reason.

Automation may repair code to restore a baseline. It may not change the solver
family, oracle revision, physical defaults, tolerance bands, or checked
baseline without an explicit measured decision.
