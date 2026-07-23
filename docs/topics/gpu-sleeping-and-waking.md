# GPU sleeping and waking

Status: implemented default-on product capability, updated 2026-07-23.
Tactical S23 is complete, the post-review contact-graph defects are corrected
and qualified, and S30's configurable shared threshold,
local-4/local-8/exact grouping, cycle diagnostics, and cross-host comparison
surface are complete. The later explicit S32 human promotion selects sleeping
on and exact current-contact-component grouping as the ordinary defaults;
sleeping-off, local-4, and local-8 remain available through the same controls.
S24/S25's awake limitations remain the solver verdict and are not masked. An
opt-in GPU sleep-state visualization is available for interactive review; it
does not change physics. This topic is the master record for the
sleeping/waking feature: its motivation, selected direction, frozen
experimental policy vocabulary, wake-trigger semantics, toggle and
experimentation surface, validation contract, execution plan, and accepted
disposition.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md), which
already records per-body timers with bounded neighbor wake propagation as the
likely GPU-resident sleep shape. Execution foundation:
[S22 active-work compaction](../tactical/022-active-work-compaction.md)
supplies the deterministic GPU live-slot set and indirect-argument plumbing
that awake filtering consumes. Scale claims and the sleeping-city workload
family are owned by [performance and scale](performance-and-scale.md).
Pacing and simulated-time semantics are owned by
[simulation time and overload](simulation-time-and-overload.md).
Configuration ownership follows
[physics configuration and solver profiles](physics-configuration-and-solver-profiles.md).
The awake-quality boundary — sleeping must never conceal awake motion — is
owned by [awake settling and rest stability](awake-settling-and-rest-stability.md).
The current structural hierarchy — including the blocking 12-stack and the
non-blocking 16-stack diagnostic — is owned by
[rigid quality targets](rigid-quality-targets.md).
The rigid behavior authority is
[Box2D rigid authority](box2d-rigid-authority.md); contact identity is owned
by [collision and contact persistence](collision-and-contact-persistence.md).
The independently evolving whole-island alternative, pinned Box2D source
study, and performance plan are recorded in
[GPU sleep-island connectivity research](gpu-sleep-island-connectivity.md).
The execution tracker is
[tactical S23](../tactical/023-gpu-sleeping-and-waking.md).
The authorized configurable threshold/grouping follow-up is
[tactical S30](../tactical/030-configurable-sleep-grouping.md).
The later default promotion is
[tactical S32](../tactical/032-sleep-default-promotion.md).

## Feature overview and tracking

This table is the compact answer to "where is sleeping?" Update it in the
same change that materially advances or reverses an area. The capability
ledger rows SOLVE-3 (sleeping and waking) and SCALE-4 (mostly-sleeping scale)
change only with implementation and evidence, not because this design exists.

| Area | Status | Current state / next gate |
| --- | --- | --- |
| Policy vocabulary and oracle traces | complete | S23.1 freezes experimental policy V1 and a replay-exact 40-row Box2D/PhysBox baseline matrix. |
| Versioned GPU sleep state | complete | Generation-checked `SleepStateV1`, sorted awake-dynamic list, indirect arguments, and exact inert replay (S23.2). |
| Wake roots and bounded propagation | complete | Generation-safe event roots and deterministic contact-neighbor rounds traverse every reserved point slot for each committed stable-pair manifold before solve; persisted resting contacts are not roots. |
| Automatic sleep admission | complete | Saturating per-body timers and four-round contact blockers coordinate admission within the selected bound, zero transition velocities, and prevent dormant drift. |
| Awake-driven solve and integration | complete | Dense awake points and bounded solver bodies size solve/integration while live collision, persistence, and rendering retain sleepers (S23.5). |
| Toggle and configuration surface | complete; S32 promotion | Launch-owned state and the engine-rendered `MENU → WORLD` controls make sleeping effective on request, configure one shared PhysBox/Box2D threshold, switch PhysBox among exact/local-4/local-8, and persist the choices. Sleeping-on/exact are the defaults; every choice remains reversible. |
| Interactive state visualization | complete | The engine-rendered `SLEEP VISUALS` toggle consumes GPU sleep/grab state directly and distinguishes active, quiet, sleeping, and forced/recently woken dynamics without readback. |
| Sleeping-city performance curves | complete | Valid matched 1%/10%/100% awake curves show bounded work reduction; S23.6 retains live pair processing and makes no public product-rate claim. |
| Cross-host qualification and promotion | complete | S23/S30 qualify the maintained implementation and comparison surface. S32 records the later human sleeping-on/exact promotion and passes native, browser, headless, ARM64/APK, and physical Android behavior gates while preserving the S24/S25 awake verdict. |

## The concern

Sleeping is the remaining half of the scale story. S22 made hot execution
follow live work instead of reserved capacity; sleeping makes hot execution
follow awake work instead of live work. The performance topic's provisional
expectations — the sleeping-city family at 1%, 10%, and 100% awake, and the
100K mostly-sleeping practicality claim — are unreachable without it, and the
capability ledger names GPU-owned sleep state, awake filtering, island wake
propagation, and wake triggers as the next rigid-state milestone after S22.

Sleeping is also product behavior, not only cost. Settled piles should become
still and stay still without solver micro-motion, and grabs, impacts, erases,
and support changes must wake exactly the affected bodies with low, explained
latency. Wake latency and the partial-stack or wake-wave behavior still
possible across paths longer than the selected bound are user-visible
consequences of the selected algorithm and need their own fixtures and bands.

## Boundary with awake settling: the mask rule

Sleeping must not rewrite the solver verdict. The current hierarchy requires
sleep-off 2/4/8 foundation results and a blocking 12-stack pass; sleep-assisted
survival cannot satisfy any of them. The 14-stack stretch and 16-stack
frontier diagnostic likewise retain their sleep-off outcomes even when sleep
changes what a reviewer sees.

The selected soft default already quiets the historical governed 16-stack far
below any plausible sleep threshold (`0.007701 m/s` settled maximum against
the Box2D `0.05 m/s` seed) and then still loses the structure at step 407 in
sleep-off execution. If sleep-enabled execution puts that stack to sleep
before step 407 and preserves its shape, the result is reported as "sleep
masked the awake loss," never as a stability fix. The sleep-off 16-stack
diagnostic remains failed, but that failure is non-blocking under the current
hierarchy. The same rule covers every structure whose sleep-off outcome
differs from its sleep-enabled outcome.
Every promotion package and the final capability-ledger row must carry this
limitation explicitly. The awake-settling topic's closed no-survivor record
and the S18 accepted residual band remain measured awake inputs to threshold
design; sleeping reports that provenance rather than erasing it.

The residual band also constrains the quiet predicate from the other side: a
threshold below the measured settled residuals of the selected soft default
would never sleep the structures sleeping exists for, while a threshold far
above them would sleep visibly moving bodies. Threshold selection is
therefore an evidence decision against the recorded settled windows, not a
copied constant.

## Established foundation

- S22 closed with a deterministic, capacity-accounted GPU live-slot list,
  GPU-owned live/dynamic/fixed counts, indirect compute/render arguments, and
  no simulation-dependent CPU readback. Sparse 256/1K/10K step cost is
  approximately flat while dense work still grows honestly.
- Body metadata already carries a sleeping flag (`FLAG_SLEEPING` in
  `crates/physbox-engine/src/body_state.rs`), but it is presentation/diagnostic
  state only: solver work is derived from live counts, so the bit is not a
  sufficient sleep state model.
- Interactive pacing executes zero or one outer fixed step and slips excess
  wall time permanently. Sleep timers must therefore be integer outer-step
  counters in simulated time, derived from the configured fixed `dt`;
  zero-step presentation frames advance nothing, and solver substeps are not
  timer ticks.
- The small interactive capacity tier uses a workgroup-island solver dispatch.
  It is a ≤64-capacity execution optimization, not a semantic island model,
  and is deliberately not the vehicle for wake semantics.

## Box2D authority behavior

The clean pinned checkout (`reference/box2d`, commit `3a4f0da...`) establishes
the behavior target's sleeping model; these are research seeds and comparison
facts, not automatically copied product defaults:

- A body's sleep speed measures its fastest point, `|v| + |ω|·extent`, and the
  solver also observes position correction at half weight (pinned
  `src/solver.c`).
- The default speed threshold is `0.05 m/s` (`sleepThreshold` in pinned
  `src/types.c`) and the quiet duration is `B2_TIME_TO_SLEEP = 0.5` simulated
  seconds (pinned `src/constants.h`).
- Sleeping is island-based: any one unquiet body keeps its whole island awake,
  and the island sleeps together (pinned `src/solver.c`). Sleeping bodies and
  their touching contacts are retained in sleeping solver sets and restored on
  wake (pinned `src/solver_set.c`).
- A new touching contact wakes the sleeping set before that contact is solved
  (pinned `src/island.c`).
- Moving a body's transform does not itself wake touching neighbors; Box2D
  exposes a separate explicit `b2Body_WakeTouching` operation (pinned
  `src/body.c`).

The remaining research is behavioral capture through the existing
process-isolated oracle, not source instrumentation:

- add sleep-enabled per-step traces using public `b2Body_IsAwake`;
- record first-sleep and wake steps for isolated circles/rectangles, stacks,
  pyramids, two independent structures, grabs, impulses, and projectile
  impacts;
- run 30/60/120 Hz and relevant substep rows to separate elapsed simulated
  time from outer-step count effects;
- verify whole-island simultaneous sleep, same-step contact wake, and the
  velocity treatment at the sleep transition;
- overlay Box2D transitions with PhysBox farthest-point-speed histories, using
  `0.05 m/s` / `0.5 s` as seeds for PhysBox's own declared policy.

### S23.1 frozen experimental seed and evidence

Clean revision `6c70367` produced
`/tmp/tmp.uIcbmASFU9/s23.1/manifest.json` on the AMD Radeon 890M RADV Vulkan
adapter. All 40 rows pass: eight governed fixtures at 30/60/120 Hz with four
substeps and at 60 Hz with one/eight substeps. Every Box2D result and public
per-step trace replays exactly against clean authority revision
`3a4f0da8374af61293a03021c9a0b3ebcfe67948`; all 25 action-free PhysBox GPU
histories also replay exactly. The bounded generated-contact history runner
does not consume neutral actions, so its impulse, projectile, and grab rows
are explicitly reported unsupported rather than silently projected without
their action. Box2D owns those three behavioral transition traces.

Experimental `SleepPolicyV1` is now version 1 and freezes:

- quiet metric: farthest-point speed `|v| + |omega| * radial_extent`;
- speed threshold: `0.02 m/s`, above the selected soft solver's recorded
  `0.007701 m/s` settled residual and below Box2D's `0.05 m/s` seed;
- quiet duration: 500 simulated milliseconds, converted without floating-
  point rounding to `ceil(500 * fixed_hz / 1000)` integer outer steps;
- transition velocity: zero linear and angular velocity;
- grouping vocabulary: per-body timers with bounded contact-neighbor admission
  and wake propagation, whose round count remains an S23.3 decision.

The evidence observes zero farthest-point speed at every Box2D sleep
transition. At the ordinary 60 Hz/four-substep row, Box2D first sleeps the
isolated circle/rectangle at step 62, the four-stack together at step 35, and
the pyramid together at step 30. The PhysBox sleep-off overlay first reaches
the experimental quiet-duration candidate at steps 57/67 for the isolated
circle/rectangle; the four stack bodies qualify individually at steps 31 and
40, exposing the admission-coordination question before any transition is
implemented. Applied impulse and grab wake on the first
completed step after the step-120 action (step 121); the projectile wakes its
own body at step 121 and all four stack bodies together on contact at step 195.
The public API exposes awake state but no internal transition cause, so the
artifact labels those causes as stimulus inferences rather than authority
facts.

Construction, erase/reuse, reset, and automatic support-motion waking are
specified primarily as PhysBox invariants because they do not map cleanly
through the neutral Box2D scenario contract.

### S23.2 inert representation checkpoint

Implementation commit `47542c0` establishes the representation seam without
changing physics behavior:

- a 32-byte `SleepStateV1` lane is allocated for every body slot and records
  generation, awake/eligible flags, quiet and transition steps, cause bits,
  policy version, and layout version;
- one GPU pass scans the S22 sorted live list, writes a sorted dynamic-only
  awake list plus count/indirect arguments, excludes fixed bodies, and
  initializes a changed generation awake;
- construction, erase/reuse, failed mutation, reset, sparse authored slots,
  and full-capacity lifecycle cases have direct readback assertions; and
- `PhysicsConfigurationV2`, launch parsing, browser preference version 3, and
  scene contract version 9 transport and distinguish requested from effective
  sleeping. Effective sleeping is hard-false.

UI addendum commit `a1b5453` exposes that existing request as
`MENU → WORLD → SLEEP REQUEST`. The engine-rendered control rebuilds the full
shared configuration immediately, preserves capacity and tool state, and
labels both states (`REQUEST ON/OFF`, `EFFECTIVE OFF`) so it cannot imply that
sleep transitions exist. Browser validation covers on, persisted reload, and
off through the actual canvas control. With no physical device available, the
hardware-backed `jstorrent-tablet` AVD exercised the same control, reported
requested true/effective false after a successful scenario, produced a
visually complete capture, and shut down successfully. This addendum does not
begin S23.3 or change the off default.

The awake list is deliberately not consumed by integration, solver-body work,
or solver points. Every live dynamic body remains awake and eligible, so S23.2
cannot conceal an awake solver outcome. The correctness-first compaction pass
currently uses one GPU invocation to scan live slots. That deterministic form
is sufficient to freeze layout and lifecycle semantics, but it is not a
parallel-compaction or mostly-sleeping scale result; parallelization must be
qualified before awake work becomes a performance claim.

Clean revision `47542c08b572316dccc14b678d1be778a012d761` produced
`/tmp/physbox5-s23.2-47542c0/sleep-policy/manifest.json` on AMD Radeon 890M
RADV Vulkan. All 40 authority rows pass, all 25 supported action-free PhysBox
histories replay exactly, and their snapshot histories equal the S23.1
`6c70367` baseline exactly. The clean sparse-capacity artifact at
`/tmp/physbox5-s23.2-47542c0/sparse-capacity.json` is replay-exact and
overflow-free through the 10K reserve tier while reporting the new state/list
allocation explicitly. Ordinary/solver-lab browser, headless requested-on,
ARM64, dual-ABI release-native, release APK, and authorized hardware-backed AVD
qualification pass. No physical device was available for this closure; the
AVD used the host Radeon Vulkan adapter, produced a visually complete capture,
and shut down successfully.

### S23.3-S23.5 effective sleeping checkpoint

Commits `09f5e2e` and `c900a17` make the previously inert representation
effective. Wake policy V1 uses `K = 4` deterministic generational
contact-neighbor rounds and a `0.05 m/s` persisted-impact threshold. Direct
grab, construction, reset, erased-support, new-contact, and impact roots are
combined as cause bits before solve. The original checkpoint observation that
the `K+1` body woke on the next outer step was later identified as an
implementation defect: every awake body was being seeded as a contact root on
every step. The post-review correction below supersedes that observation.

Post-solver admission evaluates the frozen `0.02 m/s` farthest-point speed and
500 ms integer duration: exactly 15, 30, or 60 outer steps at 30, 60, or 120
Hz. A transition zeros linear and angular velocity; sleeping poses do not
drift. The engine-rendered request toggle now reports effective sleeping on
when selected, without changing the off default.

S23.5 consumes the awake representation. Solver points touching at least one
awake dynamic are compacted densely, solved, and scattered through a
dense-to-live mapping so persistent contact identity and cached impulses keep
their original order. The public awake list stays sorted and dynamic-only. A
separate bounded solver-body list contains those awake dynamics plus fixed
endpoints required by active contacts; this distinction is necessary for
fixed-support topology without reintroducing sleeping dynamics. Live SAP,
narrowphase, persistence, support, and rendering still include sleepers.
Focused readback observes zero body/point solve work for dormant contacts and
full restoration on a grab wake. Exact replay across submission boundaries,
the fixed-floor grab regression, the maintained product suite, and the
ordinary fast/browser lane pass. S23.6 establishes that the correctness-first
compactors yield a useful bounded mostly-sleeping curve on the named native
adapter.

### S23.6 sleeping-city evidence checkpoint

The clean `d2a94fa` evidence artifact matches 1,000 live bodies at 1%, 10%,
and 100% awake under one 1,024-slot configuration. Awake solver bodies are
10/100/1,000 and awake points are 6/66/666 while live collision and
persistence retain 666 points in every row. GPU end-to-end P50 is
`5.307/6.563/7.450 ms`; the 1%-awake row is 28.8% below the dense control.
Scheduler body slots and conflict-winner work follow the awake counts, all
rows replay exactly, timestamps are valid, and no capacity overflows.

The 1%-awake reserve comparison remains bounded across 256/1K/10K capacity
with P50 `4.431/4.405/4.864 ms` (max/min `1.104`) while allocation grows
honestly. Collision plus persistence occupies 26.9% of the matched 1%-awake
P50 and persistence alone 0.9%. Although live points outnumber awake points
111:1, the predeclared 50% dominance gate is not met, so S23.6 records
`retain-live-pair-processing`. Dormant sleeping-pair records are not admitted.
This measurement supports a limited mostly-sleeping capability, not a 60 Hz,
10K, 100K, rendered-world, or CPU-crossover claim.

Proportional native, browser, ARM64, APK, hardware-backed AVD, and physical
Pixel 7a gates pass. Browser, AVD, and Pixel all exercise the actual
engine-rendered toggle and visibly report `SLEEP REQUEST ON  EFFECTIVE ON`;
the shared rigid-floor scenario completes, the emulator shuts down, and the
physical product app is stopped after capture. These checks validate the
implementation boundary. S30's later qualification and the recorded S23.7
human disposition complete that review without changing this evidence.

### S30 configurable grouping and cycle-diagnostic checkpoint

S30 completes the interactive causal experiment requested after S27. The
versioned launch configuration and engine-rendered WORLD page expose one
bounded sleep-speed threshold to both PhysBox and Box2D and three PhysBox-only
groupings: local-4, local-8, and exact. Browser preference version 4 and shared
query projection preserve the values across reload. Box2D receives only its
public threshold; no private timer, island, or wake-cause state is patched or
shown.

Exact rebuilds current touching dynamic-dynamic components on the GPU,
reduces/broadcasts wake roots before awake compaction, and reduces/broadcasts
readiness blockers before admission. A deterministic 16-body chain wakes
5/9/16 bodies in local-4/local-8/exact and reports 1/1/0 mixed cuts. Public
cycle diagnostics report per-step sleep/wake cohorts, current mixed cuts,
accumulated awake-body-time, and stable sleep only after the full quiet
confirmation horizon. Interactive delivery remains asynchronous; bounded
headless validation may read the final diagnostic after advancement.

The motivating 100-body poured pile at a shared `0.05 m/s` threshold gives the
important behavioral result: local-4 and local-8 retain all 100 bodies awake
through step 1,200, while exact reaches stable all-body sleep at step 189;
matched-threshold Box2D reaches stable all-body sleep at step 192. Awake-body
time is 118,881/118,154/18,800/12,800 respectively. The result confirms that
bounded grouping can interact with settling exactly as interactive review
suggested, while Box2D's lower awake-body-time confirms that exact PhysBox
components are not an implementation claim of Box2D island equivalence.

On the clean matched 1%-awake 1K-body native GPU cost row,
local-4/local-8/exact record 1,527/1,543/1,560 dispatches and GPU P50
6.294/6.374/5.408 ms; exact allocates 8,288 bytes more than the bounded modes.
All rows replay exactly and remain capacity-clean. Because that evidence spans
one adapter and the natural behavioral win spans one dense fixture, the
unattended S30 implementation retained exact as the recommended explicit
comparison rather than silently replacing local-4. The later S23.7 disposition
also retained sleeping off/local-4 at that checkpoint. Complete provenance and
cross-host gates for that historical disposition are in
[tactical S30](../tactical/030-configurable-sleep-grouping.md).

### S32 default-promotion decision

Subsequent direct interactive comparison changed the product decision without
changing S30's measurements. Exact visibly removes the small-island
sleep/support/wake cycle and tracks Box2D's whole-pile transition closely
enough to be the more useful ordinary behavior. On 2026-07-23 human review
therefore explicitly promotes sleeping on with exact
current-contact-component grouping. The shared threshold remains
`0.02 m/s`; this decision does not adopt the `0.05 m/s` diagnostic threshold
used by S30's poured-pile comparison.

The default selection pays bounded, capacity-derived GPU work rather than CPU
readback. At the 64-slot rigid-floor smoke, sleep-on exact uses 175 dispatches
and 819,940 bytes versus sleep-on local-4's 158 dispatches and 819,332 bytes.
S30's 1K 1%-awake evidence records exact at 1,560 dispatches and 13,035,892
bytes versus local-4 at 1,527 and 13,027,604; exact was not slower in that
sample. These are accounting and one named-adapter measurements, not a
universal performance-rate claim.

Sleeping remains independently switchable off and grouping remains selectable
among exact, local-4, and local-8. Sleep-off still owns awake solver-quality
acceptance, and comparator mode remains excluded from performance evidence.
S32 owns implementation, migration, and proportional cross-host promotion
gates.

### Post-S32 pair-slot span correction

Commit `32dd77d` corrects the contact scan consumed by exact grouping, bounded
wake/blocker propagation, and cycle diagnostics. Stable-pair persistence
reserves two solver-point slots per committed manifold. Circle manifolds
normally fill only the first slot, producing valid indices `0, 2, 4, ...`;
the prior sleep passes instead dispatched the committed scalar-point count and
treated that count as a dense prefix. They therefore omitted roughly half of
a circle pile's contact graph. Rectangle manifolds commonly fill both slots,
which is why the rectangle bucket did not expose the defect.

The selected scan now visits both reserved slots for every committed
stable-pair manifold. The pair bridge writes the invalid-point sentinel to
every solver-point slot before conditionally writing current live points on
every step. An unused second slot, including one left by a manifold changing
from two points to one, is therefore inert. Full-key oracle storage retains
its genuinely packed scan, and the superseded packed stable-pair scan remains
reachable only as a solver-laboratory regression control.

Clean artifact
`/tmp/physbox5-circle-sleep-promoted-32dd77d.json` at revision `32dd77d`
records the 1,200-step A/B. Exact pair-slot scanning sleeps all 100 circles
together at step 145 with 100 sleep events, zero wakes, and zero mixed-state
dynamic pairs. The packed control ends 95 awake / 5 asleep after 470 sleep
and 465 wake events with 9,924 mixed-pair samples. Both rectangle exact lanes
sleep all 100 bodies at step 195. Corrected local-8 also ends with all circles
asleep and zero mixed pairs; corrected local-4 retains its intentionally
bounded behavior, ends asleep after recurring local transitions, and reduces
mixed-pair samples from 8,436 to 414.

Matched clean 1,000-body sleeping-city artifacts compare 20 retained native
GPU timestamp samples before (`dcfb555`,
`/tmp/physbox5-sleep-scan-packed-timing-dcfb555.json`) and after (`32dd77d`,
`/tmp/physbox5-sleep-scan-promoted-timing-32dd77d.json`). Solver-plus-sleep
P50 changes by +0.07% / +0.08% / +0.21% for local-4/local-8/exact; P95 changes
are at most +0.16%. Dispatch counts and allocations are unchanged. These
named-adapter measurements reject a material regression for this promotion;
they are not a universal performance-rate claim.

Promotion qualification passes the maintained production fast lane and
clippy, including the real browser WebGPU smoke; a fresh 1,200-step headless
`circle-bucket-100` reports 100 sleeping circles, zero mixed cuts, stable
sleep at step 145, and clean capacity. The ARM64 check, release-native APK,
and physical Pixel 7a/Mali-G710 circle-bucket capture also pass. The physical
capture reports exact grouping, effective sleeping on, 546 solver dispatches,
and no capacity overflow.

### Post-review Box2D alignment correction

Interactive visualization revealed that even two stationary touching bodies
could alternate sleep and wake forever when their per-body quiet timers were
out of phase. Review against pinned Box2D showed the cause: PhysBox seeded
every awake body as a contact root every step and admitted sleep independently,
whereas Box2D wakes on started touching and prevents a ready member from
sleeping while its island has an unready member.

Commit `8859276` corrects those invariants in the selected bounded GPU form.
New persistent pairs, impacts, grabs, reset, construction, and erased support
remain roots; persisted resting contacts do not. Per-body counters saturate,
not-ready blockers propagate for four rounds through the same
generation-checked contact graph, and unblocked bounded neighborhoods admit
together. A deterministic two-body test starts counters 29 steps apart,
observes a simultaneous step-30 transition, and observes no recurrence.
Allocation is unchanged; sleeping-enabled execution adds nine dispatches
(145 to 154) while default-off execution retains 145. The tactical records
the passing maintained/browser/ARM64/APK/headless/Pixel qualification and
artifact hashes. This is a corrective clarification of experimental policy V1
before promotion, not a default change or a claim of whole-island equivalence.

## Selected implementation direction

Per-body sleep timers with a configurable GPU contact-grouping decision:
exact GPU current-contact components are the default, while local-4 and
local-8 remain bounded-radius comparison controls. One semantic implementation
serves every capacity tier; no grouping requires CPU readback.

Each quiet timer remains per-body. Under a local grouping, a ready counter
saturates instead of immediately sleeping while a not-ready contact neighbor
reaches it within `K` rounds, and the bounded neighborhood admits together
once no blocker reaches it. Paths longer than `K` can still partially sleep or
wake. Under exact grouping, current touching dynamic-dynamic components
reduce/broadcast roots and blockers so no component edge crosses the
awake/asleep cut. This is not Box2D's persistent island/cold-storage
implementation, but it supplies the whole-current-component transition
invariant on the GPU. The ≤64-tier workgroup-island representation is
explicitly not reused, so sleep semantics cannot depend on the capacity
preset.

### Live bodies and awake bodies are different sets

The S22 active list is the sorted live-slot list: it drives collision,
rendering, and construction. Sleeping adds a second, sorted awake-dynamic list
with its own GPU count and indirect arguments. Sleepers stay in the live list
— they must keep rendering, keep their broadphase proxies, keep persistent
contacts and cached impulses, and keep providing collision support. Only
solver body work and solver points compact to awake work.

SAP, narrowphase, and persistence continue to run over all live bodies
including sleepers. S23.6 did not admit frozen or dormant sleeping-pair
records: collision plus persistence was only 26.9% of the matched 1%-awake
P50, below the predeclared dominance gate. Any later change remains a
significant persistence-contract decision requiring new evidence.

### Versioned sleep state

A `SleepStateV1` lane, generation-checked against body slots, records at
least:

- body generation;
- awake/asleep and sleep-eligible flags;
- quiet outer-step count;
- last transition step;
- last transition cause bitset;
- policy/layout version.

Causes are a bitset combined by atomic OR, so grab, reset, new contact, impact,
and propagation arriving in one step produce a deterministic record rather
than a first-writer race. An ordinary persisted resting contact is not a wake
event. Slot reuse initializes the new generation awake, and every stale wake
seed is made inert by the generation check. Timers are integer
outer-step counts derived from configured fixed `dt` (the `0.5 s` seed is 30
outer steps at 60 Hz), keeping sleep in simulated time independent of wall
pacing and substeps.

### Step ordering

The frozen shape of one admitted outer step:

```text
GPU construction/body/grab commands ─┐
live SAP → narrowphase → persistence ├→ generation-checked wake roots
                                     └→ K bounded contact-neighbor rounds
                                           ↓
                              stable awake-body/awake-point compaction
                                           ↓
                                solve and integrate awake work
                                           ↓
                          quiet evaluation and sleep transitions
                                           ↓
                                    render all live bodies
```

Direct actions wake in the same admitted outer step. Impact waking runs after
contact generation but before solving, so a newly awakened body participates
in the impact that woke it. Sleep admission runs after solving and takes
effect the following step. Rendering always draws all live bodies.

### Wake trigger semantics

Specified explicitly; each trigger records its cause bit:

- Grab begin and every held-grab step wake the body in the same step; a held
  body is sleep-ineligible.
- Reset or a support/transform change wakes the body and seeds its retained
  contact neighbors. This deliberately diverges from Box2D, which leaves
  neighbor waking to the explicit `WakeTouching` call; PhysBox support motion
  wakes touching bodies automatically.
- Construction: a new generation starts awake.
- Erase: retained neighbors are woken before the old generation's pairs
  retire.
- Reuse: wake records from the old generation cannot affect the new one.
- A new speculative or physical contact involving an awake body wakes the
  sleeping endpoint before solve.
- A significant persisted-contact impact wakes via a declared pre-solve
  closing-speed or predicted-impulse rule; ordinary resting contact must not
  wake its endpoints every step.
- Configuration or world rebuild initializes live dynamics awake with a
  recorded configuration-change cause.
- Global reset restores the authored state; ordinary product scenarios author
  bodies awake.

### Bounded propagation policy

The neighbor-wave bound `K` is a named policy with evidence, not an incidental
loop constant. A small predeclared comparison — 1/2/4/8 rounds on 16- and
64-body structures — selects one value from wake latency, false support,
dispatch cost, and mobile behavior. Fixtures cover contact-path lengths at
`K-1`, `K`, and `K+1`, and reports state both root-wake latency and
propagated-wake latency in outer steps. Zero-step presentation frames do not
advance propagation.

## Toggle and experimentation surface

Sleeping must be reversible and comparable from day one:

- `sleeping_enabled` lives in the launch-owned versioned physics
  configuration, its configuration hash, the scene contract, the web query
  mapping, and the engine-rendered `MENU → WORLD` request control, following
  the S19 ownership pattern: one projection into `EngineConfig`, adapters
  transport it unchanged, and requested and effective values are both
  reported. The control changes only requested state and explicitly labels
  effective state separately. S32's explicit review selects the on default;
  the same control preserves a direct opt-out.
- Sleep-off and sleep-enabled share one operation graph. Sleep-off forces
  every live dynamic body into the awake list and resets/ignores timers; it
  must not select another solver, pipeline layout, or compilation path.
  Before the first sleep transition, sleep-enabled execution must match
  sleep-off exactly or within the existing exactness contract. This is the
  standing equivalence gate that keeps the toggle honest.
- Sleep policy is versioned and named like solver profiles: a policy id plus
  its complete parameters — quiet predicate, speed threshold, angular/extent
  term, quiet duration, propagation bound `K`, impact-wake rule, and
  transition velocity treatment — recorded in the scene contract and
  validation artifacts so any screenshot or metric is reproducible.
  Experimental policies register through the same registry pattern as solver
  profiles and may be hidden behind laboratory access, but a hidden preset
  may not change the ordinary default. S30 promotes the bounded speed
  threshold and grouping choice into public configuration while the remaining
  policy constants stay fixed.

### Interactive sleep-state visualization

`MENU → WORLD → SLEEP VISUALS` is a presentation-only, engine-rendered review
mode. It is off by default, is not part of `PhysicsConfigurationV3`, and
survives an in-session scenario or physics-configuration rebuild so a reviewer
can compare states without re-enabling it. Turning it on does not request
sleeping; useful review normally enables `SLEEP REQUEST` as well.

Four states are deliberately exposed. More permanent colors would make piles
harder to read, while wake cause remains better represented as a short-lived
transition or an interaction-held mark:

- teal with the ordinary fill: active awake;
- amber with a bottom progress bar: below the speed threshold and accumulating
  quiet outer steps toward admission;
- blue with a procedural `Z`: asleep;
- pink with a procedural `!`: recently woken, or held visibly for the duration
  of an active GPU grab that forces the body awake.

Fixed bodies retain their gray styling. Classification reads generation- and
layout-checked `SleepStateV1` plus the existing GPU grab state in the body
vertex shader; it introduces no simulation-dependent CPU readback, no new
physics state, and no alternate host path. UI report version 9 exposes
`sleepVisualsEnabled`; the compact Android marker reserves bit 30 for the same
presentation state. The normal renderer remains pixel-compatible when the
mode is off.

## Validation contract

- Behavioral fixtures: isolated circle/rectangle rest, stacks, pyramids, two
  independent structures, grab begin/hold/release, applied impulse, and
  projectile impact, each reporting first-sleep step, wake steps, causes, and
  awake counts; propagation chains at `K-1`/`K`/`K+1`.
- Equivalence: sleep-off versus pre-first-transition sleep-enabled exactness;
  exact replay, capacity cleanliness, and fail-closed overflow for the new
  state lane and compaction.
- Lifecycle negatives: construction, erase, reuse, reset, and rebuild against
  authored sleepers, proving generation-inert stale seeds and neighbor waking
  before pair retirement.
- Oracle comparison: Box2D sleep traces as the behavioral reference with the
  declared divergences (bounded partial-stack sleep or wake waves beyond `K`,
  automatic support-motion waking) named in the report rather than averaged
  away.
- Performance: matched 1%/10%/100% awake sleeping-city curves under the
  performance topic's validity rules; the sparse-capacity probe stays flat;
  a dense awake control still grows honestly. These curves also decide the
  conditional dormant-pair question.
- Cross-host: this is an intentional cross-host physics-visible change, so
  qualification includes the ordinary browser representative, the ARM64
  Android check, a release APK, and one physical Android validation when
  available; a hardware-backed AVD is the permitted recorded fallback.
- The mask rule is a decisive quality gate in every package: any structure
  whose sleep-enabled outcome differs from its sleep-off outcome is reported
  as masked, with the sleep-off verdict retained as the solver verdict.

## Execution plan

Tactical S23 (`023-gpu-sleeping-and-waking.md`) executes this topic in ordered
slices; the tactical owns commands, artifacts, and checkbox state, while this
table stays the feature-level overview:

1. Freeze policy vocabulary, capture Box2D oracle sleep traces, and record
   sleep-off baselines for every governed fixture.
2. Add versioned GPU sleep state and deterministic awake compaction while
   sleeping remains disabled; prove sleep-off equivalence and replay.
3. Implement generation-safe wake roots and bounded K-round propagation
   against authored sleepers; select and freeze `K` with the predeclared
   comparison.
4. Add automatic sleep admission: integer outer-step timers, quiet
   evaluation, cause bitsets, and no-drift guarantees for sleeping poses.
5. Drive solver bodies and solver points from awake work while retaining live
   collision, persistence, and rendering over sleepers.
6. Produce matched 1%/10%/100% awake performance curves; take the
   dormant-pair optimization decision only from that measurement.
7. Cross-host qualification, mask-rule disclosure, and the manual
   default-promotion review. S30 plus the initial 2026-07-23 S23.7 disposition
   completed the opt-in implementation. S32 records and qualifies the later
   sleeping-on/exact default selection.

A hard authoritative rejection in an early slice stops later expensive lanes.
Slice completion requires the tactical's stated gates and fresh artifacts, not
merely code existence.

## Alternatives considered

- **Full island sleeping via GPU connected components** (iterative label
  propagation or union-find passes): retained as the recorded alternative if
  bounded-neighborhood semantics prove behaviorally unacceptable; rejected
  initially because convergence detection without readback either costs fixed
  worst-case rounds every step or reintroduces CPU dependence.
- **CPU-side island management**: rejected; it violates GPU residency and the
  no-readback frame contract.
- **Reusing the ≤64-tier workgroup-island solver representation**: rejected;
  it would make wake semantics depend on the capacity preset instead of one
  shared algorithm.
- **Immediately freezing sleeping-pair collision records**: not admitted;
  S23.6 measured the 1%-awake curve below the predeclared dominance gate. It
  remains a future persistence-contract change only if new evidence reopens it.
- **Sleeping as a settling remedy**: rejected permanently by the
  awake-settling topic; the mask rule above is its enforcement here.

## Open questions

- Whether a later policy version needs an observed position-correction term;
  V1 intentionally freezes farthest-point speed only, and any expansion must
  preserve the S23.1 baseline rather than changing it in place.
- Whether a later policy should alter retained warm-start impulses on wake;
  V1 retains them and the current wake-restoration/exact-replay gates pass.
- Whether later coupled-fluid or broader natural-pile evidence justifies
  changing the selected exact/on defaults or the `0.02 m/s` threshold. Any
  such change requires a new explicit review; it must not be inferred from one
  screenshot or performance row.

## Reconsideration triggers

- The measured 1%-awake curve is dominated by sleeping–sleeping broadphase,
  narrowphase, or persistence work: open the dormant-pair record decision as
  its own contract change.
- Interactive or recorded review rejects partial-stack sleep or wake-wave
  behavior across governed paths longer than `K`: execute the observation-only
  exact-component probe and cost gates in
  [GPU sleep-island connectivity research](gpu-sleep-island-connectivity.md)
  before weakening bands.
- No threshold exists that both sleeps the intended settled structures and
  respects the awake residual band: return to threshold/policy design with
  fresh settled-window evidence rather than widening the rest band.
- Mostly-sleeping scale claims (10K/100K rows): owned and gated by the
  performance topic; this topic supplies the mechanism, not the claim.
- A future joints milestone: joint-connected neighbors must join wake
  propagation; revisit the propagation contract when the first joint exists.
