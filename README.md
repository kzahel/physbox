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
valid, and none preserved the 16-stack. The measured hard/direct residual band
is therefore accepted as the state against which broadphase selection and
sleeping design proceed, without reclassifying the awake solver as still. The
scheduler promotion is
an architecture and latency improvement, not acceptance of the current CPU
comparison, residual-motion band, or liquid performance.
The repository still contains no Rapier or CPU product physics backend, fluid
solver, legacy settings UI, or PhysBox 4 scene compatibility.

Read:

- [Authoritative GPU-physics direction](docs/topics/physbox5-unified-gpu-physics.md)
- [Performance and scale contract](docs/topics/performance-and-scale.md)
- [Simulation time and overload contract](docs/topics/simulation-time-and-overload.md)
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
On Linux, repo launchers recover the active GNOME/systemd graphical-session
environment when invoked from a
remote shell. Android AVD automation stays headless while retaining that
session's hardware GPU; the desktop smoke uses Xwayland so it can progress
while the GNOME session is locked. Display-less machines use Xvfb for Android
emulator startup; the hardware desktop smoke still requires an accessible
graphical session.

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

### Interactive solver inspection

Candidate comparison is an explicit laboratory build; candidate profiles stay
out of ordinary production builds and do not alter the selected solver or its
defaults. Start the browser lab with:

```sh
npm run inspect:web
```

Open **MENU → SOLVERS** on the engine-rendered canvas. Choose **SELECTED** or
**BRANCH ROBUST**, adjust the candidate's hertz, damping, and static-contact
scale, then use **STACK 8**, **STACK 16**, or **APPLY**. **SWITCH A/B** swaps
between the active and previous profile. A change rebuilds the shared GPU
world immediately without reloading the page; camera position and tool
selection are retained. The browser stores the active profile and coefficients
under `physbox5.solver-lab.v1`, restores them on the next lab launch, and keeps
the URL synchronized automatically for reproducibility. No query-string
editing is required.

The same opt-in profile is available to the shared desktop host when direct
launch control is useful:

```sh
cargo run -p physbox-desktop --features solver-lab -- \
  --scenario solver-lab-stack-16 \
  --solver branch-robust \
  --solver-options hz=30,damping=10,static-scale=8
```

Adding a later candidate means registering another lab-only solver profile;
the menu, launch transport, scene contract, and browser persistence remain the
common inspection surface.

## Validation tiers

The Cargo workspace defaults to the maintained production surface: desktop,
headless, web, Android, engine, contracts, launch, and shell. Routine Rust
validation therefore avoids historical lab, evidence-generator,
native-oracle, and benchmark targets and does not execute candidate-only
engine tests. Native experiment APIs require the `physbox-engine/solver-lab`
feature, enabled by the opt-in `physbox-lab` and Android GPU benchmark
packages. Cargo automatic binary discovery is disabled in the lab; eight
maintained commands remain registered and closed study sources are retained
without being compile targets:

```sh
npm run test:rust:production
npm run clippy:rust:production
```

Changes to laboratory or oracle tooling should run only the affected package
and target. The explicit full-workspace audit remains available, but is not a
routine pre-push gate:

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
