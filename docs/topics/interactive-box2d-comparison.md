# Interactive Box2D comparison

Status: decided and authorized for implementation under
[S29](../tactical/029-interactive-box2d-comparison.md), 2026-07-23.

Governing product direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Rigid authority:
[Box2D rigid authority](box2d-rigid-authority.md).
Performance exclusions:
[performance and scale](performance-and-scale.md).

## Decision

PhysBox needs a directly visible, interactive Box2D A/B lane in the existing
application. Batch metrics and generated playback remain necessary, but they
do not replace a human rapidly constructing, disturbing, settling, sleeping,
and waking many worlds while switching the visible result.

The default-enabled `box2d-comparator` compile feature adds one engine-rendered
menu choice:

```text
ENGINE: PHYSBOX / BOX2D
```

Desktop, browser, and Android expose the same shared Rust UI, tools, camera,
scene, settings projection, and behavior. Switching changes only the rendered
and interactively selected shadow; it does not copy state between engines.
Both worlds start from the same authored scenario and receive the same
ordered, stable-identity actions.

This is a validation feature, not a product fallback. PhysBox remains the only
authoritative product state and the only state eligible for product behavior,
save data, completion, performance, capacity, or release claims. Box2D cannot
replace a failed PhysBox step, satisfy a PhysBox gate, or provide mechanical
feedback to the GPU world.

## Compile and packaging contract

- `box2d-comparator` is enabled by default for maintained interactive builds.
- An explicit feature-off build removes the Box2D runtime, all FFI and
  packaging, its menu choice, shadow buffers, and runtime cost.
- Removing the feature may later become the ordinary release policy without
  changing the scene, UI, input, or PhysBox engine contracts.
- The clean ignored `reference/box2d` checkout remains the acceptance
  authority and is never copied wholesale.
- A comparator distribution may contain only an enumerated subset built from
  revision `3a4f0da8374af61293a03021c9a0b3ebcfe67948`, with the MIT license,
  source manifest, build settings, and generated-artifact digests recorded.
- Native desktop and Android may compile that subset directly. Browser may
  load an equivalent pinned WebAssembly module through an opaque leaf bridge.
  Platform adapters do not own Box2D scene, action, setting, UI, or report
  semantics.

The process-isolated native Box2D executable remains the only rigid acceptance
authority. The interactive runtime is visibly labeled review output and must
be checked periodically against the authority process on identical projections
so packaging drift cannot masquerade as a PhysBox disagreement.

## Shared-world and action contract

The shared host owns the dual execution only while the feature is present:

- scenario activation, restart, capacity change, and effective shared-setting
  change recreate both worlds from one neutral source;
- the shared clock admits zero or one PhysBox outer step and supplies the same
  fixed `dt` and substep count to Box2D;
- creation, repeated placement, erase, impulse/throw, grab begin/target/end,
  gravity, and reset are mirrored in monotonic order;
- successful creation, erase, and hit selection preserve one stable
  generational body identity in both worlds even after trajectories diverge;
- neither engine is initialized from the other's current transforms, motion,
  contacts, impulses, or sleep state; and
- no Box2D result is required for PhysBox to advance its next frame.

PhysBox's GPU-owned selection and allocation results may reach the comparison
lane through compact asynchronous feedback. The shadow may queue bounded
review work while an identity is unresolved, but that queue cannot block,
replay, or alter PhysBox time. A visible Box2D selection may issue an explicit
stable-identity mirror command to PhysBox; it may not upload Box2D mechanics.

## UI and visualization

The current workbench remains the comparison UI. No timeline, transport
controls, split screen, DOM panel, or separate camera are added.

- `ENGINE` appears in the engine-rendered menu only when compiled in.
- `PHYSBOX` is the initial visible selection after a fresh launch.
- The two worlds continue to advance while either is visible.
- Shared world settings remain visible in both modes.
- PhysBox-only solver controls are hidden while Box2D is visible; their values
  remain unchanged and reappear when switching back.
- Camera, tool, menu location, and pointer policy do not change on a switch.
- Box2D bodies use the same shapes, body colors, renderer, and debug glyph
  geometry as PhysBox.
- `b2Body_IsAwake` supplies authoritative Box2D awake/asleep display.
  Quiet-candidate or recent-transition display may be derived from public
  motion/history only when labeled as an observation. Internal Box2D sleep
  causes are not invented.
- The visible HUD names the engine and exact Box2D revision so screenshots
  cannot be misidentified.

## Parameter equivalence

Geometry, stable identities, transforms, velocities, mass/inertia, material,
gravity, fixed `dt`, substeps, sleeping request, and neutral actions are shared
where Box2D supports them exactly.

PhysBox contact hertz, damping, static multiplier, speculative distance,
depenetration limit, iteration policy, scheduler, and other implementation
controls are not translated into invented Box2D equivalents. They are hidden
in Box2D mode and remain source-only provenance. Box2D retains its pinned world
and solver policy. The UI and reports distinguish shared inputs from
engine-specific controls.

## Performance and behavior boundaries

Comparator-enabled frames are ineligible for PhysBox performance claims even
when PhysBox is visible. Their CPU time, foreign WebAssembly time, state upload,
and shadow storage are validation overhead. Performance tools must compile the
feature out or explicitly prove the shadow is absent.

Box2D disagreement is evidence, not an automatic PhysBox failure. Existing
invariants, scalar specifications, declared scenario bands, and oracle
precedence continue to decide acceptance. Conversely, a visually appealing
Box2D result cannot conceal a PhysBox regression.

## Gates

- one shared Rust engine selection and report contract;
- default feature-on and explicit feature-off compilation;
- no Box2D symbol, asset, menu entry, state buffer, or runtime work in the
  feature-off artifact;
- identical source scenario hash, body identities, shared settings, action
  sequence, and completed-step count across shadows;
- restart and configuration changes recreate both worlds without state copy;
- stable-ID placement, repeated placement, erase, grab/throw, and reset after
  divergent trajectories;
- Box2D interactive final state agrees with the clean process oracle for
  governed identical projections;
- default PhysBox rendering and interaction remain unchanged with the feature
  enabled but Box2D unselected;
- native desktop, Browser WebGPU, ARM64 Android, release APK, and physical
  Android execution;
- browser code remains an opaque runtime/FFI leaf with all semantics in shared
  Rust; and
- comparison mode remains excluded from every performance artifact.

## Reconsideration triggers

- The shadow materially perturbs default PhysBox interaction or frame pacing
  even while unselected: stop stepping it while hidden or make the compile
  feature release-off; do not weaken performance gates.
- A platform cannot run the same pinned Box2D behavior: stop that platform's
  promotion rather than substitute another engine or approximate settings.
- Stable-identity mirroring requires blocking GPU readback: redesign the
  asynchronous action-result path rather than make PhysBox CPU-dependent.
- Box2D packaging repeatedly drifts from the authority process: remove the
  packaged runtime until the build provenance is reproducible.
- The feature is no longer useful after rigid parity: compile it out or remove
  it in one explicit cleanup change with the ordinary product path unchanged.
