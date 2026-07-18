# Tactical workstreams

Status: S1 and S2 complete; the S3 combined representation gate passes.
Forward manifold-block i2 with bounded parallel coloring is integrated as the
selected solver baseline; the small interactive tier uses island dispatch and
larger tiers retain per-color dispatch. S6.5 is closed as a complete negative study:
human review rejected the one S6.5d candidate, deferred soft settling, and
retained hard/direct manifold-block i2 as the product. Reopening requires the
recorded 16-stack energy and cross-Android portability evidence.
S7 is complete: its Box3D source-only study rejected every broad transplant
hypothesis and its recommended static-late order experiment was authorized by
human review as one arm of S9 rather than a standalone factorial.
Broadphase contract/correctness work remains authorized, while product-scene
ranking and selection wait for the S6 awake gate. S4.3 grab feel and S4.4
construction mechanics are complete.
S8 closed the physical mobile small-world pacing regression with a bounded
dispatch policy and a maintained presentation/physics cadence gate.
S9 is complete as a negative result: matched 16-stack attribution localized
warm-start energy, but every predeclared material arm was discarded, no
combination was eligible, and restitution timing was byte-inert. The selected
hard/direct product remains unchanged.
S10 is active: it follows the S9.2 warm-start localization with a
warm-start on/off differential in both engines, stored-impulse stationarity
measurement against the authority band, and at most two scalar-first
corrections. S10.3 names inter-substep stale-persistence reseeding with a
four-body minimum and records only partial point-normal unification with the
deferred soft failure; S10.4 now tests the routed composition correction.
With no survivor it returns the deferred-soft formulation decision package
instead of further hard-family arms.

Tactical filenames use a three-digit zero-padded prefix ordered by their first
Git creation timestamp. This repository starts at `001`. Never renumber an
existing tactical, recycle a number, or copy PhysBox 4's historical numbering.
This README remains the unnumbered index.

## Workstreams

| ID | Workstream | Document | Status |
| --- | --- | --- | --- |
| S1 | Oracle foundation and rigid solver laboratory | [001-rigid-solver-laboratory.md](001-rigid-solver-laboratory.md) | complete 2026-07-15 |
| S2 | GPU body state and fixed-floor contact | [002-gpu-body-state.md](002-gpu-body-state.md) | complete 2026-07-15; limited body core accepted |
| S3 | Small-world collision and contact persistence | [003-small-world-collision-persistence.md](003-small-world-collision-persistence.md) | combined representation gate passed; S5.4a quality experiment authorized, 2026-07-16 |
| S4 | Interactive evaluation workbench | [004-basic-scene-navigation-camera.md](004-basic-scene-navigation-camera.md) | S4.4 complete; S4.5 construction-feel review ready, 2026-07-16 |
| S5 | Interactive rigid quality, construction, and scale | [005-interactive-rigid-quality-and-scale.md](005-interactive-rigid-quality-and-scale.md) | S5.4c scheduler integrated; S5.5 waits for S6 awake gate, 2026-07-17 |
| S6 | Awake settling stabilization | [006-awake-settling-stabilization.md](006-awake-settling-stabilization.md) | S6.5 closed; soft settling deferred by human review, 2026-07-18 |
| S7 | Box3D planar stability investigation | [007-box3d-planar-stability-investigation.md](007-box3d-planar-stability-investigation.md) | complete; static-late arm later rejected by S9, 2026-07-18 |
| S8 | Physical mobile frame pacing | [008-physical-mobile-frame-pacing.md](008-physical-mobile-frame-pacing.md) | complete; Pixel 7a bootstrap sustains rendering and physics cadence, 2026-07-18 |
| S9 | Awake settling schedule differential | [009-settling-schedule-differential.md](009-settling-schedule-differential.md) | complete; no schedule-differential survivor, 2026-07-18 |
| S10 | Warm-start and stored-impulse stationarity | [010-warm-start-impulse-stationarity.md](010-warm-start-impulse-stationarity.md) | active; S10.3 names stale reseeding, S10.4 next, 2026-07-18 |

## Conventions

- Start with a stable workstream ID, status, goal, and links to its governing
  topic and bring-up stage.
- State explicit non-goals and ownership boundaries so a slice cannot expand
  the product accidentally.
- Divide work into ordered, independently verifiable slices with checkboxes.
- Put commands, numerical gates, artifacts, and completion criteria in the
  tactical rather than relying on session memory.
- Status is one of `proposed`, `active`, `manual checkpoint`, `blocked`,
  `complete`, or `superseded`; include a date when changing it.
- Update the index and tactical status in the same commit as the evidence.
- Do not mark a slice complete merely because code exists: its stated gates and
  fresh artifacts must pass.
- Preserve completed tacticals as decision history. A replacement links the
  superseded document rather than rewriting its original intent.

## Creating the next tactical

Find the largest existing numeric prefix, add one, use a descriptive kebab-case
suffix, and add the new row here. If multiple tacticals are first committed
together, record their intended order in this index before creating later
numbers.
