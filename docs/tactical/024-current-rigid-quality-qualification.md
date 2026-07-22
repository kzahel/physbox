# S24 — Current rigid quality qualification

Status: active, 2026-07-22.

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

- [ ] Extend the maintained `physbox-check` vocabulary to 2/12/14 without
  changing its existing historical lanes or defaults.
- [ ] Run the current soft tuple in release mode for exact 1,200-step
  2/4/8/12/14/16 sleep-off rows on a named native GPU.
- [ ] Require finite state, exact structural survival, clean capacity, and the
  existing quiet-speed observation for 2/4/8/12. Record 14 and 16 without
  turning either into a blocker.
- [ ] If 12 fails, stop before cross-host expansion, record the current
  blocking failure and return the awake solver/configuration decision to human
  review.

## S24.2 — durable native package

This slice runs only if S24.1 passes 12.

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

This slice runs only after the durable native 12 gate passes.

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

- [ ] Update the rigid-quality topic and capability ledger with the actual
  2/4/8/12/14/16 status. Do not infer a missing host or role.
- [ ] State whether S23.7 may resume, remains held behind an awake failure, or
  needs a separate human decision.
- [ ] Record exact artifact paths/hashes and commands, then mark this tactical
  complete only when every admitted slice is committed and the worktree is
  clean.

## Completion condition

S24 completes with either:

1. a fresh passing native/browser/physical-Android 2/4/8/12 package, reported
   14/16 diagnostics, updated current status, and an explicit handoff back to
   S23.7; or
2. a fresh native blocking failure, no expensive cross-host inference, updated
   current status, and a human awake-solver/configuration checkpoint.
