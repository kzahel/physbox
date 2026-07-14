# PhysBox 5 agent rules

Read `README.md`, `docs/topics/physbox5-unified-gpu-physics.md`,
`docs/bringup-plan.md`, and `docs/platform-architecture.md` before changing
runtime, rendering, input, scheduling, validation, deployment, or platform
code. Read `docs/topics/README.md` and `docs/tactical/README.md` before adding
or reorganizing project documentation.

## Repository documentation

`docs/topics/physbox5-unified-gpu-physics.md` is the authoritative technical
direction. `docs/vision.md` is only its compact entry point. A topic document
is a durable record of a technical concern: decision state, evidence,
alternatives, validation, and reconsideration triggers. Topic filenames are
unnumbered kebab-case and the topic index must list every active topic.

Tactical documents are executable workstream plans. Their filenames use a
three-digit zero-padded prefix assigned in first Git-creation order; never
renumber or reuse a number. `docs/tactical/README.md` is the unnumbered status
index. A tactical must define its goal, exclusions, ownership constraints,
ordered delivery slices, gates, and completion condition. Update its status and
checkboxes in the same change that makes them true.

## One shared host

PhysBox 5 has one engine, one simulation state, one frame path, one renderer,
one input policy, one scene contract, and one validation-scenario source.
Desktop, browser, headless, and Android are adapters around that shared host.
They may own lifecycle, surface/target creation, raw event collection, native
argument transport, packaging, logging, and presentation. They may not own
physics, tools, camera semantics, render passes, shaders, scenario outcomes, or
product defaults.

`EngineHost::render_frame(FrameInput, RenderFrameContext)` is the shared frame
boundary. No GPU result may be required on the CPU to advance the next
interactive frame. Compact diagnostics may arrive asynchronously; bounded
headless validation may flush after its final frame.

The browser document is a fullscreen canvas host. Do not add ordinary product
UI, debug panels, status chrome, or diagnostics in HTML/CSS. A minimal DOM
message is allowed only when WebGPU startup fails before the engine can render.

## Physics direction

The product physics path is GPU-resident and portable WebGPU. Do not add a CPU
physics backend, Rapier dependency, platform fallback, or backend-substitution
facade. Box2D and PhysBox 3 are validation oracles only. PhysX, wgmath, and
PhysBox 4 are source references, not dependencies.

The first physics milestone is the engine-owned rigid solver laboratory from
`docs/bringup-plan.md`. Do not add liquid, joints, tools, settings UI, scene
compatibility, or deferred product features before its gates are satisfied.

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

For normal shared changes run:

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

After shared runtime, rendering, shader, launch, or pixel changes, also run
`npm run android:apk` and the maintained Android device/AVD validation lane
when its environment is available. Visually inspect fresh pixel artifacts.

Never weaken a gate to accommodate platform-local engine behavior. Add a
negative fixture when a new duplication shape is discovered.
