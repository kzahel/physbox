# S18: solver check loop and coefficient exploration

Status: complete; the fast check loop and validation-only persistent-key fix
pass, the 30-point sweep has no stands-quiet-valid survivor, the measured
hard/direct residual band is accepted for sleeping design, and a later
explicit human request opened an interactive comparison surface,
2026-07-21.

This workstream is deliberately different in character from S9-S17. Human
direction (2026-07-20): the study-chain method is retired. S18 is a normal
engineering loop — build a fast scoreboard tool, fix one named bug, sweep one
coefficient space — with a pre-committed stop. Its failure mode is closure,
not a successor study. Do not create an S19 formulation study out of this
document.

Governing topics:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns the residual-band decision this workstream can trigger;
[contact solver parallelism](../topics/contact-solver-parallelism.md) owns
execution/capacity contracts;
[performance and scale](../topics/performance-and-scale.md) owns any cost
claim that leaves this document.

## Context (read this before the code)

The S17 branch-robust candidate is still and cheap but structurally broken.
Fresh Metal head-to-head against the selected product
(`evidence/p5.2b-s17.4-metal-product-relative/README.md`):

- settled 16-stack speed: selected `0.824 m/s` (16x above Box2D's `0.05 m/s`
  sleep threshold — the product's deep stacks visibly rattle and can never
  sleep) versus candidate `0.0077 m/s` (3x under the `0.025 m/s` awake gate);
- complete-step GPU cost: candidate 25-60% cheaper at 32/256/1K manifolds;
- but: 16-stack collapses at step 407; nine scenes fail geometry margins
  (sag/drift — the soft trade); and the rectangle bucket corrupts its
  persistent contact keys (duplicate/unsorted) at exact step 11, the 32/64
  stacks at 300/134. Note the candidate holds 32/64 *structure* much longer
  than selected (225/129 vs 29/31) before the key corruption kills it.

Two named defects, two different characters: geometric softness (physics
trade, attack with coefficients) and persistent-key corruption (data-structure
invariant violation, almost certainly an implementation bug, attack with a
debugger). The S17 coefficients 30 Hz / damping 10 / static x2 are Box2D's
defaults and were frozen through every prior study; nobody has ever swept
them.

## Starting point

A laboratory-only tunable-coefficient lane on the exact branch-robust GPU
execution. All in `crates/physbox-engine`, product paths untouched:

- `GpuContactCoefficientPolicyV1::AuthorityTunableSoft`
  (`body_floor_solver.rs`, enum near line 62): validates soft contact +
  four substeps, no 30/10 pin; `pass_params` does *not* force the static x2
  override for it, so the scenario's own
  `static_contact_hertz_multiplier` is honored.
- `validate_branch_robust_tunable_settings` and
  `validate_branch_robust_tunable_runtime_tuple`
  (`branch_robust_gpu_promotion.rs`).
- `GpuGeneratedContactGraph::new_branch_robust_tunable`
  (`generated_contacts.rs`), routed in both the observation-series path and
  the pass-trace path; the series validator owns the tunable case explicitly
  and the frozen S17 `AUTHORIZED` descriptor is *not* attested on tunable
  series.
- Public entry point:
  `GpuGeneratedContactRunner::run_branch_robust_tunable_observation_series_with_submission_batch(scenario, observation_interval_steps, completed_steps, submission_batch_steps)`.
  Coefficients ride on `scenario.settings.contact_softness`
  (`ContactSoftnessPolicyV1 { contact_hertz, damping_ratio, static_contact_hertz_multiplier }`);
  set it on the scenario before running (pattern: `candidate_softness()` in
  `physbox-s17-product-relative.rs`).
- `report.rs` compact encoding maps the new variant to `3`.

At handoff, `cargo check -p physbox-engine` and `cargo clippy -p
physbox-engine --lib -- -D warnings` passed with and without `--features
production-validation`. The lane had not yet been executed on a GPU and had
no test. It is now exercised by the S18 check loop and the focused
authority-key regression below.

Coefficient math you must know before sweeping
(`gpu_contact_softness_coefficients_validated`, `body_floor_solver.rs:329`):
dynamic hertz is capped at `0.125 / substep_dt` (= 30 Hz at 1/60 dt, 4
substeps) *before* the static multiplier, so raising `--hz` above 30 is
inert for dynamic contacts, while `static_scale` multiplies the capped value
without a cap. `bias_rate = omega / (2*damping + substep_dt*omega)`, so
*lowering* damping stiffens position correction (damping 10 -> 2 is roughly
4x stiffer). Real dials: `hz <= 30` (down only), `damping` (both ways),
`static_scale`. `max_depenetration_velocity` in the contact policy is a
further untouched lever.

## Slices

### S18.1 — `physbox-check` scoreboard binary

- [x] Add `crates/physbox-lab/src/bin/physbox-check.rs`: run a small decisive
  corpus through both solvers and print a pass/fail scoreboard in seconds.
  No run ids, manifests, provenance, or evidence directories — this is an
  inner-loop dev tool, not an evidence lane.

Design (validated against existing APIs):

- Scenes: stacks 4/8/16 via
  `p5_2_centered_rectangle_stack_scenario(n, 1200)` (`scenario.rs:467`;
  legal n: 2, 4, 8..=16, 32, 64) and the rectangle bucket via
  `p5_2_bucket_scenario(shape, 100, 600)` (`scenario.rs:862`). Optional
  `stack32` and `stack64` diagnostics.
- Lanes: selected =
  `run_selected_observation_series_with_submission_batch(&scenario, 1, steps, 64)`;
  candidate = the tunable entry point above with
  `scenario.settings.contact_softness = Some(tuple from flags)`.
- Columns per scene/lane:
  - STANDS: `physbox_lab::settling::centered_stack_structure(scenario,
    &snapshot.bodies, overflowed, 0.0)` per snapshot -> first-loss step and
    final verdict; bucket = all bodies finite + capacity clean (see
    `structure()` in `physbox-s17-product-relative.rs`).
  - QUIET: max linear speed over the last quartile of steps (900-1200 for
    stacks, 450-600 for the bucket); pass at `<= 0.025 m/s` (the awake gate,
    2x under Box2D's default sleep threshold).
  - VALID: first `snapshot.failure` (kind@step; `PersistenceMismatch` is the
    key corruption) plus `capacities` accepted/not-overflowed.
  - COST: wall-clock around the whole series divided by steps. The bounded
    validation readback windows and their reclamation are included in that
    elapsed inner-loop cost; no TIMESTAMP_QUERY is needed.
- CLI: `--hz --damping --static-scale --lane both|selected|candidate
  --scenes stack4,stack8,stack16,bucket --steps N`; print the *effective*
  (post-cap) coefficients in the header so a clamped `--hz 60` cannot
  mislead a sweep.
- Budget: one 1,200-step 17-body lane is ~2 s on Apple M4 Pro; the default
  scoreboard should land in ~15-20 s. If it does not, fix that first — the
  loop is the deliverable.
- Sanity anchors for first light: selected 16-stack quiet ~= `0.824 m/s`,
  candidate (30/10/2) ~= `0.0077 m/s`, collapse at 407, bucket
  `PersistenceMismatch` at step 11.

### S18.2 — persistent-key corruption bug

- [x] Root-cause the duplicate/unsorted persistent-key state on the
  branch-robust pipeline. Repro is cheap: rectangle bucket, step 11 (last
  valid prefix 10) — seconds per attempt with the check tool. Start where
  the persistent cache (`cache: Vec<PersistentContactV1>`, StablePairSlot
  topology) is built and ordered in the AUTHORITY_GPU_PROBE path, and diff
  behavior against the selected path, which never corrupts. The pass-trace
  entry (`generated_contacts.rs`, pass-trace runner near line 3180) already
  routes the tunable lane for per-pass inspection.
- [x] Fix it in the probe-pipeline code (validation-lane only; the product
  solver does not share this path), then confirm bucket and 32/64 rows go
  VALID in the scoreboard.

### S18.3 — coefficient sweep

- [x] Shell-loop `physbox-check` over a small grid, e.g. damping
  {2, 4, 6, 8, 10} x static_scale {2, 4, 8} x hz {15, 30}. Goal: a
  configuration where every scene is STANDS + VALID + QUIET
  (`<= 0.025 m/s`). Record the scoreboard lines for the frontier points in
  this document; that is all the evidence the loop needs.
- [x] No qualifying point exists, so the conditional promotion review does
  not run. The residual-band branch of the pre-committed stop is taken without
  weakening the existing S17 promotion bar.

### S18.4 — optional interactive exposure (human ack first)

- [x] Triggered by explicit human request on 2026-07-21: expose the tunable
  lane as an opt-in interactive profile so the human can feel candidate versus
  product. Delivered as the `solver-lab` feature shared by engine, launch, and
  all four host adapters. The engine-rendered **SOLVERS** menu
  selects the product or branch-robust profile, edits all three swept
  coefficients, loads governed 8/16-stack scenes, and applies changes without
  a page reload. A later UX correction replaces the ambiguous pairwise A/B
  action with an explicit active-solver label, a dynamically laid-out registry
  of profiles, and one **USE SOLVER** action; each registered profile retains
  its last in-session configuration. Rebuilding remains inside `EngineHost`,
  so all hosts keep one state owner, renderer, input path, and pacing policy.
  Scene contract v6 reports
  the active profile, canonical options, scenario hash, execution,
  scheduling, and coefficient policy. The web lab persists only this opt-in
  selection in the versioned `physbox5.solver-lab.v1` local-storage key and
  synchronizes the URL after changes. The deployed web release deliberately
  enables this inspection feature, while native production builds remain
  gated and exclude it. `npm run inspect:web` is the local
  no-query-parameter entry point. No solver default changes.

Fresh exposure validation on 2026-07-21: `npm run test:web:lab` succeeded on
Apple M4 Pro Metal with run id `f40dbe1e-cb9e-48de-8397-8f49b9273538`; it
selected branch-robust, changed static scale to 2.5, activated the 16-stack,
switched to selected and back without reload, then removed the URL parameters
and proved local-storage restoration on reload. Matched 1,200-step headless
captures kept the selected 16-stack upright and showed the candidate's known
collapse with capacity clean in both contracts. An opt-in arm64 APK exposed
the same complete touch menu and activated branch-robust stack-16 on the
physical Pixel 7a/Vulkan; the ordinary production APK/device lane also passed.

## Completion result

`cargo run --release -p physbox-lab --bin physbox-check` completes the default
eight rows in `15.16 s` on the Apple M4 Pro Metal adapter. First light
reproduced the expected selected and candidate values before the key fix:
selected 16-stack `0.824075 m/s`, candidate `0.007701 m/s`, candidate
structural loss at step 407, and rectangle-bucket persistent-key failure at
step 11.

The tunable entry point initially exposed one missing constructor route: the
solver recognized only the frozen exact and branch-robust authority tuples as
authority-probe execution. It now recognizes the already-validated tunable
tuple and still rejects product selection.

The step-11 defect was in validation-only authority identity. That path first
ordered two contact points geometrically, then rewrote their feature tuples to
authority midpoint/vertex identities. A face-index wrap such as vertex 3
followed by vertex 0 could reverse the resulting persistent-key order.
Pair-slot persistence already canonicalized those rewritten keys before the
solve, so mechanics used canonical order while the mapped validation manifold
failed its sorted-key invariant. The authority-identity branch now
canonicalizes the rewritten two-point manifold too. A focused 12-step
rectangle-bucket GPU regression fixes the exact reproducer. Full candidate
rows now remain VALID for the 600-step rectangle bucket and the 1,200-step
32/64 diagnostics; those deep stacks retain their known structural losses at
steps 225/129.

The complete predeclared grid is `hz {15,30} x damping {2,4,6,8,10} x
static-scale {2,4,8}`. All 30 points and every default scene are VALID. The
frontier scoreboard rows are below; QUIET values are maximum last-quartile
linear speed in meters per second and COST is release wall milliseconds per
authored step.

| Hz/damping/static | Scene | STANDS | QUIET | VALID | COST |
| --- | --- | --- | ---: | --- | ---: |
| 15/10/2 | stack4 | PASS | PASS 0.012054 | PASS | 0.601 ms |
| 15/10/2 | stack8 | PASS | FAIL 0.030370 | PASS | 0.648 ms |
| 15/10/2 | stack16 | FAIL@182 | PASS 0.018194 | PASS | 0.888 ms |
| 15/10/2 | bucket | PASS | PASS 0.022369 | PASS | 4.155 ms |
| 30/8/8 | stack4 | PASS | PASS 0.011687 | PASS | 0.601 ms |
| 30/8/8 | stack8 | PASS | PASS 0.022246 | PASS | 0.649 ms |
| 30/8/8 | stack16 | FAIL@423 | PASS 0.010728 | PASS | 0.886 ms |
| 30/8/8 | bucket | PASS | PASS 0.018815 | PASS | 4.082 ms |
| 30/10/2 | stack4 | PASS | PASS 0.004970 | PASS | 0.600 ms |
| 30/10/2 | stack8 | PASS | PASS 0.016011 | PASS | 0.648 ms |
| 30/10/2 | stack16 | FAIL@407 | PASS 0.007701 | PASS | 0.887 ms |
| 30/10/2 | bucket | PASS | PASS 0.018204 | PASS | 4.069 ms |
| 30/10/8 | stack4 | PASS | PASS 0.007547 | PASS | 0.601 ms |
| 30/10/8 | stack8 | PASS | PASS 0.018624 | PASS | 0.647 ms |
| 30/10/8 | stack16 | FAIL@439 | PASS 0.009340 | PASS | 0.887 ms |
| 30/10/8 | bucket | PASS | PASS 0.019440 | PASS | 4.064 ms |

No tuple is stands-quiet-valid because every one loses the governed
16-stack. The best structural frontier is 30/10/8 at step 439; lowering hertz
or damping does not cross the structure gate. Stop condition 2 is therefore
taken. Hard/direct remains the selected product, its measured residual motion
is accepted as the state sleeping design must handle honestly, and no
successor formulation study is opened. The later explicitly requested
interactive surface changes access to the already-rejected candidate, not its
disposition or promotion status.

## Stop condition (pre-committed)

S18 completes in exactly one of two ways:

1. A stands-quiet-valid configuration exists -> human promotion review with
   the existing cross-host lanes.
2. S18.2 is fixed and the sweep finds no qualifying point -> record
   acceptance of the residual band in the awake-settling topic, close this
   workstream, and open no further formulation study. Sleeping design then
   proceeds against the product as it actually behaves.

Either outcome is success. "We learned something" without one of these two
endings is failure; that is the recorded human directive.

## Non-goals and ownership

- No product default, product constructor, or launch default changes.
- No weakening of required gates, oracle precedence, authority bands, or the
  frozen S17 evidence; the S17 descriptor stays pinned to 30/10/2.
- No new evidence bureaucracy for the inner loop; heavyweight lanes remain
  reserved for actual promotion.
- Track B (authority parity) stays paused; this document cannot reopen it.
