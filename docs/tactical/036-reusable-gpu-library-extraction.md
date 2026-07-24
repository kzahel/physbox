# S36 — Reusable GPU library extraction

Status: correction implemented 2026-07-24; final cross-host requalification is
in progress. The supported consumer result is one physics-only
`physbox-sim` crate. The initially extracted reusable renderer was rejected and
removed in S36.8.

Governing architecture:
[reusable GPU physics library boundary](../topics/reusable-gpu-library-boundary.md).
Authoritative engine direction:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Application composition:
[presentation and modular workbench architecture](../topics/presentation-and-workbench-architecture.md).
Platform boundary:
[platform architecture](../platform-architecture.md).
Pacing:
[simulation time and overload](../topics/simulation-time-and-overload.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).

## Goal

Extract a real `physbox-sim` workspace crate that an independent application
can consume by filesystem path. Preserve the existing first-party engine and
every maintained host while proving a Box2D-shaped rigid-world
construction/step surface, caller-owned GPU submission, supported PhysBox
configuration, canonical same-device physics observation, and asynchronous
snapshots.

The completion proof is a repository-local external-style consumer that has no
dependency on `physbox-engine`, `physbox-launch`, `physbox-ui`, the Box2D
comparator, an app crate, or a surface-owning renderer. The separate
`physbox-quest` repository then consumes the same crate without an upstream
Quest dependency.

## Exclusions

- Do not publish the crate, set `publish = true`, select a crates.io name,
  or promise semantic-version stability.
- Do not make `EngineHost`, `SurfaceRenderer`, workbench UI, launch presets,
  scenario catalogs, reports, or the Box2D shadow the reusable API.
- Do not expose a product/debug renderer, rendering surface, render target,
  camera, draw contract, or private solver-buffer view.
- Do not add a CPU product backend, automatic fallback, per-frame mechanical
  readback, or CPU mirror of live body state.
- Do not introduce a PhysBox-owned generic backend trait for Box2D comparison.
- Do not change physics equations, pass ordering, buffer semantics, solver,
  sleeping, liquid/coupling, capacity, pacing, or presentation defaults.
- Do not add new shapes, joints, materials, authoring features, or Quest/OpenXR
  platform code.
- Do not make PhysBox 5 tests or CI require a sibling checkout.
- Do not move validation-only scenario contracts, oracle policy, benchmarks,
  or product reports into the reusable crates merely to reduce engine files.

## Ownership constraints

- `physbox-sim` owns mechanical GPU state, command encoding, supported semantic
  configuration, fixed-step admission, stable logical handles, and bounded
  mechanical diagnostics and observations.
- `physbox-engine` owns first-party host composition, scenario projection,
  tools, camera/input policy, world rendering, UI, reports, validation, and
  comparator policy.
- `physbox-launch` owns external names, parsing, persistence, and projection
  into simulation configuration; `physbox-sim` may not depend on it.
- Platform adapters retain lifecycle, surface/target creation, raw event
  collection, native transport, packaging, and presentation only.
- The caller owns the WGPU device/queue, command encoders, submission,
  surfaces/OpenXR images, and final composition.
- Physics observation may remain GPU-to-GPU. No CPU body extraction may become
  necessary for the next frame.

## Unattended execution contract

Implementation may proceed without intermediate architecture decisions through
S36.1-S36.7. Commit each coherent slice after its focused checks pass. Stop
early only if:

- preserving simulation behavior requires a different dependency direction;
- a stable-handle design requires an unbounded CPU mechanical mirror;
- WGPU cannot express a safe same-device rendering seam;
- a maintained host cannot compile or execute after reasonable in-scope
  correction;
- a physics/configuration/scenario result changes unexpectedly; or
- a hard gate repeatedly rejects the extraction.

Private module placement, temporary compatibility re-exports, and exact public
type names may be adjusted while implementing, provided the governing
ownership and consumer experience remain intact.

## Ordered delivery slices

### S36.1 — Architecture and dependency contract

- [x] Add the durable reusable-library architecture topic.
- [x] Record caller, simulation, renderer, and first-party engine ownership.
- [x] Record the Box2D-shaped API intent and asynchronous-observation
      difference.
- [x] Record the supported/experimental/internal configuration boundary.
- [x] Record that Box2D and a generic backend wrapper remain consumer-owned.
- [x] Add this tactical to the status index in numeric creation order.

Gate: the architecture identifies one dependency direction, one GPU ownership
contract, explicit exclusions, ordered extraction seams, validation, and
reconsideration triggers before public code is added.

### S36.2 — Real simulation crate foundation

- [x] Add `physbox-sim` to workspace members and maintained default members
      with no engine, launch, UI, comparator, shell, or app dependency.
- [x] Move the fixed-step interactive clock and pacing types into the crate.
- [x] Define supported semantic `WorldDef`, capacity, quality, step, body,
      shape, material, geometry, and logical-handle types.
- [x] Split launch spelling/presets and engine menu labels from semantic
      simulation validation and defaults.
- [x] Retain temporary engine re-exports where they prevent unrelated caller
      churn.
- [x] Add dependency negatives and focused public contract tests.

Gate: `physbox-sim` owns real pacing/configuration behavior, the engine consumes
it without behavior change, and an independent crate can compile against its
definitions without pulling application vocabulary.

Implementation checkpoint, 2026-07-24: `physbox-sim` is a maintained leaf
crate owning the unchanged interactive zero-or-one-step clock, semantic physics
configuration and validation, capacity, sleeping, coupling-quality, and neutral
world/body/shape definitions. `physbox-launch` retains spelling, parsing,
migration, persistence, and compatibility re-exports while depending on the
simulation owner. The platform-boundary checker rejects engine, launch, UI,
comparator, shell, and application vocabulary in the simulation crate.

Focused simulation and launch tests pass 43/43. Warnings-denied clippy passes
for `physbox-sim`, `physbox-launch`, and the production engine library. The
engine library run passed 207/208 before the existing asynchronous
atomic-capacity assertion failed once; its exact isolated rerun passed. No GPU
source, simulation equation, dispatch graph, default, or first-party call site
changed in this slice.

### S36.3 — Mechanical/render resource separation

Historical intermediate slice, superseded in its public-API aspect by S36.8.
Separating mechanical and presentation ownership remains correct; making the
private render-resource bridge a consumer contract does not.

- [x] Remove target color format, body/particle render pipelines, background
      rendering, camera/view state, and visual theme from the mechanical world.
- [x] Introduce an explicit read-only same-device render view over the minimum
      GPU resources and metadata needed to draw.
- [x] Keep render metadata distinct from mechanical state and mutation.
- [x] Adapt the existing engine-owned renderer to consume the view before
      crossing the crate boundary.
- [x] Prove unchanged first-party render composition, simulation reports,
      dispatch graphs, and offscreen results.

Gate: the mechanical world constructs and steps without a render target, while
the unchanged first-party host can still render it without live-state readback.

### S36.4 — Complete simulation-world extraction

- [x] Move rigid state/lifecycle, construction, interaction mechanics,
      broadphase, narrowphase, contact persistence, solver, awake/sleep work,
      and mechanical timing into `physbox-sim`.
- [x] Move particle and fluid/solid coupling mechanics without changing their
      compute order, configuration, capacity, or defaults.
- [x] Expose a supported world constructor and caller-recorded outer step.
- [x] Implement generational body/shape handles and queued create/destroy
      operations through the existing GPU command path.
- [x] Keep scenario catalogs, validation verdicts, reports, and benchmark
      orchestration in engine/lab/tooling owners.
- [x] Project current rigid-only engine scenarios through neutral world
      definitions and operations instead of exporting `ScenarioV1`; retain the
      coupled particle seed behind the private adapter until particle
      authoring has a supported public façade.

Gate: `physbox-engine` consumes `physbox-sim` as its only product mechanical
implementation, ordinary scenarios retain results and graph accounting, and a
surface-free public rigid-world smoke steps successfully.

Extraction checkpoint, 2026-07-24: body storage and generational slot
allocation, awake/sleep state and work compaction, exact sleep grouping,
lifecycle retirement, construction intent mechanics, grab intent mechanics,
particle state, grid/density/correction passes, fluid/solid boundary response,
particle lifecycle/tools, and their GPU shaders now live in `physbox-sim`.
Temporary hidden re-exports keep engine validation callers source-compatible.
Engine profile wrappers no longer enter mechanical construction; they project
once into the semantic `PhysicsConfigurationV3` value.

The public `World` validates neutral definitions, reserves generational
body/shape handles, records one fixed outer step into the caller's encoder,
queues bounded post-initialization construction/destruction through the
existing GPU command path, exposes bounded counters and canonical physics
observation, and supports an explicitly submitted asynchronous body snapshot. The
first-party rigid host projects its authored state through `World`, `BodyDef`,
and `ShapeDef`, including exact optional mass-property overrides and authored
initial sleeping state. `ScenarioV1` remains engine-owned for reports,
validation metadata, and Box2D shadow projection. The current liquid seed
retains a private coupled adapter because S36 explicitly does not add a public
particle-authoring API.

### S36.5 — Optional reusable renderer

Historical intermediate slice, rejected and reversed by S36.8. These
checkboxes record what was tried; they do not describe the supported
architecture.

- [x] Add `physbox-render` with no engine, launch, UI, comparator, shell, or app
      dependency.
- [x] Move body and particle rendering behind the read-only `RenderView`.
- [x] Accept caller-owned target view, extent, world-to-clip transform, load
      operation, and encoder.
- [x] Keep surface creation, device creation, background choice/final
      composition, camera gestures, tools, and UI outside the crate.
- [x] Adapt `SurfaceRenderer` and `OffscreenRenderer` as first-party wrappers.
- [x] Add dependency negatives and focused render-contract tests.

Gate: first-party pixels and render accounting remain qualified, while an
external-style caller can render the same world into its own texture without a
surface or synchronous mechanical readback.

Implementation checkpoint, 2026-07-24: `GpuBodyWorld` and
`GpuParticleWorld` no longer accept a target format or construct presentation
pipelines. They expose a lifetime-bound, read-only same-device `RenderView`.
The new maintained `physbox-render` crate owns body, grab, particle, and
optional PhysBox background rendering while the caller chooses the target,
extent, view transform, load/clear policy, encoder, submission, and
composition. First-party host, benchmark, and evidence paths now compose that
renderer explicitly.

The complete 207-test engine library lane, both production and solver-lab
compile lanes, the particle pixel-order regression, focused renderer tests,
dependency negatives, and warnings-denied all-feature clippy pass. Mechanical
dispatch code, defaults, scenario projection, and GPU state layouts are
unchanged.

### S36.6 — External-consumer proof and API documentation

- [x] Add a non-default workspace example or fixture that depends only on
      `physbox-sim`.
- [x] Construct a rigid world with Box2D-shaped definitions and stable handles.
- [x] Record a fixed step into a caller-owned encoder and submit it from the
      consumer.
- [x] Observe canonical physics state without constructing any texture,
      renderer, camera, or surface.
- [x] Exercise one supported PhysBox quality toggle and one asynchronous
      observation.
- [x] Document local-path use, current-checkout semantics, device ownership,
      step ordering, observation latency, feature policy, and unsupported
      internals.
- [x] Verify that `physbox-quest` can replace the fixture with sibling path
      dependencies without any PhysBox 5 dependency on Quest.

Gate: the proof compiles and runs from public APIs only, produces fresh bounded
evidence, and no sandbox, launch, scenario, comparator, or surface-owner type
appears in its dependency graph.

Corrected implementation checkpoint, 2026-07-24: the non-default
`physbox-external-consumer` package imports only `physbox-sim`. It owns its
WGPU lifecycle, disables sleeping through supported configuration, constructs
a floor and ball, submits four steps, and completes an explicitly requested
body snapshot. It constructs no renderer, texture, camera, or surface. Its
fresh run emitted
`PHYSBOX_EXTERNAL_CONSUMER_SUCCESS bodies=2 steps=4 ball_y=0.876389 sleeping=false`.
The sibling `physbox-quest` repository imports only
`../physbox5/crates/physbox-sim`, owns its `PhysBoxBackend` and entity/handle
map, exposes the canonical GPU physics observation through that adapter, and
passes `cargo check` at Quest revision `b49f4cf`. PhysBox 5 has no dependency
on the sibling repository.

### S36.7 — Cross-host and performance qualification

- [x] Run focused crate, engine, renderer, and external-consumer tests while
      iterating.
- [x] Run `npm run test:fast`.
- [x] Run production clippy with warnings denied.
- [x] Run a fresh headless scenario with `/tmp` provenance.
- [x] Run `npm run test:web:lab`.
- [x] Prove default comparator feature-on, direct comparator tests, and
      explicit feature-off behavior.
- [x] Run focused performance probes and explain any material movement in GPU
      time, CPU encoding, allocation, or dispatch counts.
- [x] Run the ARM64 Android check and build a release APK.
- [x] Exercise the unchanged first-party world on a physical Android device;
      use and immediately stop an AVD only when no device is available or an
      emulator-specific question exists.

Gate: every affected maintained lane passes; simulation results, dispatch
graphs, ordinary defaults, pacing, and first-party behavior remain unchanged;
and the extraction introduces no unexplained material performance regression.

Final qualification checkpoint, clean code revision `4fef6b8`, 2026-07-24:

- focused runs pass 37 simulation unit tests, the public surface-free GPU
  integration test, 2 renderer tests, 130 engine tests, the external
  executable, warnings-denied all-feature clippy for the affected crates, the
  dependency negatives, and `cargo check` in `../physbox-quest`;
- `npm run test:fast` passes in 34.17 seconds, including Browser WebGPU run
  `ac2d9b83-5de9-4b9c-9e32-8b618641b5ca`, and production clippy passes with
  warnings denied;
- fresh headless bootstrap provenance is
  `/tmp/physbox5-s36-final-headless.json`, and solver-lab Browser run
  `ffaa1cd3-948d-4967-a1a3-c308da21239b` passes the engine-rendered menus,
  default-enabled comparator, and scene contract;
- both direct comparator tests pass, while the explicit release feature-off
  lane emits `PHYSBOX5_BOX2D_FEATURE_OFF_SUCCESS` with dependency and runtime
  markers absent;
- the focused performance probe reports 2.711 ms host P50, 2.741 ms host P95,
  and 2.350 ms CPU P50 for the 32-body free-flight sample. Those warm native
  timings vary across local runs and do not support a speed claim. The
  structural regression contract is unchanged at 175 solver dispatches and
  819,940 allocated bytes, with exact replay and clean capacity;
- the NDK-wrapped ARM64 check, ARM64 release-native/debug APK, and Gradle
  release APK pass. The 15,688,100-byte debug APK has SHA-256
  `bce612153ef604074305e2b281f1001d84af6be6b43cb8fd9e67bf4852125ade`;
  the 8,562,269-byte unsigned release APK has SHA-256
  `2a304ef23b03e2684d00da00815e5daecaa81364c8037a80a23fe03f32ff428d`;
  and
- physical Pixel 7a validation passes the unchanged bootstrap world on
  Mali-G710, captures a non-black 1080×2400 app screenshot, writes
  `/tmp/physbox5-s36-final-android-device.png.json`, and stops the product app.
  No AVD was started.

### S36.8 — Physics-only consumer correction

- [x] Delete the standalone `physbox-render` package and its consumer
      dependency.
- [x] Return the 2D body/particle/debug renderer and shaders to private
      `physbox-engine` ownership.
- [x] Remove `RenderView` and `ParticleRenderView` from the supported
      `physbox-sim` root API and public rustdoc.
- [x] Add a versioned canonical `GpuBodyObservation` that projects only pose,
      motion, generational identity, and public live/sleeping/fixed flags into
      a dedicated consumer-readable GPU buffer.
- [x] Keep shape presentation, indirect draw data, target formats, cameras,
      shaders, and private mechanical layouts out of that observation.
- [x] Convert the external fixture and sibling Quest adapter to depend only on
      `physbox-sim`.
- [x] Update dependency negatives and architecture documentation to reject a
      reusable PhysBox renderer.
- [ ] Re-run the full S36.7 cross-host qualification against the corrected
      boundary.

Gate: an independent physics adapter can create, step, and observe a world
without importing rendering or first-party application code; public rustdoc
contains no render-view/product-renderer contract; and the first-party renderer
continues to behave identically from its private engine owner.

Correction checkpoint, code revision `504e0ff` and Quest revision `b49f4cf`,
2026-07-24: all focused simulation tests, the no-surface canonical-observation
integration test, 132 engine tests including the private particle pixel
regression, the simulation-only external executable, dependency negatives,
public rustdoc scan, and Quest `cargo check` pass. Final maintained-host
qualification remains to be recorded below before closing the checkbox.

## Validation artifacts

Fresh artifacts remain untracked under `/tmp`. They record run ID, source
revision and dirty state, adapter/host, capture phase, complete scene and
configuration contract, simulation crate version, WGPU adapter, and
consumer proof outcome.

The external proof may perform one final asynchronous state snapshot. That
flush is evidence after the final requested frame, not a product dependency or
interactive synchronization precedent.

## Completion condition

S36 is complete when:

1. `physbox-sim` owns the declared production mechanics and has an enforced
   physics-only dependency and API boundary.
2. `physbox-engine` uses that crate as its mechanical owner and privately owns
   the first-party world renderer.
3. A public-API-only external fixture constructs, steps, and observes a rigid
   world with caller-owned GPU lifecycle and no rendering dependency.
4. PhysBox 5's maintained native, Browser WebGPU, comparator feature-on/off,
   headless, performance, ARM64, release APK, and physical Android gates pass
   in proportion to the changed boundaries.
5. The separate Quest consumer can use local path dependencies without an
   upstream Quest dependency or exposure of first-party sandbox machinery.
6. The governing architecture topic records the final physics-only API and any
   measured deviations.

The initial six conditions passed for the superseded two-crate boundary.
S36.8 reopens cross-host qualification before the corrected physics-only
boundary is complete. Publication, API stabilization beyond the recorded
support classes, Quest feature implementation, public particle authoring, and
a Quest-owned Box2D backend require separate consumer or release work and do
not block completion.
