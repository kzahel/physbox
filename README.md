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
The repository still contains no Rapier or CPU product physics backend, fluid
solver, legacy settings UI, or PhysBox 4 scene compatibility.

Read:

- [Authoritative GPU-physics direction](docs/topics/physbox5-unified-gpu-physics.md)
- [Performance and scale contract](docs/topics/performance-and-scale.md)
- [Simulation time and overload contract](docs/topics/simulation-time-and-overload.md)
- [Physics configuration and solver profiles](docs/topics/physics-configuration-and-solver-profiles.md)
- [Current product capability status](docs/topics/product-capability-status.md)
- [Vision summary](docs/vision.md)
- [Bring-up plan](docs/bringup-plan.md)
- [Platform architecture](docs/platform-architecture.md)
- [Topic index and conventions](docs/topics/README.md)
- [Tactical workstreams and conventions](docs/tactical/README.md)
- [Source provenance](docs/source-provenance.md)
- [Local source references and oracle map](reference/README.md)
- [Deployment](docs/deployment.md)

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
cargo run -p physbox-headless -- \
  --output /tmp/physbox5.png \
  --report /tmp/physbox5.json \
  --scenario bootstrap
npm run build:web
npm run test:web
```

The browser host is a fullscreen canvas. Run `npm run serve:web` after building
and open the printed local URL.

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
under `physbox5.physics.v2`, migrates the older solver-only key, restores the
complete configuration on the next launch, and keeps the URL synchronized
automatically for reproducibility. No query-string editing is required.

Open **MENU → WORLD** to choose **64 SLOTS**, **256 SLOTS**, **1K SLOTS**, or
**10K SLOTS**. Selection rebuilds and restarts the current scenario immediately
while retaining camera and tool state. The HUD reports live/effective body
slots, point capacity, allocated bytes, and overflow. The browser stores this
choice separately under `physbox5.world.v1` and restores it on the next launch.
The higher tiers are deliberately labeled as potentially slow: 10K proves that
the real product graph can be constructed and executed, not that every device
can simulate or render 10K bodies at 60 Hz. The cross-host default remains 64;
an authored scenario that starts larger receives the minimum power-of-two
headroom needed to load and reports both requested and effective capacity.

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
