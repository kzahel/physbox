# S30 — Configurable sleep threshold and grouping

Status: active, 2026-07-23.

Governing topics:
[GPU sleeping and waking](../topics/gpu-sleeping-and-waking.md) and
[GPU sleep-island connectivity research](../topics/gpu-sleep-island-connectivity.md).
Interactive reference:
[interactive Box2D comparison](../topics/interactive-box2d-comparison.md).

## Goal

Make sleep comparisons meaningfully interactive by applying one configurable
public sleep-speed threshold to PhysBox and Box2D, offering PhysBox-only
`LOCAL 4`, `LOCAL 8`, and `EXACT` grouping, and reporting the transition and
awake-time evidence needed to distinguish bounded-connectivity wake waves from
awake solver motion.

## Exclusions

- Do not change the default sleeping request; it remains off.
- Do not expose or patch private Box2D timers, islands, or wake causes.
- Do not translate PhysBox solver controls into approximate Box2D settings.
- Do not claim exact grouping makes PhysBox's solver or cold storage identical
  to Box2D.
- Do not promote exact grouping as the default without the evidence below.
- Do not use sleeping to satisfy an awake solver-quality gate.

## Ownership constraints

- `physbox-launch` owns the versioned threshold/grouping configuration, bounds,
  parsing, query projection, and browser preference migration.
- Shared Rust owns menu visibility, application, diagnostics, and reports.
  Grouping is hidden in Box2D mode; threshold remains shared.
- Box2D receives the threshold only through its public body definition/API.
- Exact PhysBox grouping consumes current touching dynamic-dynamic contacts.
  Static bodies do not connect components.
- Exact wake and admission remain GPU-resident and cannot require CPU readback.
- Any exact-component failure or capacity issue fails closed to awake; it may
  not silently fall back to a bounded grouping.

## Ordered delivery

### S30.1 — Versioned configuration

- [ ] Add a shared bounded sleep threshold and enumerated
  `local-4`/`local-8`/`exact` grouping to the physics configuration.
- [ ] Migrate browser preferences and query mappings while retaining the
  current `0.02 m/s`, `local-4`, sleeping-off defaults.
- [ ] Report requested/effective threshold and grouping in configuration
  hashes, scene contracts, and compact host diagnostics.

Gate: native parsing, migration, serialization, query, and configuration-hash
tests pass; malformed/out-of-range values fail closed.

### S30.2 — Existing-menu interaction

- [ ] Add `SLEEP THRESHOLD` and `SLEEP GROUPING` to `MENU → WORLD`.
- [ ] Offer `LOCAL 4`, `LOCAL 8`, and `EXACT` only while PhysBox is selected.
- [ ] Keep the shared threshold visible and effective in both engine views.
- [ ] Recreate both worlds from the same configuration without changing
  camera, tool, sleep-visual, or engine selection state.

Gate: shared UI and `npm run test:web:lab` cover hit routing, visibility,
persistence, reload, and exact configuration identity.

### S30.3 — Matched Box2D threshold

- [ ] Pass the shared threshold through the native/Android and browser
  comparator ABI using public Box2D sleep-threshold state.
- [ ] Reapply it on construction/restart/configuration and report the effective
  comparator value.
- [ ] Keep Box2D countdown and private island state absent.

Gate: direct native and browser runtime tests prove `0.02` and `0.05` produce
the requested public threshold and preserve authority equivalence when the
authority request uses the same value.

### S30.4 — Applied exact GPU decisions

- [ ] Reuse the S26 exact component labels and S27 component reductions without
  validation-only trace allocation.
- [ ] Aggregate/broadcast wake roots before awake compaction.
- [ ] Aggregate/broadcast post-solve blockers before admission so an entire
  exact component sleeps together.
- [ ] Retain local-4 byte behavior and add local-8 as the bounded control.
- [ ] Prove no current touching dynamic edge crosses an awake/asleep cut under
  exact grouping.

Gate: scalar and GPU chains shorter than, equal to, and longer than eight
contacts differentiate all three modes; exact labels/reductions match CPU,
replay exactly, and fail closed.

### S30.5 — Cycle diagnostics

- [ ] Report per-step sleep/wake cohort sizes.
- [ ] Report touching awake/asleep contact cuts.
- [ ] Accumulate awake-body-time over admitted outer steps.
- [ ] Report first stable-sleep step only after the configured confirmation
  horizon remains asleep.

Gate: deterministic two-body, long-chain, centered-stack, and poured-pile
fixtures prove counters against CPU replay without synchronous interactive
readback.

### S30.6 — Qualification and disposition

- [ ] Compare local-4, local-8, exact, and matched-threshold Box2D through the
  existing one-view interactive application.
- [ ] Measure incremental dispatches, allocation, GPU time, awake-body-time,
  transition cohorts, mixed cuts, and stable-sleep step.
- [ ] Run ordinary code gates, browser lab, fresh headless evidence, ARM64,
  release APK, and Android interaction on a physical device or an explicitly
  authorized hardware-backed AVD substitute when none is available.
- [ ] Record whether exact becomes the selected experimental grouping or
  remains an opt-in diagnostic.

Gate: all configurations are visibly and mechanically identified; no result
weakens awake quality, capacity, overflow, or performance validity rules.

## Completion condition

The existing cross-host application can match the public Box2D/PhysBox sleep
threshold, switch PhysBox among local-4, local-8, and exact grouping, and
explain the resulting sleep/wake cycle with cohort, mixed-cut, awake-body-time,
and stable-sleep diagnostics. Exact grouping is promoted or retained as an
explicit option only from measured behavior and cost.
