# S23: GPU sleeping and waking

Status: manual checkpoint 2026-07-22. S23.1 through S23.6 are complete and
work is stopped before S23.7's reserved human promotion/default decision.
Sleeping is effective when explicitly requested through the shared menu or
launch configuration, but remains off by default. The user initially
authorized the hardware-backed AVD for this unattended checkpoint; an attached
Pixel 7a later became available and also passed the proportional lane.

Governing topics:
[GPU sleeping and waking](../topics/gpu-sleeping-and-waking.md),
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md),
[performance and scale](../topics/performance-and-scale.md),
[simulation time and overload](../topics/simulation-time-and-overload.md),
[physics configuration and solver profiles](../topics/physics-configuration-and-solver-profiles.md),
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md),
and [Box2D rigid authority](../topics/box2d-rigid-authority.md). S22 supplies
the deterministic live-body set and GPU-owned indirect-argument foundation.

## Goal

Add one versioned GPU-resident sleeping/waking mechanism whose policy,
transitions, causes, work reduction, and behavioral differences are observable
and reproducible. Live collision, persistence, rendering, identity, and
capacity remain distinct from awake solver work. No simulation-dependent GPU
result is read back to admit or size the next interactive frame.

The authorized first checkpoint is deliberately inert: freeze the policy
vocabulary and authority traces, record PhysBox sleep-off baselines, then add
generation-safe sleep state and a deterministic awake-dynamic list without
using either to alter solver behavior. This makes the representation and its
exact-equivalence gates reviewable before transition semantics exist.

## Exclusions and ownership

- S23.1 and S23.2 do not put a body to sleep, propagate a wake, filter a solver
  body or point, change contact persistence, or claim mostly-sleeping scale.
- Sleeping remains off by default. A request may be transported and reported
  before it becomes effective, but ordinary default promotion is reserved for
  the manual S23.7 review.
- Sleeping cannot change an awake solver verdict. The topic's mask rule is a
  mandatory disclosure and gate; it may not reclassify the accepted awake
  settling residual or loss.
- Box2D is the declared rigid behavioral authority, not a product dependency.
  Oracle work stays process-isolated and uses only public API observations.
- Platform adapters may transport raw arguments, lifecycle, and presentation;
  they may not own sleep timers, wake rules, alternate policy, or CPU mirrors.
- Sleep timers use integer outer fixed-step counts. Zero-step presentation
  frames and solver substeps do not advance them, and overload never catches
  them up.
- The S22 live list continues to own collision and rendering. The new awake
  list is sorted dynamic work only and is capacity-accounted scratch.
- The existing body-core sleeping flag remains diagnostic until a later slice
  explicitly connects authoritative sleep state to presentation.

## Ordered delivery slices

### S23.1 — policy vocabulary and frozen baselines

- [x] Version the policy and transition vocabulary: farthest-point speed,
  quiet duration expressed in outer steps, threshold source, transition
  velocity treatment, transition/wake causes, requested/effective state, and
  declared Box2D-versus-PhysBox differences.
- [x] Extend the process-isolated Box2D lane to capture public per-step awake
  state and farthest-point speed without source instrumentation.
- [x] Record the governed authority fixtures at 30/60/120 Hz with four
  substeps and at 60 Hz with one/eight substeps: isolated circle and rectangle,
  stack, pyramid, two independent structures, applied impulse, projectile
  impact, and grab begin/hold/release where the neutral contract supports it.
- [x] Record matching PhysBox sleep-off motion histories and exact replay for
  the governed fixtures, naming any fixture that cannot share an action
  projection rather than manufacturing parity.

Gate: a clean, fresh manifest records source/oracle pins, complete
configuration, trace hashes, first-sleep and wake observations, transition
velocity evidence, and every declared unsupported comparison. The maintained
oracle process tests and the directly affected evidence target pass.

Evidence, 2026-07-22: clean revision `6c70367` produced
`/tmp/tmp.uIcbmASFU9/s23.1/manifest.json` on AMD Radeon 890M RADV Vulkan. All
40 Box2D rows and all 25 supported action-free PhysBox histories replay
exactly. The five configurations are 30/60/120 Hz at four substeps plus 60 Hz
at one/eight substeps. All authority fixtures sleep, every observed Box2D
sleep transition has zero farthest-point speed, and the impulse, projectile,
and grab rows report governed wake transitions. The generated-contact history
runner's lack of neutral-action execution is retained as an explicit PhysBox
limitation on those 15 rows. `cargo test -p physbox-contracts`,
`cargo test -p physbox-box2d-oracle`, and warnings-denied clippy for the new
evidence target pass.

### S23.2 — inert GPU state and awake compaction

- [x] Add a versioned, generation-checked `SleepStateV1` GPU lane with awake
  and eligible flags, quiet-step count, last-transition outer step, cause bits,
  policy version, layout version, and deterministic reset/lifecycle behavior.
- [x] Add a sorted awake-dynamic slot list, count, and indirect arguments,
  compacted from the sorted live list on the GPU before the solver seam.
- [x] Keep automatic sleep and solver filtering disabled: every live dynamic
  body is awake, every fixed body is excluded, and the selected solver still
  consumes its existing live-work inputs.
- [x] Transport and report versioned requested/effective sleeping state without
  changing the ordinary off default; the engine-rendered request control
  explicitly reports that effective sleeping remains false at this checkpoint.
- [x] Prove sparse authored slots, construction, erase, generation reuse,
  failed mutation, reset, capacity-full behavior, and exact replay. Stale state
  from an older generation must be inert.
- [x] Prove sleep-off results are exact before and after the inert graph change,
  including the selected structural corpus and sparse-capacity probe.

Gate: awake readback agrees exactly with live dynamic metadata after every
lifecycle case, new state/arguments remain capacity-clean and deterministic,
and selected solver outputs are byte-identical because no consumer changed.
The ordinary lane, production clippy, fresh headless smoke, ARM64 check,
release APK, ordinary browser result, and authorized hardware-backed AVD run
pass. The AVD is shut down even after failure.

**Authorized unattended stopping point:** commit the passing S23.2 checkpoint,
update the topic, capability ledger, and this tactical with evidence and known
gaps, then stop. Do not begin S23.3 without a later user request.

Checkpoint evidence, 2026-07-22: implementation commit `47542c0` adds a
32-byte version-1 state lane per body slot and a sorted GPU awake-dynamic list
with count and indirect arguments. One deterministic pre-solver compaction
pass scans the S22 sorted live list, excludes fixed bodies, initializes reused
generations awake, clears stale list entries, and leaves every existing solver
input unchanged. Lifecycle readback covers sparse slots, construction,
erase/reuse, generation exhaustion and other failed mutation, reset, and a
full 64-slot capacity. Requested sleeping is transported by
`PhysicsConfigurationV2` and `--sleeping on|off`; scene contract version 9
reports requested and effective state separately, with effective sleeping
hard-false.

UI addendum commit `a1b5453` exposes the request through the real
`MENU → WORLD → SLEEP REQUEST` canvas control without beginning S23.3. UI
contract version 6 labels requested and effective state separately and applies
the complete shared configuration immediately, preserving capacity and tool
state. The clean ordinary browser flow toggles on, verifies version-3
preference and URL/reload persistence, then toggles off; `npm run test:fast`
passes in 27.27 seconds, warnings-denied production clippy passes, and the
solver-lab browser lane passes. ARM64 check, dual-ABI release-native/debug APK,
and Gradle release APK builds pass. With no physical device available, the
hardware-backed `jstorrent-tablet` AVD uses the actual menu control, completes
the bootstrap scenario with requested true/effective false, produces a
visually complete capture, and emits the successful shutdown marker.

Clean revision `47542c08b572316dccc14b678d1be778a012d761` produced
`/tmp/physbox5-s23.2-47542c0/sleep-policy/manifest.json` on AMD Radeon 890M
RADV Vulkan. All 40 authority rows pass and replay exactly. All 25 supported
action-free GPU histories replay exactly and their snapshot histories match
the clean S23.1 baseline at `6c70367` exactly; the 15 action rows retain their
declared unsupported PhysBox projection. The clean durable sparse-capacity
artifact `/tmp/physbox5-s23.2-47542c0/sparse-capacity.json` reports exact,
overflow-free 64/256/1K/10K rows. The new state/list allocation grows honestly
from `2,048 + 336` bytes at 64 slots to `320,000 + 40,080` bytes at 10K; each
sparse row still contains two live dynamic bodies.

`npm run test:fast` passes in 29.73 seconds, production and directly affected
tool clippy pass with warnings denied, and `npm run test:web:lab` passes. A
fresh 120-step headless rigid-floor run with `--sleeping on` reports requested
true, effective false, and a successful scene. The ARM64 check, release-native
arm64-v8a/x86_64 builds, debug APK, and Gradle `assembleRelease` pass. With no
physical device available, `jstorrent-tablet` passes the shared rigid-floor
scenario using the host Radeon Vulkan adapter; its screenshot is visually
complete, provenance is clean, and
`PHYSBOX5_ANDROID_AVD_SHUTDOWN_SUCCESS` plus an empty `adb devices` list prove
shutdown.

Known boundary: the correctness-first compaction pass uses one GPU invocation
to scan live slots. It is deterministic and remained reserve-flat in the
bounded sparse probe, but it is not a parallel-compaction or mostly-sleeping
scale claim. No body transitions asleep, no wake root propagates, and neither
the awake list nor sleep state sizes solver work. Those changes begin only
after explicit S23.3 authorization and receive their own scale gate before
awake-driven solving.

### S23.3 — generation-safe wake roots and bounded propagation

- [x] Add explicit wake roots for impulse, projectile/new touching contact,
  grab begin/hold, construction/support change, reset/rebuild, and authored
  test state.
- [x] Propagate roots over current generational contact neighbors for a fixed
  number of deterministic GPU rounds; reject stale slot generations.
- [x] Select `K` using predeclared `K-1`/`K`/`K+1` chains and 64-body mobile
  wake-wave evidence.

Gate: wake causes and latency match the declared policy, neighbors wake before
pair retirement, capacity presets share semantics, and overflow fails closed.

Checkpoint evidence: commit `09f5e2e` selects four deterministic propagation
rounds and a `0.05 m/s` persisted-impact wake threshold. Generation-checked
roots cover grab begin/hold, construction, reset, erased support, new contact,
and impact. The 1/2/4/8-round comparison runs on 16- and 64-body chains; the
`K-1` and `K` bodies wake in the root step, the `K+1` body wakes on the next
outer step, and stale generations remain inert.

### S23.4 — automatic sleep admission

- [x] Evaluate the frozen quiet predicate and integer outer-step duration on
  the GPU, set cause bits, and perform the selected velocity treatment.
- [x] Establish no pose drift while asleep and sleep-off versus
  pre-first-transition exactness when enabled.

Gate: authority overlays and PhysBox-specific invariants pass without masking
an awake failure or advancing timers on zero-step/substep-only work.

Checkpoint evidence: commit `c900a17` evaluates the frozen `0.02 m/s`
farthest-point predicate after solve, admits sleep after exactly 15/30/60
outer steps at 30/60/120 Hz, and zeros transition velocities. Focused GPU
readback proves no sleeping pose drift and no substep-derived timer advance.
Requested sleeping is now effective when enabled; the menu reports
`EFFECTIVE ON`, while the ordinary default remains off.

### S23.5 — awake-driven solve and integration

- [x] Drive solver bodies and integration from awake work and compact solver
  points that touch at least one awake dynamic body.
- [x] Retain all sleepers in live collision, persistence, support, and render
  work with cached impulses ready for wake.

Gate: awake work reduction is observable while wake transitions preserve
contact identity, exact replay, and declared latency.

Checkpoint evidence: deterministic GPU compaction now produces dense awake
solver points with an original-contact mapping, while persistence receives
solved impulses scattered back to its unchanged live-contact order. The
sorted public awake list remains dynamic-only; a separate bounded solver-body
list adds only fixed endpoints required by active awake contacts. Integration,
solver body work, and point work therefore fall to zero for two authored
sleepers, restore to two bodies/two points on a grabbed wake root, and preserve
the fixed-floor grab result. Live SAP, narrowphase, persistence, rendering,
and cached dormant impulses remain unchanged. The maintained 110-test product
suite, the two previously sensitive exact-replay tests, and the focused wake
restoration test pass; the complete non-production engine suite has only its
independently verified pre-existing 197-versus-195 experiment-dispatch
assertion failure.

### S23.6 — scale evidence and dormant-pair decision

- [x] Produce valid matched sleeping-city curves at 1%, 10%, and 100% awake,
  retain the sparse-capacity flatness probe, and include a dense awake control.
- [x] Attribute remaining work by stage and take the dormant sleeping-pair
  decision only if the 1%-awake row demonstrates that need.

Gate: reports satisfy the performance topic's provenance, work-accounting,
timer-validity, overflow, and cross-capacity comparison rules.

Checkpoint evidence: `benchmark:sleeping-city` at clean revision `d2a94fa`
records valid native GPU timestamps, exact replay, clean capacity, complete
work vectors, four warmup steps, and 20 retained samples on AMD Radeon 890M
RADV Vulkan. The matched 1,000-body/1,024-capacity rows at 1%/10%/100% awake
report 10/100/1,000 awake solver bodies and 6/66/666 awake points while all
retain 666 live points. GPU end-to-end P50 is
`5.307/6.563/7.450 ms` (P95 `5.859/7.439/8.521 ms`); the 1%-awake row is
28.8% below the dense control. Solver winner visits scale honestly from
10,240 through 1,024,000. The 1%-awake reserve probe remains flat enough at
256/1K/10K capacity (`4.431/4.405/4.864 ms`, max/min `1.104`) while accounted
allocation grows from 3,556,548 to 125,073,624 bytes. The existing two-body
sparse control remains bounded through the same 10K tier (max/min `1.220`).

The predeclared dormant-pair gate is not met. Collision plus persistence is
26.9% of 1%-awake GPU P50, persistence alone is 0.9%, and the live/awake point
ratio is 111x; the recorded decision is therefore
`retain-live-pair-processing`, not a persistence-contract change. This is a
useful measured work-reduction result, not a rendered 60 Hz, 10K, 100K, CPU
crossover, or public product-rate claim. Artifact
`/tmp/physbox5-s23.6-d2a94fa.json` has SHA-256
`8c3fd6d925fa62e3c9d3385973dc173eae5df137bce5923af7d71de17f080f2a`.

Proportional qualification also exposed and fixed the zero-awake topology
clear seam (`0e6168e`): a 120-step sleeping floor now decodes empty solver
evidence without stale color state. `npm run test:fast` passes all 112
maintained engine tests, production clippy passes with warnings denied, and
the solver-lab browser lane exercises the real menu toggle including
persistence and off restoration. ARM64 check, dual-ABI release-native/debug
APK, and Gradle release APK builds pass. Clean headless revision `0e6168e`
reports requested/effective sleeping on and a complete rigid-floor result;
its visually inspected PNG/report SHA-256 values are
`b7e1a8e0107d9ae777e4d804733002ee98574bf43b67d8e77dcd6165fac5349d` and
`367308f80a9868452b7a930fd5ac176a2bc9b79afd8cb863bb739d00e052692b`.
Clean AVD revision `d0f5de3` taps `MENU → WORLD → SLEEP REQUEST`, waits for
requested/effective on and the complete shared rigid-floor result, captures
the visible on-state, and shuts down successfully. Its PNG/provenance SHA-256
values are `de3530c235b5d863481529de5d4b5865d640d3f5357a53702d9012804cfdbbbc`
and `2e86565903ed77b5d7994ab60e84415ee2eee6d70b1456018550327952fec866`.
Physical-device revision `74960b9` repeats that actual-menu interaction on a
Pixel 7a with its Mali-G710 Vulkan adapter, waits for requested/effective on
and the complete shared rigid-floor result, captures a visually complete
portrait frame, and leaves the product app stopped. Its PNG/provenance
SHA-256 values are
`c978ab61b7cfd9109d5b41348bd35a8cdea6f5d7e21e63a8f4753c2dcd61889d`
and `73743617bc11f87ddc7911a2772c501f2441babbc898a690223062ed7ad87840`.
These proportional gates do not complete S23.7's broader mask-rule and human
promotion review.

#### Post-S23.6 interactive observability addendum

Human review requested an interactive way to tell whether a body is sleeping.
Commits `3c58e17`, `70bb606`, and `f45676d` add an independent engine-rendered
`MENU → WORLD → SLEEP VISUALS` toggle and consume GPU sleep/grab state directly
in body rendering. Teal is active, amber plus a progress bar is the quiet
countdown, blue plus `Z` is asleep, and pink plus `!` is a recent or actively
grab-forced wake. Fixed bodies remain gray; the visualization is off by
default, does not enter the physics configuration or scenario hash, and adds
no CPU readback. UI contract version 8 reports the mode and compact Android
markers expose it at bit 30.

Clean revision `f45676d` passes `npm run test:fast` with all 113 maintained
engine tests, production clippy with warnings denied, `npm run test:web:lab`,
the ARM64 Android check, release-native/debug APK construction, and Gradle
`assembleRelease`. The ordinary browser lane taps both menu controls, verifies
that the presentation mode survives the sleeping world rebuild without
changing the physics preference, captures a visually complete blue-`Z` body,
and restores both controls to off. Its visual PNG has SHA-256
`b7dc078e8490b3d86592143c288160a24486413c64057bf9e857696ff3030dc7`.

A fresh clean 120-step headless rigid-floor run reports sleeping requested and
effective, the presentation mode off by default, and a complete shared result;
its PNG/report SHA-256 values are
`b7e1a8e0107d9ae777e4d804733002ee98574bf43b67d8e77dcd6165fac5349d` and
`4ce99e54dbbcc5e985601716a0d4b8ce9f4cb00e6fd83b98b67667307ce52b02`.
On the attached Pixel 7a, the maintained physical runner taps `SLEEP REQUEST`
and `SLEEP VISUALS`, verifies bit 30 plus requested/effective sleeping and the
complete rigid-floor result on Mali-G710 Vulkan, and captures the blue `Z`.
Its PNG/provenance SHA-256 values are
`795ad2ad81516bbefaa93c5d4dd0fe57a301fb398b7661a5f35d21bcafe5e4ed` and
`cf32a14ecc53ed9d3658332c460016596326f5b0402846bc3ee05149e746fef8`.
A follow-up held-grab capture verifies the same marker with grab active and a
visible pink `!`; its PNG/provenance SHA-256 values are
`06db41074008b10f67492aa1977c9ce9f04cf5925b35fd0aaf6344ca7aa38eac` and
`3d77343991ac8f6f08bf25b0c58d81b303165ab3b0862991850586b345610b2d`.
Both physical runs leave the app stopped. This addendum improves the review
surface but deliberately neither executes nor completes S23.7's mask-rule and
default-promotion decision.

### S23.7 — cross-host qualification and promotion review

- [ ] Run the full proportional browser/Android qualification and record the
  mask-rule disclosure plus every accepted authority divergence.
- [ ] Return effective menu behavior, default state, and any dormant-pair
  follow-up to a human promotion decision.

Gate: a human review explicitly promotes or retains sleeping as opt-in; no
test, evidence tool, or unattended run may make that product decision.

## Completion condition

S23 is complete only when sleeping and waking are fully GPU-owned, versioned,
observable, generation-safe, bounded, exact-replaying, capacity-clean,
behaviorally disclosed against Box2D, measurably reducing mostly-asleep work,
qualified on the maintained hosts, and given an explicit human default
decision. The authorized S23.2 stopping point is an implementation checkpoint,
not feature completion.
