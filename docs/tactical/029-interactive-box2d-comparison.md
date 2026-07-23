# S29 — Interactive Box2D comparison

Status: active, 2026-07-23.

Governing topic:
[interactive Box2D comparison](../topics/interactive-box2d-comparison.md).
Technical direction:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Authority:
[Box2D rigid authority](../topics/box2d-rigid-authority.md).

## Goal

Add a default-enabled, compile-removable Box2D shadow world to the existing
shared workbench so a person can switch the one rendered view between PhysBox
and Box2D while constructing and interacting with both through the same UI.
Deliver the same behavior on desktop, browser, and Android.

## Exclusions

- No automatic fallback or Box2D-derived PhysBox mechanics.
- No timeline, play/pause/single-step controls, split screen, or separate
  comparison application.
- No translation of PhysBox-only solver knobs into approximate Box2D settings.
- No Box2D result as a PhysBox correctness, capacity, or performance pass.
- No platform-owned tool, menu, scene, action, camera, or settings semantics.
- No full reference checkout or dirty source in tracked third-party content.
- No sleeping-policy, solver-default, tolerance, or scale-claim change.

## Ownership constraints

- Shared Rust owns the engine selector, dual-world lifecycle, action ordering,
  stable identities, setting visibility, body-state projection, renderer
  choice, sleep visualization, and reports.
- PhysBox GPU state remains authoritative and never waits for Box2D.
- The pinned authority executable remains process isolated. The interactive
  runtime is non-authoritative and must match it on governed projections.
- Browser JavaScript may initialize and call an opaque pinned WebAssembly
  runtime only; it may not interpret workbench semantics.
- Comparator-enabled execution is never a performance measurement.

## Ordered delivery

### S29.1 — Decision and executable plan

- [x] Amend the over-broad CPU-backend prohibition with the narrow shadow
  comparator exception.
- [x] Record compile, source, ownership, UI, action, parameter, performance,
  cross-host, and removal contracts.
- [x] Add this tactical and topic-index entry.

Gate: documentation links and whitespace pass; no product code changes.

### S29.2 — Pinned runtime and compile boundary

- [x] Add the `box2d-comparator` default feature through maintained apps and
  the shared engine.
- [x] Add an enumerated pinned source/runtime distribution with license,
  provenance manifest, reproducible build entry point, and artifact hashes.
- [x] Implement one shared Box2D world API for native/Android and browser.
- [ ] Add a feature-off boundary test proving removal of dependencies, assets,
  menu vocabulary, and runtime work.

Gate: direct runtime tests create, step, inspect awake state, destroy, and
replay the same neutral source on native and browser; ARM64 compiles.

### S29.3 — Dual-world lifecycle and rendering

- [x] Create/recreate the Box2D shadow from the same scenario, stable IDs,
  settings projection, and capacity.
- [x] Advance it from the shared admitted step stream without gating PhysBox.
- [x] Upload its public body state into the shared renderer's alternate input.
- [x] Switch one rendered view without copying either world's mechanical state.
- [x] Report selected engine, shadow step, alignment, and exact revision.

Gate: PhysBox output is unchanged when selected; repeated switches expose two
independently evolved states at one step identity.

### S29.4 — Stable-identity interaction mirroring

- [ ] Mirror placement and hold-repeat results with exact allocated IDs.
- [ ] Mirror erase and grab selection by stable ID after trajectories diverge.
- [ ] Mirror target/release/throw, gravity, reset, restart, and configuration.
- [ ] Keep GPU-origin identity feedback compact and asynchronous.
- [ ] Fail closed and report any action/result queue overflow or identity
  disagreement.

Gate: deterministic construction/erase/reuse/grab traces preserve complete
ordered identity equality without a blocking PhysBox readback.

### S29.5 — Existing-menu integration and sleep visualization

- [ ] Add `ENGINE: PHYSBOX / BOX2D` to the current engine-rendered menu.
- [ ] Keep PhysBox initially selected and both shadows continuously advanced.
- [ ] Hide PhysBox-only solver controls in Box2D mode while preserving values.
- [ ] Retain shared settings, tools, camera, and layout across switches.
- [ ] Render Box2D awake/asleep state and only explicitly derived public-state
  observations through the existing sleep-visual language.

Gate: shared UI tests cover visibility, hit routing, engine switching, hidden
settings, state persistence, and feature-off absence.

### S29.6 — Cross-host packaging and boundary validation

- [ ] Integrate desktop native, browser WebAssembly, Android ARM64, and
  headless validation through the shared host.
- [ ] Keep browser glue opaque and add a negative fixture for semantic leakage.
- [ ] Make build/deploy scripts carry comparator assets only when enabled.
- [ ] Verify clean startup failure reporting for a missing or mismatched
  comparator runtime without falling back silently.

Gate: native, real Browser WebGPU, ARM64, release APK, and one physical Android
run select both engines and exercise one shared raw-event validation.

### S29.7 — Qualification and closure

- [ ] Compare governed interactive-runtime results with fresh process-isolated
  authority output.
- [ ] Run the feature-off package inspection and default PhysBox regression.
- [ ] Publish fresh provenance and representative cross-host captures.
- [ ] Update the topic, capability ledger, README, and this tactical with the
  exact completed state and limitations.

Gate: `npm run test:fast`, production clippy with warnings denied,
`npm run test:web:lab`, affected comparator tests, fresh headless smoke, ARM64
check, release APK, physical Android validation, and feature-off inspection all
pass.

## Completion condition

The existing application can switch one view between continuously evolving
PhysBox and pinned Box2D worlds on desktop, browser, and Android; all supported
interactive actions retain stable identity in both; Box2D sleep state is
visible; the compile feature is default-on and cleanly removable; the
authority process still owns acceptance; and no PhysBox behavior or
performance claim depends on the comparator.
