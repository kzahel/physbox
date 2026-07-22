# S25 — Bounded awake configuration qualification

Status: complete at human default-review checkpoint, 2026-07-22. The bounded
screen selected one eight-substep custom finalist, and its native product,
browser, and physical-Android qualification passes the blocking 2/4/8/12
contract. The solver family, equation, awake bands, sleeping policy, and
ordinary four-substep product default remain unchanged.

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

- [x] Run exact 1,200-step 2/4/8/12 blockers and 14/16 stretch/diagnostic rows,
  plus bucket, bilateral impact, off-center response, friction, restitution,
  and short-stack guards with sleeping off.
- [x] Repeat the complete candidate rows and require exact structural,
  validity, and measurement replay. Require 2/4/8/12 to meet both linear and
  angular-edge quiet limits and the baseline-relative geometry guard.
- [x] Keep 14/16 failures reported but non-blocking. Require every response
  and bucket row to remain finite, capacity-clean, and inside its existing
  predicate.
- [x] Select at most one finalist. An 8-substep finalist must expose its
  increased per-step/per-simulated-second cost and may not be described as a
  free quality improvement.

Clean S25.2 result at revision
`77a5505d918e4ce762bfb8f0e4c454fa5f434dbb` runs the sole survivor twice:

```sh
cargo run --release -p physbox-lab --bin physbox-check -- --sim-hz 60 --substeps 8 --iterations 1 --hz 30 --damping 10 --static-scale 2 --lane candidate --scenes stack2,stack4,stack8,stack12,stack14,stack16,bucket,impact,off-center,friction,restitution,short-stack
```

| Scene | Structure | Linear | Angular edge | Penetration | Final height | Result |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| stack 2 | pass | `0.000957` | `0.000672` | `0.001176` | `0.498546` | blocking pass |
| stack 4 | pass | `0.004333` | `0.001736` | `0.003453` | `0.992811` | blocking pass |
| stack 8 | pass | `0.010482` | `0.002915` | `0.007986` | `1.968382` | blocking pass |
| stack 12 | pass | `0.017233` | `0.005768` | `0.012868` | `2.925068` | blocking pass |
| stack 14 | pass | `0.014716` | `0.006246` | `0.014782` | `3.397960` | stretch pass |
| stack 16 | fail at step 840 | `1.868055` after loss | `1.783489` after loss | `0.004026` after loss | `0.992802` after loss | diagnostic failure |

Every metric, structural result, validity result, bucket speed (`0.006988
m/s`), and first-loss step replays exactly; timing is intentionally not an
exact-replay field. Candidate penetration improves over the same-adapter
baseline at every blocking depth. Final height improves at 4/8/12 and is only
`0.000057 m` lower at depth 2, inside the `0.0002 m` guard. The rectangle
bucket and all five short response predicates pass and remain capacity-clean.
The checker's legacy all-requested-scenes footer says `QUALIFY candidate FAIL`
because the command includes the intentionally non-blocking 16 diagnostic;
the S25 blocker verdict is pass.

Revision `2df1a06` expands the already-computed energy report before finalist
selection. At depth 2 the candidate energy changes from `5.49e-7` to
`5.54e-7 J` with a 15-step positive run, versus the baseline's decreasing
`1.677e-6` to `1.398e-6 J`. At depth 4 it changes from `1.2418e-5` to
`1.4257e-5 J`, versus the baseline's decreasing `6.4146e-5` to `4.7591e-5 J`.
Thus the frozen envelope boolean regresses on the two shallow rows, but both
candidate endpoints remain materially below the baseline's absolute energy
while linear speed, the declared geometry guard, and time-to-rest all pass.
This remains
an explicit finalist limitation and a required S25.3/native-product report;
energy was predeclared as visible ranking evidence, not an S25 blocker.

Validation wall cost is higher for the eight-substep graph and varies with
readback/runner state; recorded stack rows span approximately `4.76-5.49 ms`
per authored step across the two full replays. S25.3 must publish qualified
complete product cost per step and per simulated second before any default
decision. The sole finalist is therefore `resolution-control`: soft-coupled
60/8/i1, contact 30 Hz/damping 10/static x2, sleeping off.

## S25.3 — conditional shared-product qualification

This slice runs only if S25.2 selects one finalist. It validates a custom
configuration; it does not change the default.

- [x] Project the finalist through the canonical product configuration and
  engine-owned stack source exactly once. Produce the durable native package
  deferred by S24: all six roles, settling/geometry/energy, replay, work,
  allocation, capacity, adapter, hashes, revision/dirty state, run id, and
  exact replay command.
- [x] Qualify 2/4/8/12 and report 14/16 on ordinary Browser WebGPU and the
  attached physical Android device with the same configuration/scenario
  identities and fresh provenance. Do not use an AVD while the physical lane
  is available.
- [x] Run the proportional production gates required for a physics-visible
  cross-host validation addition: `npm run test:fast`, production clippy with
  warnings denied, one fresh headless smoke, ARM64 check, release APK,
  `npm run test:web:lab` when the existing configuration interaction is used,
  and the physical-device lane.

Clean revision `c71eabc1c3d756c19edd75114aa38d0b8dbeae4d` owns the
complete S25.3 chain. The canonical finalist configuration hash is
`87dfec84f20c6adfab96618809e79fbc65da83220f81c34b6477ec12db9ed6b2`.
`SolverProfileConfig::apply_to_scenario` projects that launch-owned
configuration exactly once into each engine-owned centered stack.

The durable native command is:

```sh
cargo run --release -p physbox-lab --bin physbox-check -- \
  --evidence s25-awake-finalist \
  --output /tmp/physbox5-s25-native-finalist.json
```

It completes in `91.586 s` on AMD Radeon 890M RADV/Vulkan, records clean
source/adapter/configuration/scenario provenance, and passes exact serialized
series replay for all six roles. Its SHA-256 is
`7b5e9d83cce8e87d42764022e1a5fe6c49e14e661f08fe16beb0d69b40dcaffe`.

| Stack | Structure | Linear | Angular edge | Penetration | Height | Observation-series wall cost | Result |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 2 | pass | `0.000957` | `0.000672` | `0.001176` | `0.498546` | `4.514 ms/step`, `270.841 ms/sim-s` | blocking pass |
| 4 | pass | `0.004333` | `0.001736` | `0.003453` | `0.992811` | `4.515 ms/step`, `270.893 ms/sim-s` | blocking pass |
| 8 | pass | `0.010482` | `0.002915` | `0.007986` | `1.968382` | `4.472 ms/step`, `268.298 ms/sim-s` | blocking pass |
| 12 | pass | `0.017233` | `0.005768` | `0.012868` | `2.925068` | `4.887 ms/step`, `293.237 ms/sim-s` | blocking pass |
| 14 | pass | `0.014716` | `0.006246` | `0.014782` | `3.397960` | `4.568 ms/step`, `274.082 ms/sim-s` | stretch pass |
| 16 | fail at step 840 | `1.868055` after loss | `1.783489` after loss | `0.004026` after loss | `0.992802` after loss | `5.005 ms/step`, `300.279 ms/sim-s` | diagnostic failure |

Speeds are m/s and geometry is meters. The four blocker geometry comparisons
pass against fresh default controls in the same package. The finalist still
has the declared shallow-stack energy-envelope limitation at depths 2 and 4;
it is not hidden by the passing quality verdict.

A same-revision, same-adapter four-substep control reports
`3.928/3.404/3.332/3.411 ms` per fully observed step at depths 2/4/8/12,
versus `4.514/4.515/4.472/4.887 ms` for the finalist. The validation-wall
increase is therefore approximately `15%/33%/34%/43%`; it is not a free
quality improvement and must not be mistaken for an uninstrumented frame-time
benchmark. The finalist graph records 170 dispatches per blocker step and
26,156 through 144,500 allocated bytes as depth grows. The shared headless
product contract at depth 12 reports 173 solver dispatches, 826,420 bytes of
64-slot solver capacity, no overflow, and the same configuration/scenario
hashes.

The focused browser command builds once and executes all six roles:

```sh
node scripts/qualify-awake-finalist.mjs \
  --native-evidence /tmp/physbox5-s25-native-finalist.json \
  --output /tmp/physbox5-s25-web-finalist.json
```

Browser WebGPU completes exactly 1,200 interactive outer steps per role with
zero or one step per frame, matching all six native scenario hashes and the
finalist configuration hash. Its SHA-256 is
`54635d0b61ceceae1eab2bedc9d5c876144e8c31e32b04554297423b8e9594a2`.

The attached Pixel 7a/Mali-G710 Vulkan lane uses the same native package:

```sh
android/qualify-awake-finalist.sh \
  --native-evidence /tmp/physbox5-s25-native-finalist.json \
  --output-directory /tmp/physbox5-s25-android
```

All six roles emit a hardware-adapter success, complete screenshots, success
sentinel, matching hashes, sleep-off state, and clean capacity. The 12-stack
capture remains upright; the 16-stack capture visibly retains its native
diagnostic loss. Browser/Android completion validates the shared product path,
identity, capacity, and pixels; native readback owns numerical quiet and
structure, so their completion sentinels do not relabel the failed 16-stack.

Artifact inventory:

| Host | Artifact | SHA-256 |
| --- | --- | --- |
| Native | `/tmp/physbox5-s25-native-finalist.json` | `7b5e9d83cce8e87d42764022e1a5fe6c49e14e661f08fe16beb0d69b40dcaffe` |
| Browser | `/tmp/physbox5-s25-web-finalist.json` | `54635d0b61ceceae1eab2bedc9d5c876144e8c31e32b04554297423b8e9594a2` |
| Physical Android | `/tmp/physbox5-s25-android/android-s25-awake-finalist.json` | `a153acb680f02666e59a4d7c5836aa9063262541c38fcb3245de8ed733604401` |
| Headless stack 12 | `/tmp/physbox5-s25-headless-stack12.json` | `b3a5f1ab0aa1087ba24a5198237258f80bfb81147ca3a39952951d0aa98e2c6b` |

Proportional gates pass: `npm run test:fast` in `24.06 s`, production clippy
with warnings denied, fresh headless stack 12, ARM64 Android check with the
solver-lab feature, `npm run test:web:lab`, arm64 release-native/debug APK,
Gradle `assembleRelease`, and the physical Pixel lane. An initial bare Gradle
invocation could not locate the SDK; sourcing `android/toolchain.sh` and
`physbox5_android_detect_sdk` made the declared release build pass. No AVD was
started.

## S25.4 — disposition

- [x] Update the quality topic, configuration topic, capability ledger,
  bring-up plan, and tactical index with every measured row and missing lane.
- [ ] If no S25.1/S25.2 survivor exists, record `no-configuration-only-
  solution`, leave the default unchanged, keep S23.7 held, and return any
  solver-formulation work to human review without opening it automatically.
- [x] If one S25.3 finalist passes, leave the default unchanged and stop at a
  human subjective/default decision with the exact interactive configuration
  available through the existing menu.
- [x] Commit each admitted implementation boundary before producing clean
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
