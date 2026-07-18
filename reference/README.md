# Local references and oracle map

This directory holds reproducible source checkouts used to understand,
compare, and diagnose physics behavior. Checkout contents are gitignored; this
README and `pins.toml` are the only tracked files. None of these repositories
is a PhysBox 5 product dependency.

Run:

```sh
npm run references:sync
npm run references:status
```

`references:sync` clones missing managed checkouts, checks out their exact
commits in detached-HEAD state, and refuses to overwrite local changes.
`references:status` is read-only and fails for a missing, dirty, incorrectly
pinned, or wrong-origin managed checkout. It also validates the local PhysBox
3 sibling without modifying it. The currently known PhysBox 3 worker changes
are reported as a warning.

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

- [PhysBox 3 product tree](../../physbox3) at
  `4169eba477201fe8220b49d95473555c4ac22995`;
- [LiquidFun source checkout](../../physbox3/reference/liquidfun) at
  `7f20402173fd143a3988c921bc384459c6a858f2`;
- [PhysBox 3's Box2D v3 source](../../physbox3/vendor/box2d3-wasm/box2d) at
  `3a4f0da8374af61293a03021c9a0b3ebcfe67948`;
- [Box2D WASM bridge and build](../../physbox3/vendor/box2d3-wasm) at
  `9288709b4c04e0c7a1a1d325b74b6e114cbc3750`.

Use these sources for different questions:

- The clean managed Box2D checkout owns rigid acceptance comparisons.
- PhysBox 3's particle sidecar compiled natively with its Box2D v3 source is
  the intended routine coupled-behavior oracle.
- Original LiquidFun supplies later particle/body algorithms, tests, and an
  independent native behavior reference; it is not an exact substitute for
  PhysBox 3's reduced sidecar.
- The running PhysBox 3 web product supplies a small calibration corpus and
  subjective review, not the routine unattended correctness lane.

The PhysBox 3 tree currently has pre-existing modifications in
`worker/index.js` and `worker/wrangler.toml`. A run against it must record the
root revision, nested revisions, build/settings, and a digest of relevant
dirty patches. A commit hash alone is not reproducible oracle provenance.

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

Keep Box2D v3, original LiquidFun's Box2D 2.3, and any optional engine
comparator in separate processes. Foreign engine code never links into the
PhysBox product engine. A checked calibration trace may be a reviewed baseline,
but it is not proof of the current PhysBox 3 web build; claims about current
web behavior require a fresh capture.

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
- The actual reduced coupled sidecar is under
  `../physbox3/vendor/box2d3-wasm/box2d3-wasm/csrc/particle_sidecar/`; the
  Emscripten bindings are a separate concern in `csrc/glue.cpp`.

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

`pins.toml` is the machine-readable authority for revisions and locations.
Change a pin only through a reviewed oracle/reference decision, not as an
incidental refresh.
