# S9: awake settling schedule differential

Status: active, 2026-07-18. Authorized by human review of the S7.6 checkpoint.
The review broadened the recommended standalone static-late factorial into this
bounded program of schedule-differential experiments against the pinned Box2D
authority. Every arm is a named unpromoted laboratory policy. The product
remains hard/direct manifold-block i2 with the selected bounded parallel
scheduler for the entire workstream; even an unambiguous winner returns to
human review before promotion.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns settling quality gates and the frozen measurement contract;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns
scheduling/order ownership and its promotion rules;
[scalar contact specification](../topics/scalar-contact-specification.md) owns
stage math;
[performance and scale](../topics/performance-and-scale.md) owns timing rows;
[Box3D planar stability reference](../topics/box3d-planar-stability-reference.md)
records the static-late arm's provenance.

## Goal

Determine which solver-schedule departures from the pinned Box2D v3 authority
sustain PhysBox's awake settling residual at stack depth, then measure a
predeclared set of bounded, unpromoted arms until one is an unambiguous
quality winner or the set is exhausted. Every arm — surviving or discarded —
receives a dated checkpoint in this tactical and a verified evidence package.
A negative result over the whole set is a valid completion.

## Verified schedule differential (2026-07-18)

Source-verified departures of the selected product from pinned Box2D
`3a4f0da...`. PhysBox references are the product path
`crates/physbox-engine/src/body_floor_solver.rs:3842-4018` and
`body_floor_solver.wgsl`; Box2D references are the clean `reference/box2d`
checkout.

| Concern | Box2D v3 authority | Selected PhysBox product | Tested? |
| --- | --- | --- | --- |
| normal/tangent staging | fused per contact inside one colored solve kernel: normal points then friction, friction bound uses the just-updated accumulated normal impulse (`src/contact_solver.c:281-367` scalar, `1837-2021` SIMD) | tangent is a separate global color sweep after all colors' normals, in biased (`body_floor_solver.rs:3907`) and relaxation (`:3982`) stages | never — named in the S6.5d checklist, only relaxation arms ran |
| static-contact priority | static contacts colored from the high end, executed late (`src/constraint_graph.c:113-130`) | no static class in coloring (`body_floor_solver.wgsl:205-398`) | never — the S7.6 recommendation |
| static-contact stiffness | `staticSoftness = b2MakeSoft(2 * contactHertz, ...)` (`src/physics_world.c:781`) | none under hard (bias_rate `0.2` for all contacts); the 2x multiplier exists only in the disabled soft policy | never under the hard formulation |
| iterations per substep | one biased + one relax (`src/solver.c:27-28`) | two biased + two relax; i1 loses the 16-stack | symptom, not an arm: PhysBox's per-sweep operator is weaker |
| substep Jacobian | anchors/effective mass fixed at prepare; separation updated analytically from accumulated delta transforms (`src/contact_solver.c:1608-1616`, `1849-1850`) | full per-substep prepare/refresh recomputing anchors, effective mass, separation (`body_floor_solver.rs:3851`, `:3932`) | never |
| restitution timing | once after all substeps (`src/solver.c:1182-1196`) | every substep (`body_floor_solver.rs:4001`, inside the loop closing at `:4018`) | never; inert for resting stacks under the `1.0 m/s` eligibility threshold |

Fresh S9.1 S6.2-v2 baselines are selected-product settled max linear speed
`0.03873 / 0.17960 / 0.70853 m/s` on the 4/8/16 stacks, with 16-stack last-
quartile energy `0.33661`; serial control is `0.70533 / 1.77033` on the
16-stack, and pinned Box2D final speed is `0.000158 m/s`. The initial
authorization's `0.03652 / 0.12402 / 0.44197` orientation row and `0.54254`
energy were historical S5.4c values at source `62f2fa1`, not the later clean
S6.4/S6.5 current-product record. S9.1 corrects that attribution with fresh
current-source evidence; no product setting or acceptance band changed. The
authority gap remains roughly three orders of magnitude.

## Arms and rationale

- **E1 — per-pass energy attribution, 16-stack, hard/direct (diagnostic).**
  The S6.5b/S6.5d matched-precision instrument has only ever run on the
  permanent four-body reproducer, and only against soft candidates. Which pass
  family sustains the hard product's 16-stack limit cycle has never been
  measured. The S6.5d closing decision already requires 16-stack per-pass
  attribution (biased-normal, tangent, integration, support-transfer
  boundaries) before any future formulation work. E1 routes the remaining
  arms; it changes nothing.
- **E2 — static-late color partition.** The S7.6-recommended arm, verbatim:
  static contacts occupy a disjoint late-executing partition; conflict freedom
  preserved; fail closed above 32-color capacity; schedule identity exposed;
  all physical settings fixed.
- **E3 — contact-major fused normal+tangent.** Inside each colored solve
  dispatch, each manifold solves its block normals then its tangent points in
  one kernel, with the friction bound reading the just-updated accumulated
  normal impulse; applied to biased and relaxation sweeps. This matches how
  both quiet references execute. Every failing PhysBox arm ever run — hard or
  soft, point or block, serial or parallel — shared the global-tangent-sweep
  staging, and it is the leading untested candidate for why the authority
  recurrence destabilizes under the PhysBox schedule and why PhysBox needs i2
  where the authority needs i1. Equations unchanged; only order changes.
  Side effect to record explicitly: per-iteration colored dispatch count
  halves.
- **E4 — static-stiff-hard.** Static contacts use `bias_rate = 0.4` (2x the
  dynamic `0.2`), all other coefficients fixed — the hard-formulation analog
  of the authority's 2x static contact hertz. One predeclared value; no
  coefficient sweep. This is an explicit unpromoted physical-policy
  experiment. Note the authority pairs static-late order *with* static
  stiffening; E2 alone may be under-powered by construction.
- **E5 — fixed-anchor substep refresh (conditional).** Authority-style:
  prepare once per outer step; per-substep separation updated analytically
  from accumulated delta position/rotation; no per-substep effective-mass
  recompute. Activated only if E1 routing implicates refresh/integration
  boundaries, or if E2/E3/E4 and their combinations all miss the
  material-effect gate.
- **E6 — restitution-after-substeps (conditional hygiene).** Move restitution
  outside the substep loop to match the authority. Must be byte-inert on the
  resting-stack corpus (restitution-ineligible there); evaluated on the S6.2
  drop fixtures against fresh authority rows. Run only after a survivor
  exists or all primary arms are discarded; it cannot gate the program.

Predeclared combinations, at most three, drawn from material survivors in this
preference order: E2+E4 (the authority's actual static handling), E3+E2,
E3+E2+E4. A combination survives only if it beats its best component on
16-stack settled max speed and energy.

## Exclusions and ownership constraints

- No product default, tolerance, quality-band, oracle-precedence, or
  capacity-policy change anywhere in this workstream. Every arm is a named
  laboratory execution/policy selected only by validation constructors;
  product constructors stay on the selected hard/direct manifold-block i2 and
  selected scheduling.
- No sleeping, no tolerance widening, no baseline refresh to a worse result,
  no structural-gate weakening, no averaging away a failed seed.
- No new soft-contact recurrence, soft-coefficient retune, or reopening of the
  S6.5 soft direction. E4's single declared value is the only physical-policy
  variation. If E1 attribution shows the injector requires formulation-level
  change, stop for human review instead of deriving one.
- The solver family stays manifold-block normals with warm start and
  relaxation. No CPU fallback, no readback in the interactive frame path, no
  broadphase, pacing, liquid, or product-feature work.
- 32-color capacity remains fail-closed; arms must preserve the atomic
  rejection semantics and the existing dispatch-form (per-color indirect vs
  workgroup-per-island) body/cache/work equality tests, extended to each arm.
- Scenario/action/completion definitions stay in shared Rust; launch names
  belong to `physbox-launch`; no host-authored physics.
- Fresh pinned Box2D rows are required wherever an authority comparison is
  claimed. No Box3D code is copied; Box3D remains a source reference only.

## Frozen measurement and gate contract

All quality runs use the frozen S6.2 measurement policy v2: 1,200 exact steps,
settled window 900–1200, rest bands `0.025 m/s` linear and angular-edge,
energy epsilons `1e-12` absolute / `1e-5` relative, positive-energy run limit
5, the existing stable-support and unexplained-churn definitions, exact replay,
and capacity cleanliness.

Corpus: 4/8/16 centered stacks, the permanent four-dynamic-body reproducer,
the seven frozen S6.2 fixtures, 32/64-stack diagnostics (first-loss step
recorded, not an accepted band), and both 100-body buckets.

Three gate tiers, applied in order:

1. **Implementation gate** (per arm, before the corpus): exact replay twice;
   capacity fail-closed; schedule-identity attestation — work-vector digests
   must differ exactly where the arm claims an order change and be
   byte-identical where it claims none (E3 negative control: a single-manifold
   single-color fixture is byte-identical between staged and fused forms;
   positive control: a multi-contact chain differs); no readback; full
   repository gates below.
2. **Material-effect gate** (per arm, vs the S9.1 re-pinned selected-product
   baseline): no structural loss on any accepted structure or S6.2 fixture; no
   replay, capacity, or diagnostic first-loss regression on the 32/64 stacks;
   at least 25% lower settled max linear speed AND settled energy on two of
   4/8/16 including the 16-stack; at most 5% regression on the remaining
   stack; no worse 16-stack energy trend (positive-run max and quartile
   envelope). Miss → discard, record, next arm.
3. **Winner gate** ("unambiguous win"; produces a promotion recommendation,
   never a promotion): the full awake completion gate — every accepted
   structure and S6.2 fixture preserves structure; settled max linear and
   angular-edge speed at most `0.025 m/s` on all of 4/8/16 and all fixtures;
   zero unexplained churn in stable-support intervals; energy envelope
   non-rising with positive runs under 5 everywhere including the permanent
   reproducer; penetration, separation, drift, height, friction, restitution,
   and first-impact bands preserved; Metal complete-step P50 at 256 and 1K
   within +5% of the pinned baseline (improvements recorded); cross-host
   structure held through step 1,200 on Metal, Chrome WebGPU, the
   hardware-backed AVD, and available physical Android, with the AVD permanent
   reproducer keeping positive runs under 5. Unavailable hardware is recorded
   as a gap, not inferred as a pass.

The serial full-rebuild lane remains an explicit comparison control and may
not be silently substituted for the selected baseline.

Repository gates for every code slice:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown -p physbox-web
cargo check --target aarch64-linux-android -p physbox-android
cargo run -p physbox-headless -- --output /tmp/physbox5-smoke.png --report /tmp/physbox5-smoke.json --scenario bootstrap
npm run check:boundaries
npm run test:web
```

Every evidence lane emits `PHYSBOX5_VALIDATION_SUCCESS` with a run id and
fresh provenance (source revision, dirty state, host, capture phase, scene
contract) and ships a `verify.mjs`.

## Recording discipline

Every arm and combination — including every discarded one — gets: a dated
checkpoint paragraph in this tactical stating the disposition and the numbers
that decided it; a verified package under `evidence/p5.2b-s9.*`; and checkbox
updates in the same change. Interim states are committed per slice with
conventional-commit subjects. No result may exist only in a commit message or
session memory.

## Ordered delivery slices

### S9.1 — program freeze and baseline re-pin

- [x] Freeze this contract: arms, gates, routing rules, combination cap, and
  evidence naming, before any measurement.
- [x] Re-run and pin the selected product baseline (hard/direct, selected
  scheduling) over the full corpus with fresh provenance; retain the serial
  control lane.
- [x] Produce fresh process-isolated pinned Box2D authority rows for the same
  corpus.

Outcome: one pinned baseline every arm compares against; no stale-number
comparisons.

Artifacts: `evidence/p5.2b-s9.1-baseline-repin/`.

Completion gate: baseline replays exactly, matches the frozen-record values
within measurement policy, and the authority rows are fresh and clean.

Commit boundary: tactical/topic authorization records; baseline evidence.

Program-freeze checkpoint, 2026-07-18: commit `d65c345` records the complete
arm set, routing rules, combination cap, gates, evidence naming, autonomous
boundary, and human stop conditions before any S9 measurement. The dedicated
S9.1 runner keeps the product-selected and serial-control schedules distinct,
uses the frozen S6.2 v2 observation policy, and runs fresh process-isolated
Box2D rows over the same governed corpus. No product constructor, physical
default, tolerance, or authority setting changes in the re-pin.

Baseline checkpoint, 2026-07-18: the
[verified S9.1 package](../../evidence/p5.2b-s9.1-baseline-repin/README.md)
at clean source `211395f` pins all 15 governed scenes. All 13 accepted
selected-product, per-color-control where applicable, and serial-control lanes
preserve structure, replay exactly, finish capacity-clean, and match their
final-only body/manifold/cache state. The selected 4/8/16 v2 residuals are
`0.03872548 / 0.17960109 / 0.70852673 m/s`; 16-stack last-quartile energy is
`0.3366095`, versus serial `1.7703259`. The 32/64 diagnostic first losses stay
at steps 29/31. Fresh clean pinned-Box2D final speeds are
`0.0000015425 / 0.0000027144 / 0.0001575757 m/s`, and every authority final
replay is exact. The package reconciles the stale S5.4c orientation prose
against the already-established later S6.4/S6.5 hard-control record rather
than weakening a gate or accepting a new regression.

### S9.2 — E1 matched per-pass energy attribution at depth

- [x] Extend the S6.5b/S6.5d version-3 matched-precision instrument from the
  four-body reproducer to the governed 16-stack under the product hard/direct
  policy, with the 8-stack as a control lane.
- [x] Capture a settled step (step 900; add 1000/1100 if one step is
  ambiguous) at every logical boundary: integrate-velocity, prepare/refresh,
  warm start, each biased normal sweep, each biased tangent sweep,
  integrate-position, post-integration refresh, each relaxation normal and
  tangent sweep, restitution. Strict ordered-`f32` and compensated energy
  accounts, scalar-mirror agreement within the S6.5b comparison contract, GPU
  command order preserved, one map after the target step.
- [x] Publish the ranked injector table and the routing decision.

Routing rules (predeclared; routing orders the arms and activates
conditionals, it does not veto E2/E3, which run regardless):

- tangent passes (biased or relax) are material injectors → E3 first;
- biased-normal injection concentrated in colors carrying static support →
  E2/E4 first;
- refresh or integration boundaries are material injectors → activate E5;
- the injector is attributable to no predeclared arm and would require a
  formulation change → stop for human review.

Outcome: the 16-stack injector is localized under the product formulation for
the first time; arm order is evidence-based rather than guessed.

Artifacts: `evidence/p5.2b-s9.2-pass-attribution/`.

Completion gate: GPU/scalar boundary agreement within the declared bounds; a
signed routing decision recorded here and in the package.

Commit boundaries: instrument extension; clean attribution evidence.

Attribution checkpoint, 2026-07-18: the
[verified S9.2 package](../../evidence/p5.2b-s9.2-pass-attribution/README.md)
extends the version-3 direct-over-compacted-color-capacity trace to clean
hard/direct centered 8- and 16-stacks at settled steps 900, 1000, and 1100.
All six traces preserve the 56-boundary/57-state GPU command order and map
once after the target step. GPU/scalar maximum absolute state difference is
at most `4.66598e-7` against the frozen `2e-5` contract, and compensated
static/dynamic support accounting closes within `9.29471e-9`.

Warm start is the only material non-gravity injecting family at all three
16-stack phases: `+3.02338 / +0.148891 / +0.708266` compensated kinetic-energy
units at steps 900/1000/1100. Biased normal and relaxation are net sinks;
tangent positive energy and refresh/non-gravity-integration positive energy
are zero. The signed route is E2 static-late first because it changes colored
support-transfer order including warm start, then mandatory E3 fused
normal+tangent, then E4 static-stiff-hard. E5 is not activated, and the
localized scheduled warm-start transfer does not require a formulation-level
stop. No product/default/tolerance/capacity/oracle contract changed.

### S9.3 — arm implementations as named unpromoted policies

- [x] E2 static-late partition (constructor-selected policy; conflict-freedom
  invariant test; fail-closed capacity test; schedule identity exposed).
- [x] E3 contact-major fused normal+tangent for biased and relaxation sweeps
  (single-manifold byte-identity negative control; multi-contact divergence
  positive control; dispatch-count delta recorded).
- [x] E4 static-stiff-hard `bias_rate = 0.4` for static contacts (policy
  attested in scene contract; no other coefficient touched).
- [x] Conditional E5 implemented only after its primary-arm-exhaustion trigger
  fired.
- [ ] Conditional E6 implemented only after its hygiene trigger fired.
- [x] Each active arm passes the implementation gate before entering S9.4; both
  dispatch forms (per-color indirect, workgroup-per-island) remain exact per
  the existing equality tests.

Outcome: every arm is selectable, attested, reversible, and product-inert.

Artifacts: per-arm implementation notes inside the S9.4 packages.

Completion gate: implementation gate green per arm; product constructors
unchanged (negative test).

Commit boundary: one commit per arm.

E2 implementation checkpoint, 2026-07-18: the
[implementation note](../../evidence/p5.2b-s9.4-arm-static-late/IMPLEMENTATION.md)
records the named `full-rebuild-parallel-speculative-static-late` laboratory
policy. Dynamic/dynamic contacts retain the selected candidate order inside
colors 0–27; static contacts occupy the disjoint late colors 28–31. Both
partitions preserve shared-body conflict freedom and use the existing atomic
solver-color rejection on exhaustion. The per-color and island dispatch forms
produce exact bodies, contacts, manifolds, cache, point-color assignments, and
committed work on the centered 8-stack; a dynamic-only chain is byte-identical
to selected scheduling, while the 29th mutually conflicting dynamic constraint
and fifth mutually conflicting static constraint fail their disjoint
partitions closed. The full repository gate and visually inspected Metal/WebGPU
smokes pass. Product constructors still select only `SELECTED` or
`SELECTED_SMALL_WORLD`; E2 has not entered the product or the S9.4 quality
matrix yet.

E3 implementation checkpoint, 2026-07-18: the
[implementation note](../../evidence/p5.2b-s9.4-arm-fused-normal-tangent/IMPLEMENTATION.md)
records the named per-color and workgroup-per-island fused policies. Biased
stage 7 and relaxation stage 8 solve each manifold's block normals followed
immediately by its tangent points, while keeping selected colors and their
order unchanged. A single-manifold/single-color fixture is byte-identical to
the staged policy; an eight-manifold chain preserves assignment and compacted
order but diverges in final pose/motion bytes. The centered hard/direct
8-stack replays exactly and matches across both dispatch forms. Fusion removes
512 per-color or 16 island launches per outer step, and soft/non-block/no-relax
construction fails closed. The full repository gate and visually inspected
Metal/WebGPU/AVD smokes pass. Product constructors still select only
`SELECTED` or `SELECTED_SMALL_WORLD`; E3 has not entered the product or the
S9.4 quality matrix yet.

E4 implementation checkpoint, 2026-07-18: the
[implementation note](../../evidence/p5.2b-s9.4-arm-static-stiff-hard/IMPLEMENTATION.md)
records the separate `static-stiff-hard` coefficient policy. Dynamic contacts
retain `[bias, mass, impulse] = [0.2, 1, 0]`; contacts with a static endpoint
read exactly `[0.4, 1, 0]`. A parameter-word test proves no other coefficient
or control word changes. A dynamic-only chain is byte-identical, while the
centered hard/direct 8-stack changes body state with selected scheduler
digests intact, replays exactly, and matches across per-color and island
dispatch. The 32-color failure remains atomic and soft construction fails
closed. Scene contract version 5 adds coefficient-policy identity, while all
product constructors attest only `settings-derived`. The full repository gate
and visually inspected Metal/WebGPU/AVD smokes pass. E4 has not entered the
product or the S9.4 quality matrix yet.

E5 implementation checkpoint, 2026-07-18: the
[implementation note](../../evidence/p5.2b-s9.4-arm-fixed-anchor-substep-refresh/IMPLEMENTATION.md)
records the named `fixed-anchor-analytical-separation` policy. It prepares once
per outer step, freezes anchors and effective masses, and refreshes only
separation from current poses and rotated local anchors after each position
integration. This removes the three inter-substep refresh dispatches without
adding storage or changing capacity. The centered hard/direct 8-stack changes
body state, replays exactly, preserves selected scheduler digests, and matches
between per-color and island dispatch. Product constructors and output attest
only `refresh-anchors-and-mass`; soft or non-direct construction fails closed.
The full repository gate and visually inspected Metal/WebGPU/AVD smokes pass.
E5 has not entered the S9.4 quality matrix yet.

### S9.4 — Metal quality matrix and combinations

- [ ] Run each active arm over the full corpus; apply the material-effect
  gate; record survive/discard with numbers, win or lose.
- [ ] Run at most three predeclared combinations among material survivors;
  apply the combination rule.
- [ ] Minimize and preserve any new failure the way S6 did (permanent
  reproducers, exact first-loss steps).

Outcome: a ranked table of arms with dispositions; discarded arms are
permanent recorded evidence, not silence.

Artifacts: `evidence/p5.2b-s9.4-arm-<name>/` per arm and combination.

Completion gate: every active arm and combination has a disposition
checkpoint in this tactical.

Commit boundaries: one evidence commit per arm/combination.

S9.4 runner checkpoint, 2026-07-18: the S9 evidence runner now admits exactly
E2, E3, E4, triggered E5, and the three predeclared compositions. It selects
the already-proved island dispatch form through 64 bodies and the scalable
per-color form
above that boundary, while retaining the arm's scheduling, coefficient, and
substep-contact identity. Each clean run covers all 15 frozen scenes, captures
a per-step series and exact replay, checks the final-only
body/manifold/cache state, and
computes every material-gate clause directly against the hashed clean S9.1
manifest. For the previously prose-only 16-stack trend clause,
"nonregressing positive run" means no longer than S9.1 and "nonrising
quartile envelope" uses the existing S6.2 absolute/relative epsilon on the
candidate's first-to-last settled quartiles. Partial-corpus evidence fails
closed. Every first structural loss also publishes the exact shortest failing
time prefix and any atomic execution failure, so a discarded arm cannot leave
its reproducer only in a trace. The runner changes no product constructor or
solver policy.

E2 quality checkpoint, 2026-07-18: the
[verified static-late package](../../evidence/p5.2b-s9.4-arm-static-late/README.md)
discards E2. It improves 4-stack speed/energy by `99.36% / ~100%` and 8-stack
by `13.79% / 56.72%`, but 16-stack speed improves only `22.05%` while last-
quartile energy regresses `62.11%` to `0.5456647` and its quartile envelope
rises. Only one stack meets the paired 25% threshold. The 32-stack first loss
regresses from step 29 to 23. More decisively, the disjoint four-color static
partition atomically exhausts the unchanged 32-color capacity on the accepted
small-mixed-pile fixture at step 52 and on the two 100-body buckets at steps
2/5. Exact structural-loss and execution-failure prefixes are preserved; the
existing fifth-conflicting-static-contact test is the body/contact-minimal
capacity reproducer. All series replay exactly and match final-only state.
E2 is not a material survivor and therefore cannot participate in any of the
three combinations, all of which require E2.

E3 quality checkpoint, 2026-07-18: the
[verified fused package](../../evidence/p5.2b-s9.4-arm-fused-normal-tangent/README.md)
discards E3. Fusion is exact-replay and capacity-clean, and delays the 32/64
diagnostic first losses from steps `29/31` to `281/112`. It nevertheless loses
the accepted centered 16-stack structure at step 17. No stack meets the paired
25% threshold: 8-stack speed/energy regress `7.12% / 38.37%`, while 16-stack
speed improves `18.41%` but last-quartile energy regresses `13.61%` to
`0.3824171` and its envelope rises. The step-17 prefix is preserved with no
execution or capacity failure. E3 is not a material survivor and cannot enter
either E3 combination. E4 remains the last independent primary arm.

E4 quality checkpoint, 2026-07-18: the
[verified static-stiff package](../../evidence/p5.2b-s9.4-arm-static-stiff-hard/README.md)
discards E4. Every accepted structure, replay, capacity, and both diagnostic
first-loss clauses pass. No stack clears the paired 25% threshold: 4-stack
speed/energy improve `6.14% / 17.37%`; 8-stack energy improves `41.39%` but
speed regresses `27.28%`; and 16-stack speed improves `13.30%` while energy
regresses `30.59%` to `0.43956766`. The 16-stack trend clauses pass but cannot
override the magnitude failures. The permanent reproducer's last-quartile
energy also rises from `0.0004200171` to `0.005612511`, with a 139-interval
positive run. E4 is not a material survivor. All primary arms are now
discarded; no E2-containing combination is eligible, the conditional E5
trigger fires, and E6 hygiene must follow primary-arm exhaustion.

### S9.5 — survivor cost, cross-host, winner determination

- [ ] For the best at most two material survivors by 16-stack settled energy:
  Metal 256/1K complete-step timing (plus 1K/10K authored rows if the arm
  changed scheduling shape), Chrome WebGPU, hardware-backed AVD, and available
  physical Android structure runs.
- [ ] One AVD matched-boundary overlay for the best arm if any rising-energy
  mode remains.
- [ ] Apply the winner gate.

Outcome: a survivor is either a winner-gate promotion candidate or a
recorded near-miss with the exact failed clause.

Artifacts: `evidence/p5.2b-s9.5-survivor-<name>/`.

Completion gate: winner-gate clauses individually recorded pass/fail; no
inferred platform result.

Commit boundary: survivor evidence separate from arm evidence.

### S9.6 — review package (always runs)

- [ ] Assemble the compact decision package: pinned baseline, attribution
  tables, every arm and combination disposition with links (including
  discarded), costs, cross-host matrix, and a specific recommendation —
  promote-candidate, close the schedule-differential direction, or one named
  follow-up.
- [ ] Update this tactical, the governing topics, and the tactical index in
  the same change.

Outcome: a reviewer can decide promotion or closure without reading the
engines.

Artifacts: `evidence/p5.2b-s9.6-schedule-differential-review/`.

Completion gate: package verifies from a clean tree; every claim links to raw
evidence; no product or authority setting changed.

Commit boundary: review package and status updates only.

## Autonomous boundary and stop conditions

Autonomous after this authorization: instrumentation, arm implementation as
unpromoted policies, corpus runs, combination runs, failure minimization,
exact replay, oracle rows, cross-host validation, performance accounting,
discard decisions strictly by the predeclared gates, per-slice commits, and
the final package.

Check back with the human only when:

1. the winner gate passes — promotion is always a human decision;
2. E1 routing lands on "formulation-level change required" — do not derive a
   new recurrence, coefficient set (beyond E4's declared value), or
   accumulation architecture;
3. an arm result genuinely conflicts with a scalar invariant or the pinned
   Box2D authority — preserve the reproducer and stop;
4. a gate could only be met by weakening a frozen band, tolerance, structural
   contract, or capacity rule;
5. the last surviving arm shows a cross-host contradiction the gates cannot
   rank (a non-final arm with such a contradiction is recorded as
   promotion-blocked, like S6.4, and the program continues); or
6. a required gate cannot run without changing product code paths.

Ordinary build, harness, schema, determinism, or evidence failures are
diagnosed and minimized autonomously without weakening the contract. Do not
stop merely because results are negative; exhaustion with a complete record
is a designed outcome.

## Completion condition

The workstream is complete when either a winner-gate survivor is packaged for
human promotion review, or every predeclared arm and eligible combination has
a recorded disposition and the negative-result package is returned. Every
intermediate and discarded result is preserved in evidence and in this
document. Completion never changes product defaults, quality bands, oracle
precedence, or authority interpretation.
