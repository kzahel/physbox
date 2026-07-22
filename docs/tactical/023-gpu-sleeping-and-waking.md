# S23: GPU sleeping and waking

Status: manual checkpoint 2026-07-22. S23.1 and the authorized inert S23.2
checkpoint are complete. Work stops here before wake propagation; S23.3
requires a later user request. No physical Android device was available for
the closing run, so the explicitly authorized hardware-backed AVD substituted
for that lane and was shut down after validation.

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

- [ ] Add explicit wake roots for impulse, projectile/new touching contact,
  grab begin/hold, construction/support change, reset/rebuild, and authored
  test state.
- [ ] Propagate roots over current generational contact neighbors for a fixed
  number of deterministic GPU rounds; reject stale slot generations.
- [ ] Select `K` using predeclared `K-1`/`K`/`K+1` chains and 64-body mobile
  wake-wave evidence.

Gate: wake causes and latency match the declared policy, neighbors wake before
pair retirement, capacity presets share semantics, and overflow fails closed.

### S23.4 — automatic sleep admission

- [ ] Evaluate the frozen quiet predicate and integer outer-step duration on
  the GPU, set cause bits, and perform the selected velocity treatment.
- [ ] Establish no pose drift while asleep and sleep-off versus
  pre-first-transition exactness when enabled.

Gate: authority overlays and PhysBox-specific invariants pass without masking
an awake failure or advancing timers on zero-step/substep-only work.

### S23.5 — awake-driven solve and integration

- [ ] Drive solver bodies and integration from awake work and compact solver
  points that touch at least one awake dynamic body.
- [ ] Retain all sleepers in live collision, persistence, support, and render
  work with cached impulses ready for wake.

Gate: awake work reduction is observable while wake transitions preserve
contact identity, exact replay, and declared latency.

### S23.6 — scale evidence and dormant-pair decision

- [ ] Produce valid matched sleeping-city curves at 1%, 10%, and 100% awake,
  retain the sparse-capacity flatness probe, and include a dense awake control.
- [ ] Attribute remaining work by stage and take the dormant sleeping-pair
  decision only if the 1%-awake row demonstrates that need.

Gate: reports satisfy the performance topic's provenance, work-accounting,
timer-validity, overflow, and cross-capacity comparison rules.

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
