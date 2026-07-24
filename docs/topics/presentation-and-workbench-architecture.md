# Presentation and modular workbench architecture

Status: decided direction, 2026-07-23; implementation and bounded subjective
polish are tracked in [S33](../tactical/033-presentation-foundation.md). This
topic owns the durable visual direction, reusable canvas-UI boundary, and the
intended separation between the PhysBox application and a future publishable
GPU simulation library. It does not itself authorize publishing a crate or
change physics behavior.

The physics/application separation proposed here is now specified for
local-path consumption by
[the reusable GPU library boundary](reusable-gpu-library-boundary.md) and
[S36](../tactical/036-reusable-gpu-library-extraction.md). This topic continues
to own application composition and presentation.

Authoritative engine direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Platform ownership:
[platform architecture](../platform-architecture.md).
Current capability inventory:
[product capability status](product-capability-status.md).
Predecessor evidence:
[PhysBox 3 and PhysBox 4 feature usage](predecessor-feature-usage.md).
Performance obligations:
[performance and scale](performance-and-scale.md).

## Decision

The next high-impact product work should establish a presentation foundation
rather than add another isolated control or reskin the current monolithic menu
in place.

The first implementation will:

- create a reusable `physbox-ui` GPU-canvas UI crate;
- retain PhysBox 5's compact tabbed menu organization;
- adopt the rounded icon toolbar and slider interaction strengths demonstrated
  by PhysBox 4;
- adopt the quieter, flatter world palette and unobtrusive background character
  preferred from PhysBox 3 and PhysBox 4;
- remove the background grid from ordinary presentation;
- preserve the existing sleep-cycle visualization as an explicit debug
  override; and
- leave physics, scenarios, tools, comparator policy, persistence, and launch
  semantics in their existing authoritative owners.

This presentation work is also the first deliberate modularity step toward an
engine that can be consumed outside the PhysBox sandbox. The eventual solver
library must not depend on menus, tools, camera policy, authored validation
scenarios, the Box2D review shadow, or platform adapters.

The workspace remains `publish = false` while these boundaries mature.
Designing a crate so it can later be published is not authorization to publish
it, select a public package name, promise semantic-version stability, or expose
an immature physics API.

## Why this is the selected priority

The existing shared UI is efficient but structurally over-combined.
`crates/physbox-engine/src/ui.rs` is currently 2,798 lines. One module owns:

- WGPU pipelines, buffers, glyph atlas use, batching, and static-write caching;
- fixed pixel layout and hit testing;
- pointer capture;
- toolbar and menu state;
- PhysBox tool availability;
- scenarios and world-capacity choices;
- solver configuration editing;
- sleeping controls and diagnostics;
- Box2D visibility policy; and
- conversion of all of those product concepts into engine actions.

The same file imports `physbox-launch` types, engine solver configuration,
comparison-engine state, reports, construction diagnostics, and WGPU. That
combination makes every menu improvement a product-policy and rendering edit,
discourages reusable widgets, and would force an eventual simulation consumer
to depend on sandbox concerns if the current boundary were published.

The current body shader also hard-codes the ordinary visual identity. It draws
a world-space grid over a saturated blue gradient and gives dynamic bodies a
bright cyan/green gradient and rim. Those choices are independent of mechanics
and should not be embedded as if they were solver behavior.

Presentation has unusually high leverage at the current stage:

- every existing and future feature needs a discoverable interactive control;
- direct human comparison has already found sleep and settling behavior that
  batch results alone did not communicate;
- a coherent toolbar and menu make desktop, browser, and Android review faster;
- the visual refresh improves the product immediately without changing a
  physical default; and
- extracting generic UI mechanics removes one clear dependency knot before
  more tools and settings accumulate.

## Evidence from PhysBox 3 and PhysBox 4

The preferred direction is selective, not a request to transplant either
predecessor architecture.

The source review used:

| Source | Revision | Continuing presentation evidence |
| --- | --- | --- |
| PhysBox 3 | `b11d73dae7dfe96f2efcba9dc5e3e53d10390867` | Dark neutral `#1a1a2e` background, no simulation grid, flatter varied object colors, thin light outlines, compact translucent controls |
| PhysBox 4 | `92f745eb85908243c69080a4fa4dba24f05c22cf` | Rounded procedural tool icons, responsive canvas-rendered settings layout, real draggable sliders, dark cards, bounded GPU diagnostics |
| PhysBox 5 baseline | `eeaf7a73cd4a9aa944dab9e26abe19c4cdbd0ad6` | One cross-host engine-rendered UI path, compact tabbed menu, combined instanced quad/glyph renderer, useful sleep-state visualization |

Relevant PhysBox 4 sources include
[`toolbar.rs`](https://github.com/kzahel/physbox4/blob/92f745eb85908243c69080a4fa4dba24f05c22cf/crates/physbox-engine/src/toolbar.rs),
[`toolbar.wgsl`](https://github.com/kzahel/physbox4/blob/92f745eb85908243c69080a4fa4dba24f05c22cf/crates/physbox-engine/src/toolbar.wgsl),
[`options.rs`](https://github.com/kzahel/physbox4/blob/92f745eb85908243c69080a4fa4dba24f05c22cf/crates/physbox-engine/src/options.rs),
and
[`options.wgsl`](https://github.com/kzahel/physbox4/blob/92f745eb85908243c69080a4fa4dba24f05c22cf/crates/physbox-engine/src/options.wgsl).
PhysBox 5's existing UI already records selective provenance from PhysBox 4
revision `4295957e15480ef701e6006c7cafb5f0728ef101` for its atlas, instanced
glyph batch, static-write caching, and toolbar sizing. Any further copied or
closely ported implementation must retain exact file, revision, and license
provenance.

PhysBox 3's DOM and Canvas 2D implementation is visual evidence only. It cannot
become PhysBox 5's platform architecture. PhysBox 4's CPU-rigid/GPU-fluid
separation likewise remains rejected. Only presentation techniques that fit
the one shared Rust/`wgpu` host may be reused.

## Target layering

The desired dependency direction is:

```text
desktop / browser / Android / headless adapters
                         |
                         v
                  physbox-engine
             application composition host
               /                    \
              v                      v
       physbox-ui                physbox-sim
     generic canvas UI         GPU physics library
```

`physbox-sim` is now the implemented unpublished local-path physics crate. The
first-party 2D world renderer remains private to `physbox-engine`; there is no
public `physbox-render` crate. S33 created only `physbox-ui`, while S36 later
extracted and then corrected the simulation boundary.

### `physbox-ui`

`physbox-ui` owns reusable presentation mechanics:

- logical and pixel rectangles, insets, alignment, and responsive scale;
- row, column, wrapping, overlay, panel, card, and scroll/clipping layout;
- label, button, icon button, tab, segmented control, toggle, separator,
  discrete slider, and continuous slider primitives;
- stable opaque widget identifiers;
- pointer hit testing and capture, including drag and touch cancellation;
- theme tokens for color, opacity, spacing, radii, borders, and sizes;
- glyph/icon/shape instances and bounded static/dynamic batches;
- the WGPU UI pipeline, texture atlas, buffer uploads, and render pass;
- explicit capacity/overflow and upload/draw diagnostics; and
- semantic frame descriptions that tests and future focus navigation can
  inspect without knowing product vocabulary.

It must not depend on `physbox-engine`, `physbox-launch`,
`physbox-contracts`, `physbox-box2d-comparator`, an application crate, or a
native UI toolkit. Its public vocabulary must not contain solver profiles,
scenarios, tools, sleep policy, body state, Box2D, or platform-specific events.

Neutral input may describe pointer identity, phase, button, normalized or pixel
position, target extent, and cancellation. Neutral output may describe widget
activation, a normalized or snapped value change, pointer capture, and whether
world input remains available. Product meaning is assigned by the caller.

The crate may depend on `wgpu` and small data/layout utilities. It is a GPU
canvas toolkit, not an operating-system widget abstraction. It must function
through the same render target on every maintained host.

### `physbox-engine`

During and after S33, `physbox-engine` remains the shared application
composition host. It owns:

- workbench tools and their availability;
- menu pages and product wording;
- scenario, capacity, solver, sleeping, and comparison choices;
- mapping neutral widget events to `UiAction` and engine configuration;
- deciding which controls are visible under the rigid-only Box2D view;
- camera and world/tool input arbitration;
- reports and validation semantics; and
- composition of physics, world rendering, debug rendering, and UI.

Moving the generic renderer into another crate does not move semantic ownership
into platform code. `EngineHost::render_frame` remains the only shared frame
boundary.

The first extraction may retain a product-specific `workbench_ui` module in
`physbox-engine`. It should become substantially smaller and describe the
menu using generic widgets rather than reimplementing geometry, hit testing,
pointer capture, or draw batching.

### Physics library and private world rendering

The GPU simulation crate owns the mechanical world, GPU
resource layouts, command encoding, configuration validation, stepping, and
bounded diagnostics needed to operate it. It should not own:

- menus, toolbar tools, or camera gestures;
- validation-scenario catalogs;
- Box2D comparison;
- browser preferences or launch parsing;
- desktop, browser, or Android lifecycle;
- application color choices; or
- the PhysBox sandbox's scene-selection and review workflow.

The first-party engine owns body, particle, and debug drawing. External
applications implement their own renderers from authored geometry and
versioned same-device physics observations. The observation contract contains
physics state only; it does not expose private mechanical layouts or turn the
simulation into a 2D presentation library. This separation must not introduce
per-frame CPU body extraction.

Before any crate is actually published, a separate release decision must cover
API stability, documentation, examples, licensing, dependency surface,
feature policy, package contents, versioning, and `cargo package` validation.

## Visual language

### World

The ordinary background should be a quiet dark navy based on PhysBox 3's
`#1a1a2e`, optionally with restrained vertical lighting. The current
world-space grid is removed from the default. A grid may return only as an
explicit debug or construction aid whose state is presentation-only.

Rigid bodies should use flatter, less saturated fills:

- circles use a blue family;
- rectangles use a violet family;
- fixed ground and surfaces use muted green, gray, or slate;
- water remains recognizably blue; and
- future material colors come from render metadata or a visual theme, not
  solver equations.

Shapes retain a thin, pixel-stable light outline and sufficient contrast
against both the background and overlapping bodies. Edge antialiasing should
remain stable across camera zoom and device pixel ratio. A subtle fill
variation is acceptable, but the current neon cyan/green gradient and bright
rim are not the intended ordinary style.

Sleep visualization remains deliberately conspicuous. Quiet progress, asleep,
recently woken, `Z`, and `!` markings may override the ordinary body palette
because their purpose is debugging rather than product decoration. Box2D
continues to show only the authoritative awake/asleep information its public
surface supplies.

### Toolbar

The toolbar uses rounded dark buttons with a restrained selected border and
distinct procedural or atlas-backed icons. It should adapt the proven PhysBox
4 icon language for the PhysBox 5 tool set rather than present wide uppercase
text buttons.

Icon geometry belongs to the reusable UI/render layer; the association between
an icon and Grab, Circle, Rectangle, Erase, Water, Surface, or Menu belongs to
the workbench adapter. Adding a tool must not require a new shader entry point
or draw call.

Toolbar targets must remain comfortable for touch at the smallest supported
viewport. Wrapping or another deterministic responsive arrangement must
prevent controls from becoming unreachable.

### Responsive density

The shared host owns UI density. Platform adapters may forward only the raw
number of physical pixels per logical display point together with the render
target extent. They must not select a phone, desktop, or host-specific layout.

The initial workbench density policy preserves the established sizing through a
logical short edge of 520 points, then interpolates to a compact desktop density
at 600 points. The compact targets are independent by role: a 36-point toolbar,
78 percent menu scale, and 75 percent diagnostics scale. Each resolved physical
scale is capped by the former short-edge scale, so the policy never enlarges an
existing target. Equivalent 1x and 2x desktop displays produce equivalent
logical geometry, while narrow high-density phones retain the previous physical
geometry and touch behavior.

Drawing and hit testing consume the same resolved scales. The semantic UI report
publishes the raw pixel ratio and resolved toolbar, menu, and diagnostics scales
for validation; browser automation consumes those reported values rather than
reimplementing responsive policy.

### Compact menu

PhysBox 5's compact Scenes/Solvers/World organization is retained. PhysBox 4's
large settings page is not copied wholesale.

The compact menu should use:

- rounded panels and cards;
- tabs or segmented choices for small enumerations;
- real drag-capable sliders with visible exact values;
- snapped discrete sliders where integer configuration requires them;
- toggles for binary choices;
- consistent spacing and hierarchy; and
- clipped or scrollable content when a mobile viewport cannot display every
  control.

Shared settings remain visible in both comparison modes. PhysBox-only solver
controls remain hidden when Box2D is selected. These are engine policies
expressed through generic widgets.

The existing bitmap glyph atlas may remain in the first slice if it is legible
at qualified desktop and mobile sizes. A new font stack, vector text, or
signed-distance glyph system requires separate licensing, packaging, quality,
and performance justification; it is not required merely to establish the
crate boundary.

## Rendering and performance contract

Presentation work must preserve the GPU-resident simulation contract:

- UI and world rendering consume GPU state but never make a readback necessary
  for the next frame.
- All canvas UI uses one render pass and no more than the existing two batched
  instance draws unless measured evidence justifies a different bound.
- Adding widgets does not add a render pass or draw call per widget.
- Instance and glyph capacities are bounded, reported, and fail visibly rather
  than silently dropping a semantically required control.
- Static geometry/text is uploaded only when its layout, theme, or content key
  changes; live values use a bounded dynamic batch.
- Theme changes do not alter physics buffers, hashes, steps, or scenario
  outcomes.
- The Box2D comparator continues to use the same selected world renderer and
  UI, and comparator-enabled results remain excluded from performance claims.

S33 records before/after UI pass, draw, instance, upload, and full-frame smoke
evidence. A prettier menu is not sufficient if it introduces unbounded
allocation, host-local layout, or material frame cost on mobile.

## First implementation stopping point

The first stopping point is a complete presentation foundation rather than a
partial toolkit demo:

1. `physbox-ui` exists with no product or physics dependencies.
2. The existing workbench is fully migrated to its layout, event, widget, and
   renderer primitives.
3. The toolbar uses rounded icons for the current tool set.
4. Scenes, Solvers, and World remain compact and use the new controls,
   including robust sliders and toggles.
5. The background grid is absent by default and the selected flat world/body
   palette is active.
6. Sleep visualization and PhysBox/Box2D switching retain their current
   semantics.
7. Desktop, browser, headless, and Android use the same shared implementation.
8. Comparator-feature on/off, UI interaction, performance, and cross-host
   gates pass.
9. A desktop/browser/mobile capture set reaches one final subjective review.

No physics equation, solver default, sleeping default, capacity default, or
scenario result should change in this workstream.

## Alternatives considered

### Reskin the existing `ui.rs`

Rejected as the primary route. It would deliver some immediate appearance
improvement but deepen the coupling that blocks reusable widgets and a future
simulation-library boundary.

### Copy the PhysBox 4 toolbar and options modules unchanged

Rejected. Their presentation is useful, but their fixed tool enumeration,
full-page settings model, and engine-local assumptions are not the desired
generic API. Selective porting with explicit provenance is allowed.

### Use DOM, Android views, or desktop-native widgets

Rejected. Host-local UI would duplicate semantics, break visual and interaction
equivalence, and violate the fullscreen-canvas browser boundary.

### Split and publish the complete solver first

Publication remains deferred. S36 extracted an unpublished local-path physics
crate, but release naming, compatibility, package contents, and stability
still require a separate decision. Extracting generic UI removed a known
dependency knot without freezing the physics API.

### Create a general-purpose application framework

Rejected. `physbox-ui` should solve the concrete retained-mode/immediate
description, input, widget, and GPU batching needs exercised by PhysBox. Window
creation, persistence, scene documents, application routing, and simulation
ownership remain outside it.

## Validation

The implementation workstream must include:

- unit tests for layout, hit testing, pointer capture, drag cancellation,
  slider snapping, overflow, and narrow/wide responsive arrangements;
- an engine-level semantic UI contract proving the same tools, pages, actions,
  engine visibility, and settings remain reachable;
- maintained native, Browser WebGPU laboratory, and production clippy gates;
- comparator default-feature, direct-test, and feature-off qualification;
- fresh offscreen captures for closed/open menu, selected tools, narrow mobile
  layout, PhysBox sleep visualization, and Box2D view;
- ARM64 Android build and release APK;
- physical Android interaction when a device is available, or an explicitly
  recorded AVD substitution when none is available, followed by immediate AVD
  shutdown; and
- before/after presentation work and full-frame timing sufficient to catch a
  meaningful regression.

Pixel captures are evidence for review, not a replacement for semantic
interaction tests. Brittle full-image goldens should not freeze antialiasing
differences across GPU adapters.

## Reconsideration triggers

Revisit this direction if:

- one-pass batched widgets cannot supply required clipping, scrolling, or
  interaction without a measured material cost;
- a maintained host cannot implement the neutral pointer/capture contract;
- the future simulation split demonstrates that a different GPU resource
  ownership boundary is necessary;
- a concrete accessibility requirement needs a semantic platform bridge that
  cannot be served from the canvas tree under the current browser rule;
- user review rejects the combined PhysBox 3/4 visual language; or
- publishing requirements reveal unacceptable WGPU, licensing, package-size,
  or API-stability constraints.

Those triggers may revise implementation details. They do not by themselves
authorize host-local product UI, CPU simulation state, or a dependency from
the future solver library back into the PhysBox workbench.
