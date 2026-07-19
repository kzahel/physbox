# S14: branch-robust authority-band formulation

Status: complete at the human checkpoint, 2026-07-19. The sole formulation
passes derivation and local controls but fails the frozen scalar robustness
gate at depth 16, so no GPU work is admitted. Authorized by human decision at
the S13.6 checkpoint. This workstream studies exactly one algebraically derived,
cancellation-robust reformulation of the near-zero separation expression
identified from the retained S13.2 divergence. It is a scalar-first decision
study and, only after scalar qualification, an unpromoted portable-GPU probe.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns response quality and the frozen authority bands;
[scalar contact specification](../topics/scalar-contact-specification.md) owns
the independent scalar recurrence and rounding-perturbation gate;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns the
portable colored execution contract; and
[performance and scale](../topics/performance-and-scale.md) owns the unchanged
32/256/1K measurement contract. Predecessor:
[S13 exact authority-structure GPU execution probe](013-authority-structure-gpu-probe.md).

## Human fork resolution

Human decision, 2026-07-19:

- do not promote the exact S13 candidate;
- start from the retained S13.2 divergence trace and identify the exact
  near-zero-separation expression in the biased normal pass that amplifies
  rounding-level differences;
- derive one cancellation-robust reformulation with a written error-bound
  argument, without empirical damping, deadbands, or coefficient sweeps;
- prove authority-band response in the scalar laboratory under a frozen
  rounding-perturbation robustness gate; and
- only after that scalar gate passes, rerun the S13 GPU probe contract
  unchanged: the same authority bands, exact replay, overflow and fail-closed
  capacity fixtures, and 32/256/1K timing matrix.

Stop at the recorded checkpoints. This authorization changes neither the
selected hard/direct manifold-block-i2 product nor any product default,
authority band, oracle pin or precedence, contact coefficient, schedule,
capacity, timing method, or source projection.

## Goal

Determine whether the S12 coupled soft mechanism can be expressed so that
portable binary32 evaluation does not amplify rounding-scale separation error
into a materially different branch and long-run response. A successful result
is one scalar- and GPU-qualified unpromoted candidate for a later human
promotion decision. A clean negative is equally complete when the written
bound cannot be established, the scalar perturbation gate fails, or the
unchanged GPU contract fails.

## Sole candidate and derivation boundary

The sole candidate ID is `authority-band-cancellation-robust-v1`.

S14.2 must first name the exact S13 expression, all operands, binary32 bits,
real-arithmetic identity, branch predicate, and first downstream state delta.
Only then may the candidate replace that expression by one real-arithmetic-
equivalent evaluation whose forward absolute error bound removes the diagnosed
large cancellation terms. The derivation may rearrange additions,
subtractions, dot products, and incremental-rotation displacement math needed
to evaluate the same separation. It may not alter the mathematical separation,
the `separation > 0` predicate, speculative or penetrating equations,
unilateral projection, warm-start/cache state, coefficient schedule, pass or
point order, X1-X4 structure, current X5 timing, or any source.

No second formulation arm is available. A defect in the one derivation may be
corrected to conform to its written identity and bound; choosing another
formula after observing governed response requires a new human decision.

## Written binary32 error-bound contract

Use round-to-nearest binary32 unit roundoff `u = 2^-24` and
`gamma(n) = n*u/(1-n*u)`. The S14.2 derivation must:

1. write the original and proposed real expressions and prove equality;
2. give the operation DAG and count every rounded add, subtract, multiply,
   dot-product component, and incremental-rotation displacement operation;
3. produce a per-evaluation absolute bound `E_sep` from operand magnitudes,
   input representation error, and `gamma(n)`, with no fitted constant;
4. account separately for shader transcendental results as materialized
   binary32 operands rather than claiming unproved exact trigonometry;
5. prove that the proposed bound removes the original absolute anchor-scale
   cancellation term and instead scales with prepared separation plus
   within-step translation/rotation displacement; and
6. validate the bound against an `f64` shadow on every retained localization
   sample while treating a bound violation as a hard derivation failure.

The bound is a diagnostic and perturbation radius, not a tolerance for GPU
response or permission to hide disagreement.

## Frozen scalar rounding-perturbation gate

The gate is frozen before candidate response evidence. At every candidate
separation refresh, compute the round-to-nearest binary32 result `s` and its
written bound `E_sep`, then run these seven complete deterministic scalar
lanes from identical authored state:

1. `nearest`: use `s`;
2. `uniform-low`: use the lower representable endpoint enclosing `s-E_sep`;
3. `uniform-high`: use the upper representable endpoint enclosing `s+E_sep`;
4. `point-alternating`: choose low/high by manifold-plus-point parity;
5. `substep-alternating`: choose low/high by substep parity;
6. `step-alternating`: choose low/high by outer-step parity; and
7. `hashed-sign-v1`: choose low/high from the low bit of
   `step*0x9e3779b9 ^ substep*0x85ebca6b ^ manifold*0xc2b2ae35 ^ point*0x27d4eb2f`.

Endpoints are rounded outward with adjacent-float operations. Perturbation is
applied only to the diagnosed expression result; it does not modify velocity,
impulse, coefficients, anchors, time, or another pass. Every lane must replay
exactly, remain finite and capacity-valid, and pass both unchanged depth-11
and depth-16 S13 authority gates. The two-step source must retain its declared
response invariants and expose the candidate expression in its trace. The
permanent four-body source remains finite/structure/product-ceiling context
and cannot rescue a governed failure.

Passing means robustness under this declared bounded rounding model, not a
proof over arbitrary real input or every possible floating-point program.

## Unchanged authority and GPU contracts

The governed sources remain the exact depth-11 and depth-16 projections:
1,200 outer steps, settled window 900-1,200 inclusive, four substeps, awake,
and unchanged bodies/material/gravity/cadence/generations.

For each source and each required lane, retain all S13 predicates unchanged:

- final structure and support topology match the bound native authority;
- settled maximum linear and angular-edge speed are no greater than twice the
  native maximum plus `2e-5`;
- final dynamic center-height sum differs by no more than the larger of
  `2e-4 m` and native maximum penetration;
- settled maximum penetration is no worse than native plus `2e-4 m`;
- the settled last/first energy-quartile ratio is no greater than the larger
  of one and the native ratio, with `1e-12 + 1e-5*native-ratio` slack;
- settled positive-energy run is no greater than the larger of four and the
  native run; and
- the unchanged `0.025 m/s` product ceiling is reported independently.

If scalar qualification passes, the GPU candidate changes only the diagnosed
expression evaluation. It must retain the S13 candidate's exact X1-X4/common
recurrence/current-X5 descriptor, 20 dynamic plus three static-capable colors,
serial overflow first, ascending in-place colors, candidate-only 12-byte body
transform slice, selected 96-byte prepared-point layout, and non-selectability.

Rerun the exact S13 contracts without alteration:

- primary and exact replay at depth 11 and depth 16 under every authority
  predicate above;
- the 21-manifold star assigning colors 0-19 plus exactly one overflow item at
  lane 23, executed overflow-first;
- manifold capacity 20 against demand 21, rejected with zero accepted and
  committed work; and
- selected control versus candidate at 32/256/1,024 two-point manifolds on one
  named adapter, five warmups, 20 alternating retained samples, timestamped
  GPU complete-step time, separate CPU encode time, exact work/dispatches,
  allocation, replay, and nearest-rank median/p95.

No scalar perturbation lane is substituted for actual GPU arithmetic. The GPU
result must pass the unchanged authority bands directly.

## Ownership and exclusions

- Validation-only scalar and GPU code may read complete state only after a
  bounded final diagnostic run. No result may control the next GPU step.
- Use portable WGSL and bounded existing command boundaries. No CPU physics,
  fallback, subgroup requirement, device-wide synchronization, source copy,
  comparator engine, or native-only arithmetic.
- No damping, separation deadband, hysteresis band, coefficient change,
  coefficient sweep, empirical attenuation, branch override, higher-precision
  product state, source filtering, tolerance change, or approximate selected
  kernel port.
- The product descriptor and selected construction must remain byte-identical
  and must reject the S14 candidate.
- Cross-host candidate qualification and product promotion remain outside S14
  and require the final human decision.

## Evidence and verifier contract

Every package records run ID, source revision/dirty state, candidate and
selected descriptor hashes, source/projection hashes, exact formula version,
operation DAG and error-bound version, perturbation policy, complete settings,
raw outputs, adapter where applicable, capacity, work, and replay. Verifiers
must recompute formula identity samples, outward endpoints, perturbation lane
membership, authority predicates, timing statistics, hashes, and unchanged
S13 contract fields. README assertions are not evidence.

The final verifier recursively invokes all admitted slice verifiers and
identifies exactly one disposition: derivation rejected; scalar robustness
rejected; GPU authority quality rejected; GPU execution constraint; or
qualified branch-robust candidate for later human decision.

## Ordered delivery slices

### S14.1 — fork and frozen study contract

- [x] Record this authorization, sole candidate, derivation/error-bound rule,
  seven-lane perturbation gate, unchanged S13 rerun, exclusions, and stops.
- [x] Materialize a machine-readable contract that rejects formula-arm,
  perturbation-lane, authority-band, overflow/capacity, or timing expansion.
- [x] Prove the selected product and S13 base descriptors are unchanged.

Artifacts: `evidence/p5.2b-s14.1-branch-robust-contract/`.

Completion gate: tactical, index, governing-topic fork, and executable contract
are committed before a candidate expression is implemented.

### S14.2 — retained divergence localization and derivation

- [x] Re-run and retain the S13.2 first-divergence trace with exact operand
  identities, decimal values, binary32 bits, ULP deltas, branch outcomes, and
  the first downstream impulse/body-state difference.
- [x] Write and verify the original expression, candidate identity proof,
  operation DAG, original/candidate forward bounds, and `f64` shadow checks.
- [x] Freeze the one candidate formula/hash before governed scalar execution.

Artifacts: `evidence/p5.2b-s14.2-separation-derivation/`.

Completion gate: the localized expression explains the first S13 branch
amplification and the candidate has a mechanically checked real identity plus
sound retained-sample error bound. Otherwise close at a human checkpoint.

### S14.3 — scalar implementation and local robustness

- [x] Implement the one candidate and seven perturbation policies in the
  independent scalar laboratory only.
- [x] Add analytic, metamorphic, branch, outward-rounding, exact-replay,
  finite-state, and bound-violation controls.
- [x] Prove baseline candidate changes only the derived expression evaluation
  and preserves all other trace identities/settings.

Artifacts: `evidence/p5.2b-s14.3-scalar-implementation/`.

Completion gate: local formula/bound/perturbation controls pass without a
governed response result or second formulation.

### S14.4 — scalar authority and perturbation qualification

- [x] Run all seven lanes at both governed depths, plus the two diagnostic
  sources under their frozen roles, twice for exact replay.
- [x] Retain complete observations, bound high-water, branch counts,
  structures, support topology, and every unchanged authority predicate.
- [x] Admit the GPU port only if every governed lane passes. The verified
  10/14 result rejects admission, so S14.5 and S14.6 are not executed.

Artifacts: `evidence/p5.2b-s14.4-scalar-robustness/`.

Completion gate: all fourteen governed rows pass and replay exactly, or a
verified failure closes S14 at the scalar human checkpoint.

### S14.5 — unchanged GPU response and capacity rerun

Not executed. S14.4 qualified only 10/14 governed scalar rows, so the frozen
scalar-first gate denied GPU admission.

- [ ] Port only the frozen candidate expression to the validation-only GPU
  path and retain focused scalar/GPU expression/phase traces.
- [ ] Run exact primary/replay at depths 11 and 16 against unchanged bands.
- [ ] Re-run the unchanged 21-manifold overflow and one-below-demand fail-
  closed capacity fixtures.

Artifacts: `evidence/p5.2b-s14.5-gpu-qualification/`.

Completion gate: focused implementation, both governed rows, overflow-first,
fail-closed zero-work rejection, selected-inertness, and replay all verify, or
the exact failing disposition is retained.

### S14.6 — unchanged 32/256/1K work and timing rerun

Not executed. Timing was conditional on scalar qualification and a valid GPU
response lane; neither condition was met.

- [ ] Measure the selected control and S14 candidate under the exact S13
  source, sample-order, timestamp, work, allocation, and statistic contract.
- [ ] Classify execution constraints separately from completing cost and
  reject timing claims for any quality/capacity-invalid row.

Artifacts: `evidence/p5.2b-s14.6-gpu-work-time/`.

Completion gate: six cells retain exact work/allocation and 20 valid samples,
or a reproducible hard execution failure is retained.

### S14.7 — decision package and human checkpoint

- [x] Recursively verify every admitted slice and publish exactly one final
  disposition plus one recommendation.
- [x] Update this tactical, tactical index, and governing topics in the same
  change.
- [x] Stop before cross-host candidate qualification or product promotion.

Artifacts: `evidence/p5.2b-s14.7-branch-robust-review/`.

Completion gate: a clean-tree recursive package reaches the human checkpoint
without changing a frozen authority, product, or S13 execution contract.

## Autonomous boundary and stop conditions

Autonomous after authorization: exact tracing, scalar derivation and proof
tooling, one candidate implementation correction, frozen perturbation runs,
qualified GPU port, unchanged S13 reruns, evidence/verifiers, per-slice commits,
and repository/platform gates.

Stop and return to the human when:

1. no real-equivalent cancellation-robust expression with the written bound
   can explain the retained divergence;
2. any governed scalar perturbation lane fails after implementation defects
   are exhausted;
3. GPU qualification fails after exact implementation defects are exhausted;
4. a frozen source, band, perturbation policy, coefficient, branch, schedule,
   capacity fixture, or timing contract would have to change;
5. portable execution requires CPU feedback, a fallback, an unbounded pass,
   source copying, or nonportable arithmetic; or
6. S14.7 reaches the final human checkpoint.

## Completion condition

S14 completes when the divergence expression and written bound have a verified
disposition, scalar robustness has a verified disposition, every admitted GPU
contract is rerun unchanged, all evidence recursively verifies, and the study
stops at the required human checkpoint. Completion alone authorizes neither
cross-host qualification nor product/default promotion.
