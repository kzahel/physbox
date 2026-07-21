# S19: soft default and physics configuration

Status: active, 2026-07-21. Human review selects the interactively preferred
soft-coupled family as the product direction, accepts its current compliance
and tall-stack limitation as a visible starting trade-off, and authorizes a
bounded product configuration for fixed rate, substeps, iterations, and
contact response. The current code and ordinary deployed default remain
hard/direct until the migration slices below make the change truthfully.

Governing topics:
[physics configuration and solver profiles](../topics/physics-configuration-and-solver-profiles.md)
owns the new decision and configuration surface;
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md)
owns the solver and shared-host architecture;
[simulation time and overload](../topics/simulation-time-and-overload.md)
owns fixed-delta pacing and permanent slip;
[awake settling and rest stability](../topics/awake-settling-and-rest-stability.md)
owns the known response evidence; and
[performance and scale](../topics/performance-and-scale.md) owns cost and scale
claims. Predecessor:
[S18 solver check loop and coefficient exploration](018-solver-check-loop-and-coefficient-exploration.md).

## Goal

Turn the current laboratory-quality rigid engine into one configurable product
physics path. Make soft-coupled the ordinary default on every host, preserve
hard/direct as an explicit comparison profile, remove hardcoded 60/4/i2
assumptions from shared runtime ownership, and expose a small set of honest
configuration controls and presets through the existing engine-rendered menu
and `physbox-launch`.

Completion means a fresh browser or native launch selects soft-coupled without
a feature flag or saved preference; the same versioned configuration reaches
the same shared GPU path on desktop, browser, Android, and headless; rate,
substeps, iterations, and contact response can be selected inside bounded
product limits; requested/effective values and cost are reported; and fresh
cross-host evidence characterizes the known physical trade-offs.

## Human decision and acceptance interpretation

Human review on 2026-07-21 records:

- soft-coupled is subjectively much better than hard/direct and is good enough
  to become the foundation for subsequent product work;
- its Box2D-derived soft response is preferred even though tall stacks show
  compliance and rectangle overlap;
- the initial product seed is the already exposed 60 Hz / four-substep /
  one-biased-plus-relaxation branch-robust execution at contact 30 Hz,
  damping 10, and static multiplier 2;
- `branch-robust` becomes a historical/internal alias; the product name is
  **SOFT COUPLED**; and
- configurability is product work, not another closed candidate study.

S18's no-survivor result remains accurate: no tuple passed its frozen 1,200-
step 16-stack gate. That gate no longer decides whether the reviewed soft
family may become the early product default. S19 does not delete or relabel
the failure. The exact first-loss step, penetration, stack height, and drift
remain required diagnostics, and the 16-stack remains a reconsideration
fixture. The initial migration is blocked by non-finite state, invalid contact
keys, capacity failure, replay failure, incorrect physical response, or
cross-host execution failure—not by rediscovering the already accepted step-
407 structural loss.

The ordinary 8-stack, bucket, impact, friction, restitution, reset/grab, and
construction fixtures remain blocking product behavior. Sleeping may not hide
an awake failure. A later preset cannot regress the frozen soft seed silently.

## Exclusions and ownership constraints

- No CPU physics backend, platform fallback, native-only solver, or alternate
  simulation state.
- No variable `dt` inside a running world, adaptive stepping, multi-step
  interactive catch-up, or platform-owned accumulator.
- No fluid, joints, broadphase selection, or sleeping implementation in this
  tactical.
- No automatic fallback from soft to hard after an error. Fail closed and
  report the configuration.
- No arbitrary web DOM settings panel. The browser remains a fullscreen canvas
  and the shared engine renders the controls.
- No platform-owned defaults or parsing. `physbox-launch` owns names, defaults,
  arguments, web mappings, and Android argv; `EngineHost` owns application,
  rebuilds, pacing, reporting, and one GPU state.
- No unbounded Cartesian evidence campaign. Shipped presets receive full
  qualification; custom values receive contract, safety, and honest-status
  coverage without implying a product recommendation.
- No claim of Box2D parity merely because its timestep, substeps, or contact
  tuning can be selected.

## Slices

### S19.1 — decision and configuration contract freeze

- [x] Record soft-coupled as the selected product direction, hard/direct as a
  comparison profile, the initial seed tuple, the accepted tall-stack
  limitation, and the distinction between current code default and intended
  default.
- [x] Define world-time, solver-resolution, contact-response, recovery-policy,
  and solver-family ownership without adding a platform settings model.
- [x] Freeze the zero-or-one interactive step rule for every configured rate
  and require requested/effective settings plus cost in reports.

Gate: the governing topics, tactical index, README, and bring-up direction
agree; no document falsely says that ordinary builds already default to soft.

### S19.2 — canonical configuration and launch ownership

- [x] Add one versioned engine configuration carrying solver profile, outer
  fixed rate, substeps, biased iterations, soft contact tuning, and the
  existing contact-recovery policy. Prefer one canonical rate representation
  and derive `dt` exactly once.
- [ ] Give `physbox-launch` the ordinary soft default, bounded parsing, help,
  web mappings, and Android JSON argv decoding. Preserve explicit aliases for
  old `selected` and `branch-robust` launches during migration, while reports
  emit canonical ids.
- [x] Project launch configuration into `EngineConfig` once and into the
  neutral scenario once. Remove constructor-local replacement of those values.
- [x] Version/hash the complete applied configuration in scene contracts and
  validation provenance; reject duplicate, unknown, non-finite, zero, or out-
  of-range fields before GPU allocation.

Provisional product editor bounds: integer outer rate 15..=240 Hz with named
30/60/120 presets; integer substeps 1..=8; integer biased iterations 1..=4;
and finite contact Hz/damping/static multiplier inside engine-owned ranges.
Explicit lab APIs may retain substeps through 16 and iterations through 8.
S19.2 must freeze or narrow these bounds from validation before public parsing;
it may not widen them silently after release.

Gate: launch/parser/config unit tests prove identical canonical configuration
through native argv, web mappings, Android JSON argv, and saved browser state;
platform adapters contain no physics defaults.

### S19.3 — configurable fixed clock

- [x] Construct `SimulationClock` from configured fixed `dt`; remove
  `FIXED_STEP_HZ`/`FIXED_STEP_SECONDS` as behavioral constants from admission,
  exact stepping, reporting, frame parameters, and simulated-time accounting.
- [x] Preserve pause, resume, rebase, single-step, lifecycle discard, at-most-
  one interactive step, fractional retention, and permanent whole-step slip at
  30, 60, and 120 Hz.
- [x] Apply a rate change only through an atomic world restart/rebuild. Never
  mutate `dt` during an encoded step or retain old-rate accumulator debt.
- [ ] Report requested rate, exact `dt`, effective scale, presentation rate,
  and whether the host is sustaining the request. A 120 Hz world on a 60 Hz
  presentation host must visibly run slow rather than catch up.

Gate: deterministic clock tests cover the maintained cadence/stall/lifecycle/
time-scale matrix at every shipped rate, and no interactive frame reports more
than one outer step.

### S19.4 — parametric soft GPU execution

- [x] Generalize the soft-coupled constructor and pass graph so allowed
  substep and biased-iteration counts are configuration, not validation-only
  source pins. Preserve point-sequential normal/friction ownership, warm start,
  fixed-anchor refresh, branch-robust separation, relaxation, restitution
  placement, coloring, persistence, and fail-closed capacity semantics unless
  a separately named profile changes one.
- [ ] Derive internal substep `dt`, dynamic/static contact-Hz caps, coefficient
  values, pass count, dispatch count, and allocation from the applied
  configuration. Reject integer overflow and configurations exceeding declared
  pass/capacity limits before work is committed.
- [ ] Keep the selected soft seed byte-for-byte or metric-identical to the
  currently reviewed branch-robust 60/4/i1 30/10/staticx2 lane before changing
  its public name/default.
- [ ] Add exact replay and negative-capacity tests at 1/2/4/8 substeps and
  1/2/4 biased iterations, including switch/rebuild tests with no stale cache,
  pass, or parameter state.

Gate: the maintained GPU safety corpus stays finite, replayable, capacity-
clean, and shader-validation clean; configuration changes produce the declared
work graph and never require simulation-dependent CPU readback.

### S19.5 — soft product-default migration

- [ ] Rename the canonical public profile to `soft-coupled`, keep the old
  branch-robust id as an input alias, and label the comparison profile
  `HARD DIRECT`.
- [ ] Make soft-coupled 60/4/i1 30/10/staticx2 the ordinary default in
  `physbox-launch`, `EngineConfig`, scenario construction, fresh browser
  storage, native apps, Android, and headless.
- [ ] Remove the solver-lab feature gate from the selected soft product path.
  Experimental presets may remain gated; the default physics may not.
- [ ] Preserve hard/direct as an explicitly requested registry entry and
  validation control. Prove no runtime failure selects it automatically.
- [ ] Record measured before/after hard-versus-soft response, penetration,
  first-loss, work, GPU time, memory, and capacity results and the human reason
  for accepting the change in the implementation commit and review artifact.

Gate: a fresh ordinary launch on every maintained host reports soft-coupled and
the exact seed configuration without query parameters, feature flags, or prior
local storage. Existing stored branch-robust preferences migrate to the same
canonical profile.

### S19.6 — shared configuration UX

- [ ] Extend the existing n-profile solver menu with `SIM HZ`, `SUBSTEPS`, and
  `ITERATIONS`; retain contact `HZ`, `DAMPING`, and `STATIC SCALE`.
- [ ] Show requested and effective contact Hz when the substep cap differs,
  internal substep rate or `dt`, preset/custom state, and an honest compact
  cost/pacing warning. Do not leave inert values such as 31-120 requested Hz
  looking effective under 60/4.
- [ ] Provide one restore-preset action and atomic apply. Preserve camera and
  tool state where safe, close active grabs/construction transactions
  deliberately, and restart scenario/physics state when required.
- [ ] Version the local-storage schema, migrate `physbox5.solver-lab.v1`, and
  restore complete configuration without requiring query editing. URL
  synchronization remains a reproducibility detail, not the primary UX.
- [ ] Keep the layout usable with at least six profiles on 456x1024,
  932x430, 960x600, and 2400x1080 targets.

Gate: browser automation changes rate, resolution, and contact settings;
verifies the applied scene/configuration contract; switches profiles without
reload; restores after reload; and captures readable phone/landscape menus.

### S19.7 — useful presets and subjective comparison

- [ ] Measure a small 60 Hz soft resolution matrix containing 2/4/8 substeps
  and 1/2 biased iterations on 4/8/16 stacks, rectangle bucket, impact,
  friction, restitution, reset/grab, and construction. Do not assume more work
  improves quality.
- [ ] Ship one `SOFT HIGH QUALITY` preset only if it presents a distinct useful
  response/cost trade-off and remains safe/capacity-valid. Otherwise retain
  only editable soft-coupled plus hard/direct.
- [ ] Re-expose the existing S6 30/10/staticx1 prior-contact soft lane as a
  named subjective profile only if it can be represented by shared solver and
  identity configuration rather than a special scenario.
- [ ] Run an explicit human review of the shipped presets on ordinary and tall
  stacks, piles, impacts, frictional motion, and direct manipulation. Record
  preferred feel and visible drawbacks beside objective metrics.

Gate: every shipped preset has a stable semantic identity, complete applied
configuration, fresh artifacts, and a human-readable reason to exist. No
preset is named `Box2D` or claims parity without matched authority evidence.

### S19.8 — cross-host, performance, and closure

- [ ] Run the production Rust gates plus focused solver-lab/configuration tests,
  wasm and Android checks, boundary checks, browser smoke, release APK, and
  physical Android validation. Use an AVD only when available and shut it down
  immediately afterward.
- [ ] Record default-soft and hard-comparison 32/256/1K complete-step work,
  P50/P95 GPU time, CPU encode/submit, allocation, and capacity on the named
  Metal adapter; record useful browser and Pixel 7a interactive pacing.
- [ ] Report cost per outer step and per simulated second for every shipped
  preset, including effective scale/slip. Do not call slow-time execution
  sustained real-time performance.
- [ ] Update all current-status documentation, examples, screenshots, labels,
  and deployment assertions to the canonical soft default; preserve S6-S18 as
  historical evidence rather than rewriting them.
- [ ] Commit and push the exact validated change, deploy the exact pushed SHA,
  verify public asset hashes, and inspect the live configuration UI at phone
  and desktop sizes.

Gate: the deployed ordinary site and all maintained app defaults select the
same soft configuration; hard/direct remains explicitly selectable; fresh
artifacts expose known limitations and complete cost; no pending deployment or
undocumented platform difference remains.

## Required implementation gates

Run the repository production gates from `AGENTS.md`. In addition, every slice
touching the configuration or soft path runs focused tests with the
`physbox-engine/solver-lab` feature until the path is no longer experimental.
S19.4-S19.8 require fresh GPU-backed tests on the available Metal adapter.
S19.5 and later require browser and physical Android inspection because this
is an intentional physics-default and pixel-visible change.

The default-changing commit body must state:

- hard-before and soft-after profile/configuration ids;
- response, penetration, structural-loss, cost, work, memory, and capacity
  measurements;
- known accepted regressions and limitations;
- exact validation commands, hosts, and artifact paths; and
- the human decision that subjective soft quality justifies building forward
  from the characterized seed.

## Completion condition

S19 completes only when:

1. soft-coupled is the fresh ordinary default on every host;
2. fixed rate, substeps, biased iterations, and soft contact tuning have one
   bounded shared configuration and one engine-rendered interactive surface;
3. the clock, GPU graph, scene contract, persistence, launch parsing, and
   reports all use the applied configuration without a hardcoded 60/4/i2
   behavioral path;
4. hard/direct is a named comparison with no automatic fallback;
5. shipped presets have exact replay, finite/capacity-safe cross-host results,
   honest response limitations, and complete cost per simulated second;
6. subjective review is recorded; and
7. the exact pushed SHA is deployed and verified live.

Failure of a proposed preset removes that preset, not the configuration model.
A fundamental soft-family safety or portability failure stops at human review;
it does not silently restore hard/direct as the selected direction.
