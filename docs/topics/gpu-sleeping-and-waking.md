# GPU sleeping and waking

Status: active design, 2026-07-22. Tactical S23 is authorized through its
inert S23.2 checkpoint; no sleeping behavior is implemented yet. This topic is the
master record for the sleeping/waking feature: its motivation, the provisional
selected direction, the frozen policy vocabulary, wake-trigger semantics, the
toggle and experimentation surface, the validation contract, the execution
plan, and the running status overview. Direction and step ordering below are
provisional until the first tactical slice freezes them with oracle traces and
sleep-off baselines; nothing here changes a product default, and enabling
sleeping as the ordinary default requires a recorded human promotion review.

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
The rigid behavior authority is
[Box2D rigid authority](box2d-rigid-authority.md); contact identity is owned
by [collision and contact persistence](collision-and-contact-persistence.md).
The execution tracker is
[tactical S23](../tactical/023-gpu-sleeping-and-waking.md).

## Feature overview and tracking

This table is the compact answer to "where is sleeping?" Update it in the
same change that materially advances or reverses an area. The capability
ledger rows SOLVE-3 (sleeping and waking) and SCALE-4 (mostly-sleeping scale)
change only with implementation and evidence, not because this design exists.

| Area | Status | Current state / next gate |
| --- | --- | --- |
| Policy vocabulary and oracle traces | active | Freeze quiet predicate, thresholds-as-policy, and Box2D behavioral captures (S23.1). |
| Versioned GPU sleep state | authorized | `SleepStateV1` lane and deterministic awake compaction while sleep stays disabled (S23.2). |
| Wake roots and bounded propagation | proposed | Generation-checked roots plus K-round contact-neighbor waves against authored sleepers (S23.3). |
| Automatic sleep admission | proposed | Integer outer-step timers, cause bitsets, no-drift guarantees (S23.4). |
| Awake-driven solve and integration | proposed | Solver bodies/points sized by awake work; live collision and rendering retained (S23.5). |
| Toggle and configuration surface | proposed | Launch-owned `sleeping_enabled`, shared graph, sleep-off equivalence (S23.2 onward). |
| Sleeping-city performance curves | proposed | Matched 1%/10%/100% awake evidence; conditional dormant-pair decision (S23.6). |
| Cross-host qualification and promotion | proposed | Browser plus physical Android, mask-rule disclosure, human default review (S23.7). |

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
latency. Wake latency, partial-stack sleep, and wake waves are user-visible
consequences of the selected algorithm and need their own fixtures and bands.

## Boundary with awake settling: the mask rule

Sleeping must not rewrite the solver verdict. The selected soft default
already quiets the governed 16-stack far below any plausible sleep threshold
(`0.007701 m/s` settled maximum against the Box2D `0.05 m/s` seed) and then
still loses the structure at step 407 in sleep-off execution. If sleep-enabled
execution puts that stack to sleep before step 407 and preserves its shape,
the result is reported as "sleep masked the awake loss," never as a stability
fix. The same rule covers tall-stack compliance and any structure whose
sleep-off outcome differs from its sleep-enabled outcome. Every promotion
package and the final capability-ledger row must carry this limitation
explicitly. The awake-settling topic's closed no-survivor record and the S18
accepted residual band remain the measured awake input to threshold design;
sleeping reports that provenance rather than erasing it.

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

Construction, erase/reuse, reset, and automatic support-motion waking are
specified primarily as PhysBox invariants because they do not map cleanly
through the neutral Box2D scenario contract.

## Provisional selected direction

Per-body sleep timers with bounded contact-neighbor wake propagation, executed
entirely on the GPU, one shared algorithm at every capacity tier.

Full Box2D-style island sleeping is a connected-components problem that is
awkward without readback, as already recorded in the authoritative topic.
Per-body sleep permits partial-stack sleep and produces measurable wake waves;
both are accepted, declared behavioral differences from the island authority
and receive their own fixtures and bands rather than a silent parity claim.
The ≤64-tier workgroup-island representation is explicitly not reused, so
wake semantics cannot depend on the capacity preset.

### Live bodies and awake bodies are different sets

The S22 active list is the sorted live-slot list: it drives collision,
rendering, and construction. Sleeping adds a second, sorted awake-dynamic list
with its own GPU count and indirect arguments. Sleepers stay in the live list
— they must keep rendering, keep their broadphase proxies, keep persistent
contacts and cached impulses, and keep providing collision support. Only
solver body work and solver points compact to awake work.

For the first correctness slice, SAP, narrowphase, and persistence continue to
run over all live bodies including sleepers. Frozen or dormant sleeping-pair
records are introduced only if the measured 1%-awake curve proves that
sleeping–sleeping narrowphase or persistence dominates; that is a significant
persistence-contract change owned by a separate explicit decision, not
smuggled into the first implementation.

### Versioned sleep state

A `SleepStateV1` lane, generation-checked against body slots, records at
least:

- body generation;
- awake/asleep and sleep-eligible flags;
- quiet outer-step count;
- last transition step;
- last transition cause bitset;
- policy/layout version.

Causes are a bitset combined by atomic OR, so grab, reset, impact, and
propagation arriving in one step produce a deterministic record rather than a
first-writer race. Slot reuse initializes the new generation awake, and every
stale wake seed is made inert by the generation check. Timers are integer
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
  mapping, and the engine-rendered menu, following the S19 ownership pattern:
  one projection into `EngineConfig`, adapters transport it unchanged, and
  requested and effective values are both reported. The ordinary default is
  off until a recorded human promotion review changes it.
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
  may not change the ordinary default. Thresholds and propagation policy are
  initially reported policy constants, not general user tuning knobs.

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
  declared divergences (partial-stack sleep, wake waves, automatic
  support-motion waking) named in the report rather than averaged away.
- Performance: matched 1%/10%/100% awake sleeping-city curves under the
  performance topic's validity rules; the sparse-capacity probe stays flat;
  a dense awake control still grows honestly. These curves also decide the
  conditional dormant-pair question.
- Cross-host: this is an intentional cross-host physics-visible change, so
  qualification includes the ordinary browser representative, the ARM64
  Android check, a release APK, and one physical Android validation.
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
   default-promotion review.

A hard authoritative rejection in an early slice stops later expensive lanes.
Slice completion requires the tactical's stated gates and fresh artifacts, not
merely code existence.

## Alternatives considered

- **Full island sleeping via GPU connected components** (iterative label
  propagation or union-find passes): retained as the recorded alternative if
  per-body semantics prove behaviorally unacceptable; rejected initially
  because convergence detection without readback either costs fixed worst-case
  rounds every step or reintroduces CPU dependence.
- **CPU-side island management**: rejected; it violates GPU residency and the
  no-readback frame contract.
- **Reusing the ≤64-tier workgroup-island solver representation**: rejected;
  it would make wake semantics depend on the capacity preset instead of one
  shared algorithm.
- **Immediately freezing sleeping-pair collision records**: deferred; it
  changes the persistence contract and is admitted only by the measured
  1%-awake curve.
- **Sleeping as a settling remedy**: rejected permanently by the
  awake-settling topic; the mask rule above is its enforcement here.

## Open questions

- The exact quiet predicate: farthest-point speed only, or also an observed
  position-correction term as in the authority, and the declared threshold
  values against the measured soft-default residual band.
- Velocity treatment at the sleep transition (zeroed versus frozen) and its
  interaction with warm-start impulses on wake; decided with oracle evidence
  in slice 1.
- The impact-wake rule's declared closing-speed or predicted-impulse constant.
- The selected `K` and whether 64-body wake waves meet mobile pacing.
- Whether and when `sleeping_enabled` appears in the product menu versus
  remaining a launch/laboratory control before promotion.
- Whether the 1%-awake curve forces the dormant-pair persistence decision.

## Reconsideration triggers

- The measured 1%-awake curve is dominated by sleeping–sleeping broadphase,
  narrowphase, or persistence work: open the dormant-pair record decision as
  its own contract change.
- Interactive or recorded review rejects partial-stack sleep or wake-wave
  behavior on governed structures: evaluate the GPU connected-components
  island alternative before weakening bands.
- No threshold exists that both sleeps the intended settled structures and
  respects the awake residual band: return to threshold/policy design with
  fresh settled-window evidence rather than widening the rest band.
- Mostly-sleeping scale claims (10K/100K rows): owned and gated by the
  performance topic; this topic supplies the mechanism, not the claim.
- A future joints milestone: joint-connected neighbors must join wake
  propagation; revisit the propagation contract when the first joint exists.
