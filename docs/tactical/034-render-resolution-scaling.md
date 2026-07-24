# S34 — Render resolution scaling

Status: complete, 2026-07-24.

Governing topic:
[render resolution scaling](../topics/render-resolution-scaling.md).
Presentation architecture:
[presentation and modular workbench architecture](../topics/presentation-and-workbench-architecture.md).
Platform boundary:
[platform architecture](../platform-architecture.md).
Performance contract:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Deliver one manual 100/75/50 percent render-scale setting through the shared
engine-rendered menu and launch contract. Reduced presets render the world at
the bounded internal extent, linearly composite it to the native target, and
draw native-resolution UI without changing physics, input, camera, scenario,
or comparison semantics. Qualify the same behavior on desktop, browser,
headless, Android, and comparator feature-on/off builds.

## Exclusions

- Do not add automatic or adaptive resolution.
- Do not change surface presentation cadence, fixed-step rate, catch-up,
  interpolation, solver work, world capacity, or physics defaults.
- Do not put render-scale meaning, defaults, persistence, or query mapping in
  browser JavaScript, Winit, Android, or desktop app code.
- Do not add host-local UI or durable native preference stores.
- Do not change world style, smooth water, antialiasing families, shaders
  beyond the bounded composite, or the existing S33 subjective disposition.
- Do not claim a universal performance gain from one adapter.

## Ownership constraints

- `physbox-launch` owns the preset type, external name/default, parsing,
  browser query projection, and preference migration.
- `physbox-engine` owns the menu action, effective state, world target,
  composition, reports, and invariant tests.
- `physbox-ui` remains unaware of rendering or product settings.
- Platform adapters forward only native extent, display metrics, lifecycle,
  and raw input.
- The headless path uses the same composition behavior and retains final
  capture dimensions.
- PhysBox and Box2D share one selected render scale and compositor.

## Ordered delivery slices

### S34.1 — Contract and decision record

- [x] Record the durable render-scale decision, alternatives, reporting, and
  reconsideration triggers.
- [x] Create this successor tactical without expanding S33.
- [x] Add both documents to their indexes.

Gate: the topic and tactical define native presentation versus scaled world
extent, manual presets, ownership, evidence requirements, and completion.

### S34.2 — Shared setting and menu

- [x] Add a bounded 100/75/50 preset owned by `physbox-launch`.
- [x] Add shared CLI, web-query, and Android-JSON parsing with 100 percent as
  the default.
- [x] Project the setting into engine presentation configuration exactly once.
- [x] Add one engine-rendered menu control visible in PhysBox and Box2D.
- [x] Apply changes without scenario/world rebuild, physics hash change,
  camera reset, or tool reset.
- [x] Migrate browser presentation persistence and URL synchronization in
  shared Rust.

Gate: focused launch, preference, engine UI, feature-on, and feature-off tests
prove the complete setting contract and browser JavaScript remains semantic
free.

Evidence:

- `cargo test -p physbox-launch --lib`
- `cargo test -p physbox-engine --lib
  presentation_launch_preference_reaches_the_shared_engine`
- `cargo test -p physbox-engine --lib ui::tests`
- `cargo test -p physbox-engine --lib --no-default-features
  presentation_launch_preference_reaches_the_shared_engine`
- `cargo test -p physbox-engine --lib --no-default-features ui::tests`
- Maintained host compilation:
  `cargo check -p physbox-engine -p physbox-shell -p physbox-desktop
  -p physbox-web -p physbox-android -p physbox-headless`

### S34.3 — Shared composition path

- [x] Separate native presentation and world-render targets inside the one
  frame boundary.
- [x] Retain direct world/UI rendering and zero intermediate allocation at
  100 percent.
- [x] Allocate a bounded sampleable color target and encode one linear
  fullscreen composite at 75/50 percent.
- [x] Render the canvas UI at native extent after composition.
- [x] Recreate only size-dependent presentation resources on resize or scale
  change.
- [x] Exercise the same path in surface and offscreen renderers.
- [x] Preserve selected scale through native surface recreation.

Gate: target, resize, direct/composite, camera/input, offscreen, and GPU
validation tests pass with no change to scenario or physics identity.

Evidence:

- Focused offscreen WebGPU tests cover 100/50 paths, odd-size rounding, native
  output pixels, menu-driven resource changes, camera state, UI density,
  scenario identity, and physics identity, both with the default comparator
  feature and `--no-default-features`.
- Browser WebGPU run `0d9a4fc0-09f9-4d21-bb18-09bdab650ab9` exercised
  100/75/50 transitions, preference reload, resize at 75 percent, return to
  the allocation-free native path, and a 960x600 final output. Its 50-percent
  capture is `/tmp/physbox5-web-render-scale-50.png`.

### S34.4 — Reports and timing

- [x] Report native and world extents, requested/effective scale, path, and
  intermediate bytes.
- [x] Add a distinct asynchronous GPU composite bucket without blocking frame
  advancement.
- [x] Display the applied scale and target in the existing bounded HUD.
- [x] Update semantic/compact report tests and relevant browser/Android
  consumers.

Gate: supported and unsupported timestamp adapters report honestly; async
readback remains bounded and no report value is required for the next frame.

Evidence:

- Timestamp decoding and HUD-line tests cover the rigid, liquid, body,
  particle, composite, UI, and total buckets.
- The full browser report publishes the direct path at 960x600 with zero
  intermediate bytes; the captured reduced frame published a 480x300 world
  target and a distinct composite measurement.
- The compact Android marker test retains its sub-940-byte worst-case bound
  while including the render-scale report.

### S34.5 — Cross-host and performance qualification

- [x] Run focused package and engine tests while iterating.
- [x] Run `npm run test:fast`.
- [x] Run production clippy with warnings denied.
- [x] Produce fresh native-size headless 100/50 percent captures and reports.
- [x] Run Browser WebGPU menu/persistence/resize coverage plus paired
  high-pixel-count 100/50 percent evidence.
- [x] Prove comparator default feature on and explicit feature off.
- [x] Run the ARM64 Android check and build a release APK.
- [x] Exercise the setting and pixels on one physical Android device.
- [x] Measure paired 100/50 percent compute, render, composite, UI, total GPU,
  full-frame, and pacing on the 4K Chromebook when available; otherwise record
  the exact representative-hardware gap without inventing a claim.

Gate: required commands exit successfully, artifacts are fresh and
capacity-clean, every host exposes the same setting, 100 percent retains the
current path, and performance conclusions match the measured bottleneck.

Evidence:

- `npm run test:fast` passed in 26.75 seconds, including 175 production engine
  tests, bounded performance smoke, host/tool tests, platform negatives, and
  Browser WebGPU run `d7081a0e-9a00-4203-9a53-8c6a5015a874`.
- `npm run clippy:rust:production` passed with warnings denied.
- Direct comparator tests passed 2/2. `npm run test:box2d-feature-off` emitted
  `PHYSBOX5_BOX2D_FEATURE_OFF_SUCCESS`; `npm run test:web:lab` passed as run
  `eae45c16-0dfc-4293-a844-b1629bc2713c`.
- Clean revision `cc688a1fce3b34c3f852194caa7e15c482d2aa5f` produced paired
  960x600 headless artifacts in `/tmp/physbox5-s34-headless.hpuUDI`.
  `native-100.json` is
  `4d14fa992ed6b8564777ed672a593751158ae492ece8104d89763e8c868b9665`;
  `scaled-50.json` is
  `4126c718c45b30c2a352f6f1c6e0dfa77c1710297c9737e77782522b68110b18`.
  Both PNGs remain 960x600, both reports share physics and scenario hashes,
  and both remain scene/UI capacity-clean.
- Desktop Vulkan passed at 50 percent as run
  `04068d3b-dc4d-43aa-8d99-ba4a1fa10806`, reporting 480x300 world pixels
  inside a 960x600 presentation target.
- The ARM64 NDK check, ARM64 release-native/debug APK, and Gradle release APK
  passed. The 15,688,100-byte debug APK is
  `25d2916dec1f0d4668ac3709f9144cdf3eaa1d7ace8ddfb133648d65fe87fea5`;
  the 8,458,477-byte unsigned release APK is
  `568d013a0a072a0781430ea2433a95be53c8c39b1177db5355dea7571970b016`.
- Physical Pixel 7a run `1784879796-3224363` used Mali-G710 Vulkan and the
  engine-rendered menu to apply 100 → 75 → 50 percent. The clean provenance
  at `/tmp/physbox5-s34-android-pixel7a.png.json` records 1080x2400 native,
  540x1200 world, unchanged physics/camera, and a passing scenario. The
  screenshot SHA-256 is
  `697d787bffd922b7937f42a48151d964435b59e319293eb5ece35a3d582b74ab`;
  the app was stopped after capture.
- Clean revision `f7a482106ac8b8781dd987d56f623acb680ac3d6` produced paired
  3840x2160 Browser WebGPU runs
  `f6d8c469-49da-43a3-82df-baab68b93512` and
  `d4130ee0-b65f-45bc-aa3f-259519b2954c`. The 100-percent sample reported
  1.329 ms total GPU; 50 percent reported 1.364 ms after a 0.711 ms
  composite. Both retained approximately 100 percent simulation scale, zero
  slip, and clean capacity. The available AMD Radeon 890M host therefore
  shows no bootstrap-scene gain. The target 4K Chromebook was unavailable,
  so no claim is made for it.

## Completion condition

S34 is complete. The fresh default remains native resolution; 75 and 50
percent use one bounded shared scaled-world/native-UI composition path; launch,
menu, browser persistence, reports, timing, resize, lifecycle, offscreen,
comparator, and cross-host gates pass; physics/input/scenario identity remains
unchanged; and paired high-pixel-count evidence records the measured
limitation without extrapolation.
