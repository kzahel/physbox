# S23: GPU sleeping and waking

Status: active 2026-07-22. Human authorization permits unattended execution
through S23.2 and requires a stop before wake propagation. No physical Android
device is currently available; the closing Android behavior lane may use the
hardware-backed AVD as the explicitly authorized substitute, must record that
limitation, and must shut the emulator down after the run.

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

- [ ] Version the policy and transition vocabulary: farthest-point speed,
  quiet duration expressed in outer steps, threshold source, transition
  velocity treatment, transition/wake causes, requested/effective state, and
  declared Box2D-versus-PhysBox differences.
- [ ] Extend the process-isolated Box2D lane to capture public per-step awake
  state and farthest-point speed without source instrumentation.
- [ ] Record the governed authority fixtures at 30/60/120 Hz with four
  substeps and at 60 Hz with one/eight substeps: isolated circle and rectangle,
  stack, pyramid, two independent structures, applied impulse, projectile
  impact, and grab begin/hold/release where the neutral contract supports it.
- [ ] Record matching PhysBox sleep-off motion histories and exact replay for
  the governed fixtures, naming any fixture that cannot share an action
  projection rather than manufacturing parity.

Gate: a clean, fresh manifest records source/oracle pins, complete
configuration, trace hashes, first-sleep and wake observations, transition
velocity evidence, and every declared unsupported comparison. The maintained
oracle process tests and the directly affected evidence target pass.

### S23.2 — inert GPU state and awake compaction

- [ ] Add a versioned, generation-checked `SleepStateV1` GPU lane with awake
  and eligible flags, quiet-step count, last-transition outer step, cause bits,
  policy version, layout version, and deterministic reset/lifecycle behavior.
- [ ] Add a sorted awake-dynamic slot list, count, and indirect arguments,
  compacted from the sorted live list on the GPU before the solver seam.
- [ ] Keep automatic sleep and solver filtering disabled: every live dynamic
  body is awake, every fixed body is excluded, and the selected solver still
  consumes its existing live-work inputs.
- [ ] Transport and report versioned requested/effective sleeping state without
  exposing an ordinary product-menu default; effective sleeping remains false
  at this checkpoint.
- [ ] Prove sparse authored slots, construction, erase, generation reuse,
  failed mutation, reset, capacity-full behavior, and exact replay. Stale state
  from an older generation must be inert.
- [ ] Prove sleep-off results are exact before and after the inert graph change,
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
- [ ] Return default state, menu exposure, and any dormant-pair follow-up to a
  human promotion decision.

Gate: a human review explicitly promotes or retains sleeping as opt-in; no
test, evidence tool, or unattended run may make that product decision.

## Completion condition

S23 is complete only when sleeping and waking are fully GPU-owned, versioned,
observable, generation-safe, bounded, exact-replaying, capacity-clean,
behaviorally disclosed against Box2D, measurably reducing mostly-asleep work,
qualified on the maintained hosts, and given an explicit human default
decision. The authorized S23.2 stopping point is an implementation checkpoint,
not feature completion.
