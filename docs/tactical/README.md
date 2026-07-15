# Tactical workstreams

Status: S1 and S2.1–S2.3 complete; S2.4 reset/grab implementation is awaiting
cross-host evidence, then scale evidence remains before the body-layout and
command-ABI human checkpoint.

Tactical filenames use a three-digit zero-padded prefix ordered by their first
Git creation timestamp. This repository starts at `001`. Never renumber an
existing tactical, recycle a number, or copy PhysBox 4's historical numbering.
This README remains the unnumbered index.

## Workstreams

| ID | Workstream | Document | Status |
| --- | --- | --- | --- |
| S1 | Oracle foundation and rigid solver laboratory | [001-rigid-solver-laboratory.md](001-rigid-solver-laboratory.md) | complete 2026-07-15 |
| S2 | GPU body state and fixed-floor contact | [002-gpu-body-state.md](002-gpu-body-state.md) | active; S2.3 complete 2026-07-15 |

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
