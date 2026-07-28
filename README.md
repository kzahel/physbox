# PhysBox

An experimental GPU-resident 2D physics engine for high-particle-count water
sandboxes, built with Rust, `wgpu`, and WebGPU.

**[Try the PhysBox Web Showcase](https://kzahel.github.io/physbox/)**

## Why PhysBox?

PhysBox exists to make a water sandbox game: pour and erase large amounts of
water, build containers and machines, and let rigid toys float, tumble, and get
hit by waves. The motivating thesis is that a GPU-native 2D engine can make
that experience work at particle counts beyond CPU-oriented physics engines
while keeping the water and rigid bodies in one coherent world.

The architecture grew out of several working sandbox generations:

- [PhysBox Matter](https://github.com/kzahel/physbox-matter) established the
  browser construction-toy loop with placeable bodies, cars, walls, grabbing,
  erasing, and touch controls.
- [PhysBox 2](https://github.com/kzahel/physbox2) moved the sandbox to
  Planck.js/Box2D and added finite-volume column water, buoyancy, and granular
  particles. Its sand was still ordinary CPU rigid bodies and was capped at
  1,000 grains.
- [PhysBox 3](https://github.com/kzahel/physbox3) moved rigid physics to Box2D
  v3 WASM and replaced column water with a LiquidFun-derived particle sidecar.
  Water and rigid bodies interacted synchronously inside one WASM module, but
  the default fluid ceiling was 4,000 particles and the water remained an
  early CPU-oriented implementation.
- PhysBox 4 moved the particle solver and renderer to WebGPU, with explicit
  stress capacities reaching hundreds of thousands of particles. Its rigid
  world remained on the CPU: collider poses went to the GPU and fluid reaction
  packets came back later. That delay could make floating bodies lag, gain
  energy, or feel disconnected from the water.
- This PhysBox generation puts rigid bodies and water in the same GPU-resident
  fixed-step graph. Equal-and-opposite response happens against current state,
  and rendering consumes the resulting GPU buffers directly.

The current repository is the engine extracted from that game lineage, not the
finished game. `physbox-sim` lets the Web Showcase, Quest, and future sandbox
clients supply their own WGPU lifecycle and rendering while reusing bounded
world construction, fixed-step physics, deferred interaction, and
render-neutral GPU observations. The
[technical direction](docs/topics/physbox5-unified-gpu-physics.md#why-this-is-a-new-engine-direction)
records the detailed PhysBox 3 → 4 → current architecture decision.

The project is experimental. The higher-than-CPU particle-count thesis still
needs controlled comparative evidence; it is not a benchmark claim. The
public API is not yet stable, water remains preview quality, and no crate has
been published to crates.io.

## What works

- GPU-resident circles and boxes with density-derived mass, contact response,
  friction, restitution, sleeping/waking, and scalable collision candidate
  generation.
- Deferred GPU-native point queries, point constraints, body destruction, and
  asynchronous command results without a per-frame CPU body mirror.
- Preview GPU water with same-world rigid coupling, runtime emission,
  asynchronous counts, and a canonical GPU particle observation.
- The same shared engine behind desktop, browser, headless, and Android
  adapters.
- A standalone Web Showcase and a separate Quest application that consume
  `physbox-sim` like an external library.
- An optional, compile-removable Box2D shadow for human rigid-body A/B review.
  It is never a fallback, simulation authority, or PhysBox performance lane.

PhysBox deliberately has no CPU product backend or automatic fallback.
Joints, multiple fluid materials, smooth water meshing, comprehensive Box2D
parity, and a stable versioned package are not current capabilities. See the
[capability ledger](docs/topics/product-capability-status.md) for the detailed
status and limitations.

## Try the Web Showcase

The standalone showcase is the simplest capability tour. It has compact HTML
scene/tool controls while simulation and rendering use the public
`physbox-sim` surface.

**[Open the Web Showcase](https://kzahel.github.io/physbox/)**

```sh
npm run setup:host
npm run build:showcase
npm run serve:showcase
```

The server listens on `0.0.0.0:8080` by default. Open
`http://127.0.0.1:8080/` on the same machine.

WebGPU requires a secure context for remote browsers. A raw LAN or Tailscale
IP over HTTP can load the page but generally cannot expose WebGPU; put the
listener behind HTTPS when testing from another device. The showcase does not
require `SharedArrayBuffer`, cross-origin isolation, or COOP/COEP headers.

## Advanced workbench

**[Open the advanced WebGPU workbench](https://physbox5.kzahel.com/)**

On a supported Linux or macOS development host:

```sh
npm run check:host
npm run setup:host
cargo run -p physbox-desktop
```

Other useful entry points:

```sh
# Headless GPU run with an image and structured report
cargo run -p physbox-headless -- \
  --scenario bootstrap \
  --output /tmp/physbox.png \
  --report /tmp/physbox.json

# Fullscreen browser workbench
npm run build:web
npm run serve:web
```

The workbench is the advanced first-party integration and validation surface.
It includes engine-owned tools, configuration, scenarios, diagnostics, and the
optional Box2D comparison. The Web Showcase is the smaller canonical public
demo and library consumer, not another implementation of the workbench.

## Use the library

`physbox-sim` is the reusable physics-only crate. While it remains unpublished,
consume a checked-out revision by path:

```toml
[dependencies]
physbox-sim = { path = "../physbox/crates/physbox-sim" }
```

The caller owns WGPU lifecycle and presentation. The library exposes
definitions and stable handles for authored worlds, explicit fixed-step
encoding, deferred interaction commands, asynchronous compact results, and
same-device GPU observations. It does not create a window or surface and does
not provide a renderer, camera, application scene catalog, or Box2D backend.

Start with:

- [`physbox-sim` API guide](crates/physbox-sim/README.md)
- [minimal external consumer](examples/external-consumer/README.md)
- [reusable library boundary](docs/topics/reusable-gpu-library-boundary.md)
- [Web Showcase architecture](docs/topics/web-showcase-and-public-water-consumption.md)

Run the surface-free consumer proof with:

```sh
cargo run -p physbox-external-consumer
```

## Architecture

PhysBox has one authoritative GPU simulation world. Interactive callers encode
zero or one fixed outer step per presented frame; overload permanently slips
wall time instead of running multi-step catch-up. Solver substeps are part of
that one outer step. Compact diagnostics arrive asynchronously, and no GPU
result is required on the CPU to advance the next interactive frame.

Desktop, fullscreen browser, headless, and Android applications are adapters
around one shared host. The standalone Web Showcase and sibling Quest
application instead prove the narrower public library boundary with their own
presentation and input policy.

The durable technical records are:

- [GPU physics direction](docs/topics/physbox5-unified-gpu-physics.md)
- [performance and scale contract](docs/topics/performance-and-scale.md)
- [simulation time and overload](docs/topics/simulation-time-and-overload.md)
- [coupled liquid/rigid surface](docs/topics/coupled-liquid-rigid-constraint-surface.md)
- [platform architecture](docs/platform-architecture.md)
- [topic index](docs/topics/README.md)
- [tactical workstreams](docs/tactical/README.md)

Historical `PhysBox 5` and `PHYSBOX5_*` names remain in validation markers,
evidence, compatibility paths, and technical history. The public product and
repository name is PhysBox.

## Validation

The maintained development gate formats the workspace, runs production Rust
tests and bounded performance smokes, checks platform boundaries, builds WASM,
and exercises a real Browser WebGPU path:

```sh
npm run test:fast
npm run clippy:rust:production
```

Focused showcase validation is:

```sh
npm run test:showcase
```

Longer oracle, benchmark, Android, and physical-device gates are documented in
the relevant tactical records. Their artifacts and live numbers should not be
treated as portable benchmarks without the complete scene, hardware, build,
capacity, timing-scope, and provenance contract.

## License

PhysBox source owned by this repository is available under the
[MIT License](LICENSE). Dependencies and incorporated third-party source retain
their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md), including
the preserved Box2D comparator license and source manifest.
