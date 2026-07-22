# S24 — Current rigid quality qualification

Status: complete negative, 2026-07-22. The clean native discriminator misses
the blocking 12-stack quiet-speed observation, so the conditional durable and
cross-host slices were not admitted. S23.7 remains held at a human awake-
solver/configuration checkpoint.

Governing topic:
[rigid quality targets](../topics/rigid-quality-targets.md).
Related behavior record:
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md).
Capability ledger:
[product capability status](../topics/product-capability-status.md).
Sleeping boundary:
[GPU sleeping and waking](../topics/gpu-sleeping-and-waking.md).

## Goal

Establish the first fresh current-default result against the decided awake
stack hierarchy before S23.7 changes sleep behavior or its default. Qualify
the selected soft-coupled 60/4/i1, contact 30 Hz/damping 10/static x2
configuration with sleeping disabled on 2/4/8/12 centered stacks, then report
14 as stretch and 16 as the matched-Box2D frontier diagnostic.

## Exclusions and ownership

- Do not change the solver equation, coefficient tuple, contact identity,
  scheduler, recovery policy, numerical bands, Box2D pin, or authority
  precedence.
- Do not tune, promote, or change the default for sleeping. Every S24 physics
  result is sleep-off; sleep-enabled survival cannot satisfy a gate.
- Do not rewrite S5-S18 results or checkboxes. They remain historical evidence
  under their original 16-stack contracts.
- Do not add the quality fixtures to the ordinary product menu. Cross-host
  fixtures are shared validation scenarios, not product content.
- Do not claim comprehensive rigid parity, liquid readiness, CPU crossover, or
  a general tower limit from this one hierarchy.

## S24.1 — native blocking discriminator

- [x] Extend the maintained `physbox-check` vocabulary to 2/12/14 without
  changing its existing historical lanes or defaults.
- [x] Run the current soft tuple in release mode for exact 1,200-step
  2/4/8/12/14/16 sleep-off rows on a named native GPU.
- [x] Require finite state, exact structural survival, clean capacity, and the
  existing quiet-speed observation for 2/4/8/12. Record 14 and 16 without
  turning either into a blocker.
- [x] If 12 fails, stop before cross-host expansion, record the current
  blocking failure and return the awake solver/configuration decision to human
  review.

## S24.2 — durable native package

This slice runs only if S24.1 passes 12. It was not admitted because the clean
native 12-stack row missed the quiet-speed observation.

- [ ] Add one current-target evidence command that projects
  `SolverProfileConfig::soft_coupled()` into the engine-owned stack generator
  exactly once and runs the same soft GPU recurrence as the shared host.
- [ ] Record all six roles, every-step structural outcome, first loss,
  settling summary, penetration/drift/height envelope, work, allocation,
  capacity, adapter, configuration and scenario hashes, revision/dirty state,
  run id, and exact replay in one atomically written artifact.
- [ ] Make the command exit unsuccessfully when 2/4/8/12 fails while retaining
  14/16 failures as visible stretch/diagnostic results.
- [ ] Commit the implementation before producing the clean retained artifact.

## S24.3 — browser and physical Android qualification

This slice runs only after the durable native 12 gate passes. It was not
admitted. An authorized physical Pixel 7a was attached, but no APK was built,
installed, or run because the predeclared native hard stop owned the result.

- [ ] Add launch-only shared 2/4/8/12/14/16 quality scenarios with one engine-
  owned source and completion contract. Adapters transport only their names.
- [ ] Qualify 2/4/8/12 with sleeping off on the ordinary Browser WebGPU path
  and the attached physical Android device. Capture 14/16 on the same hosts as
  reported stretch/diagnostic evidence.
- [ ] Require the same soft profile/configuration hash, scenario identity,
  exact requested step count, clean capacity, success sentinel, and fresh
  provenance. Preserve synchronized final captures for structural review.
- [ ] Run `npm run test:fast`, production clippy with warnings denied, one
  fresh headless smoke, the ARM64 check, a release APK, and the physical-device
  lane required for a user-visible cross-host validation change.

## S24.4 — disposition

- [x] Update the rigid-quality topic and capability ledger with the actual
  2/4/8/12/14/16 status. Do not infer a missing host or role.
- [x] State whether S23.7 may resume, remains held behind an awake failure, or
  needs a separate human decision.
- [x] Record exact artifact paths/hashes and commands, then mark this tactical
  complete only when every admitted slice is committed and the worktree is
  clean.

## Result

Clean revision `66d85dde3449897dcda9ec80de46f8a0aa245992` ran on:

`AMD Radeon 890M Graphics (RADV GFX1150) (Vulkan, IntegratedGpu, driver: radv Mesa 25.2.8-0ubuntu0.24.04.2)`.

The checker explicitly identified its legacy `candidate` lane as the tunable
soft recurrence whose default tuple is the current soft-coupled product:
60 Hz, four substeps, one iteration, contact 30 Hz, damping 10, static-contact
scale 2, sleeping off, and the existing `0.025000 m/s` quiet limit. Each row
ran exactly 1,200 outer steps.

| Stack | Role | Structure | Last-quarter maximum linear speed | Finite/capacity validity |
| ---: | --- | --- | ---: | --- |
| 2 | foundation | pass | `0.001747 m/s`, pass | pass |
| 4 | foundation | pass | `0.007952 m/s`, pass | pass |
| 8 | foundation | pass | `0.021373 m/s`, pass | pass |
| 12 | blocking awake target | pass | `0.029767 m/s`, **fail** | pass |
| 14 | stretch | pass | `0.028236 m/s`, fail | pass |
| 16 | frontier diagnostic | **fail at step 463** | `0.009305 m/s` after structural loss; not interpreted as settled success | pass |

Exact commands:

```sh
cargo fmt --all -- --check
cargo check -p physbox-lab --bin physbox-check
cargo run --release -p physbox-lab --bin physbox-check -- --lane candidate --scenes stack2,stack4,stack8,stack12
cargo run --release -p physbox-lab --bin physbox-check -- --lane candidate --scenes stack14,stack16
```

The first release command printed `QUALIFY candidate FAIL` because the
12-stack speed exceeded the quiet band by `0.004767 m/s`. This is a native
discriminator failure even though the structure stayed upright. The 14-stack
motion is also above the band, and the current 16-stack first loss at step 463
replaces the older step-407 observation for this exact revision/run without
changing its non-blocking role.

No retained evidence artifact, browser result, Android result, artifact hash,
or cross-host inference exists for S24: those belonged to conditional S24.2
and S24.3 and were intentionally not admitted. The two exact clean-revision
commands above are the complete S24 result. No solver equation, configuration,
quiet band, physics default, or sleeping policy changed.

S24 therefore completes under completion outcome 2. S23.7 may not resume
automatically; the next action is a human decision about the awake solver or
configuration, followed by a new clean 12-stack attempt under an explicitly
authorized workstream.

## Completion condition

S24 completes with either:

1. a fresh passing native/browser/physical-Android 2/4/8/12 package, reported
   14/16 diagnostics, updated current status, and an explicit handoff back to
   S23.7; or
2. a fresh native blocking failure, no expensive cross-host inference, updated
   current status, and a human awake-solver/configuration checkpoint.
