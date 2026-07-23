# S28: Browser platform-boundary hardening

Status: active, 2026-07-23

## Goal

Make the browser a mechanically small, engine-agnostic input/display adapter
and guard that shape against future JavaScript or TypeScript duplication.
Shared Rust must own browser preference/query policy, while semantic browser
automation must be isolated from ordinary product operation.

This work is governed by
[platform architecture](../platform-architecture.md) and
[physics configuration and solver profiles](../topics/physics-configuration-and-solver-profiles.md).
It preserves the one-host and launch-ownership contracts in `AGENTS.md`.

## Exclusions

- No physics, solver, tool, grab, placement, menu, camera, pacing, or rendering
  behavior change.
- No new product UI, DOM settings model, debug panel, or browser-specific
  default.
- No storage reset. Existing `physbox5.physics.v3`,
  `physbox5.world.v1`, and supported legacy records remain readable.
- No change to scenario outcomes, validation tolerances, or performance gates.
- No deployment or push.

## Ownership constraints

- `physbox-launch` owns browser storage keys and schemas, migrations, field
  validation, default elision, and web-query projection.
- `physbox-engine` exposes only the effective shared launch selection needed
  for persistence; it does not acquire browser storage or URL concerns.
- `apps/web` may implement domain-blind storage and surface lifecycle around
  the shared launch/engine APIs.
- Authored production `.js`, `.mjs`, `.ts`, and `.tsx` may acquire browser
  facts, call the raw WASM bridge, apply opaque results, and show startup-fatal
  text. They may not interpret engine vocabulary or semantic reports.
- A separately named, explicit smoke-only observer may expose shared reports
  to Playwright. It is observational and cannot influence product execution.

## Ordered delivery slices

### S28.1 Contract and inventory

- [x] Record allowed and forbidden browser leaf responsibilities.
- [x] Record preference, query, and diagnostic ownership.
- [x] Preserve a clean pre-change boundary-check baseline.

Gate: architecture and tactical indexes agree on the active ownership rule.

### S28.2 Shared preference and query policy

- [ ] Add typed, tested Rust ownership of current and legacy browser records.
- [ ] Restore preferences before shared launch parsing without changing
      explicit URL precedence.
- [ ] Persist effective configuration and emit canonical default-elided query
      updates from Rust.
- [ ] Remove storage keys, solver/configuration fields, migrations, bounds, and
      query mappings from browser-authored JavaScript.

Gates:

- focused `physbox-launch` tests cover current records, both migrations,
  malformed records, explicit-query precedence, and canonical query output;
- web crate tests cover domain-blind storage integration where practical.

### S28.3 Opaque product frame bridge

- [ ] Make the ordinary render return operational and opaque to JavaScript.
- [ ] Move semantic report/global publication into an explicit smoke observer.
- [ ] Update maintained browser automation to request that observer.
- [ ] Prove observer removal does not change input, simulation, or rendering.

Gate: the maintained real-browser smoke passes through the gated observer.

### S28.4 Durable source locks

- [ ] Discover all authored `.js`, `.mjs`, `.ts`, and `.tsx` browser files.
- [ ] Reject engine vocabulary and browser-owned configuration policy outside
      the named observer exception.
- [ ] Lock the raw pointer/wheel/resize/timestamp/render shape and a small
      authored-glue size/file budget.
- [ ] Add negative fixtures proving a newly added TypeScript file and semantic
      product glue fail the boundary check.

Gate: `npm run check:boundaries` passes and its negative fixtures
reject each declared duplication shape.

### S28.5 Production qualification and closure

- [ ] Run `npm run test:fast`.
- [ ] Run production clippy with warnings denied for the maintained workspace.
- [ ] Record exact validation and close the workstream.

## Completion condition

S28 is complete when ordinary authored browser glue contains only the bounded
raw input/display bridge, shared Rust owns compatible preference/query policy,
semantic browser reports exist only through the smoke-gated observer, the
boundary checker discovers future JavaScript/TypeScript files and rejects the
declared duplication fixtures, and all S28.5 gates pass.

## Validation record

- 2026-07-23 pre-change baseline: `npm run check:boundaries` passed, including
  the existing negative fixtures.
