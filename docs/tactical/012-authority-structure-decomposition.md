# S12: authority solve-structure decomposition

Status: active, 2026-07-19. Authorized by human decision after the verified
[S11 soft formulation review](../../evidence/p5.2b-s11.5-soft-formulation-review/README.md).
The recorded comparator-engine trigger remains satisfied but no comparator
study is authorized. The chosen continuation is an independent scalar
implementation of the pinned Box2D authority solve structure, followed by a
bounded bidirectional decomposition against the existing PhysBox scalar-soft
endpoint. This is a decision laboratory, not a product solver change.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns response quality and the authority-band decision;
[scalar contact specification](../topics/scalar-contact-specification.md)
owns the current PhysBox scalar endpoint;
[box2d rigid authority](../topics/box2d-rigid-authority.md) owns source,
projection, and oracle execution;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns GPU
accumulation feasibility; and
[performance and scale](../topics/performance-and-scale.md) owns any cost or
scale claim. Predecessor:
[S11 soft formulation study](011-soft-contact-formulation.md), which rejected
the two local rewind corrections but did not test the authority structure as
a coupled endpoint.

## Human fork resolution and laboratory authority

Human decision, 2026-07-19:

- do not authorize a comparator-engine study; retain its reconsideration
  trigger as recorded;
- authorize an authority-structure decomposition study against the existing
  clean pinned Box2D v3 process;
- for this laboratory only, response ownership is the native Box2D authority
  band rather than exact current PhysBox response;
- treat that ownership as an explicit new experimental gate, not a weakening
  of a failed PhysBox gate; and
- deliver the minimal sufficient stillness structure and its compatibility
  with the selected colored-parallel GPU architecture, without changing the
  product.

The independent Rust implementation may reuse PhysBox-owned neutral schemas,
geometry projections, metrics, and finite-value helpers. It may not link or
copy Box2D code. The clean pin
`3a4f0da8374af61293a03021c9a0b3ebcfe67948` is source material and the native
process remains the acceptance authority. Source citations are
`reference/box2d/src/solver.c`, `contact_solver.c`, `constraint_graph.c`,
`physics_world.c`, and `types.c`. Any later microscope needs a separate
worktree and is diagnostic only.

## Goal

Determine which coupled subset of the pinned authority solve structure is
sufficient for awake stillness on the governed soft fixtures, and whether that
subset can be represented by the selected bounded colored-parallel GPU graph
without CPU readback, unbounded dispatch, a product fallback, or weakened
quality. A complete negative is valid: if the sufficient structure is
incompatible with the GPU ownership constraints, record that architectural
constraint instead of approximating it away.

## Exclusions and ownership constraints

- No product constructor, selected hard/direct policy, coefficient default,
  scheduler, capacity, tolerance, oracle precedence, launch behavior, or
  platform path changes.
- No sleeping or sleep-derived stillness. Native Box2D and scalar lanes are
  awake throughout the measured window.
- No Rapier2D, new engine, CPU product path, backend substitution, reference
  dependency, or copied foreign implementation.
- No coefficient sweep, damping knob, penetration deadband, empirical
  attenuation, randomized ordering, response averaging, or post-measurement
  gate refresh.
- The bridge has exactly five binary structural axes. Two endpoints, ten
  bidirectional single-axis hybrids, and at most three routed two-axis
  interactions are the complete measurement space: at most 15 configurations.
- A scalar result cannot promote a product policy. GPU work in S12 is limited
  to a static schedule/layout/dispatch feasibility proof and, only after a
  human checkpoint, an unpromoted minimal-structure execution probe.

## Source-derived endpoint contract (frozen before implementation)

The bridge is an independent solver loop with one versioned policy descriptor.
Its all-PhysBox endpoint must reproduce the existing S11 scalar control before
any hybrid is interpreted. Its all-authority endpoint must qualify against a
fresh native Box2D process before decomposition begins.

| Axis | PhysBox scalar endpoint `P` | Pinned authority endpoint `A` |
| --- | --- | --- |
| X1 preparation / anchors / impulse transport | regenerate contacts, anchors, effective masses, and separation each substep; restore the outer-step prepared impulse seed | collide and prepare once per outer step; use stable authority vertex-pair identities and shared-point anchors; hold normal, anchors, and effective masses fixed; retain evolving impulses; update separation from normalized incremental-rotation transform deltas |
| X2 accumulation / order | stable source-pair contact order used by the existing scalar-soft S11 control | authority constraint-graph colors in ascending order; contacts in a color are body-disjoint and therefore commute; overflow precedes graph colors |
| X3 coefficient schedule | frozen S11 `30 Hz / damping 10 / static×1` soft tuple | pinned `30 Hz / damping 10`, capped at `0.125/h`, with static contacts recomputed at `2×` capped contact Hertz |
| X4 biased / relaxation composition | two biased passes and one no-bias relaxation pass per substep | one biased pass and one no-bias relaxation pass per substep (`ITERATIONS=1`, `RELAX_ITERATIONS=1`) |
| X5 restitution timing | apply restitution after each substep | apply restitution once after all substeps, using the outer-step prepared impact velocity and accumulated collision evidence |

Source clarification: the pinned authority is not globally serial. With one
worker it still visits constraint-graph colors sequentially, while constraints
inside each color are disjoint. “Sequential versus colored” is therefore
recorded as the exact P/A order distinction above, not as a false claim that
Box2D has no coloring.

The authority endpoint additionally preserves the common, non-ablated
contract: semi-implicit gravity integration, unilateral accumulated projection,
normal-then-friction point order, fixed `dt` and substep count, finite `f32`
state, stable geometric feature persistence, maximum restitution material
projection, and exact requested outer-step count. Rolling resistance remains
zero in every governed fixture and is structurally inert; it is recorded but
not an axis. Damping and velocity clamps are zero/inactive in the fixtures and
must be checked inert rather than silently omitted.

## Frozen configuration matrix

Endpoint configurations:

| ID | X1 | X2 | X3 | X4 | X5 | Role |
| --- | --- | --- | --- | --- | --- | --- |
| `P` | P | P | P | P | P | existing PhysBox scalar-soft endpoint |
| `A` | A | A | A | A | A | independent pinned-authority structure endpoint |

Single-axis additions `P+Xi` set only axis `Xi` to A. Single-axis removals
`A-Xi` set only axis `Xi` to P. All ten are mandatory and measured even if an
earlier hybrid passes; this is a decomposition, not first-winner selection.

Conditional interaction IDs `I1`–`I3` are the only additional slots. Each
sets exactly two axes to A on P. Routing is deterministic:

1. Mark `Xi` necessary when `A-Xi` turns a fully qualified A endpoint into a
   failure on either governed stack. Mark it individually sufficient when
   `P+Xi` fully qualifies on both governed stacks.
2. Compute each `P+Xi` change in the ordered failure vector: non-finite,
   structure, authority response, penetration/support, awake speed,
   positive-run, then energy envelope. A change must introduce no earlier
   failure to count as improving; a lower value in the first still-failing
   numerical metric breaks a categorical tie.
3. Route unordered pairs of necessary axes first in fixed lexicographic axis
   order. Fill any remaining slots with pairs containing one necessary and
   one individually improving axis, ranked by the improvement rule and then
   fixed axis order. This ensures a coupled endpoint can route the interaction
   between ingredients whose removals independently break it even when
   neither addition is sufficient alone.
4. If fewer than three pairs qualify, leave remaining slots `not-routed`.
   Do not substitute a three-axis arm, knob, or unranked pair.

This routing can establish a two-axis sufficient subset. If A qualifies but no
single or routed pair does, the bounded conclusion is that stillness requires
at least three authority axes or an unmeasured interaction; S12 reports that
constraint and stops rather than expanding the factorial.

## Frozen fixtures and authority-owned gates

The exact neutral source artifacts are bound from the verified S11.3 packages:

- two-step soft response fixture (short response/control diagnostic);
- permanent awake four-body soft reproducer;
- depth-11 exact minimum rewind signature; and
- governed soft 16-stack.

Every full-length source executes 1,200 exact outer steps with settled window
900–1,200, four substeps, no sleep, and complete per-step observations. The
two-step fixture remains diagnostic under authority ownership; exact equality
to current PhysBox is required only for the independent bridge's P endpoint,
not for A or hybrids.

Endpoint P gate:

- bridge P observations, final bodies, cache, and traces replay exactly;
- bridge P is bit-exact to the existing scalar implementation on every source;
  and
- the known S11 scalar dispositions reproduce, preventing an accidentally
  improved control.

The sources have three explicit roles. The two-step source is a response
diagnostic: it gates the response metrics that its public trace exposes but
not settled contact support, because the native public contact lane begins at
step 900. The permanent mixed pile is required context: it gates finite state,
structure, and the unchanged `0.025 m/s` product ceiling, while its rolling-
circle transient is reported rather than used to introduce a sixth
narrowphase/collision axis. Depth 11 and depth 16 are the governed stacks and
own authority qualification.

Endpoint A gate:

- fresh native Box2D projection, clean pin, settings, public trace, result,
  and exact process replay are retained;
- bridge A replay is exact and finite;
- on each governed stack, structure and final support topology match the
  native authority;
- settled bridge linear and angular-edge maxima are no greater than the fresh
  authority maximum times two plus `2e-5`; this is an authority-relative
  stillness band, not exact response ownership;
- bridge final dynamic center-height sum differs from authority by at most the
  larger of `2e-4 m` and the fresh authority maximum penetration;
- settled bridge maximum penetration is no worse than authority plus
  `2e-4 m`;
- the bridge last/first settled-energy-quartile ratio is no greater than the
  larger of one and the fresh authority ratio, with
  `1e-12 + 1e-5 * authority-ratio` comparison slack, and its positive energy
  run is no greater than the larger of four and the fresh authority run; and
- source/projection identity, requested cadence, body generations, capacity,
  and deterministic replay are exact.

The multiplier, `2e-5`, and `2e-4 m` bands are frozen implementation-agreement
bands for this independent laboratory, not product quality bands. They do not
replace the existing `0.025 m/s` awake product ceiling; all configurations
continue to report that ceiling independently. If A cannot meet its fresh
authority bands, the bridge is not an endpoint and no ablation verdict may be
inferred.

Gate-consistency correction, 2026-07-19, before qualifying evidence: an
unretained diagnostic run exposed that the first draft translated “goes still
against the authority” into two self-contradictory absolute predicates. Native
depth 11 itself has positive-energy run 5 and a rising last/first quartile;
native depth 16 also has a rising quartile. The draft therefore rejected the
authority as non-authoritative. It also attempted all-step penetration
comparison against a native public trace that intentionally exposes contact
records only in the settled window. The authority-relative rules and source
roles above are the explicit resolution of the already-authorized laboratory
ownership decision. They preserve the product ceiling, structure,
penetration, deterministic replay, oracle precedence, and every product
tolerance. No clean S12 endpoint or ablation evidence predates this freeze.

Hybrid disposition uses the same A-owned structure, response, penetration,
support, awake-speed, positive-run, and energy predicates. Every result also
reports deltas from P and A; no average may turn a failing seed or source into
a pass. The four-body fixture is required context but cannot qualify a
structure without depth 11 and depth 16.

## Ordered delivery slices

### S12.1 — fork, source structure, bridge contract, and matrix

- [x] Record the human fork resolution, laboratory response ownership,
  source-derived endpoints, exact bounded matrix, routing, and stop gates.
- [x] Implement the versioned policy descriptor and fail-closed validation
  without adding a product dependency or constructor.
- [x] Record clean reference status and exact source citations.

Artifacts: `evidence/p5.2b-s12.1-authority-structure-contract/`.

Completion gate: tactical and governing topics are committed before the first
bridge measurement; policy tests enumerate exactly P, A, ten single-axis
hybrids, and three conditional slots.

Commit boundaries: documentation contract; policy/bridge scaffold.

### S12.2 — endpoint implementation and qualification

- [x] Implement bridge P independently and prove exact equivalence to the
  existing scalar control on all four sources.
- [x] Implement bridge A from the documented source contract and qualify it
  against fresh, exactly replayed native Box2D results.
- [x] If A fails, diagnose and correct implementation defects inside the
  frozen structure; an unresolved authority mismatch stops without ablations.

Artifacts: `evidence/p5.2b-s12.2-authority-endpoints/`.

Completion gate: P exact, A authority-qualified, every raw observation and
oracle artifact independently verified.

Commit boundaries: endpoint implementation; clean endpoint evidence.

### S12.3 — bidirectional single-axis decomposition

- [x] Run all `P+X1..X5` and `A-X1..X5` configurations on the complete frozen
  source set with exact replay.
- [x] Record necessity, individual sufficiency, failure vectors, P/A deltas,
  and the mechanically derived I1–I3 routes.

Artifacts: `evidence/p5.2b-s12.3-single-axis-decomposition/`.

Completion gate: all ten dispositions and all three route statuses verify;
no interaction result exists yet.

Commit boundary: single-axis evidence and routed interaction freeze.

### S12.4 — bounded interactions and minimal sufficient structure

- [x] Execute only routed I1–I3 configurations; record `not-routed` slots
  explicitly.
- [x] Identify the smallest qualified measured subset, or record the bounded
  lower constraint when no single/pair qualifies despite A qualification.
- [x] Preserve a minimized failing source for each necessary axis whose
  removal changes the endpoint verdict.

Artifacts: `evidence/p5.2b-s12.4-structure-minimization/`.

Completion gate: every routed slot has a disposition and the sufficient-
structure statement follows mechanically from the bounded matrix.

Recorded result: no routed pair qualifies. `A-X5` is the smallest qualified
measured subset, containing X1–X4; because triples are outside the frozen
matrix, the true minimum cardinality is bounded at 3–4. Within the two-source
governed minimization domain, depth 11 is the smallest retained removal
failure for X1, X2, and X3, while depth 16 is the smallest for X4 because
depth 11 `A-X4` qualifies.

Commit boundary: interaction/minimization evidence.

### S12.5 — colored-parallel GPU feasibility decision

- [ ] Map every required ingredient to selected GPU storage, passes,
  synchronization, coloring, dispatch, persistence, and capacity ownership.
- [ ] Report whether the minimal structure is exact, approximated, or
  incompatible; an approximation is not implementable for this decision.
- [ ] Produce a bounded work vector and dispatch/memory delta at 32/256/1K
  without claiming unmeasured runtime.
- [ ] Stop for human review before any unpromoted GPU execution probe or
  product change.

Artifacts: `evidence/p5.2b-s12.5-gpu-feasibility-decision/`.

Completion gate: each required ingredient has an evidence-linked mapping and
the feasibility verdict is verified against the no-readback and portable-
WebGPU contracts.

Commit boundary: feasibility decision only.

### S12.6 — final review

- [ ] Assemble endpoints, ten ablations, routed interactions, minimal
  structure, GPU feasibility, remaining options, and one recommendation.
- [ ] Update this tactical, the governing topics, and tactical index in the
  same change.

Artifacts: `evidence/p5.2b-s12.6-authority-structure-review/`.

Completion gate: a clean-tree verifier recursively checks every input and no
product/default/tolerance/oracle setting changed.

Commit boundary: review and durable status only.

## Autonomous boundary and stop conditions

Autonomous after this authorization: independent scalar code, native clean
oracle runs, exact replays, endpoint corrections that implement the frozen
source contract, all mandatory single-axis runs, mechanically routed
interactions, minimization, evidence/verifier tooling, repository/platform
gates, per-slice commits, and final packages.

Stop and return to the human when:

1. bridge A cannot qualify against native Box2D without changing a frozen
   endpoint, projection, band, or authority interpretation;
2. the required structure lies outside the five axes or needs an unplanned
   third-order/further interaction;
3. invariant and Box2D conclusions conflict;
4. a verdict would require sleeping, tolerance weakening, source copying, a
   new engine, host readback, CPU fallback, or nonportable synchronization;
5. S12.5 reaches the GPU execution/product architecture checkpoint; or
6. any product promotion or physical-default change is proposed.

Ordinary implementation, build, schema, replay, oracle-transport, and evidence
failures are diagnosed autonomously. A failed configuration is evidence, not
a blocker.

## Completion condition

S12 completes when P and A are qualified, all ten mandatory ablations and
every routed interaction have dispositions, the minimal sufficient measured
structure or bounded lower constraint is recorded, and the GPU feasibility
decision and final review verify. Completion never changes the product or
authorizes a comparator. A qualified GPU execution remains a separate human
decision.
