# S4: Interactive evaluation workbench

Status: S4.3 automated implementation/evidence complete, 2026-07-15; deployed
human grab-feel checkpoint required before S4.4.
Direction: [one shared host and renderer](../platform-architecture.md).
Bring-up checkpoint: [P5.2a](../bringup-plan.md#p52a--interactive-evaluation-workbench-authorized).
Physics predecessor: [accepted S3.6 direction](003-small-world-collision-persistence.md).

## Goal

Make the deployed fullscreen canvas useful for human evaluation of rigid-body
behavior. Add one engine-rendered interaction surface with discoverable scenes,
camera navigation, GPU-resident body selection and anchored grabbing, compact
diagnostics, and a minimal circle/rectangle construction loop without creating
a CPU physics mirror or platform-specific product.

A user opening the default deployment should be able to choose a maintained
scene, inspect its pacing and physics summary, navigate it deliberately, grab a
body at the touched point, and eventually place or erase basic bodies. This is
an evaluation workbench, not a substitute for the later P5.4 numerical quality
and P5.5 authored-scene gates.

## Human-authorized sequencing

The renewed S3.6 representation direction is accepted. Human review then
prioritized basic interaction because the current designated-body snap grab and
fixed camera prevent useful subjective feedback about stacking, settling, and
damping.

S4.1 through S4.3 therefore run before the S3 combined-representation closure
gate. They change input, camera, rendering, and grab interaction while keeping
the accepted contact representation baselines available. S4.4 placement and
erase depend on the selected pair-slot product graph because generational slot
reuse must invalidate persistent contacts correctly. P5.3 remains blocked until
both the S3 combined gate and this workstream complete.

## Accepted interaction contract

The shared engine owns all gesture meaning and applies these controls on every
interactive host:

- one touch over an eligible body attempts a grab; one touch over empty world
  does nothing and never pans the camera;
- a second simultaneous touch cancels any pending or active grab and captures
  both contacts for centroid pan plus midpoint-anchored pinch zoom;
- after a two-touch gesture, all participating contacts must lift before a new
  one-touch tool action can begin;
- primary mouse activates the current tool or UI target;
- secondary/right-button drag pans the camera and performs no reset, erase, or
  tool action;
- mouse wheel zooms around the cursor and is consumed by the canvas; and
- UI capture takes precedence over world tools, which take precedence over an
  otherwise inert one-touch empty-world gesture.

Middle mouse, single-touch empty-world pan, HTML controls, platform-specific
gesture defaults, and right-click erase are outside the accepted mapping.

## Ownership and dataflow

The engine owns camera state, raw-event interpretation, pointer capture, menu
and toolbar state, scenario selection, restart semantics, UI hit testing,
world-space hit requests, construction commands, rendering, and validation.
`physbox-launch` remains the only owner of scenario names and launch mappings.
Browser, desktop, and Android adapters transport raw pointer, contact, wheel,
lifecycle, and surface-size facts only.

Body hit testing executes on the GPU against current `BodyCoreV1` pose and
analytic shape state. A grab begin selects a deterministic nearest eligible
body and records the touched body-local anchor on device. Later pointer samples
upload only the current world target. No body result is read back to drive the
gesture or next simulation step.

Placement and erase are bounded body commands. GPU allocation owns free-slot
selection and generation changes; erase retires the live generation so stale
pairs, contacts, grab state, and commands cannot attach to a replacement body.
The CPU may own tool selection and authored command parameters, but it may not
mirror live body occupancy or choose a slot from stale readback.

Compact dynamic diagnostics may arrive asynchronously. The overlay labels the
sample age and never waits for GPU body/contact state. Static scene, camera,
pacing, and capacity facts may be displayed immediately because the shared host
already owns them.

## PhysBox 4 source boundary

PhysBox 4 revision `4295957e15480ef701e6006c7cafb5f0728ef101` is the behavior
and presentation reference for camera transforms, two-contact navigation,
body-local grab anchors, pointer tethering, toolbar layout, and the small bitmap
glyph atlas. PhysBox 5 may selectively port those focused patterns with exact
file-level provenance, but it must reshape them behind PhysBox 5 contracts.
Rapier, PhysBox 4 scene compatibility, its CPU rigid selection snapshot, fluid
tools, settings surface, and asynchronous CPU/GPU coupling are not imported.

## Scope and exclusions

S4 includes:

- deterministic shared camera transforms, recenter, right-drag mouse pan,
  wheel zoom, and two-touch pan/pinch;
- GPU-resident analytic circle/rectangle selection and body-local anchored
  grab with a visible anchor/tether;
- an engine-rendered toolbar and menu whose first panel is the maintained scene
  selector plus restart/recenter actions;
- a compact glyph-based debug overlay with pacing, body/contact, capacity,
  overflow, adapter, and asynchronous-sample-age facts;
- bounded `Grab`, `Circle`, `Rectangle`, and `Erase` tools with explicit
  command, allocation, and failure reports; and
- native, browser, Android AVD, available physical-device, and headless
  validation of the shared contracts.

S4 excludes general selection/inspectors, editable materials, save/load,
settings panels, arbitrary shape sizing, polygons, capsules, compounds, joints,
liquid, sleeping, scalable broadphase selection, polished product chrome, and
accessibility-system design. It does not accept final grab tuning, stack
quality, physical-quality bands, or CPU/GPU performance claims.

Grabbed bodies remain collidable. A ghost/non-collidable placement mode is a
future distinct tool, not the default grab behavior. Deep coincident placement
remains diagnosed and unsupported as ordinary construction; this work may not
hide it with collision disabling or an unreviewed depenetration change.

## Stop conditions

Stop for human direction before changing the no-catch-up clock, oracle
authority, accepted contact representation, physical defaults, numerical
tolerances, solver family, portable accumulation architecture, or final grab
quality bands. Also stop if placement requires CPU-owned slot allocation,
simulation-dependent readback, a CPU physics fallback, silent capacity loss,
or acceptance of deep coincident spawning.

Routine camera math, finite zoom bounds, screen-space gesture thresholds,
reversible overlay layout, colors, icons, glyph packing, bounded initial grab
compliance experiments, and raw-event plumbing remain autonomous. Preserve any
cross-host gesture or selection failure as a deterministic input trace. Initial
grab constants remain provisional until the deployed feel checkpoint.

## Delivery slices

### S4.1 — shared camera and gesture contract

- [x] Add versioned engine-owned camera state and raw wheel/view facts without
  changing platform ownership.
- [x] Implement deterministic screen/world transforms, aspect/resize handling,
  anchored zoom, recenter, finite zoom bounds, right-drag pan, and the accepted
  two-contact gesture state machine.
- [x] Make a lone empty-world touch inert and make the second touch cancel tool
  capture before navigation begins.
- [x] Prove camera motion changes only presentation and input projection: it
  must not change fixed-step admission, scenario hashes, or simulation state.
- [x] Add replayable mouse, wheel, and multi-touch traces covering cancellation,
  pointer loss, resize, second-touch promotion, and all-contacts-up rearming.

Gate: equivalent raw traces produce the same camera endpoint and projected
world targets on every host; no interactive frame adds a physics step; empty
single-touch motion is byte-inert for camera and mechanics.

Artifacts: versioned input traces, final camera records, world-target
projections, pacing reports, and negative adapter-ownership fixtures.

Commit boundary: frame/input contracts, camera transforms, shared gesture
state, raw adapter transport, deterministic tests, governing documentation, and
S4.1 status land together.

Completion evidence: [S4.1 camera and gesture package](../../evidence/p5.2a-s4.1/README.md).
Camera and input-trace schemas are version 1. The shared input state owns all
gesture meaning; native, web, and Android remain raw-fact transports. The
temporary designated-body grab now rejects an empty analytic shape hit on the
GPU, while general nearest-body selection and a body-local anchor remain
explicit S4.3 work. No physics default, scenario hash, fixed-step rule, solver
graph, tolerance, or oracle rule changed.

### S4.2 — efficient glyph UI, diagnostics, and scene discovery

- [x] Add one reusable shared UI quad/glyph renderer with a tiny immutable
  bitmap atlas, bounded instance buffers, explicit bytes/draws, and no per-glyph
  draw calls.
- [x] Render a compact toolbar plus menu affordance; the first menu panel lists
  the shared scenario catalog and exposes restart and recenter.
- [x] Add a compact debug overlay with frame cadence, effective simulation
  scale/slip, body/contact counts, capacity/overflow, adapter, and asynchronous
  diagnostic age where applicable.
- [x] Activate/restart a selected scenario at a declared frame boundary with no
  stale body, command, contact, grab, camera, or validation state.
- [x] Preserve direct URL/argument launch for validation and deep links, and
  prevent UI captures from leaking into camera or tool commands.

Gate: the default deployment makes maintained scenes discoverable; static UI
updates only when its model changes, dynamic overlay writes and draw calls are
bounded and measured, delayed diagnostics are labeled, and the DOM/app crates
contain no UI or scene policy.

Artifacts: glyph atlas/layout byte table, UI draw/write counters, scene-menu
interaction traces, zero-stale-state restart report, and fresh headless/browser
captures inspected at phone and desktop aspect ratios.

Commit boundary: shared UI primitives, toolbar/menu/debug rendering, scenario
catalog/reset contract, interaction tests, captures/evidence, provenance, and
S4.2 status land together.

Completion evidence: [S4.2 glyph workbench package](../../evidence/p5.2a-s4.2/README.md).
UI contract version 1 uses a 3,072-byte immutable R8 atlas, bounded static and
dynamic instance buffers, one UI pass, and two instanced draw calls. Static and
target-parameter uploads are cached. Shared scenario activation and restart
replace the complete engine world atomically; URL/argument launch remains
available. Desktop, phone, headless, and hardware-accelerated AVD captures pass
with declared-only diagnostics labeled `ASYNC AGE N/A`; no physical Android
device was connected.

### S4.3 — GPU selection and anchored collidable grab

- [x] Replace designated-body capture with a bounded GPU hit query over current
  analytic circles/rectangles, deterministic nearest-hit/tie ordering, and
  explicit no-hit/capacity diagnostics.
- [x] Compute and retain the touched body-local anchor on device so an
  off-center grab rotates the body instead of snapping its center to the
  pointer.
- [x] Replace the unconditional hold scan/teleport behavior with a bounded
  compliant grab constraint or equivalent measured GPU stage that keeps
  collision enabled and does not require a general joint system.
- [x] Coalesce pointer targets, cancel on second touch or pointer loss, render a
  shared anchor/tether, and retain a bounded release velocity.
- [x] Compare center/off-center, fixed-body rejection, empty-touch, obstacle,
  release, reset, generation-reuse, and high-cadence traces against invariants
  and the applicable Box2D grab behavior.

Gate: selection and anchor state never leave the GPU simulation dependency
graph; no-hit touches change no body; off-center dragging produces correctly
directed angular response; collisions remain enabled; no stale generation can
be moved; all overflow is atomic and diagnosed.

Artifacts: hit/anchor/constraint traces, tether captures, latency/pacing
diagnostics, minimized gesture failures, and synchronized Box2D comparison for
the accepted neutral grab fixtures.

Commit boundary: GPU selection, anchored-grab command/constraint path,
renderer feedback, exact/invariant tests, cross-host evidence, provenance, and
S4.3 status land together. Deploy this slice and request the human feel
checkpoint before finalizing grab constants.

Completion evidence: [S4.3 GPU anchored-grab package](../../evidence/p5.2a-s4.3/README.md).
The body-free grab-intent contract is version 1 with capacity 64. A begin runs
one deterministic GPU-resident analytic circle/rectangle scan, ordered by
nearest squared center distance then body slot and generation; fixed-only and
empty hits are rejected without moving a body. The selected generation,
body-local anchor, target, counters, and stale-generation state remain in one
64-byte GPU record. Held steps apply one sparse compliant point constraint;
the collidable body is never teleported or ghosted. While the pointer is down,
one 12-thread presentation dispatch writes a 192-byte vertex buffer consumed
directly by the shared tether draw. No simulation-dependent readback advances
selection, steering, or presentation.

The provisional spring acceleration/damping/max-acceleration values are
`45/10/80`, with linear/angular release bounds `20/30`. They selectively adapt
the named PhysBox 4 anchor/spring behavior and are deliberately not final
physical defaults. Three GPU cases replay exactly at seven checkpoints and 21
clean pinned Box2D public motor-joint processes complete with no invariant
failure. The largest observed GPU/Box2D deltas are reported, not accepted as
tolerances. Native, desktop/phone browser, headless, and the hardware-backed
`jstorrent-tablet` AVD pass; no physical device was connected.

The first AVD parallel selection shader exposed a MoltenVK code-generation
failure around workgroup-array sizing. Its minimized signature is preserved in
the evidence package. The accepted portable begin path is a serial GPU scan,
appropriate for the deliberately small P5.2a worlds. Scalable selection is
still deferred with broadphase; this slice makes no large-world picking claim.
The AVD also required compute-prepared tether vertices because direct
vertex-stage reads of the updated grab record remained stale. This is a
GPU-only presentation bridge, costs one tiny dispatch only during an active
pointer, and introduces neither CPU readback nor frame-latency dependency.

Manual checkpoint: push/deploy the exact commit, then review center and
off-center compliance, damping, retained release velocity, tether clarity,
floor/stack collision, and accidental-touch behavior. Do not freeze these
constants or proceed to construction on automated evidence alone.

### S4.4 — minimal circle/rectangle construction loop

Dependency: the S3 combined pair-slot plus stable-edge product integration and
generation-invalidation tests must pass first.

- [ ] Add bounded versioned place-circle, place-rectangle, and erase-at commands
  with finite values, authored material defaults, deterministic ordering, and
  atomic capacity failure.
- [ ] Allocate and retire body slots on the GPU, increment generations, clear
  stale grab/contact/pair state, and keep all current-state decisions on device.
- [ ] Add `Grab`, `Circle`, `Rectangle`, and `Erase` toolbar tools with touch-
  sized targets and deterministic UI/tool capture priority.
- [ ] Reject or diagnose unsupported deep-overlap placement rather than
  disabling collision, silently moving the body, or widening the accepted
  contact policy.
- [ ] Exercise place, fall, collide, grab, erase, slot reuse, reset, and scene
  switch through exact shared action sequences.

Gate: construction creates no CPU body mirror; erase plus slot reuse cannot
inherit a stale contact, grab, or command; every capacity/deep-overlap failure
is explicit; direct rendering and collision consume the new GPU state without
readback.

Artifacts: command/schema byte table, lifecycle traces, slot-generation and
pair-cache invalidation evidence, toolbar captures, capacity/deep-overlap
negatives, and cross-host scene contracts.

Commit boundary: command contracts, GPU lifecycle passes, minimal tools,
negative/exact tests, evidence, documentation, and S4.4 status land together.

### S4.5 — cross-host usability and feel checkpoint

- [ ] Capture one deterministic interaction sequence covering scene selection,
  two-touch and right-drag pan, wheel/pinch zoom, center/off-center grab,
  release, placement, erase, recenter, restart, and debug-overlay updates.
- [ ] Validate desktop, browser, headless projection, hardware-accelerated
  `jstorrent-tablet`, and the available physical Android device without
  substituting AVD timing for physical performance.
- [ ] Record fresh screenshots/reports, exact input traces, scene contracts,
  camera endpoints, UI draw/glyph counts, diagnostic age, source provenance,
  and any minimized failures.
- [ ] Visually inspect phone/desktop target sizes, gesture conflicts, clipping,
  camera stability, tether clarity, and tool discoverability.
- [ ] Deploy the exact pushed revision and stop for human review of grab feel,
  camera feel, discoverability, and whether the workbench is adequate for
  subjective stack/settling feedback.

Gate: a reviewer can open the default deployment, choose a maintained scene,
navigate it only through the accepted controls, interact with bodies without
snapping, inspect honest diagnostics, and complete the minimal construction
loop. All automated gates and freshness sentinels pass with no physics or
pacing regression.

Commit boundary: the compact S4 evidence package, final documentation, and
manual-checkpoint status land together. Final subjective acceptance is a later
human-decision commit.

## Dependencies and safe parallel work

Camera math, raw wheel transport, glyph/UI rendering, static menu layout, and
deterministic input traces share contracts but do not depend on contact layout.
They may be developed independently after the frame contract lands. GPU body
hit testing and anchored grabbing depend on current analytic shape data but not
on the final pair-cache organization.

Placement/erase is not safely independent: it must follow the combined S3 pair
slot integration because deletion and slot reuse exercise persistent identity.
Menu-triggered world replacement must also use the same reset boundary as GPU
commands; a platform-local shortcut is not safe. Final cross-host evidence runs
only after all selected paths are integrated.

## Validation baseline

Each slice runs the shared gates from `AGENTS.md`. Input, renderer, launch, or
pixel changes additionally build the Android APK, exercise the host-GPU AVD,
exercise the available physical device when connected, and visually inspect
fresh artifacts. Boundary fixtures reject HTML/app-owned menus, platform camera
semantics, duplicate scenario catalogs, CPU-owned slot selection, unlabeled
GPU diagnostics, or camera-dependent scenario results.

## Completion definition

S4 reaches its human checkpoint when one shared engine-rendered workbench works
across the maintained interactive hosts; the default deployment exposes scenes,
honest diagnostics, deliberate navigation, anchored collidable grabbing, and
minimal circle/rectangle place/erase tools; deterministic traces and cross-host
evidence pass; and no part of the work changes physics authority, pacing, or
GPU-state ownership. Human acceptance of interaction feel completes S4 and
authorizes P5.3 only if the separate S3 combined representation gate is also
closed.
