# PhysBox 5

PhysBox 5 is a fresh-history, portable WebGPU experiment toward one
GPU-resident 2D physics world. It begins deliberately small: a shared Rust,
`wgpu`, and WGSL host running unchanged behind desktop, browser, headless, and
Android adapters.

The shared-host bootstrap and oracle-driven rigid solver laboratory are
complete. P5.1 minimal rigid state and fixed-floor contact is complete. P5.2
now has selected stable generational pair slots and stable body-edge views, a
qualified combined product graph, shared camera/menu/grab and construction
tools, and the human-accepted colored in-place manifold-block solver. Human
review also accepted bounded parallel scheduler construction as the product
schedule; its serial predecessor remains an explicit validation control. S6
through S17 record the closed settling investigations and rejected candidate
chain; none changed the selected hard/direct manifold-block product solver.
S18 is complete: its 15-second solver scoreboard exposed and verified the
validation-only authority-key ordering fix, all 30 coefficient tuples remained
valid, and none preserved the 16-stack. Direct interactive review then made a
new product decision: the quieter, Box2D-derived soft-coupled family is the
intended default, with its visible tall-stack compliance accepted as the
starting trade-off. S19 completed that decision: soft-coupled 60/4/i1 is the
fresh ordinary default, and fixed rate, substeps, iterations, and contact
response are one bounded shared configuration. Hard/direct remains an explicit
comparison rather than an automatic fallback. The scheduler promotion is an
architecture and latency improvement, not acceptance of the current CPU
comparison, residual-motion band, or liquid performance.
S20 then reduced the warm everyday native/browser lane to 7.31 seconds and
published the current capability ledger. S21 clean evidence and human review
selected full-sort one-axis SAP as the scalable broadphase. SAP now owns
ordinary product candidate generation, with brute force retained only as a
bounded exact validation oracle and no automatic fallback. Clean native
100/1K/10K integrated curves and the 100-body Browser WebGPU product scene
pass. Physical Pixel 7a bootstrap and 103-body bucket execution close S21;
the bucket's `0.591` effective scale remains an explicit limitation rather
than a sustained-rate claim. Interactive world capacity is now a shared
64/256/1K/10K preset, with 64 retained as the safe default.

The current awake rigid-quality hierarchy is explicit: 2/4/8-body stacks are
the regression foundation, 12 is the blocking product target, 14 is stretch,
and 16 is a non-blocking matched-Box2D frontier diagnostic. Sleeping can never
satisfy an awake gate. Historical S5-S18 16-stack results remain evidence
under their original contracts; they no longer define the current product
minimum. See [rigid quality targets](docs/topics/rigid-quality-targets.md).

The first clean current-default checkpoint passes the native 2/4/8 foundation,
but its structurally upright 12-stack measures `0.029767 m/s` against the
existing `0.025000 m/s` awake quiet band. S25 qualified an eight-substep custom
alternative across native, Browser WebGPU, and physical Pixel paths, but human
review chose its lower-cost four-substep default and accepted the limitation
for now. The conservative product description is therefore an **8-box awake
quality stack** with demonstrated 14-box structural survival: 12 and 14 remain
standing for 1,200 steps but are not quietness passes, 15 is unmeasured in the
final default run, and 16 loses structure at step 463. Sleeping may not replace
that awake result. See [S24](docs/tactical/024-current-rigid-quality-qualification.md)
and [S25](docs/tactical/025-bounded-awake-configuration-qualification.md).

Human review accepted that measured rigid foundation as sufficient for bounded
liquid, UI, and interaction work without claiming that the remaining P5.4
parity or CPU/GPU crossover gates passed. S23 and S30 are complete; S32
promotes sleeping on with exact current-contact-component grouping as the
ordinary default. Sleeping remains independently switchable, and local-4 and
local-8 remain available for comparison.
[S31](docs/tactical/031-coupled-liquid-sandbox.md) now implements one
GPU-resident water material, same-step translation/torque/pressure response
with the PhysBox GPU rigid solver, fluid wake roots, Water/Surface/Erase tools,
compact neighborhoods, an 8K preview reserve plus an independent 32K capacity
frontier, direct rendering, process-isolated PhysBox 3/LiquidFun evidence, and
native/Browser/authorized-hardware-AVD qualification.
The 3,952-particle scene runs at full simulated time in the clean Browser
sample. After the first hands-on review, its particle allocation fell from
`3,443,844` to `887,940` bytes and clean named-native mean frame work fell from
`7.311` to `5.484 ms`; the 32K lane remains capacity evidence. Current
sleeping-on/exact AVD repeats execute at only `0.440–0.441` effective scale,
while an explicit sleep-off diagnostic reaches `0.686`. Those AVD results are
portability and attribution evidence, not physical-device speed.
S31 is at a human checkpoint for behavior bands, subjective feel, and whether
the sandbox/tools/style become ordinary defaults.

The repository still contains no Rapier or CPU product physics fallback,
legacy settings UI, multi-material fluid, smooth-surface water renderer, or
PhysBox 4 scene compatibility. The implemented
default-on, compile-removable Box2D shadow comparator is validation
presentation only: it never supplies PhysBox mechanics or performance results.
It remains rigid-only: PhysBox water is hidden in Box2D view and never
exchanges state or forces with that shadow.

Read:

- [Authoritative GPU-physics direction](docs/topics/physbox5-unified-gpu-physics.md)
- [Current rigid quality targets](docs/topics/rigid-quality-targets.md)
- [Performance and scale contract](docs/topics/performance-and-scale.md)
- [Simulation time and overload contract](docs/topics/simulation-time-and-overload.md)
- [Physics configuration and solver profiles](docs/topics/physics-configuration-and-solver-profiles.md)
- [Current product capability status](docs/topics/product-capability-status.md)
- [PhysBox 3 and PhysBox 4 feature usage](docs/topics/predecessor-feature-usage.md)
- [Presentation and modular workbench architecture](docs/topics/presentation-and-workbench-architecture.md)
- [Coupled liquid/rigid constraint surface](docs/topics/coupled-liquid-rigid-constraint-surface.md)
- [S31 coupled-liquid sandbox](docs/tactical/031-coupled-liquid-sandbox.md)
- [S33 presentation foundation](docs/tactical/033-presentation-foundation.md)
- [Vision summary](docs/vision.md)
- [Bring-up plan](docs/bringup-plan.md)
- [Platform architecture](docs/platform-architecture.md)
- [Topic index and conventions](docs/topics/README.md)
- [Tactical workstreams and conventions](docs/tactical/README.md)
- [Source provenance](docs/source-provenance.md)
- [Local source references and oracle map](reference/README.md)
- [Interactive Box2D comparison](docs/topics/interactive-box2d-comparison.md)
- [Deployment](docs/deployment.md)

## Reusable local crates

Independent applications can consume the current checkout without importing
the PhysBox sandbox host:

```toml
[dependencies]
physbox-sim = { path = "../physbox5/crates/physbox-sim" }
```

The crate remains unpublished and unversioned beyond the current repository
revision. It exposes a Box2D-shaped definition/handle/world surface with
shape-density mass derivation, an optional exact initial mass-data override,
caller-owned WGPU submission, a canonical same-device GPU physics
observation, and asynchronous snapshots.

It exposes no renderer, rendering surface, target, camera, product scene, or
Box2D comparator. Applications own presentation and may put `physbox-sim`
behind their own capability-aware physics wrapper alongside an independently
imported Box2D backend.

See the [reusable library architecture](docs/topics/reusable-gpu-library-boundary.md),
the [`physbox-sim` contract](crates/physbox-sim/README.md), and the
[external consumer proof](examples/external-consumer/README.md). Run the
bounded proof with:

```sh
cargo run -p physbox-external-consumer
```

## PhysBox 4 lineage

PhysBox 4 is the preserved evidence archive for the split CPU-rigid/GPU-fluid
investigation that motivated this engine. It is available locally at
`../physbox4` and in the private
[PhysBox 4 repository](https://github.com/kzahel/physbox4). PhysBox 5 keeps
fresh Git history and does not inherit PhysBox 4 runtime modules, scenes, or
product compatibility. Selective reuse must cite the PhysBox 4 source revision
and be reshaped behind a PhysBox 5 contract.

## Quick start

On a fresh Linux or macOS host, inspect prerequisites or install the supported
host packages and project toolchains with:

```sh
npm run check:host
npm run setup:host
```

The setup command detects the host OS, installs known native prerequisites,
adds the required Rust targets, checks Android SDK/NDK access, installs npm
dependencies, syncs declared source references, and configures the Git hooks.
When an executable sibling hook exists at
`../dotfiles/scripts/setup-physbox5-host.sh`, the install flow invokes it last
with `PHYSBOX5_REPO_ROOT` and `--install`. Machine identity, privileged policy,
and other private configuration remain owned by that sibling repository. Set
`PHYSBOX5_HOST_SETUP_HOOK` to an alternate executable or to `off` to disable
the conventional hook explicitly.
On Linux, repo launchers recover the active GNOME/systemd graphical-session
environment when invoked from a
remote shell. Android AVD automation stays headless while retaining that
session's hardware GPU. A locked GNOME session is diagnosed but not unlocked
by public repository code; the hardware desktop smoke requires an accessible,
unlocked compositor. A private sibling hook may deliberately perform a
transient current-session unlock without changing the persistent lock policy.
Display-less machines use Xvfb for Android emulator startup; an Xvfb desktop
run is software-path diagnostics rather than hardware validation.

```sh
cargo run -p physbox-desktop
cargo run -p physbox-desktop -- --scenario liquid-sandbox
cargo run -p physbox-headless -- \
  --output /tmp/physbox5.png \
  --report /tmp/physbox5.json \
  --scenario bootstrap
npm run build:web
npm run test:web
```

The browser host is a fullscreen canvas. Run `npm run serve:web` after building
and open the printed local URL.

### Render resolution

Open **MENU → WORLD** and choose **RENDER SCALE** to cycle the shared 100, 75,
and 50 percent presets. The fresh default is 100 percent. Reduced presets
render the world at the reported smaller extent, linearly composite it to the
native window/canvas, and then draw the menu, toolbar, and diagnostics at
native resolution. Physics rate, camera/input semantics, scenario state, and
final output dimensions do not change.

The same setting is available on desktop, browser, Android, and headless:

```sh
cargo run -p physbox-desktop -- --render-scale 50
cargo run -p physbox-headless -- \
  --render-scale 50 \
  --output /tmp/physbox5-50.png \
  --report /tmp/physbox5-50.json
```

For the browser, use `?render-scale=50`; shared Rust owns URL synchronization
and browser preference persistence. `FrameReport.renderScale` publishes the
native and world extents, effective scale, direct/composited path, and
intermediate color allocation. The option is manual rather than adaptive:
measure it on the target device because the native composite can offset the
saved world raster work.

### Interactive solver and capacity inspection

The deployed browser includes the explicit solver-inspection surface so a
candidate can be judged directly on desktop or mobile without editing the URL.
The fresh-browser default is **SOFT COUPLED** at 60 Hz, four substeps, one
biased iteration plus relaxation, and contact 30 Hz/damping 10/static x2.
Other profiles or edited settings activate only after an explicit menu choice
or restoration of that browser's saved configuration.
For the same surface on a local browser, run:

```sh
npm run inspect:web
```

Open **MENU → SOLVERS** on the engine-rendered canvas. The menu names the
active solver and lists every registered profile; choose **SOFT COUPLED** or
**HARD DIRECT**, adjust simulation Hz, substeps, iterations, contact Hz,
damping, and static-contact scale, then use **STACK 8**, **STACK 16**, or
**APPLY**. **RESET PRESET** restores the selected profile's reviewed tuple.
The list grows and reflows as profiles are registered; there is no pairwise
A/B slot. A change rebuilds the shared GPU world immediately without reloading
the page; camera position and tool selection are retained. The browser stores
the active profile and coefficients
under `physbox5.physics.v5`, migrates earlier physics and solver-only keys,
restores the complete configuration on the next launch, and keeps the URL
synchronized automatically for reproducibility. No query-string editing is
required.

Sleeping accepts `--sleeping on|off` (or the
matching web query). **MENU → WORLD → SLEEP REQUEST** provides the same
engine-rendered control and applies it immediately while preserving the rest
of the physics configuration. The browser persists the request and keeps the
URL synchronized. Sleeping is effective and on by default; PhysBox uses exact
current-contact-component grouping by default. **SLEEP GROUPING** still cycles
through exact, local-4, and local-8, and the sleeping request can be switched
off for awake-quality or comparison work. **SLEEP VISUALS** is an independent
presentation-only toggle: teal is active, amber is accumulating quiet time,
blue `Z` is asleep, and pink `!` is recently or forcibly woken. The browser
stores that presentation choice separately under `physbox5.presentation.v1`,
restores it on the next launch, and mirrors an enabled choice into the
`sleep-visuals` query setting.

Open **MENU → WORLD** to choose **64 SLOTS**, **256 SLOTS**, **1K SLOTS**, or
**10K SLOTS**, and to change **SLEEP REQUEST**. A selection rebuilds and
restarts the current scenario immediately while retaining camera and tool
state. The HUD reports live/effective body slots, point capacity, allocated
bytes, and overflow. The browser stores the capacity choice separately under
`physbox5.world.v1` and restores it on the next launch.
The higher tiers are deliberately labeled as potentially slow: 10K proves that
the real product graph can be constructed and executed, not that every device
can simulate or render 10K bodies at 60 Hz. The cross-host default remains 64;
an authored scenario that starts larger receives the minimum power-of-two
headroom needed to load and reports both requested and effective capacity.

### Interactive PhysBox / Box2D comparison

Open **MENU → WORLD → ENGINE** and choose **PHYSBOX** or **BOX2D**. PhysBox is
selected initially. Both engines start from the same configured world and
continue advancing, so switching is immediate and does not copy state.
Placement, painting, erase, grab/throw, gravity, reset, scenario, and
configuration actions are mirrored by stable body identity. Camera, tools,
shared world settings, renderer, and layout stay the same; PhysBox-only solver
controls are hidden while Box2D is visible and reappear unchanged on return.

**SLEEP VISUALS** also works in Box2D mode using Box2D's public authoritative
awake/asleep state. Box2D does not expose PhysBox's quiet-candidate and
internal wake-cause states, so those marks are omitted rather than guessed.
The HUD identifies **ENGINE BOX2D**, the exact pinned revision, step alignment,
live bodies, and any pending, overflow, or identity-disagreement failure.

This is a qualitative validation tool, not a CPU fallback or performance mode.
Comparator-enabled frames intentionally execute both worlds and are excluded
from PhysBox timing and capacity claims. That exclusion does not mean sleeping
is unhelpful: the matched 1,000-body sleeping-city probe measured a 28.8%
end-to-end GPU P50 reduction at 1% awake while solver work followed the awake
set.

The comparator is on by default. A release web package without any Box2D
dependency, asset, menu vocabulary, or runtime work can be built with:

```sh
node scripts/build-web.mjs --release --no-box2d-comparator
npm run test:box2d-feature-off
```

Native maintained crates expose the corresponding default
`box2d-comparator` Cargo feature and support `--no-default-features` for an
explicit comparator-free build. See
[interactive Box2D comparison](docs/topics/interactive-box2d-comparison.md)
for authority, equivalence, platform, and completed qualification evidence.

The same opt-in profile is available to the shared desktop host when direct
launch control is useful:

```sh
cargo run -p physbox-desktop --features solver-lab -- \
  --scenario solver-lab-stack-16 \
  --body-capacity 256 \
  --solver soft-coupled \
  --sim-hz 60 --substeps 4 --iterations 1 \
  --contact-hz 30 --contact-damping 10 --static-contact-scale 8
```

Adding a later candidate means registering another explicit inspection
profile; it joins the same selection list and retains its last configuration
during the session. The menu, launch transport, scene contract, and browser
persistence remain the common surface. Native production builds remain
feature-gated and exclude these candidate profiles unless `solver-lab` is
requested.

## Validation tiers

The everyday lane is budgeted and includes the maintained native tests, two
small performance-smoke probes, tooling tests, platform boundaries, and a real
browser WebGPU smoke:

```sh
npm run test:fast
```

It has a 120-second hard limit and currently finishes in 7.31 seconds on the
primary warm development host. `npm run test:perf:smoke` reruns only the
body-pipeline timing and exact soft-floor work/capacity probes; these already
run once inside `test:fast`.

The Cargo workspace still defaults to desktop, headless, web, Android, engine,
contracts, launch, and shell. Historical lab, native-oracle, benchmark, long
600/1,200-step, and exhaustive configuration matrices are opt-in. Run only the
affected package/target while changing those tools. The complete audit remains
available for scheduled or explicitly requested use:

```sh
npm run test:rust:full
npm run clippy:rust:full
```

## Production deployment

The Cloudflare deployment target is <https://physbox5.kzahel.com>. Install the
tracked exact-pushed-SHA hook with:

```sh
npm run hooks:install
```

After installation, every successful push of `main` schedules a background
deployment of that exact remote SHA. See [Deployment](docs/deployment.md).
