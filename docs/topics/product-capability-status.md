# Product capability status

Status: current implementation ledger, 2026-07-23. This topic is the compact
answer to “what does PhysBox 5 actually support now?” It records product
capability rather than historical research completion. Update it in the same
change that materially changes a feature status or product claim.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Current awake acceptance:
[rigid quality targets](rigid-quality-targets.md).
Execution order:
[bring-up plan](../bringup-plan.md). Validation scope:
[build, validation, and evidence health](build-validation-and-evidence-health.md).

## Status vocabulary

- **Implemented**: present in the ordinary shared product path and maintained
  across its applicable hosts.
- **Limited**: useful product-path foundation exists, but scale, behavior, API,
  or acceptance remains intentionally incomplete.
- **Not implemented**: no product implementation exists; a laboratory or
  source reference does not count.
- **Deferred**: deliberately ordered after an earlier blocking capability.

“Implemented” is not a claim of Box2D feature parity, unlimited scale, or final
quality. Known limitations remain part of the row.

## Host, state, and time

| ID | Capability | Status | Current boundary / next gate |
| --- | --- | --- | --- |
| HOST-1 | One shared engine/frame path | **Implemented** | Desktop, browser, headless, and Android adapt `EngineHost::render_frame`; adapters do not own physics. |
| HOST-2 | GPU-resident product state | **Implemented** | Interactive advancement requires no simulation-dependent CPU readback. Compact diagnostics may be asynchronous. |
| HOST-3 | Portable WebGPU execution | **Implemented** | Metal, browser WebGPU, and Android Vulkan are maintained; this does not imply identical floating-point bits across adapters. |
| TIME-1 | Fixed simulation rate | **Implemented** | Bounded 15-240 Hz configuration; 60 Hz is the reviewed default. Variable `dt` is not supported. |
| TIME-2 | Overload policy | **Implemented** | Zero or one outer step per interactive frame; excess whole-step wall time slips permanently instead of catching up. |
| TIME-3 | Solver resolution | **Implemented** | One to eight substeps and one to four biased iterations, with requested/effective rates reported. |
| DIAG-1 | Capacity and pacing diagnostics | **Implemented** | Scene contract exposes requested/effective body and contact capacity, configuration, work, allocation, overflow, effective scale, and slip. |
| DIAG-2 | General profiling/event stream | **Limited** | Detailed laboratory traces exist; compact product diagnostics and durable performance history remain incomplete. |

## Rigid bodies, collision, and constraints

| ID | Capability | Status | Current boundary / next gate |
| --- | --- | --- | --- |
| BODY-1 | Fixed and dynamic bodies | **Implemented** | GPU-owned generational slots support construction, erase, reset, reuse, and a deterministic active-slot set for sparse execution. General kinematic bodies are not exposed. |
| BODY-2 | Circles and oriented rectangles | **Implemented** | Analytic circle/circle, circle/rectangle, and clipped rectangle/rectangle contact are the complete product shape set. |
| BODY-3 | Capsules, convex polygons, chains, terrain, compounds | **Deferred** | Follow minimal rigid acceptance and a selected scalable broadphase. |
| COLL-1 | Broadphase candidate generation | **Implemented** | Full-sort GPU SAP owns the ordinary product path under an 8N candidate bound and is qualified on Metal, browser, and physical Pixel Vulkan. Brute force remains an isolated bounded exact oracle with no automatic fallback. |
| COLL-2 | Contact persistence and warm start | **Implemented** | Stable generational pair slots and body-edge views retain impulses; committed current/previous work drives matching and retirement while deterministic high-water plus retired-ring allocation fails closed on capacity overflow. |
| COLL-3 | Friction and restitution | **Implemented** | Coulomb friction and a distinct restitution stage are exercised by shared fixtures; broader parity remains P5.4 work. |
| COLL-4 | Speculative contacts and overlap recovery | **Implemented** | Bounded speculative distance and depenetration velocity are configurable; deep/tall overlap remains a known limitation. |
| COLL-5 | Continuous collision detection / TOI | **Not implemented** | Speculative contact is not a complete bullet or time-of-impact system. |
| COLL-6 | Collision filtering and sensors | **Not implemented** | No product category/mask/group filtering or trigger/sensor contract exists. |
| COLL-7 | Contact/query events | **Not implemented** | No general begin/end/hit event stream, AABB query, ray cast, or shape cast API exists. |
| COLL-8 | GPU picking | **Limited** | Shared grab/erase selection uses bounded GPU hit selection; it is not a general spatial-query API. |
| SOLVE-1 | Soft-coupled rigid solver | **Implemented** | Human review retains the lower-cost 60/4/i1, contact 30 Hz/damping 10/static x2 default. Its conservative claim is an 8-box awake quality stack with demonstrated 14-box structural survival: 12/14 exceed the quiet band, 15 is unmeasured in the final run, and 16 loses structure at step 463. Qualified 60/8/i1 remains custom evidence, not a preset. |
| SOLVE-2 | Hard/direct comparison | **Implemented** | Explicit named comparator only; never an automatic fallback. |
| SOLVE-3 | Sleeping and waking | **Implemented** | S23 and S30 qualify opt-in GPU sleeping with versioned generation-safe state, configurable shared PhysBox/Box2D threshold, PhysBox local-4/local-8/exact GPU grouping, integer outer-step admission, awake-driven solve/integration, live cycle/cut/awake-time diagnostics, and persisted engine-rendered controls. Human review retains sleeping off and local-4 as compatibility defaults; exact removes the bounded-grouping failure in the governed 100-body pile but remains explicit. Sleeping may not replace the accepted awake limitation. |
| JOINT-1 | Grab constraint | **Implemented** | Anchored GPU grab retains collision and bounded release velocity. |
| JOINT-2 | General joints | **Not implemented** | Distance/revolute or another first authored joint follows accepted free-body rigid behavior. |

## World interaction and authored content

| ID | Capability | Status | Current boundary / next gate |
| --- | --- | --- | --- |
| TOOL-1 | Camera and direct manipulation | **Implemented** | Shared pan, zoom, recenter, grab, touch policy, and visible tether. |
| TOOL-2 | Circle/rectangle place and erase | **Implemented** | Fixed-size placement and erase exercise the real GPU construction path. A quick tap places once; a 0.25-second hold paints circles or rectangles at a bounded 10 Hz cadence while following the pointer. |
| TOOL-3 | Solver/configuration inspection | **Implemented** | Engine-rendered responsive menu selects any registered solver profile, bounded physics settings, and 64/256/1K/10K world-capacity presets. Browser choices persist locally. |
| TOOL-4 | Selection and property editing | **Not implemented** | No persistent user selection, transform/material editor, or deliberate drag-to-size authoring. |
| TOOL-5 | Scene document and save/load | **Not implemented** | Validation scenarios are reproducible contracts, not an authored product document format. |
| TOOL-6 | Material and broader sandbox tools | **Limited** | Shared Water, particle-aware Erase, and drag-preview-release fixed Surface tools are implemented with engine-rendered state on every host. Undo/redo, property editing, scene documents, and the complete PhysBox 4 tool catalog remain deferred. |

## Scale, quality, and comparison

| ID | Capability | Status | Current boundary / next gate |
| --- | --- | --- | --- |
| SCALE-1 | Small-world rigid scenes | **Implemented** | The safe 64-slot default, authored-scene headroom, and explicit 256/1K/10K opt-ins use selected SAP and reserve-independent active work; the 100-body diagnostic buckets execute correctly but are not sustained-rate claims on every device. |
| SCALE-2 | Scalable broadphase | **Implemented** | SAP is selected and integrated; exact native 100/1K/10K curves, a zero-slip 100-body browser row, and physical Pixel bootstrap/bucket execution pass. This does not establish a 1K-plus rendered product-world rate. |
| SCALE-3 | 1K-100K product-world claims | **Not implemented** | Existing authored-contact/isolated-solver measurements are not scalable end-to-end body claims. |
| SCALE-4 | Mostly-sleeping scale | **Limited** | Valid named-adapter 1%/10%/100% awake curves show solver work and GPU time falling with awake bodies/points, and reserve probes remain bounded through 10K capacity. Live pair processing is retained; no rendered 10K/100K, 60 Hz, or cross-adapter product-rate claim exists. |
| QUAL-1 | Box2D process oracle | **Implemented** | Clean pinned native process covers current circle/rectangle projections, public manifolds, selected actions, and deterministic results. |
| QUAL-2 | Comprehensive rigid acceptance | **Limited** | The ordinary 60/4/i1 default passes the 2/4/8 awake foundation but misses 12 quietness; human review accepts that gap for now and declines the qualified costlier 60/8/i1 alternative. Product wording is 8-box awake quality plus demonstrated 14-box structural survival, not comprehensive parity. The 12 target remains future work. |
| QUAL-3 | Correctness-qualified CPU/GPU crossover | **Not implemented** | Requires broadphase, sleeping, equivalent-quality worlds, and scoped solver/collision/end-to-end/full-frame curves. |
| QUAL-4 | Interactive Box2D A/B comparison | **Implemented** | Default-on, compile-removable shared-host shadow switches the existing renderer between independently evolving PhysBox and pinned Box2D rigid worlds and mirrors rigid tools by stable ID. Desktop, headless, browser WebGPU, ARM64 build, APK, authority equivalence, feature-off, and explicitly authorized hardware-backed AVD substitution gates pass. Comparator-enabled runs are qualitative review and never performance or fallback evidence. S31 keeps this lane rigid-only: PhysBox fluid is hidden and never exchanges mechanics with Box2D. |
| TEST-1 | Everyday fast lane | **Implemented** | `npm run test:fast` runs format, maintained Rust plus performance smoke, tool tests, boundaries, and browser WebGPU under a 120-second hard budget. |
| TEST-2 | Exhaustive research/audit lane | **Implemented** | Historical lab/oracle matrices remain opt-in; they are not ordinary pre-push gates. |

## Coupled material and later breadth

| ID | Capability | Status | Current boundary / next gate |
| --- | --- | --- | --- |
| FLUID-1 | GPU liquid particles | **Implemented** | The shared product path owns one water material in bounded 32K-slot GPU storage, a 3,952-particle sandbox, compact cell ranges, density/correction/velocity passes, deterministic reset/spawn/erase/reuse, fixed boundaries, direct rendering, and final-only diagnostics. Clean Browser WebGPU and authorized hardware-backed AVD mechanics/pixel lanes pass. This is not a 32K-at-60 or multi-material claim. |
| FLUID-2 | Two-way rigid/liquid coupling | **Limited** | The PhysBox GPU path exchanges bounded equal-and-opposite boundary-density/pressure, non-penetration, and surface-relative velocity response with dynamic circles/rectangles in the same outer step, including torque, stable tiled body gathering, generation checks, GPU fluid-wake requests, sleeping-off/local-4/local-8/exact qualification, and explicit multi-body overflow. Invariants pass, but measured PhysBox 3 final-body disagreements still require human behavior-band and subjective-feel disposition. Box2D stays rigid-only. |
| FLUID-3 | PhysBox 3/LiquidFun coupled oracle | **Implemented** | Separate process-isolated native PhysBox 3/Box2D v3 and original LiquidFun/Box2D 2.3 executables consume the shared seven-source projection. Each supports six rows and explicitly rejects grabbed-boundary. Joined evidence records population, centroid, body, exchange, overflow, and replay deltas without majority voting. |
| BREADTH-1 | Ragdolls, machines, soft bodies | **Deferred** | Depend on general joints, broader shapes, accepted scale, and stable rigid behavior. |

## Current execution order

1. Review the measured PhysBox 3 behavior deltas and subjective pouring,
   floating, wave-impact, grab-through-water, and construction feel.
2. Decide whether the 4K sandbox, Water/Surface tools, and direct particle
   style become ordinary defaults or remain explicit preview choices.
3. Open smooth-water rendering, multiple materials, scene save/load, or
   property editing only from an explicit measured/product decision.

The retained 12/14 quietness failures, 16-stack loss, comprehensive rigid
parity, and correctness-qualified CPU/GPU crossover remain visible parallel
debt. They no longer block the bounded liquid entry, and liquid evidence may
not be used to relabel them as passed.

## Reconsideration triggers

Change a status only with implementation and evidence, not because source or a
laboratory candidate exists. Split this topic if the table becomes too large
to answer the current-capability question quickly; preserve stable capability
IDs when moving rows so roadmap and release notes retain a durable reference.
