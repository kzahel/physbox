# S4: Basic scene navigation and camera

Status: proposed; begins only after S3.6 human acceptance.
Direction: [one shared host and renderer](../platform-architecture.md).
Bring-up checkpoint: [P5.2a](../bringup-plan.md#p52a--basic-scene-navigation-and-camera-queued).
Predecessor: [S3 small-world collision and persistence](003-small-world-collision-persistence.md).

## Goal

Make the deployed fullscreen canvas minimally discoverable and navigable once
P5.2 supplies a useful multi-shape world. Add an engine-rendered scene menu,
shared camera pan/zoom/recenter semantics, and complete mouse, touch, and wheel
transport without moving scene or camera policy into a platform adapter.

A user opening the default web deployment should be able to discover the
available shared scenes, activate one, pan and zoom its world, recenter the
view, and continue grabbing bodies with correct world-space pointer mapping.

## Sequencing and ownership

S4 starts after the S3.6 review package is accepted and completes before P5.3
broadphase work. Keeping it outside S3 prevents presentation and input changes
from contaminating contact-representation evidence or delaying the mandatory
P5.2 decision.

The engine owns camera state, gesture interpretation, menu state, scene
selection, restart semantics, rendering, pointer-to-world conversion, and
validation outcomes. `physbox-launch` remains the only owner of scenario names
and launch mappings. Browser, desktop, and Android adapters transport raw
pointer, contact, wheel, lifecycle, and surface-size facts only. Headless may
set an explicit camera through the shared capture contract when a fixture needs
it; it does not implement interactive controls.

The web document remains a fullscreen canvas plus its startup-fatal message.
The menu is rendered by the shared engine, not HTML/CSS, and no adapter owns a
different list of scenes or camera defaults. Scene activation must replace or
reset the shared engine world at a declared frame boundary; it may not create a
second live simulation or preserve hidden state from the previous scene.

## Scope and exclusions

S4 includes:

- a compact engine-rendered menu that exposes the shared scenario catalog and
  restart action;
- mouse/touch drag panning, wheel/pinch zoom, and an explicit recenter action;
- unambiguous gesture arbitration between body grabbing, camera navigation,
  and menu interaction;
- camera-aware hit testing and pointer-to-world conversion;
- resize/aspect handling and bounded zoom/pan behavior; and
- native, browser, Android AVD, available physical-device, and headless
  validation of the shared contracts.

S4 excludes construction tools, settings panels, save/load, polished product
chrome, keyboard remapping UI, accessibility-system design, scene authoring,
new rigid features, scalable broadphase work, joints, and liquid. It does not
change physics defaults, simulation time, scenario results, oracle acceptance,
or GPU body/contact layouts.

## Stop conditions

Stop for human direction if scene switching would require platform-owned
scenario semantics, a second engine/world, simulation-dependent readback, or a
different camera/input policy by host. Also stop before changing shared
physics, clock, oracle, tolerance, or accepted contact-layout decisions to
make a view or interaction pass.

Routine camera math, reversible overlay layout, gesture thresholds, colors,
icons, and raw-event plumbing remain autonomous implementation choices. Record
the final interaction mapping and preserve any cross-host gesture failure as a
deterministic input trace.

## Delivery slices

### S4.1 — shared camera and gesture contract

- [ ] Add versioned engine-owned camera state and raw viewport-input facts for
  mouse pan, touch pan/pinch, wheel zoom, and recenter.
- [ ] Define deterministic screen/world transforms, zoom bounds, resize/aspect
  behavior, and gesture arbitration with the existing grab command path.
- [ ] Prove camera motion changes only presentation and input projection: it
  must not change fixed-step admission, scenario hashes, or simulation state.
- [ ] Add replayable mouse and multi-touch traces, including cancellation,
  pointer loss, resize during a gesture, and zoom around a focal point.

Gate: equivalent raw traces produce the same camera and world-space targets on
every host; body grabbing remains correct after arbitrary accepted pan/zoom;
no adapter owns gesture meaning or camera defaults.

Commit boundary: shared input/camera contracts, transforms, negative tests,
and tactical status land together before any menu behavior.

### S4.2 — engine-rendered scene menu and restart

- [ ] Expose one shared scenario catalog from the existing launch authority and
  render a touch-sized menu affordance inside the shared renderer.
- [ ] Activate a selected scenario and restart the current scenario at a
  declared engine frame boundary with no stale body, command, contact, camera,
  or validation state.
- [ ] Retain direct query/argument launch for automation and deep links while
  proving that the menu selects the same scenario contract.
- [ ] Add focus/open/close/selection/cancel traces and prevent menu gestures
  from leaking into grab or camera commands.

Gate: the default deployed canvas makes the maintained scenes discoverable;
selection and restart are engine-owned and structurally identical across
native, browser, and Android; the DOM and app crates contain no menu policy.

Commit boundary: shared catalog projection, menu rendering, scenario
activation/reset, interaction tests, and tactical status land together.

### S4.3 — cross-host usability evidence

- [ ] Capture a short deterministic interaction sequence covering menu open,
  scene selection, pan, zoom, body grab/release, recenter, and restart.
- [ ] Validate desktop, browser, headless projection, host-GPU
  `jstorrent-tablet`, and the available physical Android device.
- [ ] Record fresh screenshots/reports, exact input traces, scene contracts,
  camera endpoints, source provenance, and any minimized failures.
- [ ] Visually inspect touch target size, gesture conflicts, clipping, aspect
  changes, and camera stability without treating subjective approval as a
  numerical physics baseline.

Gate: a reviewer can open the default deployment, discover and activate the
shared P5.2 scenes, navigate them with mouse or touch, and return to the
authored view without platform-specific instructions. All automated gates and
freshness sentinels pass with no physics or pacing regression.

Commit boundary: the compact S4 evidence package, final documentation, and
completed tactical status land together. P5.3 begins only after this basic
usability checkpoint is demonstrably available.

## Validation baseline

Each slice runs the shared gates from `AGENTS.md`. Input, renderer, launch, or
pixel changes additionally build the Android APK, exercise the host-GPU AVD,
attempt the physical-device lane when available, and visually inspect fresh
artifacts. Boundary fixtures reject HTML/app-owned menus, platform camera
semantics, duplicate scenario catalogs, or camera-dependent scenario results.

## Completion definition

S4 completes when one shared engine-rendered menu and one shared camera policy
work across the maintained interactive hosts, the default deployment is
discoverable without editing its URL, pointer-to-world interaction remains
correct under pan/zoom, deterministic traces and cross-host evidence pass, and
the work has not changed physics or created another engine.
