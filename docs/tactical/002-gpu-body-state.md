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

Implementation evidence, 2026-07-15: the default bootstrap and explicit
`rigid-floor` scenario now run the accepted contact-centric Jacobi/gather
schedule against a precomputed two-point floor manifold. GPU count, prefix,
and scatter build the incidence topology; every impulse stage emits paired
records and gathers by body through ping-pong motion buffers. The exact
schedule remains four substeps, one iteration, and the accepted coefficients,
for 68 dispatches per outer step. Live separation and effective mass are
recomputed from GPU pose, while general narrowphase and outer-step contact
persistence remain deferred.

The [fresh evidence package](../../evidence/p5.1-s2.3/README.md) records clean
native, browser, `jstorrent-tablet` AVD, and physical Pixel 7a runs. At 120
steps the product settles at `y=0.19945242` with `vy=6.05e-7`; the clean pinned
Box2D authority settles at `y=0.19988221`, for an objective vertical delta of
`0.00042979`. Maximum product penetration is `0.000547588`, both contact
impulses are projected, solver scratch is 552 bytes, and no capacity
overflow occurs. Seven-checkpoint product and scalar/Box2D playbacks preserve
the trajectory and complete oracle provenance. No tolerance, default, source
pin, or solver decision changed.

- [x] Feed a precomputed two-point rectangle/floor manifold into the accepted
  contact-centric Jacobi path.
- [x] Build fixed topology on the GPU, emit equal-and-opposite contribution
  records, gather by body, and run four substeps with one iteration and the
  accepted parameters.
- [x] Keep the fixed floor in the same solver/body contract without adding
  general collision generation or persistent outer-step contacts.
- [x] Compare isolated passes with the scalar specification and compare the
  bounded landing trajectory with the unmodified Box2D authority projection.
- [x] Preserve first-divergence and capacity artifacts for any failure.

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

Implementation checkpoint, 2026-07-15: the shared host now converts primary
pointer facts into bounded begin/target/end commands and secondary-down into
reset. Commands remain queued across zero-step frames, motion targets
coalesce before the next admitted step, and an exact multi-step request
interleaves the same engine-owned scheduled command stream at authored step
boundaries. A one-thread ordered GPU command pass applies the batch before
the selected solver; a parallel hold pass keeps grabbed bodies at their
neutral target after each step without mapping live body state. Reset restores
the original pose and motion exactly, released or stale pointers no longer
move the body, and overflow rejects the complete pending batch rather than
executing a prefix.

The new `reset-grab` scenario owns six deterministic commands over 150 steps,
its canonical source hash, completion criteria, and command counters in shared
Rust. Exact GPU tests prove target following, release, overflow diagnosis, and
bit-identical replay after the second reset. Platform scripts only accept and
transport the shared scenario name; new boundary negatives reject body-command
types and numerical scenario payloads in app crates, JavaScript, and shell.
Cross-host clean-revision captures remain the evidence half of this slice and
must pass before the checklist below is marked complete.

The desktop adapter also has a bounded smoke wrapper that waits for shared
scenario completion, terminates the interactive event loop, and publishes a
fresh report with source and adapter provenance plus the standard success
sentinel. It does not add a desktop-owned scenario or alternate mechanics
path.

Completion evidence, 2026-07-15: the
[S2.4 package](../../evidence/p5.1-s2.4/README.md) retains clean reports from
exact native headless, interactive desktop, browser, `jstorrent-tablet` AVD,
and physical Pixel 7a lanes. All five report the same scenario hash, scene
contract, six executed commands, two resets, released final state, and no
overflow. Native GPU tests prove target following, stale-release rejection,
zero-step coalescing, atomic overflow, and bit-identical replay after reset.
The browser records one raw pointer event without changing the scripted
mechanics result. All four retained pixel captures were visually inspected.
AVD and physical pacing retain 0.15 s and 0.05 s of diagnosed permanent slip
rather than catch-up.

- [x] Convert primary pointer facts into engine-owned neutral grab begin,
  target, and release commands at fixed-step boundaries.
- [x] Add an engine-owned reset command and deterministic reset/grab/release
  validation action sequence.
- [x] Coalesce host pointer motion without inventing catch-up steps or
  platform-specific physics behavior.
- [x] Run the identical scenario and scene contract through headless, browser,
  desktop smoke, Android AVD, and an available physical Android device.
- [x] Add boundary negatives preventing scenario payloads or outcomes from
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

Implementation checkpoint, 2026-07-15: a shared `GpuBodyBenchmarkSession`
now creates the engine-owned deterministic free-flight scenario and executes
the existing `GpuBodyWorld` integration, grab-hold scan, and direct renderer.
Each retained sample is exactly one fixed step. Pipeline/device creation,
authored-state reset, final full-state mapping, and timestamp resolution are
outside the measured window. The output retains raw P50/P95/P99 GPU (when
usable), CPU encode/submit, and host-completion series; exact dispatch/draw
work; all product capacity records; 64 bytes per body; benchmark scratch; and
a hash of the completely validated final pose/motion/metadata state.

One native/Android CLI drives both Metal and Vulkan targets. A maintained
wrapper targets either the repeatable `jstorrent-tablet` AVD or an explicitly
available physical device, and a browser export drives the same session with
a post-submit mapped-buffer fence. Early diagnostics exposed that the Pixel 7a
Mali-G710 advertises timestamp queries but returns zero for both endpoints,
including the prior S1 solver/primitive benchmark. The body runner now records
that as rejected, unusable timestamp evidence and uses only the separately
named host-completion clock for that device; it never presents zero as GPU
execution time. Clean 1K/10K/100K capture and layout interpretation remain to
complete this slice, so the provisional ABI is still unfrozen.

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
