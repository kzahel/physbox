# Local references and oracle map

This directory holds reproducible source checkouts used to understand,
compare, and diagnose physics behavior. Checkout contents are gitignored; this
README and `pins.toml` are the only tracked files in this directory. None of
these checkouts is a PhysBox 5 product dependency. The separately governed
interactive Box2D comparator may package only a reviewed, enumerated
redistributable subset outside this directory, with the source revision,
manifest, build recipe or artifact digest, and license preserved.

Run:

```sh
npm run references:sync
npm run references:status
```

`references:sync` clones missing declared checkouts, checks external sources
out at exact detached revisions, fast-forwards clean first-party `main`
branches, initializes declared submodules, and refuses to overwrite unexpected
local changes. `references:status` is read-only and fails for a missing,
dirty, incorrectly pinned/tracked, or wrong-origin checkout. Branch status is
relative to the locally fetched `origin/main`; run sync first when current tip
matters.

## Managed clean checkouts

| Local path | Pin | Role |
| --- | --- | --- |
| `reference/box2d` | `3a4f0da8374af61293a03021c9a0b3ebcfe67948` | Unmodified rigid authority and public narrowphase source |
| `reference/box3d` | `8441b4a06d6d09dcfb0b0f704df4d847d1437b92` (`v0.1.0`) | Soft Step, contact persistence, graph ordering, and stable-pile source reference |
| `reference/physx` | `b4b286abff6f2b3debd1d1acb120dc428765cf2e` | GPU-resident organization, batching, capacity, and diagnostics reference |
| `reference/wgmath` | `ac89dceb8a33778e895303cd606da38afb015192` | Portable WebGPU broadphase, coloring, and solver research reference |

Box3D is a source reference, not a PhysBox rigid authority or product
dependency. Its three-dimensional manifolds, friction model, and CPU execution
order may expose useful mechanisms, but they do not overrule the scalar
invariants or pinned Box2D v3 behavior lane and are not assumed to transfer to
the portable GPU schedule unchanged.

Keep the Box2D checkout clean: it is the source for the unmodified acceptance
lane. If internal instrumentation becomes necessary, create a separate clone
or worktree and record the base revision plus a stable patch digest. Patched
microscope output is diagnostic evidence and never the pass/fail authority.

## PhysBox 3 behavioral oracle

PhysBox 3 deliberately remains a sibling repository rather than being copied
here. These workspace-local links point to the implementation that produced
the behavior we want to characterize:

- [PhysBox 3 product tree](../../physbox3), tracking first-party `main`;
- [PhysBox 3 particle-sidecar source](physbox3-particle-sidecar) at
  `ef8d9526ae79377da3d610e5e4d3bd227f072cdb`; the particle files were
  introduced by `3884106dd415802d2139d163fca56e65b98645c6` and are unchanged
  at the pinned branch tip;
- [LiquidFun source checkout](../../physbox3/reference/liquidfun) at
  `7f20402173fd143a3988c921bc384459c6a858f2`;
- [PhysBox 3's Box2D v3 source](../../physbox3/vendor/box2d3-wasm/box2d) at
  `3a4f0da8374af61293a03021c9a0b3ebcfe67948`;
- [Box2D WASM bridge and build](../../physbox3/vendor/box2d3-wasm), tracking
  first-party `main` with its checked submodules.

Use these sources for different questions:

- The clean managed Box2D checkout owns rigid acceptance comparisons.
- The exact managed PhysBox 3 particle-sidecar checkout compiled natively with
  PhysBox 3's pinned Box2D v3 source is the routine coupled-behavior oracle.
- Original LiquidFun supplies later particle/body algorithms, tests, and an
  independent native behavior reference; it is not an exact substitute for
  PhysBox 3's reduced sidecar.
- The running PhysBox 3 web product supplies a small calibration corpus and
  subjective review, not the routine unattended correctness lane.

Every run against a first-party tracked checkout must record the resolved root
and nested revisions, build/settings, and any relevant dirty-patch digest.
`main` is a source acquisition policy, not sufficient artifact provenance by
itself.

## Native oracle execution

Routine comparison should not drive PhysBox 3 through a browser. Each CPU
engine runs in its own test-only native process:

1. The shared Rust runner emits one versioned canonical JSON scenario
   projection.
2. The adapter rejects unsupported shapes or actions instead of approximating
   them.
3. The adapter emits the canonical result/trace schema with complete source,
   build, settings, and dirty/patch provenance.
4. The shared runner compares that result with a fresh GPU/headless result and
   owns the structured diff and synchronized playback.

Prefer a thin Rust CLI per adapter so request/result serialization comes from
the shared schema crate. Box2D v3 can use direct C FFI; original LiquidFun and
the PhysBox 3 C++ sidecar should sit behind narrow C shims. The product engine
must not depend on these CLIs. The runner owns pin checks, timeout, fresh atomic
result enforcement, and the `PHYSBOX5_ORACLE_SUCCESS` sentinel. Hashed JSON
payloads use stable field and identifier ordering, finite numbers only, and no
unordered object maps. Preserve the source scenario hash and record a separate
projection hash over the exact request bytes.

Keep Box2D v3, original LiquidFun's Box2D 2.3, and optional authority
comparators in separate processes. The default-enabled, compile-removable
[interactive Box2D comparison](../docs/topics/interactive-box2d-comparison.md)
is the sole in-process exception and remains a non-authoritative review
shadow. A checked calibration trace may be a reviewed baseline, but it is not
proof of the current PhysBox 3 web build; claims about current web behavior
require a fresh capture.

The optimized rigid performance process under `tools/benchmarks/box2d` is
deliberately separate from the authority executable. It rebuilds this same
clean pin in Release mode with validation disabled and host SIMD enabled, then
uses a PhysBox-owned fixed worker pool for named one- and multi-worker lanes.
It consumes the shared `ScenarioV1` projection, publishes a fresh atomic result
plus its own success sentinel, and labels its measurement as a complete Box2D
world step. It does not change authority settings or turn Box2D into a product
dependency.

The pinned sources were tested for native build feasibility on 2026-07-14:

- `reference/box2d` built successfully as a native static library;
- original LiquidFun built its Box2D 2.3 plus particle solver static library;
- PhysBox 3's `csrc/particle_sidecar/particle_bridge.cpp` and
  `particle_system.cpp` compiled as native C++20 objects against its pinned
  Box2D v3 and box2cpp headers without Emscripten includes.

These checks did not create an oracle adapter or behavioral baseline. They
show that browser automation is not a prerequisite for the routine lanes.

## Useful reading entry points

- Box2D rigid behavior: `reference/box2d/src/contact_solver.c`,
  `reference/box2d/src/solver.c`, `reference/box2d/src/manifold.c`, and
  `reference/box2d/include/box2d/collision.h`.
- Box3D stable-pile organization: `reference/box3d/src/contact_solver.c`,
  `reference/box3d/src/solver.c`, `reference/box3d/src/constraint_graph.c`,
  `reference/box3d/src/convex_manifold.c`, and
  `reference/box3d/shared/benchmarks.c`.
- Box2D performance evidence: `reference/box2d/benchmark/main.c` owns the
  timing method, `reference/box2d/shared/benchmarks.c` owns its scenes, and
  the named hardware result directories under `reference/box2d/benchmark/`
  contain checked CSVs. Treat those as reference evidence and rerun current
  comparisons through the PhysBox
  [performance and scale contract](../docs/topics/performance-and-scale.md).
- wgmath portable GPU patterns:
  `reference/wgmath/crates/wgrapier/src/dynamics/solver.wgsl`,
  `coloring.wgsl`, and `warmstart.wgsl`, plus
  `reference/wgmath/crates/wgparry/src/broad_phase/`.
- PhysX organization: `reference/physx/physx/source/gpusolver/` and
  `reference/physx/physx/source/gpusimulationcontroller/`.
- PhysBox 3's [particle-system MVP plan](../../physbox3/docs/particle-system-mvp-plan.md)
  maps its LiquidFun algorithms, tests, Box2D bridge, and implemented coupled
  step; its [Box2D WASM reference](../../physbox3/docs/box2d3-wasm-reference.md)
  explains the exact binding and worker configuration.
- LiquidFun's core particle solver lives at
  `../physbox3/reference/liquidfun/liquidfun/Box2D/Box2D/Particle/`; its unit
  fixtures and visual scenes live under `Box2D/Unittests/` and
  `Box2D/Testbed/Tests/` in the same checkout.
- `tools/oracles/liquidfun` builds that exact clean checkout into a
  process-isolated secondary oracle. It shares the PhysBox-owned neutral
  projection/result runner with `tools/oracles/physbox3-particles`, but never
  links LiquidFun 2.3 and the PhysBox 3 Box2D v3 sidecar into one executable.
- The actual reduced coupled sidecar is under
  `reference/physbox3-particle-sidecar/box2d3-wasm/csrc/particle_sidecar/`;
  the Emscripten bindings are a separate concern in `csrc/glue.cpp`.

## Source-use rules

- Read each checkout's license before copying anything. Record copied source,
  origin revision, and license explicitly; prefer independent implementation
  from the documented algorithmic contract.
- Do not add reference repositories, build outputs, generated traces, or
  captures to PhysBox 5 Git history.
- Do not add a reference repository to Cargo, npm, web, or Android product
  dependency graphs.
- Record exact revisions and build/runtime settings in every oracle result.
- Run status again after changing or updating any sibling repository.

`pins.toml` is the machine-readable authority for exact external revisions,
first-party tracking branches, and locations. Change either policy only
through a reviewed oracle/reference decision, not as an incidental refresh.
