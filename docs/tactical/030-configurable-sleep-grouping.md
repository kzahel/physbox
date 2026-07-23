# S30 — Configurable sleep threshold and grouping

Status: complete, 2026-07-23. The shared threshold, three PhysBox grouping
choices, applied exact GPU path, cross-engine cycle diagnostics, matched cost
evidence, browser qualification, and authorized hardware-backed AVD
interaction all pass. Sleeping remains off and `local-4` remains the
compatibility default; `exact` is retained as an explicit option with a strong
100-body-pile result, not silently promoted from one adapter and fixture.

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

- [x] Add a shared bounded sleep threshold and enumerated
  `local-4`/`local-8`/`exact` grouping to the physics configuration.
- [x] Migrate browser preferences and query mappings while retaining the
  current `0.02 m/s`, `local-4`, sleeping-off defaults.
- [x] Report requested/effective threshold and grouping in configuration
  hashes, scene contracts, and compact host diagnostics.

Gate: native parsing, migration, serialization, query, and configuration-hash
tests pass; malformed/out-of-range values fail closed.

### S30.2 — Existing-menu interaction

- [x] Add `SLEEP THRESHOLD` and `SLEEP GROUPING` to `MENU → WORLD`.
- [x] Offer `LOCAL 4`, `LOCAL 8`, and `EXACT` only while PhysBox is selected.
- [x] Keep the shared threshold visible and effective in both engine views.
- [x] Recreate both worlds from the same configuration without changing
  camera, tool, sleep-visual, or engine selection state.

Gate: shared UI and `npm run test:web:lab` cover hit routing, visibility,
persistence, reload, and exact configuration identity.

### S30.3 — Matched Box2D threshold

- [x] Pass the shared threshold through the native/Android and browser
  comparator ABI using public Box2D sleep-threshold state.
- [x] Reapply it on construction/restart/configuration and report the effective
  comparator value.
- [x] Keep Box2D countdown and private island state absent.

Gate: direct native and browser runtime tests prove `0.02` and `0.05` produce
the requested public threshold and preserve authority equivalence when the
authority request uses the same value.

### S30.4 — Applied exact GPU decisions

- [x] Reuse the S26 exact component labels and S27 component reductions without
  validation-only trace allocation.
- [x] Aggregate/broadcast wake roots before awake compaction.
- [x] Aggregate/broadcast post-solve blockers before admission so an entire
  exact component sleeps together.
- [x] Retain local-4 byte behavior and add local-8 as the bounded control.
- [x] Prove no current touching dynamic edge crosses an awake/asleep cut under
  exact grouping.

Gate: scalar and GPU chains shorter than, equal to, and longer than eight
contacts differentiate all three modes; exact labels/reductions match CPU,
replay exactly, and fail closed.

### S30.5 — Cycle diagnostics

- [x] Report per-step sleep/wake cohort sizes.
- [x] Report touching awake/asleep contact cuts.
- [x] Accumulate awake-body-time over admitted outer steps.
- [x] Report first stable-sleep step only after the configured confirmation
  horizon remains asleep.

Gate: deterministic two-body, long-chain, centered-stack, and poured-pile
fixtures prove counters against CPU replay without synchronous interactive
readback.

### S30.6 — Qualification and disposition

- [x] Compare local-4, local-8, exact, and matched-threshold Box2D through the
  existing one-view interactive application.
- [x] Measure incremental dispatches, allocation, GPU time, awake-body-time,
  transition cohorts, mixed cuts, and stable-sleep step.
- [x] Run ordinary code gates, browser lab, fresh headless evidence, ARM64,
  release APK, and Android interaction on a physical device or an explicitly
  authorized hardware-backed AVD substitute when none is available.
- [x] Record whether exact becomes the selected experimental grouping or
  remains an opt-in diagnostic.

Gate: all configurations are visibly and mechanically identified; no result
weakens awake quality, capacity, overflow, or performance validity rules.

## Completion condition

The existing cross-host application can match the public Box2D/PhysBox sleep
threshold, switch PhysBox among local-4, local-8, and exact grouping, and
explain the resulting sleep/wake cycle with cohort, mixed-cut, awake-body-time,
and stable-sleep diagnostics. Exact grouping is promoted or retained as an
explicit option only from measured behavior and cost.

## Completion evidence and disposition

Implementation commits `5b03d94`, `7bba258`, `b1bbf1b`, and `e1e46a6`
deliver the shared configuration, comparator ABI, applied exact components,
portable diagnostics, Android interaction checks, and bounded final headless
readback. `c2155be` adds the applied-path cost runner. The exact component
path uses no CPU readback, dynamic-dynamic touching edges only, fixed bounded
phases, final root/edge checks, and fail-awake structural failure handling.
Box2D receives only its public threshold; its timer, island membership, and
wake causes remain unexposed.

The deterministic 16-body contact-chain gate differentiates the modes in one
step:

| Grouping | Awake bodies | Mixed awake/asleep cuts |
| --- | ---: | ---: |
| local-4 | 5 | 1 |
| local-8 | 9 | 1 |
| exact | 16 | 0 |

The clean `c2155bea0c2ebf6bda1398d3fe08788ffb7dcf39` 1%-awake,
1,000-body matched cost artifact is
`/tmp/physbox5-s30-sleep-grouping-c2155be.json`, SHA-256
`208576a43337ebdb823b6723b11894b35fd57f46a8a5b08e428d1a1ae87ea84a`.
It uses AMD Radeon 890M RADV Vulkan, four warmups, and 20 retained native GPU
timestamp samples. Every row is replay-exact and capacity-clean.

| Grouping | Dispatches | Graph allocation | GPU P50 / P95 |
| --- | ---: | ---: | ---: |
| local-4 | 1,527 | 13,027,604 B | 6.294 / 6.906 ms |
| local-8 | 1,543 | 13,027,604 B | 6.374 / 7.180 ms |
| exact | 1,560 | 13,035,892 B | 5.408 / 6.635 ms |

Thus local-8 adds 16 dispatches and no storage; exact adds 33 dispatches and
8,288 bytes at the 1,024-body reserve. The exact P50 happened to be lower in
this sample, so there is no measured regression, but the result is not treated
as proof that exact is faster. At the 64-body interactive reserve the matched
scene contract records local-4 at 158 dispatches/819,332 bytes and exact at
175/819,940.

Clean 1,200-step `rectangle-bucket-100` captures at the shared `0.05 m/s`
threshold provide the causal result that the prior S27 observer could not:

| Engine/grouping | Final awake / sleeping | Awake-body-time | First stable sleep |
| --- | ---: | ---: | ---: |
| PhysBox local-4 | 100 / 0 | 118,881 | none by step 1,200 |
| PhysBox local-8 | 100 / 0 | 118,154 | none by step 1,200 |
| PhysBox exact | 0 / 100 | 18,800 | step 189 |
| Box2D matched threshold | 0 / 100 | 12,800 | step 192 |

All three PhysBox final mixed-cut counts are zero; the deterministic long
chain supplies the positive-cut control and exact zero-cut proof. Exact
therefore demonstrates substantial behavioral value on the motivating pile,
while Box2D's lower awake-body-time also shows that the engines do not share
the same internal component history.

The clean headless artifact
`/tmp/physbox5-s30-exact.json` at
`e1e46a671d9e8ebb4b297fdd4dccb075dc060b61` records exact grouping,
threshold `0.05`, both engines' public cohorts, zero PhysBox cuts, and bounded
final GPU diagnostics. `npm run test:fast`, `npm run test:web:lab`,
warnings-denied production clippy, direct exact/metric/horizon tests, the
NDK-backed ARM64 check, and the dual-ABI release-native/debug APK pass.

No physical device was available. The explicitly authorized
`jstorrent-tablet` AVD used the host Radeon Vulkan adapter and clean revision
`e1e46a6`; run `1784800819-2217236` drove the real Android canvas controls to
sleeping on, threshold `0.05`, and exact grouping, enabled sleep
visualization, produced
`/tmp/physbox5-s30-android-avd.png` plus provenance, and was shut down. The
capture visibly reports `SLEEP EXACT T0.050`, one sleeper, and zero cuts.

Exact remains explicit rather than becoming the default in this unattended
closure. The pile result is compelling and makes exact the recommended
interactive comparison for this failure mode, but one native adapter and one
natural dense fixture do not justify silently replacing the compatibility
default. Reconsider default selection after the same behavioral win and a
non-regressing cost are repeated on broader natural fixtures and another
physical or native mobile adapter. The subsequent 2026-07-23 S23.7 human
review retained sleeping off and local-4 as the compatibility defaults.
