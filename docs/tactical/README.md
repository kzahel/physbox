# Tactical workstreams

Status: S1 and S2 complete; the S3 combined representation gate passes.
Forward manifold-block i2 with full rebuild/per-color dispatch is integrated
as the selected solver baseline. Human review accepted the clean S5.4c bounded
parallel-coloring recommendation, and S6 is actively integrating it before
measuring awake settling.
Broadphase contract/correctness work remains authorized, while product-scene
ranking and selection wait for the S6 awake gate. S4.3 grab feel and S4.4
construction mechanics are complete.

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
| S6 | Awake settling stabilization | [006-awake-settling-stabilization.md](006-awake-settling-stabilization.md) | S6.5a traces attribute AVD collapse to amplified contact feedback; depth minimization active, 2026-07-17 |

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
