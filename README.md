# PhysBox 5

PhysBox 5 is a fresh-history, portable WebGPU experiment toward one
GPU-resident 2D physics world. It begins deliberately small: a shared Rust,
`wgpu`, and WGSL host running unchanged behind desktop, browser, headless, and
Android adapters.

The shared-host bootstrap and oracle-driven rigid solver laboratory are
complete. Human review selected contact-centric Jacobi with GPU per-body gather
as the initial rigid foundation. P5.1 minimal rigid state and fixed-floor
contact is complete. P5.2 now has selected stable generational pair slots and
stable body-edge views, a qualified combined product graph, shared camera/menu/
grab tools, and bounded GPU circle/rectangle construction. The clean S5.4
awake bakeoff finds that none of nine reversible executions preserves the
8-body rectangle stack even though awake Box2D does; no new solver default or
quality band has been accepted. Human review authorized a laboratory-only
mass-splitting/manifold-block factorial follow-up, with colored/in-place work
conditional on its measured outcome. P5.3 broadphase remains blocked.
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

## Production deployment

The Cloudflare deployment target is <https://physbox5.kzahel.com>. Install the
tracked exact-pushed-SHA hook with:

```sh
npm run hooks:install
```

After installation, every successful push of `main` schedules a background
deployment of that exact remote SHA. See [Deployment](docs/deployment.md).
