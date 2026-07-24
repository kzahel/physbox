# Render resolution scaling

Status: decided implementation direction, 2026-07-24. Implementation and
cross-host qualification are tracked by
[S34](../tactical/034-render-resolution-scaling.md).

Governing presentation direction:
[presentation and modular workbench architecture](presentation-and-workbench-architecture.md).
Platform boundary:
[platform architecture](../platform-architecture.md).
Performance obligations:
[performance and scale](performance-and-scale.md).
Interactive pacing:
[simulation time and overload](simulation-time-and-overload.md).

## Decision

PhysBox 5 will expose one bounded manual render-scale choice on the shared
engine-rendered menu. The initial presets are 100, 75, and 50 percent, with
100 percent remaining the fresh default. The percentage is a linear scale:
50 percent maps a 3840x2160 presentation target to a 1920x1080 world-render
target and therefore one quarter of the world raster pixels.

The setting changes presentation only. It cannot change fixed-step admission,
physics configuration, scenario projection or outcomes, camera semantics,
world capacity, Box2D state, tool behavior, or input normalization.

At a reduced scale, one shared frame is encoded in this order:

1. simulation advances under the existing zero-or-one interactive step rule;
2. background, rigid bodies, comparison bodies, grab presentation, surfaces,
   and particles render into one scaled color target;
3. one fullscreen linear composite expands that target into the native
   presentation target; and
4. the canvas UI renders at native resolution over the composited world.

At 100 percent, the existing direct-to-target world and UI path remains active.
There is no intermediate color allocation or composite pass.

## Ownership and cross-host behavior

`physbox-launch` owns the external `render-scale` name, accepted presets,
default, parsing, browser query projection, and preference migration.
`physbox-engine` owns menu meaning, applied state, target-size derivation,
composition, reporting, and the invariant that a presentation change cannot
rebuild or alter mechanics. `physbox-ui` remains neutral widget and canvas
machinery.

Surface and offscreen renderers allocate targets around the same
`EngineHost::render_frame` boundary:

- browser JavaScript continues to report the native canvas extent and raw
  device pixel ratio; it does not know the render-scale setting;
- the shared Winit shell gives desktop and Android identical behavior;
- headless output retains the requested final PNG dimensions while exercising
  the same scaled-world/native-UI composition path; and
- the PhysBox and Box2D views use the same world target, composite, native UI,
  and setting.

Browser persistence remains shared Rust policy. The existing presentation
preference migrates from version 1 to a version that carries both sleep
visuals and render scale. Desktop and Android gain the shared launch option and
session menu behavior; no host-local durable preference store is introduced.
The selected presentation configuration must survive native surface
recreation within a running process.

## Geometry, input, and UI invariants

The native presentation extent continues to own:

- camera aspect and normalized screen/world projection;
- pointer, touch, wheel, hit-test, and gesture coordinates;
- raw physical pixels per logical display point;
- responsive menu and toolbar density; and
- the final `FrameReport` presentation size.

Only world rasterization uses the scaled extent. Rounding to integer texture
dimensions is deterministic, nonzero, and reported as the effective target.
The native UI avoids turning a performance choice into smaller, blurry, or
mis-hit controls.

## Reporting and measurement

Every semantic frame report publishes:

- requested render-scale preset;
- effective world-render extent and effective per-axis scale;
- native presentation extent;
- whether the direct or composited path executed; and
- intermediate color allocation.

GPU attribution separates composite work from canvas UI work. A reduced-scale
result is invalid when it omits the final native composite, hides the applied
target, changes physics or input behavior, or presents slow-time pacing as a
sustained-rate improvement.

The setting is intended to help fill-rate- or bandwidth-limited displays. It
does not reduce rigid or liquid compute work. Existing physical-device
evidence found direct body, particle, and UI rendering below one millisecond in
one sampled liquid frame, while a separate 960x600 versus 2560x1600 diagnostic
showed a roughly 1.3 ms viewport contribution. Those results make the feature
credible but do not predict a universal gain. Acceptance requires paired
100/50 percent measurements on a high-pixel-count target, including effective
simulation scale and the new composite bucket.

## Alternatives

### Resize only the browser canvas

Rejected. CSS-backed browser scaling is not the shared desktop/Android path,
would give JavaScript presentation policy, and would leave native behavior
dependent on platform surface semantics.

### Configure every native swapchain below the window extent

Rejected for the initial implementation. Browser, Vulkan, Metal, and Android
surface/compositor behavior is not one portable scaling contract. An explicit
engine-owned color target and composite produces the same result everywhere.

### Scale the canvas UI with the world

Rejected. It saves little relative to the world, reduces text and icon
clarity, and entangles display density and hit testing with the performance
setting.

### Automatic dynamic resolution

Deferred. A controller would require stable deadline measurements,
hysteresis, user-facing requested/effective policy, and cross-adapter
qualification. The first feature is explicit and deterministic.

### Fixed 1080p rather than a percentage

Rejected. A uniform scale preserves aspect and behaves predictably across
desktop windows, phones, tablets, browser zoom, and headless captures. Fifty
percent still produces exactly 1920x1080 from a 3840x2160 target.

## Validation

Implementation qualification includes:

- launch parsing, browser preference migration, query synchronization, and
  invalid-preset tests;
- target rounding, resize, 100-percent fast-path, reduced-scale allocation,
  native-UI, report, and GPU-timing tests;
- semantic input/camera/scenario equality across 100 and 50 percent;
- fresh headless native-size captures at 100 and 50 percent;
- Browser WebGPU menu, resize, persistence, and high-pixel-count captures;
- comparator feature-on and explicit feature-off coverage;
- production clippy, ARM64 Android build, release APK, and physical Android
  behavior/pixel validation; and
- paired high-pixel-count performance evidence that records all compute,
  world-render, composite, UI, full-frame, pacing, and capacity facts.

## Reconsideration triggers

Revisit the preset set, filtering policy, or composition split if:

- the native composite consumes enough bandwidth to erase the measured world
  savings on a target device;
- linear filtering produces unacceptable body, particle, or outline quality;
- native-resolution UI prevents a material saving demonstrated by evidence;
- a maintained adapter cannot sample the selected surface-compatible color
  format portably;
- a later smooth-water renderer introduces a separate resolution or
  intermediate-texture contract; or
- stable cross-adapter deadline measurements justify an explicit automatic
  controller.

No reconsideration may silently alter physics rate, overload behavior,
scenario outcomes, input semantics, or the fresh 100-percent default.
