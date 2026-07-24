# S35: Fluid/solid contact stabilization

Status: active, opened 2026-07-24 by human review.

Governing topics:

- [Coupled liquid/rigid constraint surface](../topics/coupled-liquid-rigid-constraint-surface.md)
- [Physics configuration and solver profiles](../topics/physics-configuration-and-solver-profiles.md)
- [Performance and scale](../topics/performance-and-scale.md)
- [Simulation time and overload](../topics/simulation-time-and-overload.md)

## Goal

Add one explicit, restart-bound fluid/solid coupling-quality choice and use it
to compare the current coupled step against bounded GPU-only stabilization
experiments. The first problem source is deliberately overlapping circle,
rectangle, and mixed-shape construction inside settled water. Placement
remains permissive: the coupled solver must recover rigid nonpenetration rather
than preventing the stress case from being authored.

The ordinary `standard` choice must preserve the current
`rigid-then-coupled-batch` schedule, allocation, dispatch count, and behavior.
The experimental `stabilized` choice may add GPU work, but it cannot become the
default until human review receives fresh quality, cost, capacity, browser,
and Android evidence.

## Exclusions

- Do not reject shape overlap during placement or add a CPU placement mirror.
- Do not change the ordinary soft-rigid profile, water coefficients, particle
  count, particle/body contribution bound, sleeping defaults, fixed delta, or
  overload behavior as part of the first comparison.
- Do not encode a second complete rigid step: it would integrate gravity and
  motion twice rather than isolate contact recovery.
- Do not add a CPU rigid/fluid backend, required readback, native-only
  mechanics, automatic fallback, or platform-specific quality choice.
- Do not promote `stabilized`, create a new ordinary preset, or change a
  numerical acceptance band without a measured human decision.
- Do not claim complete multi-body nonpenetration from a two-body result.

## Ownership constraints

`physbox-launch` owns the external quality names, parsing, defaults, browser
query projection, and persistence. Shared engine Rust owns validation,
restart/rebuild application, scenarios, metrics, reports, and the
engine-rendered control. The one portable WGSL graph owns every mechanical
candidate on desktop, Browser WebGPU, Android, and headless.

The quality choice is separate from the rigid solver profile. It affects only
worlds with particle/rigid coupling, but it remains part of reproducible shared
launch and report state. Changing it is an atomic world restart that preserves
camera and tool state and creates no simulation-dependent migration readback.

## Preliminary diagnosis

Local measurement before this workstream found that a deeply overlapping dry
pair could recover while the corresponding wet pair separated and later
re-entered penetration. Particle/body contribution overflow remained zero.
Reordering the existing fluid batch before the single rigid solve regressed a
rectangle case, and sweeps of existing damping, depenetration, iteration, and
substep controls were not robust across particle-lattice phases.

Those observations justify a permanent reproducer and a dedicated
post-fluid-contact experiment. They are not accepted evidence: S35 must
reproduce them through an engine-owned scenario and fresh artifacts.

## Ordered delivery

### S35.1 — restart-bound quality contract

- [x] Add `standard` and `stabilized` to the shared launch contract with
      `standard` as the fresh default.
- [x] Persist and restore the choice in shared Rust and mirror a non-default
      selection into the browser query.
- [x] Expose the choice through the engine-rendered WORLD menu in the liquid
      sandbox.
- [x] Apply a changed choice by one atomic world restart while retaining
      camera, tool, solver configuration, capacity, and presentation state.
- [x] Report the selected choice and keep `standard` mechanically and
      structurally identical to the pre-S35 path.

Gate: parser, desktop/web/Android transport, persistence/migration, UI action,
restart, feature-on/off, and unchanged-standard scene-contract tests pass.

Implementation checkpoint, 2026-07-24: commits `47b2908` and its following
shared-host slice add the separate launch/configuration choice, versioned
`physbox5.coupling.v1` browser record, non-default query projection,
liquid-only PhysBox WORLD control, atomic restart, and scene/UI reporting.
`standard` remains the default and both choices still execute the unchanged
92-dispatch particle graph at this checkpoint. The ordinary fast lane,
production clippy, comparator feature-off build, parser/persistence tests, and
native shared-host restart test pass. The first fast-lane attempt exposed one
transient generated-contact GPU replay failure that passed immediately in
isolation; a later browser attempt exposed stale scripted WORLD coordinates
after the inserted row. The coordinates are corrected and the complete fast
lane passes.

### S35.2 — permanent wet-overlap corpus

- [x] Add shared circle, rectangle, mixed-shape, and three-or-more-body sources
      that settle water before admitting overlapping construction intents.
- [x] Sweep at least four particle-lattice phases without authoring
      host-specific numerical payloads.
- [x] Record initial, worst, final, and sustained pairwise signed separation;
      separation/re-entry count; maximum linear/angular speed; and finite,
      capacity, contact, contribution, and energy-spike diagnostics.
- [x] Retain a dry control for each rigid arrangement.
- [x] Prove that no pair/contact/contribution overflow can masquerade as a
      stabilization result.

Gate: the current `standard` path reproduces at least one wet-only sustained
overlap or re-entry row while every row is finite and capacity-accounted.

Implementation checkpoint, 2026-07-24: commit `c4d03bb` and its following
evidence slice define 20 shared sources: isolated circle, three-circle,
rectangle, mixed-pair, and mixed-three-body arrangements at four quarter-cell
horizontal phases. Each settles the product water, admits ordinary
`PlaceCircle`/`PlaceRectangle` intents on one outer-step boundary, samples all
dynamic pairs every four recovery steps, and runs a matching dry control.
`0.005 m` is the significant-overlap threshold; raw signed separations remain
in the artifact. The sampler also retains speed, angular speed, energy,
construction, rigid-contact, solver-coloring, particle, contribution, and
capacity diagnostics.

Two independent dirty-tree development audits of the initial single-trial
matrix completed all 40 rows capacity-clean. They found 9 and 8 wet-only issue
sources respectively, with no dry issue, particle/body contribution overflow,
non-finite rigid contact, coloring overflow, or uncolored work. Across those
runs, circle-only three-body, rectangle-pair, and mixed-three-body sources
re-entered; one three-circle row retained significant final-window overlap.
The isolated circle pair recovered in every preliminary fixed row. Which phase
failed varied materially between runs, matching the intermittent symptom, so
the permanent command now runs two dry and two wet trials per source and
reports affected trials and source identities separately.

The resulting dirty-tree repeated-trial audit passes all 80 rows and reports
22 of 40 wet trials across 15 of 20 source/phase identities with a water-only
re-entry or sustained overlap; all 40 dry trials are clear. By arrangement,
affected wet trials are isolated-circle `1/8`, three-circle `2/8`,
mixed-pair `4/8`, mixed-three-body `7/8`, and rectangle-pair `8/8`. Five wet
trials retain significant final-window overlap. Every row remains finite and
capacity-clean with zero particle/body contribution overflow, rigid-contact
coloring overflow, and uncolored work.

Accepted clean-tree evidence, 2026-07-24:
`s35-standard-overlap-clean-20260724` at revision `eb95a19`, artifact
`/tmp/physbox5-s35-standard-overlap-clean-20260724.json`. It passes 80/80
rows, reports 17 of 40 wet trials across 12 of 20 source/phase identities with
a water-only issue, and reports 0 of 40 dry trials. Affected wet trials are
isolated-circle `1/8`, three-circle `1/8`, mixed-pair `3/8`,
mixed-three-body `7/8`, and rectangle-pair `5/8`; two wet trials retain
significant final-window overlap. Source dirty is false, patch digest is null,
and every contribution, rigid-contact finite/coloring, uncolored-work, and
capacity audit is clean. This meets the S35.2 gate while preserving the
observed trial-to-trial sensitivity for candidate comparison.

### S35.3 — bounded stabilization candidates

- [ ] First compare a post-fluid refresh over the complete portable contact
      candidate set plus one position-only nonpenetration iteration.
- [ ] Prevent immediate re-entry by projecting inward relative contact velocity
      only where the positional recovery activates.
- [ ] Account separately for candidate generation, narrowphase refresh,
      positional solve, velocity projection, storage, and overflow.
- [ ] If complete contact discovery dominates cost, compare one conservative
      pre-fluid candidate expansion that proves it misses no accepted
      post-fluid contact. A known-contact-only refresh is diagnostic unless it
      proves complete for the admitted displacement bound.
- [ ] Stop a candidate immediately if it double-integrates time/gravity,
      requires readback, silently drops contacts, or worsens a declared dry
      rigid gate.

Gate: scalar sign/effective-mass checks, GPU finite/capacity checks, dry
controls, wet two-body rows, and wet multi-body rows pass before performance
ranking.

### S35.4 — quality and cost matrix

- [ ] Compare `standard` and each surviving `stabilized` candidate on the same
      revision and adapter with raw retained samples.
- [ ] Report dispatch delta, GPU P50/P95/P99 where available, host completion,
      CPU encode/submit, memory, complete work vectors, and cost per simulated
      second.
- [ ] Include the ordinary 3,952-particle sandbox, 1K/4K water controls, sparse
      rigid controls, and the wet-overlap corpus.
- [ ] Reject timing rows whose quality, finite-state, or capacity gates fail.
- [ ] Present any material regression or reduced interactive scale for human
      review before promotion.

Gate: one candidate must materially reduce worst and sustained overlap across
the declared phase/shape/multi-body corpus without an unexplained dry-quality
regression. Cost is reported, not silently normalized away.

### S35.5 — cross-host qualification

- [ ] Run the ordinary production lane and production clippy.
- [ ] Produce fresh headless standard/stabilized artifacts with complete
      provenance.
- [ ] Run Browser WebGPU mechanics and the engine-rendered toggle.
- [ ] Run ARM64 Android checks, a release APK, and one physical Android
      mechanics/pixel/pacing comparison; use and shut down the authorized
      hardware-backed AVD only when no device is available.
- [ ] Prove compiling the Box2D comparator out leaves the same PhysBox quality
      choices and no foreign runtime dependency.

Gate: every maintained host selects the same mode and scenario contract, all
accepted mechanics rows are finite/capacity-clean, and interactive pacing is
reported honestly.

### S35.6 — human promotion checkpoint

- [ ] Present synchronized standard/stabilized captures plus the complete
      quality/cost table.
- [ ] Decide whether to retain the experiment, revise it, promote it as an
      opt-in quality mode, or make any later default proposal.
- [ ] Record the decision in the governing topics and this index.

No default promotion is authorized before this checkpoint.

## Completion condition

S35 is complete when the restart-bound shared choice and permanent corpus are
implemented, every admitted candidate has a measured disposition, required
cross-host evidence exists for the retained path, and human review records the
product/default decision. A useful negative result may complete the workstream
if all candidates are rejected with their failures and costs preserved.
