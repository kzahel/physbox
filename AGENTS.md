# PhysBox 5 agent rules

Read `README.md`, `docs/topics/physbox5-unified-gpu-physics.md`,
`docs/topics/performance-and-scale.md`,
`docs/topics/simulation-time-and-overload.md`, `docs/bringup-plan.md`, and
`docs/platform-architecture.md` before changing physics, runtime, rendering,
input, scheduling, validation, deployment, or platform code. Read
`docs/topics/README.md` and `docs/tactical/README.md` before adding or
reorganizing project documentation.

## Repository documentation

`docs/topics/physbox5-unified-gpu-physics.md` is the authoritative technical
direction. `docs/topics/performance-and-scale.md` owns the benchmark, capacity,
CPU-comparison, and scale-claim contract.
`docs/topics/simulation-time-and-overload.md` owns interactive pacing and the
no-catch-up slow-time contract. `docs/vision.md` is only a compact entry point.
A topic document is a durable record of a technical concern: decision state,
evidence, alternatives, validation, and reconsideration triggers. Topic
filenames are unnumbered kebab-case and the topic index must list every active
topic.

Tactical documents are executable workstream plans. Their filenames use a
three-digit zero-padded prefix assigned in first Git-creation order; never
renumber or reuse a number. `docs/tactical/README.md` is the unnumbered status
index. A tactical must define its goal, exclusions, ownership constraints,
ordered delivery slices, gates, and completion condition. Update its status and
checkboxes in the same change that makes them true.

## One shared host

PhysBox 5 has one engine, one authoritative product simulation state, one frame
path, one renderer, one input policy, one scene contract, and one
validation-scenario source.
Desktop, browser, headless, and Android are adapters around that shared host.
They may own lifecycle, surface/target creation, raw event collection, native
argument transport, packaging, logging, and presentation. They may not own
physics, tools, camera semantics, render passes, shaders, scenario outcomes, or
product defaults.

`EngineHost::render_frame(FrameInput, RenderFrameContext)` is the shared frame
boundary. No GPU result may be required on the CPU to advance the next
interactive frame. Compact diagnostics may arrive asynchronously; bounded
headless validation may flush after its final frame.

The default-enabled `box2d-comparator` compile feature is the sole exception
to the single-state wording: it may maintain one non-authoritative Box2D shadow
world for explicit human A/B review. The shared host owns its scene/action
projection, stepping, selection, rendering choice, and engine-rendered menu.
The shadow may never feed PhysBox mechanics, become a fallback, satisfy a
PhysBox gate, enter performance measurements, or alter behavior when compiled
out. Desktop, browser, and Android must expose the same comparison semantics;
platform code may own only the foreign runtime packaging and opaque FFI.

Interactive scheduling preserves the selected fixed `dt` but never repays wall
time with multi-step catch-up. An interactive engine frame executes zero or one
outer fixed step; excess whole-step wall time slips permanently and the whole
simulated world slows. Solver substeps and explicitly requested headless
validation steps are not catch-up. Platform adapters must not own accumulators,
time dilation, step admission, or alternate overload behavior.

The browser document is a fullscreen canvas host. Do not add ordinary product
UI, debug panels, status chrome, or diagnostics in HTML/CSS. A minimal DOM
message is allowed only when WebGPU startup fails before the engine can render.
Authored browser JavaScript/TypeScript is a leaf input/display bridge: it may
forward raw browser facts and apply opaque engine results, but it may not know
tool, menu, camera, grab, placement, solver, physics-configuration, scenario,
or report semantics. Browser persistence schemas, migrations, defaults, and
query mappings belong to shared Rust. Semantic browser reports may be exposed
only by an explicitly smoke-gated observer that is not part of product
operation.

## Physics direction

The authoritative product physics path is GPU-resident and portable WebGPU.
Do not add a CPU product backend, Rapier product dependency, platform
fallback, or automatic backend substitution. Native CPU engines otherwise
exist only in isolated test tooling under the oracle execution contract.
Box2D may additionally run through the explicitly authorized
`box2d-comparator` shadow-review feature; that exception is validation
presentation, not a second product authority. The later PhysBox 3 sidecar is a
validation oracle only. PhysX, wgmath, and PhysBox 4 are source references, not
dependencies. Rapier2D is not part of P5.0a; adding an optional non-gating
comparator requires an explicit measured decision in the authoritative topic.

The first physics milestone is the engine-owned rigid solver laboratory from
`docs/bringup-plan.md`. Do not add liquid, joints, tools, settings UI, scene
compatibility, or deferred product features before its gates are satisfied.
Every solver, layout, pipeline, broadphase, sleeping, and capacity decision
must produce the scale evidence required by
`docs/topics/performance-and-scale.md`; do not defer performance design until
the full rigid engine exists.

## Local references and oracle hygiene

Read `reference/README.md` and `reference/pins.toml` before implementing or
instrumenting physics from an external engine. Run `npm run references:status`
before relying on a checkout and `npm run references:sync` to obtain or restore
the clean pinned checkouts. Ignored checkouts must never be added wholesale.
The Box2D comparison feature may carry only an explicitly enumerated
redistributable source/runtime subset generated from the clean pin, with a
manifest, exact revision, build recipe or artifact digest, and preserved
license. That packaged subset is not the authority checkout.

The clean `reference/box2d` checkout is the rigid acceptance authority.
`../physbox3` is the product-behavior oracle; its nested LiquidFun, Box2D, and
WASM bridge paths are mapped and pinned in `reference/README.md`. Do not confuse
an instrumented reference, a dirty sibling tree, or PhysBox 3 product behavior
with the unmodified Box2D authority lane. Oracle artifacts must record every
relevant revision, build option, and dirty or patch digest.

Routine oracle automation uses process-isolated native executables that
consume the engine-owned neutral scenario projection and emit the canonical
result schema. They do not link into the product engine or author their own
fixtures. Use native unmodified Box2D v3 for rigid acceptance. Later, use the
PhysBox 3 particle sidecar compiled natively with its pinned Box2D v3 source as
the primary coupled-behavior oracle and original native LiquidFun as a
secondary algorithm/scenario reference. The PhysBox 3 web product is a small
calibration and subjective-review lane, not a routine correctness dependency.
The interactive Box2D shadow is a separate review lane: it may link or load
the same pinned source behind the compile feature, but its state and output are
never accepted as authority artifacts. Any authoritative comparison still
runs through the process-isolated oracle.

Oracle precedence is declared, not voted: invariants and the scalar
specification own mathematical/pass correctness; Box2D owns the selected rigid
behavior target; the native PhysBox 3 sidecar owns the later coupled-behavior
target. Original LiquidFun, a web calibration trace, or any future Rapier2D
lane may expose disagreement but may not overrule those authorities silently.

References are source material and validation inputs, not product
dependencies. Do not edit a clean checkout in place for microscope tracing:
create a separate worktree or clone and record its base revision and patch
digest. Copied code requires explicit provenance and license preservation;
prefer an independent implementation of the documented contract.

## Launch and validation ownership

`physbox-launch` is the only owner of launch names, defaults, parsing, help,
web query mappings, and Android JSON argv decoding. App crates transport
arguments unchanged and project `LaunchOptions` into `EngineConfig` exactly
once.

Cross-host scenarios, actions, completion criteria, and expected structural
outcomes belong in shared Rust. Browser and Android validation may inject one
raw host event and consume the engine result; they must not recreate a product
scenario or pin engine-owned numerical payloads.

Fresh validation artifacts require a run id, source revision and dirty state,
host, capture phase, and complete scene contract. A lane passes only when its
command exits successfully, emits `PHYSBOX5_VALIDATION_SUCCESS`, and produces
fresh provenance where an artifact is required.

## Deployment

The tracked pre-push hook schedules production deployment only after a
successful push of `main`. It deploys the exact pushed SHA to
`physbox5.kzahel.com` from a clean sibling worktree. Do not weaken this into a
deployment of an uncommitted working tree. Keep Wrangler resource names and
custom-domain configuration owned by `worker/wrangler.toml` and
`scripts/deploy-web.sh`.

## Commit messages

These rules define message format when a commit is requested; they do not by
themselves authorize committing or pushing.

Use an imperative Conventional Commit subject:

```text
<type>(<scope>): <specific outcome>
```

The scope may be omitted for repository-wide documentation or tooling. Use
`feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`, or `chore`.
Prefer stable scopes such as `engine`, `launch`, `shell`, `validation`, `web`,
`android`, and `deploy`. Keep the subject concise, lowercase after the colon,
without a trailing period. Examples:

```text
feat(engine): add scalar contact preparation
test(validation): compare floor impacts with Box2D
docs: define tactical workstream conventions
```

Use the body for why the change was made, non-obvious tradeoffs, and the exact
validation run. Any intentional physics-default, tolerance, oracle, numerical
baseline, or performance-gate change must state measured before/after results
and the reason. Selective PhysBox 4 reuse must cite its source commit. Avoid
vague subjects such as `update`, `fix stuff`, or milestone-only labels. The
fresh-history bootstrap root commit predates this convention and is not a
template for later subjects.

## Required gates

The workspace `default-members` are the maintained production surface: the
four app adapters plus `physbox-engine`, `physbox-contracts`, `physbox-launch`,
and `physbox-shell`. The ordinary code-change lane is:

```sh
npm run test:fast
```

It runs format, maintained native library tests including the bounded
performance smoke, host-tool tests, platform-boundary negatives, and one real
browser WebGPU smoke under a 120-second hard budget. It already compiles the
WASM product and must not be followed by duplicate standalone Rust/browser
commands merely to repeat the same scope. Use `npm run test:perf:smoke` only
when iterating on the two focused performance probes; those probes are already
included once in `test:fast`.

Add gates in proportion to the changed boundary:

- Shared Rust behavior or public contracts: run production clippy with
  warnings denied before handoff.
- Simulation, scenario, report, or offscreen-render changes: add one headless
  smoke with fresh `/tmp` artifacts.
- Android adapter, target dependencies, native argument transport, or
  Android-specific `cfg`: add the ARM64 Android check. Build/install an APK and
  use one physical device for Android behavior or pixels; do not duplicate a
  passing physical lane on an AVD without a platform-specific question.
- Intentional physics-default, solver/shader/buffer-layout, pacing, or
  user-visible cross-host changes: add the ARM64 check, release APK, and one
  physical Android validation. The everyday browser result is the browser
  representative. Use an AVD only when no device is available or the failure
  requires it.
- Solver-menu/configuration interaction changes: add `npm run test:web:lab`;
  it is not an ordinary smoke.
- Box2D comparator changes: prove the default feature on and explicit feature
  off, run the direct comparator crate tests, `npm run test:web:lab`, the ARM64
  Android check, a release APK, and one physical Android validation. The
  feature-off build must contain no comparator menu or packaged Box2D runtime.
- If Chrome/WebGPU is unavailable, record that limitation and run the
  standalone WASM check rather than silently substituting a CPU path.

Do not add `--workspace` to an ordinary command: that opt-in scope includes
closed solver experiments, evidence generators, native oracles, and benchmark
tools which are not routine pre-push gates.

When a change touches `physbox-lab`, a native oracle, or a benchmark tool, run
the directly affected library, binary, integration test, or evidence command
by package and target name. Do not sweep unrelated historical candidates.
`npm run test:rust:full` and `npm run clippy:rust:full` preserve the complete
workspace audit lanes; run them only for workspace-wide validation-tooling
changes, a scheduled audit, or an explicit human request. A solver candidate's
own tactical gates remain mandatory while that candidate is active, but a
failed or closed candidate is not a routine production gate.

Long 600/1,200-step scripts, coefficient/configuration matrices, full oracle
process suites, multi-host evidence, APK/device/AVD matrices, and complete
performance samples belong to acceptance, promotion, release, or explicitly
scheduled audits. A hard authoritative rejection stops later expensive lanes
unless the user requests characterization. When an AVD is used, shut it down
immediately after the run, including after failed or interrupted runs; do not
leave an emulator running in the background.

Never weaken a gate to accommodate platform-local engine behavior. Add a
negative fixture when a new duplication shape is discovered.
