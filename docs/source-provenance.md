# Source provenance

PhysBox 5 was initialized as a new Git repository on 2026-07-14. No `.git`
history, generated media, credentials, or untracked reference checkout was
copied from PhysBox 4.

The bootstrap design/archive source was PhysBox 4 revision
`b4700c99201e09ba6058a82414e62afbb2d19119`, with then-uncommitted handoff
changes in `docs/topics/physbox5-unified-gpu-physics.md`,
`docs/tactical/README.md`, and
`docs/tactical/019-physbox5-fresh-repository-bootstrap.md`. PhysBox 4 later
preserved that handoff as commit
`4295957e15480ef701e6006c7cafb5f0728ef101`.

PhysBox 5 subsequently imported the authoritative
`docs/topics/physbox5-unified-gpu-physics.md` text from that handoff commit and
rewrote its archival local links to pinned or PhysBox 4 web references. No
PhysBox 4 runtime code accompanied it. The PhysBox 4 tactical numbering and
completed bootstrap tactical remain archive history; PhysBox 5 begins its own
tactical sequence at `001`.

The machine-readable reference set is [reference/pins.toml](../reference/pins.toml),
and [the reference map](../reference/README.md) describes each source's oracle
role and exact workspace path. It records independent Box2D, PhysX, and wgmath
checkouts plus the PhysBox 3 sibling, its LiquidFun checkout, its Box2D v3
source, and its first-party WASM bridge checkout.

At bootstrap the PhysBox 3 sibling was at
`4169eba477201fe8220b49d95473555c4ac22995` with pre-existing changes in
`worker/index.js` and `worker/wrangler.toml`; the root web product is a
calibration reference, not a clean acceptance artifact. Its nested clean
sidecar repository supplies the later native coupled-behavior source. Its
nested Box2D source was the same
`3a4f0da8374af61293a03021c9a0b3ebcfe67948` revision selected for the
independent unmodified authority checkout. Its LiquidFun source was pinned at
`7f20402173fd143a3988c921bc384459c6a858f2`.

The reviewed first-machine Linux bring-up changed source acquisition for
first-party `kzahel` references: PhysBox 3 and `box2d3-wasm` now track clean
`main` tips, while their resolved revisions remain mandatory artifact
provenance. External Box2D, LiquidFun, PhysX, and wgmath inputs remain exact
pins.

Run `npm run references:status` before relying on local source and include its
result in oracle provenance. References are not product dependencies. Any
future copied source must be identified explicitly and preserve its license;
ordinary work should independently implement the documented algorithmic
contract.

Native-oracle feasibility was checked on 2026-07-14 on arm64 macOS with CMake
4.2.3 and AppleClang 21. The pinned Box2D v3 source built as a native static
library. The pinned original LiquidFun source built its Box2D 2.3 plus particle
solver static library with `CMAKE_POLICY_VERSION_MINIMUM=3.5`, with compiler
warnings but no errors. PhysBox 3's `particle_bridge.cpp` and
`particle_system.cpp` sidecar sources compiled independently as native C++20
objects against its pinned Box2D v3 and box2cpp headers; no Emscripten header
was needed for those objects. This established build feasibility only: no
oracle adapter or acceptance result was created, and no reference source was
copied into PhysBox 5.

The authorized S4 interactive-workbench plan uses PhysBox 4 handoff revision
`4295957e15480ef701e6006c7cafb5f0728ef101` as a selective behavior and source
reference for `camera.rs`, `input/navigation.rs`, body-local grab selection,
`toolbar.rs`, and the `options/font.rs` plus instanced-glyph renderer pattern.
Each implementation commit that copies or closely ports code must identify its
exact source file and preserve any applicable license. Rapier state, PhysBox 4
scene compatibility, fluid/settings tools, and its asynchronous coupling are
excluded. At this planning commit no PhysBox 4 runtime source has been copied.

The active
[S31 coupled-liquid sandbox](tactical/031-coupled-liquid-sandbox.md) inspected
the clean PhysBox 4 revision
`92f745eb85908243c69080a4fa4dba24f05c22cf` as selective source and behavioral
evidence. Candidate concepts are limited to its GPU particle slot lifecycle,
water PBF/XPBD passes, fixed-grid diagnostics, circle/rectangle boundary math,
water/erase/surface interactions, direct particle rendering, debug views, and
shared-renderer styling ideas. Its Rapier runtime, CPU rigid step, collider
snapshot upload, asynchronous reaction readback, delayed host impulses,
catch-up scheduler, stabilizing clamps, scene compatibility, and platform
semantics are explicitly excluded. No PhysBox 4 runtime source has been copied
for S31 at the planning boundary; any later close port must cite its exact
source file, this revision, and the applicable license.

S31.1 additionally found that the first-party particle sidecar is not present
on the tracked `box2d3-wasm` `main` revision. It lives on the repository's
`particle-system` branch. The managed
`reference/physbox3-particle-sidecar` checkout therefore pins that branch at
`ef8d9526ae79377da3d610e5e4d3bd227f072cdb`; its sidecar directory is byte-
unchanged from introducing commit
`3884106dd415802d2139d163fca56e65b98645c6`. The process-isolated native
adapter compiles those files in place with exact submodules Box2D
`3a4f0da8374af61293a03021c9a0b3ebcfe67948` and box2cpp
`45f07785abe6bfeba518fb0256a37dcd050dce32`. No sidecar source is copied into
PhysBox 5 and the product engine does not depend on the oracle crate.
