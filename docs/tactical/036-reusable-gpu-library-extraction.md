# S36 — Reusable GPU library extraction

Status: active, opened 2026-07-24 by human direction. Implementation is
authorized end to end through the repository-local consumer proof and
proportionate cross-host qualification, with coherent commits at each passing
slice.

Governing architecture:
[reusable GPU simulation and rendering library boundary](../topics/reusable-gpu-library-boundary.md).
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

Extract real `physbox-sim` and `physbox-render` workspace crates that an
independent application can consume by filesystem path. Preserve the existing
first-party engine and every maintained host while proving a Box2D-shaped
rigid-world construction/step surface, caller-owned GPU submission, optional
rendering into a caller-owned texture, supported PhysBox configuration, and
asynchronous GPU-state observation.

The completion proof is a repository-local external-style consumer that has no
dependency on `physbox-engine`, `physbox-launch`, `physbox-ui`, the Box2D
comparator, an app crate, or a surface-owning renderer. The separate
`physbox-quest` repository then consumes the same crates without an upstream
Quest dependency.

## Exclusions

- Do not publish either crate, set `publish = true`, select a crates.io name,
  or promise semantic-version stability.
- Do not make `EngineHost`, `SurfaceRenderer`, workbench UI, launch presets,
  scenario catalogs, reports, or the Box2D shadow the reusable API.
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
  mechanical diagnostics.
- `physbox-render` owns optional world rendering from a read-only same-device
  simulation view into a caller-provided target.
- `physbox-engine` owns first-party host composition, scenario projection,
  tools, camera/input policy, UI, reports, validation, and comparator policy.
- `physbox-launch` owns external names, parsing, persistence, and projection
  into simulation configuration; `physbox-sim` may not depend on it.
- Platform adapters retain lifecycle, surface/target creation, raw event
  collection, native transport, packaging, and presentation only.
- The caller owns the WGPU device/queue, command encoders, submission,
  surfaces/OpenXR images, and final composition.
- The simulation-to-render boundary is GPU-to-GPU. No CPU body extraction may
  become necessary for the next frame.

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

- [ ] Add `physbox-sim` to workspace members and maintained default members
      with no engine, launch, UI, comparator, shell, or app dependency.
- [ ] Move the fixed-step interactive clock and pacing types into the crate.
- [ ] Define supported semantic `WorldDef`, capacity, quality, step, body,
      shape, material, geometry, and logical-handle types.
- [ ] Split launch spelling/presets and engine menu labels from semantic
      simulation validation and defaults.
- [ ] Retain temporary engine re-exports where they prevent unrelated caller
      churn.
- [ ] Add dependency negatives and focused public contract tests.

Gate: `physbox-sim` owns real pacing/configuration behavior, the engine consumes
it without behavior change, and an independent crate can compile against its
definitions without pulling application vocabulary.

### S36.3 — Mechanical/render resource separation

- [ ] Remove target color format, body/particle render pipelines, background
      rendering, camera/view state, and visual theme from the mechanical world.
- [ ] Introduce an explicit read-only same-device render view over the minimum
      GPU resources and metadata needed to draw.
- [ ] Keep render metadata distinct from mechanical state and mutation.
- [ ] Adapt the existing engine-owned renderer to consume the view before
      crossing the crate boundary.
- [ ] Prove unchanged first-party render composition, simulation reports,
      dispatch graphs, and offscreen results.

Gate: the mechanical world constructs and steps without a render target, while
the unchanged first-party host can still render it without live-state readback.

### S36.4 — Complete simulation-world extraction

- [ ] Move rigid state/lifecycle, construction, interaction mechanics,
      broadphase, narrowphase, contact persistence, solver, awake/sleep work,
      and mechanical timing into `physbox-sim`.
- [ ] Move particle and fluid/solid coupling mechanics without changing their
      compute order, configuration, capacity, or defaults.
- [ ] Expose a supported world constructor and caller-recorded outer step.
- [ ] Implement generational body/shape handles and queued create/destroy
      operations through the existing GPU command path.
- [ ] Keep scenario catalogs, validation verdicts, reports, and benchmark
      orchestration in engine/lab/tooling owners.
- [ ] Project current engine scenarios through neutral world definitions and
      operations instead of exporting `ScenarioV1`.

Gate: `physbox-engine` consumes `physbox-sim` as its only product mechanical
implementation, ordinary scenarios retain results and graph accounting, and a
surface-free public rigid-world smoke steps successfully.

### S36.5 — Optional reusable renderer

- [ ] Add `physbox-render` with no engine, launch, UI, comparator, shell, or app
      dependency.
- [ ] Move body and particle rendering behind the read-only `RenderView`.
- [ ] Accept caller-owned target view, extent, world-to-clip transform, load
      operation, and encoder.
- [ ] Keep surface creation, device creation, background/final composition,
      camera gestures, tools, and UI outside the crate.
- [ ] Adapt `SurfaceRenderer` and `OffscreenRenderer` as first-party wrappers.
- [ ] Add dependency negatives and focused render-contract tests.

Gate: first-party pixels and render accounting remain qualified, while an
external-style caller can render the same world into its own texture without a
surface or synchronous mechanical readback.

### S36.6 — External-consumer proof and API documentation

- [ ] Add a non-default workspace example or fixture that depends only on
      `physbox-sim` and `physbox-render`.
- [ ] Construct a rigid world with Box2D-shaped definitions and stable handles.
- [ ] Record a fixed step into a caller-owned encoder and submit it from the
      consumer.
- [ ] Render into a caller-owned texture and read back only after the bounded
      proof completes.
- [ ] Exercise one supported PhysBox quality toggle and one asynchronous
      observation.
- [ ] Document local-path use, current-checkout semantics, device ownership,
      step ordering, observation latency, feature policy, and unsupported
      internals.
- [ ] Verify that `physbox-quest` can replace the fixture with sibling path
      dependencies without any PhysBox 5 dependency on Quest.

Gate: the proof compiles and runs from public APIs only, produces fresh bounded
evidence, and no sandbox, launch, scenario, comparator, or surface-owner type
appears in its dependency graph.

### S36.7 — Cross-host and performance qualification

- [ ] Run focused crate, engine, renderer, and external-consumer tests while
      iterating.
- [ ] Run `npm run test:fast`.
- [ ] Run production clippy with warnings denied.
- [ ] Run a fresh headless scenario with `/tmp` provenance.
- [ ] Run `npm run test:web:lab`.
- [ ] Prove default comparator feature-on, direct comparator tests, and
      explicit feature-off behavior.
- [ ] Run focused performance probes and explain any material movement in GPU
      time, CPU encoding, allocation, or dispatch counts.
- [ ] Run the ARM64 Android check and build a release APK.
- [ ] Exercise the unchanged first-party world on a physical Android device;
      use and immediately stop an AVD only when no device is available or an
      emulator-specific question exists.

Gate: every affected maintained lane passes; simulation results, dispatch
graphs, ordinary defaults, pacing, and first-party behavior remain unchanged;
and the extraction introduces no unexplained material performance regression.

## Validation artifacts

Fresh artifacts remain untracked under `/tmp`. They record run ID, source
revision and dirty state, adapter/host, capture phase, complete scene and
configuration contract, simulation/render crate versions, WGPU adapter, and
consumer proof outcome.

The external proof may perform one final bounded readback to validate pixels or
an asynchronous state snapshot. That flush is evidence after the final
requested frame, not a product dependency or interactive synchronization
precedent.

## Completion condition

S36 is complete when:

1. `physbox-sim` and `physbox-render` own the declared production behavior and
   have enforced dependency boundaries.
2. `physbox-engine` uses those crates without an alternate mechanical or world
   rendering implementation.
3. A public-API-only external fixture constructs, steps, observes, and renders
   a rigid world with caller-owned GPU lifecycle.
4. PhysBox 5's maintained native, Browser WebGPU, comparator feature-on/off,
   headless, performance, ARM64, release APK, and physical Android gates pass
   in proportion to the changed boundaries.
5. The separate Quest consumer can use local path dependencies without an
   upstream Quest dependency or exposure of first-party sandbox machinery.
6. The governing architecture topic records the final implemented API and any
   measured deviations.

Publication, API stabilization beyond the recorded support classes, Quest
feature implementation, and a Quest-owned Box2D backend require separate
consumer or release work and do not block S36 completion.
