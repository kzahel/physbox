# Contact solver parallelism and scheduling

Status: human checkpoint, 2026-07-16. The colored/manifold family is the
selected direction for the bounded S5.4b laboratory experiment. The product
execution remains the provisional point-Jacobi graph; no candidate, quality
band, tolerance, or default is promoted by this document.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Execution tracker:
[S5 interactive rigid quality and scale](../tactical/005-interactive-rigid-quality-and-scale.md).
Cost and scale claims are owned by
[performance and scale](performance-and-scale.md); pacing and any frame-rate
retarget are owned by
[simulation time and overload](simulation-time-and-overload.md); stage math is
owned by the [scalar contact specification](scalar-contact-specification.md).
Primary evidence:
[S5.4 awake settling bakeoff](../../evidence/p5.2b-s5.4/README.md) and
[S5.4a parallel-normal factorial](../../evidence/p5.2b-s5.4a/README.md).

## The concern

Rigid contact impulses are interdependent: the bottom contact of a stack must
carry the weight of every body above it, but learns that only through the
chain of contacts. How those coupled solves are ordered on a massively
parallel device decides both structural stability and portable scheduling
cost. This topic records why the independent-parallel (Jacobi) formulation
fails tall structures, which alternatives were measured, why sequential-style
(colored Gauss-Seidel) solving is the selected direction, and what its WebGPU
dispatch floor implies for the product.

## Why independent-parallel solving fails stacks

A Gauss-Seidel sweep solves contacts one at a time against continuously
updated velocities, so support information crosses the whole chain in one
sweep. Box2D does this and keeps every test structure upright. A Jacobi pass
instead solves every contact against the same start-of-pass snapshot and
applies all impulses together. The product graph is Jacobi: each point uses a
standalone two-body effective mass (`effective_mass`), solves against the
snapshot (`emit_normal`), and the gather stage sums every contribution at full
strength with no contact-count scaling (`gather`, all in
`crates/physbox-engine/src/gpu_solver.wgsl`).

Two mechanisms follow:

- **Propagation delay.** Support information moves one contact layer per
  pass, so a deep chain cannot equilibrate within a feasible pass budget
  while gravity keeps injecting new penetration.
- **Simultaneous overshoot.** An interior stack body receives full-strength
  corrections from the manifolds above and below at once. Whether the
  overshoot damps or grows is governed by the spectral radius of the Jacobi
  iteration matrix; chain length destroys the diagonal dominance that
  convergence requires.

The measured signature is decisive: past a depth threshold, more iterations
make the same structure fail *earlier* (the 16-stack fails at step 5 with
eight iterations versus step 14 with one). That is divergence, not slow
convergence. A second, independent defect is intra-manifold rocking: the two
points of a box-box manifold each compute a full standalone correction
including the angular term, so their simultaneous sum over-rotates the body
and pumps energy — the dominant visible jitter on otherwise passing scenes.

## Measured alternatives

S5.4 evaluated nine reversible executions inside the point-Jacobi family
(substeps, iterations, warm start, relaxation, bias, contact policy): every
candidate lost the awake 8-stack while awake Box2D held every structure below
`2.72e-6 m/s` / `1.78e-6 rad/s` settled residuals. S5.4a then ran a factorial
of five formulations at 1/2/4/8 iterations over seven structures including a
16-stack. Condensed result (complete matrix in the evidence package):

| Family | Best structural result | Interpretation |
| --- | --- | --- |
| Point Jacobi | 3/7 at every iteration count | Depth-limited; iteration-amplified divergence |
| Scalar mass splitting | 6/7 at i8; 16-stack falls at step 44 | Provably contractive but too slow; sags ("memory foam"), 8-stack residual 0.377 m/s |
| Manifold block normal | 6/7 from i2; 16-stack falls even at i8 | Removes rocking, moves the depth horizon, does not remove it |
| Block + mass splitting | 3/7, worse with iterations | Negative interaction; do not compose these fixes |
| Colored in-place | **7/7 at i4 and i8** | Only family with no depth horizon; blocker is scheduling cost, not physics |

Cost at 1,024 bodies (Apple M4 Pro Metal, complete graph P50): manifold block
i2 `3.564 ms` / 135 dispatches; colored i4 `19.308 ms` / 2,360 dispatches;
fastest optimized Box2D `0.185 ms`. The colored comparator encodes 32
worst-case color passes times five separate stages, while actual color counts
are only 4 (16-stack) to 12 (4-row pyramid) — the cost is dominated by
scheduling waste, not solve work.

Conclusions recorded as decided:

- Sequential propagation is required for deep rigid structures; no
  Jacobi-family tuning (damping, blocks, iterations, substeps, warm-start or
  relaxation policy, bias) removes the depth horizon. The colored family is
  the rigid direction.
- Mass splitting is excluded from the colored lane: Gauss-Seidel does not
  need its convergence guarantee and the measured composition is a
  regression.
- The two-point manifold block solve is validated and carried forward inside
  the colored schedule, where it should reduce the iterations needed at all.

## The portable dispatch floor

Sequential-by-color execution turns every color transition into a dispatch
boundary with fixed driver/queue cost, independent of pass size. The frame
contract forbids CPU readback of GPU results, so the CPU encodes the command
stream blind: the current comparator therefore encodes the 32-pass worst case
even when four colors exist, and empty passes still launch full grids that
exit on a color check. The CPU authority pays effectively nothing for the
same barrier (a user-space thread sync) and always knows its color count.

The floor is an arithmetic, not a mystery: optimized passes scale as
(actual colors) x (fused stages) x (iterations) x (substeps). With 4–12
colors, ~2 fused stages, <=2 iterations, and 4 substeps, the expected count is
a few hundred dispatches — the territory where manifold block i2 already runs
`3.564 ms`. 60 Hz at 1K on desktop is therefore plausible but unmeasured;
S5.4b exists to measure it. Two structural facts frame the decision:

- The dispatch floor is roughly constant in body count (colors track contact
  graph degree, not N), while solve work and any CPU engine scale with N. The
  floor amortizes exactly as scenes approach the product target of thousands
  of solids plus liquid. Small-fixture ratios such as 20.8x or 104.3x versus
  Box2D at 1K are not decision metrics; scale claims belong to the
  performance-and-scale contract.
- Per-dispatch overhead differs by host. The floor must be measured on
  browser WebGPU and Android, not extrapolated from Metal.

## Selected next experiment (S5.4b)

One bounded laboratory lane combining the validated pieces:

- color at manifold granularity, so both points of a pair are co-scheduled
  and the proven two-point block normal solve runs inside the colored unit;
- compact GPU-owned color ranges so a pass launches exactly its contacts;
- GPU-written indirect dispatch arguments so pre-encoded passes for absent
  colors become zero-workgroup launches, preserving the no-readback contract;
- fused compatible stages to divide the per-color pass count;
- iteration sweep downward (i1/i2 first) — if only i4 preserves all seven
  structures, the block solve is not buying convergence and that is a
  material result;
- added observations: color-count distribution and cost on the 100-body
  bucket scenes (high-degree contact graphs), and recoloring cost under
  construction/erase churn;
- exclusions: no mass splitting, no CPU readback, no second world; the same
  generated contacts, pair slots, body-edge views, caches, capacities, and
  physical settings.

Gates repeat S5.4a: all seven structures over 1,200 exact steps, complete
32/256/1K cost, exact replay, browser and AVD execution. Promotion remains a
separate explicit human decision.

## Fallback and reserve options

- **Interim baseline swap.** Manifold block i2 strictly dominates the current
  provisional point-Jacobi execution (6/7 versus 3/7 structures, `3.564 ms`
  versus `4.244 ms`). Swapping the provisional interactive baseline is a
  physical-default change requiring explicit human acceptance; recorded here
  as low-risk if S5.4b runs long.
- **Workgroup-per-island sequential solve.** Islands small enough for one
  workgroup (a 16-stack easily fits) can be solved in a single dispatch using
  `workgroupBarrier()` between color phases — full sequential semantics with
  zero inter-dispatch cost. Requires GPU island identification (connected
  components); documented reserve, not scoped into S5.4b.
- **Not portable, rejected:** persistent/cooperative kernels with device-wide
  sync and driver-generated command techniques (CUDA graphs, device-generated
  commands) do not exist in portable WebGPU.
- **Pacing retarget with render interpolation.** If the measured floor misses
  16.67 ms on a required host, a lower fixed outer rate plus interpolated
  presentation is a product option — but display rate is not the cost driver;
  substeps per second are. Halving the outer rate while preserving substep dt
  doubles per-frame solve work along with the budget, amortizing only the
  once-per-frame stages; real savings require a larger substep dt, which
  trades back convergence. Interpolated presentation is GPU-friendly (body
  states are already double-buffered) but adds up to one outer step of input
  latency, which lands on grab feel. This decision is owned by the
  simulation-time-and-overload contract, is not authorized here, and should
  not be taken before the S5.4b floor exists. The existing no-catch-up slip
  already degrades overload gracefully without a retarget.

## Anticipated end-state (exploratory)

One world with per-constraint-family schedules: colored sequential solving
for rigid contacts, relaxed Jacobi-style iteration for later particle/liquid
and coupling constraints, which tolerate it well (the FleX precedent chose
Jacobi for everything and accepted soft rigid stacking; PhysBox 5 does not).
The product bar comes from the PhysBox 3 oracle, which is built on Box2D:
crisp stacking is required sandbox behavior, and a structure that collapses
at feasible iteration budgets is a product regression, not an aesthetic
trade. This section is exploratory context for P5.6 planning, not a decision;
liquid remains downstream of accepted rigid quality.

## Prior art and references

Pinned checkouts (see `reference/pins.toml`; source material only —
independent implementation, no copying without provenance):

- **PhysX 5 GPU solver** (`reference/physx`, revision `b4b286a`):
  `physx/source/gpusolver/` — `PxgConstraintPartition` is graph coloring
  under another name (32 partitions per slab, `PX_PARTITION_COMPACTION`,
  incremental `addEdge`/`removeEdge` maintenance under churn — the model for
  our recoloring-cost question); `PxgTGSCudaSolverCore` is substepped
  Gauss-Seidel on GPU. PhysX targets large scenes and leaves small scenes on
  the CPU; its CUDA affordances (cooperative grid sync, CUDA graphs) have no
  portable WebGPU equivalent.
- **Box2D v3** (`reference/box2d`, revision `3a4f0da`): CPU constraint-graph
  coloring for worker-thread/SIMD parallelism with a serial overflow lane.
  Box2D colors for speed and would be stable solved serially; PhysBox colors
  for correctness. Its TGS Soft substepping is the accepted behavior target.
- **wgmath** (`reference/wgmath`, revision `ac89dce`): the closest prior art
  under identical portable-WebGPU constraints; read for colored-pass
  scheduling before S5.4b implementation.

Literature (conclusions replicated by S5.4/S5.4a evidence):

- Tonge, Benevolenski, Voroshilov, *Mass Splitting for Jitter-Free Parallel
  Rigid Body Simulation*, SIGGRAPH 2012 — Jacobi damping that guarantees
  convergence; measured here as too slow for deep chains.
- Harada, *A Parallel Constraint Solver for a Rigid Body Simulation*, 2011
  (Bullet 3 GPU lineage) — batched/colored Gauss-Seidel on GPU.
- Fratarcangeli, Tibaldo, Pellacini, *Vivace: a Practical Gauss-Seidel Method
  for Stable Soft Body Dynamics*, SIGGRAPH Asia 2016 — fast randomized graph
  coloring for parallel Gauss-Seidel.
- Macklin, Storey, Lu, Terdiman, Chentanez, Jeschke, Müller, *Small Steps in
  Physics Simulation*, SCA 2019 — substeps beat iterations; the basis of TGS
  and the reason the substep budget, not display rate, is the cost driver.
- Macklin, Müller, Chentanez, Kim, *Unified Particle Physics for Real-Time
  Applications*, SIGGRAPH 2014 (FleX) — the deliberate all-Jacobi design and
  its known-soft rigid stacking; the road not taken for rigid contacts.
- Catto, Box2D v2 block solver — the 2x2 coupled manifold normal solve
  validated here as the manifold block family.

## Reconsideration triggers

Revisit this topic's direction only with fresh governed evidence showing:

- the S5.4b optimized colored lane misses the 16.67 ms feasibility boundary
  or seven-structure preservation on a required host — evaluate the
  workgroup-per-island reserve first, then any pacing retarget under the
  simulation-time-and-overload contract;
- bucket or other high-degree scenes push color counts past the bounded pass
  budget, or recoloring under construction churn dominates frame cost —
  revisit coloring granularity and PhysX-style incremental maintenance;
- portable WebGPU gains device-scope synchronization or device-generated
  commands — recompute the dispatch-floor arithmetic;
- P5.6 liquid coupling requires an accumulation architecture incompatible
  with colored rigid contacts — renewed human architecture decision;
- a Soft-TGS or disjoint-batch formulation matches colored structure at
  materially lower measured cost; or
- a genuine invariant-versus-Box2D conflict requires authority review under
  the declared oracle precedence.

Do not reconsider by widening tolerances, accepting toppled or transient
structural failures, hiding unused color passes, or extrapolating rigid
small-fixture timings to liquid-heavy scenes.
