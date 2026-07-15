# S2: GPU body state and fixed-floor contact

Status: active, 2026-07-15.
Direction: [one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Bring-up stage: [P5.1](../bringup-plan.md#p51--minimal-rigid-state-and-fixed-floor-contact-active).

## Goal

Replace the bootstrap body with the first product-path, generational GPU body
state. Integrate and render oriented rectangles directly from that state,
exercise the accepted Jacobi-gather solver against a fixed-floor manifold,
and support reset and neutral grab actions without simulation-dependent
readback. Finish with body-layout, command-ABI, capacity, and scale evidence
for human review before the representation is frozen or P5.2 begins.

## Governing contracts

This tactical executes, and does not revise:

- the [one-authoritative-state](../topics/physbox5-unified-gpu-physics.md#one-authoritative-mechanical-state)
  and [minimal body model](../topics/physbox5-unified-gpu-physics.md#minimal-2d-body-model)
  contracts;
- the accepted [S1.6 Jacobi-gather decision](../topics/physbox5-unified-gpu-physics.md#s16-solver-selection--accepted-2026-07-15),
  including four substeps, one iteration, and its checked parameters;
- the [performance and scale](../topics/performance-and-scale.md) requirements
  for bytes per body and 1K, 10K, and 100K free-flight integration/rendering;
- the [slow-time and explicit-step](../topics/simulation-time-and-overload.md)
  distinction; and
- the [single shared host](../platform-architecture.md) boundary.

The implementation remains one `EngineHost`, one GPU state, one scenario
source, and one render path. Platform adapters may transport raw input and
launch arguments but do not own body commands, physics actions, validation
outcomes, or numerical scene data.

## Scope boundaries

S2 includes dynamic oriented rectangles, one fixed floor represented by a
precomputed engine-owned manifold, gravity, direct rendering, reset, and a
single neutral grab interaction. The floor manifold is a temporary P5.1 input
to the selected solver, not a product narrowphase or contact-persistence
implementation.

S2 excludes circles, general pair generation, SAT/clipping, feature
persistence, warm-start transfer between outer steps, broadphase, sleeping,
joints, save/load, construction tools, liquid, CPU product physics, and any
required native-only acceleration. It may reuse the S1 scalar/oracle contracts
for validation but may not link an oracle into an app or add a second scene
format.

The provisional body and command layouts are deliberately reviewable. Normal
implementation may reorganize them while S2 is active, but the final layout,
capacity policy, and command ABI are not frozen until S2.6 human review.

## Stop conditions

Stop before the scheduled checkpoint if progress requires changing the
accepted solver family, accumulation architecture, physical defaults,
tolerances, oracle authority or pin, fixed-step policy, portability contract,
or equal-and-opposite current-state coupling. Also stop for a required CPU
fallback, native-only product path, simulation-dependent readback, silent
command/body loss, or a body/command representation whose downstream cost is
material and cannot be resolved from S2 evidence.

Ordinary shader organization, reversible buffer plumbing, test failures,
capacity-sizing mechanics, validation harness work, and unavailable physical
hardware do not require interruption. An unavailable device is retained as a
named evidence gap.

## Delivery slices

### S2.1 — generational body and bounded-command contracts

Implementation evidence, 2026-07-15: the provisional portable layout is four
separate 16-byte storage lanes (`pose`, `motion`, `properties`, and
`metadata`) for 64 allocated bytes per body slot. Metadata carries a nonzero
generation, live/fixed/sleep/rectangle flags, and exact friction/restitution
bits. A separately bounded 48-byte command record carries an operation, slot,
generation, monotonic sequence, and reset or grab payload. The checked schema
recomputes every lane and total byte count and rejects inconsistent overflow
claims.

The engine allocation validates rectangle-only P5.1 input and device storage
limits before creating buffers. Its deterministic lowest-slot registry bumps
the generation on release, and command upload rejects the old handle. Queue
overflow remains a valid structured capacity artifact but is never an
accepted batch. GPU allocation/upload tests run on the same portable `wgpu`
device path; no live mechanical CPU mirror or product command execution has
been introduced yet.

- [x] Define a versioned product body-layout description with generational
  handles, declared buffer lanes, byte accounting, capacity, high-water, and
  overflow state.
- [x] Define bounded reset/grab command records with stable ordering, target
  generation, finite payload validation, and deterministic stale-handle and
  overflow behavior.
- [x] Add the provisional GPU SoA allocation and initialize it from one shared
  rectangle scenario; retain no authoritative CPU mirror of live mechanics.
- [x] Add unit/structural tests for byte strides, handle reuse, stale commands,
  deterministic command ordering, capacity overflow, and platform-boundary
  ownership.

Likely boundary: `physbox-contracts` owns serializable versioned evidence
records; `physbox-engine` owns GPU storage and commands; `physbox-launch` owns
the scenario name. No app crate owns a physics record.

Artifacts: schema round trips, capacity records, layout byte table, and
negative stale/overflow fixtures.

Gate: declared and allocated bytes agree; generations prevent a reused slot
from accepting an old command; no accepted command or body is silently lost;
and the shared host can initialize the provisional state on portable WebGPU.

Commit boundary: contracts, allocation, negative tests, tactical checkboxes,
and status evidence land together without integration or contact behavior.

### S2.2 — GPU integration and direct rendering

Implementation evidence, 2026-07-15: `body-free-flight` now selects the first
live P5.1 world inside the shared `EngineHost`. A 64-thread portable compute
pass applies semi-implicit gravity and advances pose from the SoA motion lane;
fixed and sleeping flags prevent integration. Rendering uses one six-vertex
instance per live rectangle, reading pose, properties, and metadata directly
from the GPU allocation. The fixed floor is the second fixed body in the same
storage and render draw. The legacy bootstrap and S1 solver-laboratory lanes
remain available while S2.3 connects this product body path to contact.

The exact three-step test matches an independent scalar `f32` update within
`1e-5`, maps only after the bounded run, and proves that an exact zero-step
frame leaves pose and motion unchanged. Fresh native Metal, Chrome WebGPU,
`jstorrent-tablet` Vulkan SwiftShader, and physical Pixel 9 Mali-G715/Vulkan
runs all reported the same two-body scene contract and success sentinel. The
free-flight screenshots intentionally show that no floor collision exists
before S2.3; the native three-step capture retains the dynamic rectangle in
view and all fresh pixel artifacts were inspected.

- [x] Integrate gravity, position, and angle for active dynamic rectangles in
  one fixed-step GPU path using the shared clock's admitted step count.
- [x] Render every live rectangle directly from GPU pose/shape buffers with no
  per-frame body-state upload or readback.
- [x] Keep the fixed floor visual and body-instance count engine-owned.
- [x] Add bounded exact-step readback tests for analytic free flight,
  generation preservation, finite state, and zero-step immobility.

Likely boundary: a focused engine body-state module and WGSL shader replace
the bootstrap state/shader; surface and offscreen renderers continue to wrap
the same `EngineHost::render_frame` boundary.

Artifacts: fresh headless image/report, captured final validation state, and
initial bytes-per-body accounting.

Gate: exact integration matches the scalar analytic result inside a declared
`f32` bound, zero-step frames do not advance mechanics, rendering consumes the
same live GPU state, and no interactive frame encodes more than one outer
step.

Commit boundary: integration, direct rendering, validation capture, governing
documentation, and passing shared-host gates land together.

### S2.3 — selected fixed-floor Jacobi solve

- [ ] Feed a precomputed two-point rectangle/floor manifold into the accepted
  contact-centric Jacobi path.
- [ ] Build fixed topology on the GPU, emit equal-and-opposite contribution
  records, gather by body, and run four substeps with one iteration and the
  accepted parameters.
- [ ] Keep the fixed floor in the same solver/body contract without adding
  general collision generation or persistent outer-step contacts.
- [ ] Compare isolated passes with the scalar specification and compare the
  bounded landing trajectory with the unmodified Box2D authority projection.
- [ ] Preserve first-divergence and capacity artifacts for any failure.

Likely boundary: the product body path may share solver equations and portable
topology primitives with the S1 laboratory, but the laboratory remains
diagnostic tooling rather than an alternate live engine.

Artifacts: scalar/GPU pass comparison, Box2D request/result, landing trace,
capacity/dispatch record, and synchronized capture.

Gate: finite state, impulse projection, equal-and-opposite response, bounded
penetration/energy, accepted scalar pass tolerances, fresh oracle provenance,
zero overflow, and no simulation-dependent readback all pass.

Commit boundary: fixed-floor solving and its evidence land together; no
general narrowphase or tolerance change is bundled into the slice.

### S2.4 — reset, neutral grab, and shared scenario

- [ ] Convert primary pointer facts into engine-owned neutral grab begin,
  target, and release commands at fixed-step boundaries.
- [ ] Add an engine-owned reset command and deterministic reset/grab/release
  validation action sequence.
- [ ] Coalesce host pointer motion without inventing catch-up steps or
  platform-specific physics behavior.
- [ ] Run the identical scenario and scene contract through headless, browser,
  desktop smoke, Android AVD, and an available physical Android device.
- [ ] Add boundary negatives preventing scenario payloads or outcomes from
  moving into JavaScript, shell scripts, or app crates.

Artifacts: fresh per-host reports and images, one raw-event transport proof,
scenario hash, command counters, and a named physical-device result or gap.

Gate: reset replays exactly, stale/released grabs cannot move the body, the
held body follows the shared neutral target without readback, every available
host reports the same structural result, and every run emits a fresh success
sentinel and provenance.

Commit boundary: input semantics, shared scenario, platform transport tests,
and validation artifacts land together.

### S2.5 — layout and free-flight scale evidence

- [ ] Add an explicit-step benchmark mode that initializes 1K, 10K, and 100K
  rectangles from the shared deterministic generator and measures integration
  plus direct rendering after warmup.
- [ ] Record GPU timestamps when supported, CPU encode/submit and host
  completion separately, P50/P95/P99, dispatches, full work vector, allocated
  and high-water bytes, bytes per body, and every capacity counter.
- [ ] Measure the named native adapter, browser adapter, `jstorrent-tablet`
  AVD, and physical Android hardware when available; never substitute AVD data
  for a missing device result.
- [ ] Compare at least the provisional grouped SoA layout with any materially
  credible hot/cold split exposed by the measurements; do not fabricate a
  second candidate when no meaningful alternative exists.
- [ ] Validate complete final state only after the bounded benchmark run; no
  timing sample may include per-step mapping, pipeline creation, or allocation.

Artifacts: one fresh atomic `evidence/p5.1-s2.5` matrix, adapter-specific raw
rows, memory/layout table, cross-host captures, and explicit invalid/gap rows.

Gate: all accepted rows pass finite-state, exact-step, capacity, provenance,
and no-readback validity; 1K, 10K, and 100K curves exist on the native adapter
and every available required platform; and the evidence identifies any
bandwidth, dispatch, memory, thermal, or portability limit without weakening
the workload.

Commit boundary: benchmark runner, retained raw evidence, performance-topic
status, and tactical completion facts land together.

### S2.6 — body layout and command ABI review

- [ ] Publish a compact review report with the proposed frozen body lanes,
  command records, capacity policy, measured bytes and curves, cross-host
  status, known minimized failures, rejected alternatives, and explicit
  reconsideration triggers.
- [ ] State whether the evidence supports freezing the provisional layout and
  advancing to P5.2, but do not mark the layout/ABI frozen without human
  review.
- [ ] Stop for human acceptance before starting circles, general narrowphase,
  contact persistence, or any other P5.2 implementation.

Gate: the reviewer can assess layout quality, downstream compatibility,
capacity behavior, and platform evidence without reading the complete
implementation or raw logs.

Commit boundary: the review package and manual-checkpoint status land alone.
The later human decision is a separate documentation commit.

## Validation baseline

Every shared implementation slice runs the repository gates from `AGENTS.md`:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown -p physbox-web
cargo check --target aarch64-linux-android -p physbox-android
cargo run -p physbox-headless -- --output /tmp/physbox5-smoke.png --report /tmp/physbox5-smoke.json --scenario bootstrap
npm run check:boundaries
npm run test:web
```

Runtime, shader, rendering, launch, or pixel changes additionally build the
Android APK, run the maintained `jstorrent-tablet` AVD lane, attempt the
physical-device lane when available, and visually inspect fresh artifacts.
Slice-specific exact-step, oracle, benchmark, and failure tests are recorded
in the corresponding commit body.

## Completion definition

S2 reaches `manual checkpoint` when S2.1 through S2.5 pass, the S2.6 review
package is committed, and no body-layout or command-ABI decision is hidden in
implementation history. Completion requires subsequent human acceptance of a
portable representation with explicit capacity, no simulation-dependent
readback, direct rendering, accepted fixed-floor behavior, and complete scale
evidence. Only that acceptance may mark S2 complete and authorize P5.2.
