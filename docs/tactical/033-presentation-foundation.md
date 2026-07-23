# S33 — Presentation foundation and modular canvas UI

Status: active, 2026-07-23. Human direction approves the governing visual and
modularity direction and authorizes continuous implementation through
technical and cross-host qualification. The work stops at the final subjective
capture review.

Governing topic:
[presentation and modular workbench architecture](../topics/presentation-and-workbench-architecture.md).
Platform boundary:
[platform architecture](../platform-architecture.md).
Authoritative engine direction:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).
Current UI foundation:
[S4 interactive evaluation workbench](004-basic-scene-navigation-camera.md).
Comparator boundary:
[interactive Box2D comparison](../topics/interactive-box2d-comparison.md).

## Goal

Deliver a visibly improved and structurally reusable presentation foundation:
extract generic GPU-canvas layout, widgets, input capture, theming, and
rendering into `physbox-ui`; migrate the complete shared workbench; install a
PhysBox 3/4-inspired world palette without the ordinary background grid; add a
rounded icon toolbar and robust compact menu controls; and qualify identical
behavior across desktop, browser, headless, Android, and the compile-removable
Box2D comparison lane.

The intended first stopping point is technically complete and usable. Human
feedback is required only to accept or request aesthetic changes after the
final capture/device review, not to choose architecture or unblock each slice.

## Exclusions

- Do not change rigid or particle equations, pass ordering, solver
  configuration, sleeping behavior/defaults, world capacities, fixed-step
  pacing, or scenario outcomes.
- Do not split or publish the future GPU simulation or world-render crates in
  this workstream.
- Do not set `publish = true`, choose a crates.io name, or promise a stable
  public API.
- Do not add DOM, Android-view, desktop-native, or host-specific product UI.
- Do not add selection/property editing, save/load, undo/redo, new tools,
  joints, shapes, materials, or smooth-water rendering.
- Do not modify Box2D to expose internal timers or other private diagnostics.
- Do not turn the optional background grid into physics state or include the
  comparator in performance measurements.
- Do not require a new font system unless the retained atlas fails the declared
  readability review.

## Ownership constraints

- `physbox-ui` owns only neutral UI geometry, layout, widgets, theming,
  pointer capture, batching, rendering, and bounded presentation diagnostics.
- `physbox-engine` owns workbench tools, menu/page content, scenarios, solver
  and sleeping meaning, comparator visibility, camera/world arbitration,
  action mapping, and report semantics.
- `physbox-launch` continues to own external setting names, defaults,
  persistence, migration, and URL/Android projection.
- Platform adapters continue to forward raw lifecycle/input/display facts and
  know no tool, menu, solver, sleep, scenario, or renderer semantics.
- World colors and UI themes are presentation data. They cannot change
  mechanical buffers, configuration hashes, or validation verdicts.
- The existing one shared render frame and no-next-frame-readback contract
  remains unchanged.

## Unattended execution contract

After explicit activation, implementation may proceed without intermediate
human decisions through S33.1-S33.5. Commit each coherent slice after its
declared focused checks pass. Stop early only when:

- the extraction requires a dependency inversion outside the governing topic;
- a physics/configuration/scenario result changes;
- the UI pass/draw bound cannot be retained without a new architecture choice;
- a required maintained host cannot compile or execute and no authorized
  substitute exists;
- copied/reference provenance cannot be established; or
- a hard gate repeatedly rejects the implementation.

Color values, radii, spacing, icon geometry, and responsive breakpoints may be
selected from the recorded PhysBox 3/4 direction without asking for individual
approval. The final S33.6 visual disposition remains a human checkpoint because
aesthetic preference cannot be established solely by automated tests.

## Ordered delivery slices

### S33.1 — Baseline and neutral crate contract

- [x] Record clean PhysBox 5, PhysBox 3, and PhysBox 4 revisions and dirty
  states before relying on presentation source.
- [x] Capture the existing closed-toolbar, open-menu, sleeping-debug, Box2D,
  desktop/browser, and narrow/mobile baselines.
- [x] Record current UI render passes, draw calls, static/dynamic instances,
  upload behavior, overflow state, and a bounded full-frame smoke.
- [x] Add `physbox-ui` as a default workspace member with no dependency on
  engine, launch, contracts, comparator, shell, or app crates.
- [x] Define neutral geometry, input, widget-ID/event, theme, frame-description,
  capacity, and render-report types with crate-level ownership documentation.
- [x] Add dependency-boundary negatives so physics/product vocabulary cannot
  silently enter the UI crate.

S33.1 baseline: clean PhysBox 5 revision
`8f7ca0553ecf436974aebe70a95f91ec83625b73`; clean PhysBox 3
`b11d73dae7dfe96f2efcba9dc5e3e53d10390867`; clean PhysBox 4
`92f745eb85908243c69080a4fa4dba24f05c22cf`. `npm run
references:status` passes every managed and sibling pin. Browser WebGPU run
`e35da22d-c002-4c62-9afa-245a5e51dff4` at 960×600 records the closed toolbar,
open solver menu, sleep visualization, and Box2D captures under
`/tmp/physbox5-s33-baseline`. The ordinary final frame uses one UI pass, two
draws, 31/238 static/dynamic instances, 25/237 glyphs, one static write,
37 dynamic writes, 1,984/562,304 static/dynamic upload bytes, 48,144 allocated
bytes, and no overflow. The Box2D frame also uses one pass and two draws with
26/250 instances and no overflow. Native Vulkan headless capture
`native-bootstrap.png` passes 40 frames at full simulated time. Narrow Browser
WebGPU run `a1f84dab-fac8-41cd-9445-a5634ceea856` repeats the capture flow at
456×1024 with the same one-pass/two-draw bound and no overflow.

Gate: the new crate builds on maintained native and WASM targets, its dependency
direction is enforced, baseline artifacts are fresh, and no product call site
uses it yet.

### S33.2 — Mechanical renderer and input extraction

- [ ] Move the glyph atlas, UI shader/pipeline, instance formats, bounded
  buffers, static-key caching, upload accounting, and UI render pass into
  `physbox-ui`.
- [ ] Move neutral rectangle/hit testing and pointer capture into the crate.
- [ ] Preserve one UI pass, no more than two instance draws, and explicit
  overflow.
- [ ] Keep `WorkbenchUi` as an engine-owned adapter that produces neutral UI
  frames and maps neutral events to existing `UiAction` values.
- [ ] Prove the same current menu/tool actions and comparator visibility before
  deliberately changing layout or style.

Gate: the complete existing UI runs through `physbox-ui`, semantic interaction
tests pass, the old engine module no longer owns generic WGPU UI machinery, and
baseline pass/draw/upload bounds do not regress.

### S33.3 — Generic widgets and compact menu migration

- [ ] Add responsive row, column, wrapping, overlay, panel/card, clipping, and
  scrollable-column layout sufficient for current narrow and wide targets.
- [ ] Add label, text/icon button, tab, segmented control, toggle, separator,
  discrete slider, and continuous slider widgets.
- [ ] Use one computed layout for drawing and hit testing.
- [ ] Qualify mouse and touch activation, drag capture, drag-outside behavior,
  cancellation, multiple pointer isolation, and slider snapping.
- [ ] Rebuild the Scenes/Solvers/World menu using these widgets while retaining
  its compact organization and all existing actions.
- [ ] Replace suitable plus/minus setting controls with drag-capable sliders
  that retain visible exact values and bounded configuration snapping.
- [ ] Preserve shared controls and hide PhysBox-only solver controls in Box2D
  mode through engine policy rather than widget special cases.

Gate: every currently reachable product action remains reachable at desktop
and narrow mobile sizes; sliders survive mouse/touch drags; no product term or
configuration type enters `physbox-ui`.

### S33.4 — Toolbar and world visual refresh

- [ ] Port or independently implement rounded, batched procedural icons for
  Menu, Grab, Circle, Rectangle, Erase, Water, and Surface with exact
  provenance for any closely reused PhysBox 4 implementation.
- [ ] Apply a coherent dark neutral UI theme with restrained selected states,
  borders, cards, radii, and touch-sized targets.
- [ ] Remove the ordinary background grid and use a quiet dark navy background
  with at most subtle lighting.
- [ ] Replace the ordinary neon body gradient with flat blue circle, violet
  rectangle, and muted fixed-surface families plus pixel-stable outlines.
- [ ] Align direct water presentation with the selected palette without adding
  smooth-surface mechanics or passes.
- [ ] Preserve quiet/asleep/recently-woken colors and markers as a conspicuous
  debug override in PhysBox view and public awake/asleep state in Box2D view.
- [ ] Keep both comparison engines on the same selected world renderer and
  visual theme.

Gate: the intended style is present in shared offscreen, desktop, browser, and
Android paths; sleep/comparator semantics are unchanged; no physics or
configuration hash changes.

### S33.5 — Cross-host and performance qualification

- [ ] Run focused `physbox-ui` and `physbox-engine` tests while iterating.
- [ ] Run `npm run test:fast`.
- [ ] Run production clippy with warnings denied.
- [ ] Run `npm run test:web:lab`.
- [ ] Run direct comparator crate tests and prove both default feature-on and
  explicit feature-off builds; feature-off contains no comparator control or
  packaged runtime.
- [ ] Produce fresh headless/offscreen captures and semantic reports for
  closed toolbar, each menu page, representative slider/toggle state,
  sleeping debug, liquid, and Box2D.
- [ ] Compare UI pass/draw/instance/upload and bounded full-frame evidence
  against S33.1; explain any material change.
- [ ] Run the ARM64 Android check and build a release APK.
- [ ] Exercise toolbar wrapping, menu scrolling, toggles, sliders, sleep
  visualization, and engine switching on a physical Android device when
  available. If none is available, record an AVD substitution and shut it down
  immediately after success or failure.

Gate: all applicable maintained lanes pass with fresh provenance, the same UI
semantics are present on every host, buffers remain bounded, and the visual
refresh has no unexplained material presentation-cost regression.

### S33.6 — Subjective capture review and disposition

- [ ] Present a compact synchronized capture set covering desktop/browser,
  narrow Android, menu closed/open, representative world interaction, sleep
  cycling, liquid, and Box2D.
- [ ] Record human disposition of palette, body contrast, icon clarity,
  toolbar placement/wrapping, menu density, slider feel, and mobile
  readability.
- [ ] Apply one bounded polish pass if requested, rerunning the affected
  technical and host gates.
- [ ] Update the governing topic, capability ledger, platform architecture,
  README, tactical index/status, and provenance record with the accepted
  outcome.

Gate: human review either accepts the presentation foundation or records a
specific bounded polish list. New application features, a solver split, or
publication work require a successor tactical rather than expanding S33.

## Validation artifacts

S33 uses untracked fresh artifacts under `/tmp`. Each artifact records run ID,
source revision and dirty state, host/adapter, viewport and device-pixel size,
capture phase, selected engine, scenario, theme identifier, UI contract
version, and complete scene/configuration contract where applicable.

The final capture matrix should include at minimum:

| View | Wide | Narrow/mobile |
| --- | --- | --- |
| toolbar and ordinary world | required | required |
| compact menu with slider | required | required |
| PhysBox sleep visualization | required | required |
| liquid sandbox | required | required |
| Box2D rigid comparison | required | required |

Cross-adapter screenshots are reviewed for semantic and perceptual consistency,
not exact pixel identity. Numerical scenario outcomes must remain unchanged.

## Completion condition

S33 is complete when `physbox-ui` owns the reusable canvas-UI machinery under
the declared dependency boundary; the entire workbench uses it; the icon
toolbar, compact slider-based menu, grid-free background, selected body/world
palette, and retained debug visuals are active on every maintained host; all
technical, comparator, performance, and Android gates pass; and human review
accepts the final capture set and any bounded polish rerun.
