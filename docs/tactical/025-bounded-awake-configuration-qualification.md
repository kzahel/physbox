# S25 — Bounded awake configuration qualification

Status: active, 2026-07-22. Human review authorizes one fast configuration-
only attempt to clear S24's native 12-stack quiet failure. The solver family,
equation, awake bands, sleeping policy, and product default remain unchanged
while the predeclared shortlist is screened.

Governing topics:
[rigid quality targets](../topics/rigid-quality-targets.md),
[physics configuration and solver profiles](../topics/physics-configuration-and-solver-profiles.md),
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md),
and [performance and scale](../topics/performance-and-scale.md).
Predecessor:
[S24 current rigid quality qualification](024-current-rigid-quality-qualification.md).

## Goal

Determine quickly whether an already-supported soft-coupled configuration can
pass the current sleep-off 12-stack target without materially increasing its
compliance or hiding motion. Initialize the native GPU once, screen exactly six
configurations on the blocking fixture, advance no more than two survivors to
the broader native corpus, and advance no more than one finalist to shared-host
qualification. Stop cleanly if configuration alone is insufficient.

## Exclusions and ownership

- Do not change the soft-coupled recurrence, contact identity, scheduling,
  recovery policy, warm start, restitution placement, numerical bands,
  authority precedence, or Box2D pin.
- Do not change the ordinary product default, add a named preset, or promote
  sleeping inside S25. A finalist remains a custom configuration until a later
  explicit human default decision.
- Do not weaken `0.025 m/s`, use sleeping, omit angular-edge motion, or accept
  a quiet result bought through materially worse penetration/height.
- Do not repeat S18's 30-cell coefficient grid, S19's complete resolution
  matrix, historical formulation studies, or a per-candidate host matrix.
- Do not infer browser or Android behavior from native results. Cross-host
  work is conditional on one native finalist.

## Frozen shortlist and rationale

All rows use the selected soft-coupled family, 60 Hz outer steps, one biased
iteration plus relaxation, contact 30 Hz, sleeping off, and 1,200 outer steps.
Only the listed values vary:

| ID | Substeps | Damping | Static scale | Reason |
| --- | ---: | ---: | ---: | --- |
| `baseline` | 4 | 10 | 2 | Reproduce S24 in the same process and own every relative comparison. |
| `static-frontier` | 4 | 10 | 8 | S18's best reported static-support frontier, never measured on the new 12-stack gate. |
| `damping-12` | 4 | 12 | 2 | One bounded step beyond S18's maximum damping. |
| `damping-16` | 4 | 16 | 2 | One separated high-damping probe inside the existing product range. |
| `damping-static` | 4 | 12 | 8 | One interaction between the new damping step and the historical static frontier. |
| `resolution-control` | 8 | 10 | 2 | The sole costlier resolution control; S19 showed later 16-stack loss but substantially higher cost and unsafe post-loss motion. |

S18 already established that lowering contact Hz or damping worsens the
relevant frontier. Requested contact Hz above 30 is dynamically inert at
60/4 because the effective cap is 30 Hz. S19 established that 4/i2 slightly
increases ordinary-stack motion and cost, two substeps is worse, and 8/i2 is
more expensive than 8/i1. Those cells are deliberately absent.

## S25.1 — one-process blocking screen

- [x] Add a named `physbox-check` screen that constructs one GPU runner and
  executes the frozen six rows in order; reject conflicting ad-hoc flags.
- [x] Reuse the frozen settling observer to report settled maximum linear and
  angular-edge speed, maximum penetration, final stack height, maximum center
  drift, energy-envelope result, structure/first loss, validity, and wall cost.
- [x] Require every survivor to preserve structure at every observation,
  remain finite/capacity-clean, and keep both settled linear and angular-edge
  speed at or below `0.025 m/s`.
- [x] For promotion beyond the screen, also require settled penetration no
  more than `0.0002 m` above the same-run baseline and final stack height no
  more than `0.0002 m` below it. This reuses the established absolute geometry
  reporting band and prevents damping from winning only through additional
  sag. Drift and energy remain visible ranking facts, not silently averaged
  gates.
- [x] Budget the complete screen at 60 seconds on the current AMD adapter. If
  it exceeds that warm budget, fix the loop before expanding evidence.

Implementation checkpoint: the named screen owns its immutable configuration
list, rejects every additional CLI flag, performs one excluded baseline warmup
step for comparable row timing, and sorts at most two advances by worst quiet
speed then cost. Ordinary ad-hoc checker use remains available and now reports
the same frozen stack-settling facts for exact 1,200-step stack rows.

Clean S25.1 result, revision
`4ea83b50f7520924b5e1b126ceccb44f67ad005d`, AMD Radeon 890M Graphics
RADV/Vulkan:

```sh
cargo run --release -p physbox-lab --bin physbox-check -- --screen current-12
```

| ID | Structure | Linear | Angular edge | Penetration | Final height | Drift | Energy | Geometry | Result |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| `baseline` | pass | `0.029767` | `0.012892` | `0.013737` | `2.916734` | `0.085140` | fail | pass | reject quality |
| `static-frontier` | pass | `0.025584` | `0.011555` | `0.014680` | `2.918886` | `0.085137` | fail | fail | reject quality |
| `damping-12` | pass | `0.028617` | `0.009783` | `0.013898` | `2.916203` | `0.085139` | fail | fail | reject quality |
| `damping-16` | pass | `0.026132` | `0.009708` | `0.013904` | `2.916845` | `0.085114` | fail | pass | reject quality |
| `damping-static` | pass | `0.025955` | `0.010793` | `0.014183` | `2.919788` | `0.084313` | fail | fail | reject quality |
| `resolution-control` | pass | `0.017233` | `0.005768` | `0.012868` | `2.925068` | `0.076357` | fail | pass | **advance** |

Speeds are m/s and geometry values are meters. Every row is finite and
capacity-clean. The same-run baseline exactly reproduces S24's `0.029767 m/s`
linear result. All five four-substep rows miss the unchanged linear quiet
limit; several additionally fail the geometry guard. The sole eight-substep
control clears both quiet limits and improves baseline penetration, height,
and drift. Energy-envelope failure is visible for every row but is not an
S25.1 gate. The complete one-runner screen takes `32.891 s`, passing its
60-second budget. Only `resolution-control` is admitted to S25.2.

## S25.2 — survivor-only native guards

This slice runs only for S25.1 survivors, sorted by quiet margin and then by
unchanged/lower cost. Admit at most two.

- [ ] Run exact 1,200-step 2/4/8/12 blockers and 14/16 stretch/diagnostic rows,
  plus bucket, bilateral impact, off-center response, friction, restitution,
  and short-stack guards with sleeping off.
- [ ] Repeat the complete candidate rows and require exact structural,
  validity, and measurement replay. Require 2/4/8/12 to meet both linear and
  angular-edge quiet limits and the baseline-relative geometry guard.
- [ ] Keep 14/16 failures reported but non-blocking. Require every response
  and bucket row to remain finite, capacity-clean, and inside its existing
  predicate.
- [ ] Select at most one finalist. An 8-substep finalist must expose its
  increased per-step/per-simulated-second cost and may not be described as a
  free quality improvement.

## S25.3 — conditional shared-product qualification

This slice runs only if S25.2 selects one finalist. It validates a custom
configuration; it does not change the default.

- [ ] Project the finalist through the canonical product configuration and
  engine-owned stack source exactly once. Produce the durable native package
  deferred by S24: all six roles, settling/geometry/energy, replay, work,
  allocation, capacity, adapter, hashes, revision/dirty state, run id, and
  exact replay command.
- [ ] Qualify 2/4/8/12 and report 14/16 on ordinary Browser WebGPU and the
  attached physical Android device with the same configuration/scenario
  identities and fresh provenance. Do not use an AVD while the physical lane
  is available.
- [ ] Run the proportional production gates required for a physics-visible
  cross-host validation addition: `npm run test:fast`, production clippy with
  warnings denied, one fresh headless smoke, ARM64 check, release APK,
  `npm run test:web:lab` when the existing configuration interaction is used,
  and the physical-device lane.

## S25.4 — disposition

- [ ] Update the quality topic, configuration topic, capability ledger,
  bring-up plan, and tactical index with every measured row and missing lane.
- [ ] If no S25.1/S25.2 survivor exists, record `no-configuration-only-
  solution`, leave the default unchanged, keep S23.7 held, and return any
  solver-formulation work to human review without opening it automatically.
- [ ] If one S25.3 finalist passes, leave the default unchanged and stop at a
  human subjective/default decision with the exact interactive configuration
  available through the existing menu.
- [ ] Commit each admitted implementation boundary before producing clean
  evidence and finish with a clean worktree.

## Completion condition

S25 completes in exactly one of two ways:

1. the frozen native screen or survivor guards produce no qualifying
   configuration, all conditional work remains unperformed, and the negative
   configuration-only result returns to human review; or
2. one custom finalist passes native product, browser, and physical-Android
   qualification with complete cost/quality evidence, the ordinary default
   remains unchanged, and human review decides whether to promote it.

The shortlist cannot grow inside S25. A missing survivor is a valid bounded
result, not permission to add configurations, formulas, or hosts.
