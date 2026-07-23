# S32 — Sleeping and exact-grouping default promotion

Status: complete, 2026-07-23. Human review explicitly selects effective
sleeping with exact current-contact-component grouping as the ordinary PhysBox
default. The implementation, preference migration, documentation, and
cross-host qualification pass while the menu, launch, URL, and persistence
opt-outs remain available.

Governing topics:
[GPU sleeping and waking](../topics/gpu-sleeping-and-waking.md),
[GPU sleep-island connectivity research](../topics/gpu-sleep-island-connectivity.md),
[physics configuration and solver profiles](../topics/physics-configuration-and-solver-profiles.md),
and [performance and scale](../topics/performance-and-scale.md).

## Goal

Promote `sleeping = on`, the existing `0.02 m/s` shared threshold, and
`sleep-grouping = exact` into the launch-owned fresh product configuration on
every maintained host. Preserve the engine-rendered controls, launch
arguments, URL projection, and browser persistence so reviewers can disable
sleeping or select local-4/local-8 at any time.

## Exclusions

- Do not change the 60 Hz/four-substep/i1 soft solver seed or the sleep speed
  threshold.
- Do not weaken the sleep-off awake-quality gates or claim that sleeping fixes
  residual solver motion.
- Do not alter Box2D internals, infer private Box2D timer state, or use the
  comparator as authority or performance evidence.
- Do not replace exact static rebuilding with persistent island maintenance.
- Do not claim a new 1K, 10K, 100K, 60 Hz, or CPU/GPU crossover result.

## Ownership constraints

- `physbox-launch` owns the defaults, parsing, help text, browser preference
  schema/migration, URL mappings, and Android JSON argv.
- The shared engine owns the existing WORLD menu toggle/grouping cycle,
  rebuild, reporting, and rendering behavior. Platform adapters gain no
  physics defaults or controls.
- Exact grouping remains the current touching dynamic-contact component
  contract documented by S30. It is not a claim of Box2D persistent-island or
  cold-storage equivalence.
- Performance comparisons continue to state whether sleeping is on or off.
  Comparator mode remains excluded from measurements.

## Ordered delivery slices

### S32.1 — Launch and persistence defaults

- [x] Make both soft-coupled and hard/direct profile seeds request sleeping
  and exact grouping.
- [x] Retain explicit `--sleeping off` and local-4/local-8 parsing through
  desktop, web, and Android transports.
- [x] Advance the browser preference to version 5, promote an untouched
  version-4 soft default, preserve customized version-4 settings, and preserve
  an explicit version-5 opt-out.

### S32.2 — Menu and report regression

- [x] Make the existing WORLD sleeping control disable the fresh request on
  its first activation and keep the grouping cycle exact → local-4 → local-8.
- [x] Keep requested/effective sleeping, grouping, configuration hash,
  dispatches, and allocation explicit in the scene contract.
- [x] Keep sleep visualization presentation-only and retain awake compaction
  infrastructure when admission is disabled.

### S32.3 — Durable decision records

- [x] Update the authoritative direction, sleeping/connectivity/configuration
  topics, capability ledger, bring-up plan, tactical index, and product
  README with the selected defaults and awake mask rule.
- [x] Preserve S23/S30's historical off/local-4 dispositions as dated evidence
  superseded by this later explicit human decision.

### S32.4 — Qualification

- [x] Run the maintained fast lane and production clippy with warnings denied.
- [x] Run a fresh headless default-on/exact result and an explicit
  sleeping-off/local-4 control with `/tmp` provenance.
- [x] Run the solver-menu Browser WebGPU laboratory gate.
- [x] Prove the default comparator feature and explicit feature-off build.
- [x] Run the ARM64 Android check and build the release APK.
- [x] Exercise the shared menu/default on a physical Android device, or on an
  AVD only when no physical device is available; shut the AVD down
  immediately afterward.

## Existing promotion evidence

S30 already measured the behavior and bounded cost being selected here. At a
shared `0.05 m/s` diagnostic threshold, the 100-body poured pile kept
`100/100` bodies awake through step 1,200 under local-4 and local-8. Exact
reached stable all-body sleep at step 189; matched-threshold Box2D reached it
at step 192. At 1K bodies in the matched 1%-awake scene, exact used 1,560
dispatches and 13,035,892 bytes versus local-4's 1,527 dispatches and
13,027,604 bytes; measured GPU P50/P95 was 5.408/6.635 ms versus
6.294/6.906 ms. At the 64-slot rigid-floor smoke, sleep-on exact used 175
dispatches and 819,940 bytes versus sleep-on local-4's 158 dispatches and
819,332 bytes. This supports the selected behavior without claiming that exact
is universally faster or that the diagnostic threshold changed the product
default.

## Qualification evidence

The final maintained lane passes in 23.09 seconds:

```text
npm run test:fast
PHYSBOX5_FAST_SUCCESS total_seconds=23.09 budget_seconds=120
```

Production clippy passes with warnings denied. Direct comparator tests pass
2/2, and `npm run test:box2d-feature-off` emits
`PHYSBOX5_BOX2D_FEATURE_OFF_SUCCESS` after a release build with no dependency,
runtime, menu, or revision markers. `npm run test:web:lab` passes real Browser
WebGPU as run `2dd673fd-1d8a-497c-aa7d-492b9bb52fc6`; its final report retains
sleeping-on/exact at the default `0.02 m/s`.

Fresh 120-step `rigid-floor` headless artifacts at dirty base revision
`a0d57ed0f7513ce3fa291b8a8ad6ae1e6f92cfbd` record both sides:

| Configuration | Dispatches | Allocation | Final state |
| --- | ---: | ---: | --- |
| fresh sleeping-on/exact | 175 | 819,940 B | one sleeper; first stable sleep step 51 |
| explicit sleeping-off/local-4 | 145 | 819,252 B | one awake body; no PhysBox sleep diagnostic |

The reports are
`/tmp/physbox5-s32-headless-Pa2MMn/default-on-exact.json` at SHA-256
`b053aa9e5d6d82f97265264d4a8e39b0368d04a668f9ac89f08bb84b4918a868`
and
`/tmp/physbox5-s32-headless-Pa2MMn/opt-out-local-4.json` at SHA-256
`5fa049abfdcc833521467ec1718f3b48b13362ec3ea2cf549b8907615b03dfc7`.
Both scenarios pass and remain capacity-clean.

The NDK-wrapped ARM64 check passes:

```text
cargo ndk -t arm64-v8a --platform 28 check -p physbox-android --lib
```

The ARM64 release-native/debug APK and Gradle release APK pass. The debug APK
is 15,688,100 bytes at SHA-256
`3e02c2ce3e0f49861d7f7dc2bbf5ff1fd9efef5f62ec4dd81bdcbb2936650909`;
the 8,325,357-byte unsigned release APK is
`a965b156f9787691b326e0a815fbf0b49ffd9945c02aeeb1c32849fa9b33be6e`.

A physical Quest 3 was attached, so no AVD was run. Android run
`1784819851-2656987` used Adreno 740 Vulkan and the real engine-rendered WORLD
menu. It verified fresh `0.02 m/s` sleeping-on/exact state, disabled and
re-enabled sleeping, cycled exact → local-4 → local-8 → exact, changed the
threshold to `0.05 m/s`, enabled sleep visuals, and waited for the restarted
scenario to pass. Provenance is
`/tmp/physbox5-s32-android-device.png.json`, SHA-256
`9e763a60422304f66b28e1ed4b1e06abb2aa65380c05fe6acabb57d34882dc70`.
Quest presents the app in a 500×800 Android panel while system screencap
returns the 4128×2208 stereo passthrough compositor, so the lane records
`behavior-only-windowed-device` rather than mislabeling compositor pixels as
an app screenshot. The product app was stopped after the run.

## Gates

- Fresh default configuration is sleeping-on/exact on every launch transport.
- Explicit sleeping-off/local-4 survives current browser persistence and every
  launch transport.
- S30's exact replay/capacity behavior remains unchanged.
- All proportional code, browser, comparator-feature, headless, Android
  packaging, and available-device gates pass with fresh provenance.
- Documentation states the current default unambiguously while preserving the
  awake-quality mask rule and historical evidence.

## Completion condition

S32 is complete: all slices and gates pass, fresh artifacts and exact commands
are recorded above, the physical Android lane observes the selected default
and disables it through the shared menu, and current product documentation
unambiguously declares sleeping-on/exact while retaining historical
off/local-4 evidence as dated context.
