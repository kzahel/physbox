# PhysBox 3 and PhysBox 4 feature usage

Status: active research record, 2026-07-23. This topic records which proposed
PhysBox 5 capabilities were actually exercised by the two first-party
predecessors. It is sequencing evidence, not a compatibility requirement and
not authorization to import either predecessor's engine architecture.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Current inventory:
[product capability status](product-capability-status.md).
Performance obligations:
[performance and scale](performance-and-scale.md).
Source roles and pins:
[local references and oracle map](../../reference/README.md).

## Conclusion

PhysBox 3, rather than PhysBox 4, is the strongest evidence for useful rigid
sandbox breadth. Its reachable product tools and prefabs depend repeatedly on:

- general joints;
- convex and compound bodies plus authored chain terrain;
- kinematic bodies and bullet continuous collision;
- hit/contact information;
- selection, resizing, and limited property actions; and
- user-facing scene persistence.

PhysBox 4 deliberately narrowed the rigid feature set while investigating a
split CPU-rigid/GPU-fluid architecture. Its strongest continuing evidence is
different: portable GPU diagnostics and capacity accounting, an engine-owned
cross-host authoring loop, active capsule use, a small overlap-query contract,
and three distinct particle materials.

Neither predecessor's reachable product behavior establishes demand for
custom collision categories/masks, sensor/trigger gameplay, a general
ray/shape-cast surface, undo/redo, or smooth-liquid rendering. PhysBox 3
contains dormant wrappers or support code for some of those features, and
both repositories contain plans for features that did not land. Presence in a
dependency, renderer, test, type definition, or plan is not counted as product
use.

This evidence changes the default sequencing recommendation after PhysBox 5's
solver and sleeping work: general joints and authored geometry should precede
a broad collision-event/query package unless a new concrete feature supplies
different demand.

## Evidence basis

The research inspected clean first-party sibling worktrees and the exact
particle-sidecar source declared by the PhysBox 5 reference map:

| Source | Revision inspected | Role in this research |
| --- | --- | --- |
| [PhysBox 3](https://github.com/kzahel/physbox3/tree/b11d73dae7dfe96f2efcba9dc5e3e53d10390867) | `b11d73dae7dfe96f2efcba9dc5e3e53d10390867` | Reachable Box2D sandbox tools, prefabs, persistence, and presentation |
| [PhysBox 4](https://github.com/kzahel/physbox4/tree/92f745eb85908243c69080a4fa4dba24f05c22cf) | `92f745eb85908243c69080a4fa4dba24f05c22cf` | Split CPU-rigid/GPU-fluid product experiment and cross-host tooling |
| PhysBox 3 particle sidecar | `ef8d9526ae79377da3d610e5e4d3bd227f072cdb` | Exact coupled particle/Box2D bridge used to distinguish live internal queries from unused TypeScript wrappers |

`npm run references:status` passed on 2026-07-23 for the managed sources and
first-party sibling mappings. Both inspected sibling roots had empty
`git status --short` output. The PhysBox 3 sibling tracks `main`; the sidecar
revision, not an unrecorded package build or dirty vendor tree, owns claims
about its internal particle/rigid query path.

The classification rules were:

- **used** means a reachable product tool, prefab, built-in scene, simulation
  path, persistence path, renderer, or diagnostics surface consumes the
  capability;
- **limited** means a narrower capability is used, or support is real but only
  validation/internal code consumes it;
- **not used** means no reachable product consumer was found, even if the
  underlying Box2D or Rapier dependency supports it; and
- tests and plans corroborate a live path but do not promote a feature to
  product use by themselves.

## Feature-usage matrix

| Capability | PhysBox 3 | PhysBox 4 | Evidence and boundary |
| --- | --- | --- | --- |
| Collision categories and masks | Not used | Not used | PhysBox 3 creates default shape/query filters and has no product assignment of category, mask, or group bits. PhysBox 4 exposes no neutral filter record. |
| Sensors and triggers | Dormant support only | Not used | PhysBox 3 can set and render `isSensor`, but no product tool or prefab creates a sensor and no sensor-event consumer exists. PhysBox 4 records neither sensors nor trigger events. |
| Contact events and inspection | Used, narrower than a general event stream | Not used | PhysBox 3 hit events drive collision audio; polled contact data drives cannon impacts, ragdoll foot state, and attachment behavior. It does not expose a general begin/end contract. PhysBox 4's completed sandbox slice explicitly excludes collision events. |
| Spatial queries | Limited, used internally | Limited, used by tools | The PhysBox 3 particle sidecar calls `b2World_OverlapAABB` for particle/body candidates; product picking otherwise scans tracked bodies and calls `b2Shape_TestPoint`. Its `castRayClosest` TypeScript wrapper has no caller. PhysBox 4 uses one backend-neutral circle-overlap query for grab and erase, implemented as collider intersection tests rather than a general AABB/ray/shape-cast API. |
| Continuous collision and bullets | Used | Not used | PhysBox 3 enables continuous collision globally and creates cannonballs with `isBullet`. PhysBox 4 exposes no CCD setting or bullet body kind. |
| Kinematic bodies | Used | Not used | PhysBox 3 conveyors are kinematic bodies with tangent surface speed. PhysBox 4's exhaustive neutral body kind contains only fixed and dynamic. |
| Capsules | Limited product breadth | Used | PhysBox 3 renders capsules and its coupled bridge and tests support particle/capsule contact, but no ordinary tool, prefab, or saved-scene shape creates one. PhysBox 4 uses capsules in built-in scenes, rigid snapshots, rendering, and particle collision. |
| Convex polygons | Used | Not used | PhysBox 3's Draw tool constructs a bounded convex hull, and cars, rockets, cannons, trains, and other prefabs use authored convex polygons. PhysBox 4 supports only circle, oriented box, and capsule primitives. |
| Chains and terrain | Used | Not used | PhysBox 3's Terrain tool simplifies a drawn polyline into a closed one-sided Box2D chain and preserves authored terrain points. PhysBox 4 supports only a drag-sized straight box surface. |
| Compound rigid bodies | Used | Not used | PhysBox 3 attaches multiple shapes to individual car, rocket, balloon, cannon, and train bodies. PhysBox 4 stores exactly one `RigidShape` per `RigidBodyRecord`. |
| General joints | Heavily used | Not used | PhysBox 3 product paths create revolute, distance/spring, weld, wheel, prismatic, and motor joints for ragdolls, ropes, springs, cars, trains, launchers, glue/attach, and grab. PhysBox 4 grab stores an anchor and applies bounded velocity steering without a joint. |
| Persistent selection and property actions | Used | Not used | PhysBox 3 retains one selected body and exposes static/dynamic toggle, motor toggle, direction reversal, camera follow, and scaling. PhysBox 4 retains only the active tool and transient grab. |
| Deliberate shape sizing | Used | Limited | PhysBox 3 supports whole-body scaling, endpoint resizing for platforms/conveyors, freehand convex drawing, and freehand terrain. PhysBox 4 drag-sizes straight surfaces, while placed balls and boxes use fixed authored dimensions. |
| User-facing scene save/load | Used, incomplete | Schema only | PhysBox 3 exposes IndexedDB save/load/delete UI for bodies, supported shapes, joints, ropes, terrain, and gravity; it does not persist live fluid particle state and not every Box2D shape is serializable. PhysBox 4 has a strict versioned JSON `SceneDocument`, but no product storage/file transport or save/load UI consumes it. |
| Undo and redo | Not used | Not used | No history or command-reversal product path exists in either sibling. |
| Smooth-liquid rendering | Not used | Not used | Both products render direct particles. PhysBox 4 documents reduced-resolution thickness/surface rendering as deferred work. |
| Multiple particle materials | Not used | Used | PhysBox 3 has one water-like sidecar material; its Sand tool creates many rigid circles. PhysBox 4 exposes Water, Goop, and Sand particle tools with distinct mass, viscosity, tensile/cohesion, friction, and color profiles. |
| Runtime and scale diagnostics | Used | Heavily used | PhysBox 3 shows Box2D step/collide/solve/bullet/sensor timing, counts, memory, awake bodies, and fluid usage. PhysBox 4 adds GPU occupancy/density/collision counters, overflow, allocated bytes, asynchronous compact readback, debug views, and headless benchmark JSON. |

## Primary implementation evidence

### PhysBox 3

The reachable product tool set is registered in `src/ui/Toolbar.ts` and
constructed in `src/interaction/InputManager.ts`. It includes authored
objects, ropes/springs, attach/detach/glue, freehand convex and terrain tools,
selection, scaling, fluid, and rigid-particle sand.

The principal feature owners are:

- `src/engine/PhysWorld.ts`: continuous-world configuration, hit-event
  processing, dormant ray/AABB wrappers, tracked bodies/joints, and live
  Box2D profiling;
- `src/prefabs/Cannon.ts`: bullet creation and polled impact contacts;
- `src/prefabs/Conveyor.ts`: product kinematic-body use;
- `src/engine/Physics.ts`: body point tests, scaling, and general joint
  helpers;
- `src/prefabs/Ragdoll.ts`, `Rope.ts`, `Car.ts`, `Train.ts`, `Launcher.ts`,
  `Jelly.ts`, and `SpringBall.ts`: concrete joint consumers;
- `src/interaction/tools/DrawTool.ts` plus `src/prefabs/Polygon.ts`: freehand
  convex creation;
- `src/interaction/tools/TerrainTool.ts` plus `src/prefabs/Terrain.ts`:
  authored chain terrain;
- `src/interaction/tools/SelectTool.ts`, `SelectionButtons.ts`, and
  `ScaleTool.ts`: persistent selection and property/size actions;
- `src/engine/SceneStore.ts` and `src/ui/SettingsPane.ts`: live IndexedDB
  scene persistence UI; and
- the pinned sidecar's
  `box2d3-wasm/csrc/particle_sidecar/particle_bridge.cpp`: AABB candidate
  queries, sensor exclusion, and circle/polygon/capsule/segment/chain contact
  adaptation for fluid coupling.

### PhysBox 4

The exhaustive neutral rigid surface is small:
`crates/physbox-rigid/src/lib.rs` defines only fixed/dynamic bodies and one
circle/box/capsule shape per body. Its Rapier adapter's `query_radius` and
`begin_grab` methods are the only product spatial-query consumers; grab is
manually steered rather than constrained by a joint.

The principal continuing evidence is:

- `crates/physbox-engine/src/toolbar.rs`: the reachable Water, Goop, Sand,
  Erase, Ball, Box, Surface, and Grab tools;
- `crates/physbox-engine/src/materials.rs`: three data-driven particle
  profiles;
- `crates/physbox-engine/src/scene.rs`: versioned neutral JSON scene records,
  fixed-size ball/box placement, drag-sized straight surfaces, built-in
  capsules, and multi-material regions;
- `crates/physbox-engine/src/rigid_scene.rs`: transactional scene mutation,
  circle-overlap erase/grab routing, and current fluid/rigid response; and
- `crates/physbox-engine/src/counter_readback.rs`, `config.rs`, and the
  headless app: compact GPU diagnostics, explicit memory/overflow reporting,
  and benchmark artifacts.

PhysBox 4's older tactical wording calls two-way fluid coupling future work,
but the inspected revision contains `apply_fluid_impulses` and reports applied
fluid contacts. The current code therefore overrides that stale planning
sentence for feature-usage classification.

## PhysBox 5 sequencing implication

Predecessor use does not prove that PhysBox 5 must recreate every toy. It does
show which foundations repeatedly enabled useful sandbox behavior. Subject to
the current bring-up gates, the default lineage-informed order after solver
and sleeping is:

1. Add one general authored joint and its endpoint interaction, using distance
   or spring as the smallest graph/constraint proof.
2. Add revolute constraints, then limits and powered behavior; add weld only
   when attachment or compounds need it.
3. Add bounded convex polygons, authored terrain/chains, and compound-body
   records rather than implementing isolated gadget prefabs first.
4. Add kinematic bodies, CCD, and the minimum hit/contact inspection required
   by a declared conveyor, projectile, grounded-character, or impact feature.
5. Add persistent selection, deliberate sizing/property actions, and
   user-facing scene persistence over stable PhysBox records.
6. Treat PhysBox 4's three particle materials as the strongest predecessor
   evidence for later multi-material work after the current one-water
   behavior/default checkpoint is resolved.

Queries should grow from concrete consumers. The existing GPU hit-selection
and erase/grab mechanisms may be extended with the smallest bounded overlap
operation needed by each tool. A comprehensive synchronous CPU-facing
AABB/ray/shape-cast API is not justified by predecessor use and must not
violate the no-next-frame-readback rule.

Collision filters, sensors, general begin/end events, undo/redo, and
smooth-liquid rendering remain legitimate future capabilities. They are not
the default next bundle solely because Box2D, Rapier, or a predecessor plan
mentions them.

## Alternatives considered

### Recreate the PhysBox 3 feature list in toolbar order

Rejected. Most visible PhysBox 3 gadgets are compositions over a much smaller
foundation. Implementing prefabs independently would obscure the actual
requirements and encourage CPU/Box2D-shaped APIs before the portable
GPU-resident constraints are designed.

### Follow PhysBox 4's narrow rigid scope

Rejected as the long-term product target. That scope was a deliberate
experiment around a CPU rigid backend and GPU fluid pipeline. It is valuable
architecture and diagnostics evidence, but it omitted features that PhysBox 3
used extensively and does not test PhysBox 5's unified GPU constraint graph.

### Build a comprehensive collision interaction API next

Not selected from predecessor evidence. Hit/contact inspection has concrete
PhysBox 3 consumers, and narrow overlap queries have consumers in both
siblings. Custom filters, sensors, begin/end events, ray casts, and shape casts
do not. Implement them incrementally when a declared feature needs them.

## Validation and update protocol

Before using this topic to admit a feature:

- identify at least one concrete PhysBox 5 scenario or tool that consumes it;
- separate product use from dependency capability and dormant wrapper code;
- preserve the one authoritative GPU-resident PhysBox state and cross-host
  shared-host contract;
- apply the scale, timing, capacity, overflow, and correctness evidence
  required by the performance topic; and
- treat PhysBox 3 behavior as product calibration and PhysBox 4 as source
  evidence, not as authorities that can overrule the declared Box2D rigid
  oracle or PhysBox 5 invariants.

When either sibling advances, a refresh must record the new root revision and
dirty state, rerun `npm run references:status`, and recheck reachable product
consumers. A plan or dependency upgrade alone does not change a row.

## Reconsideration triggers

Revisit the ordering or a matrix classification when:

- a new PhysBox 5 product scenario requires filters, sensors, begin/end
  events, ray casts, shape casts, or undo/redo before joints or geometry;
- current PhysBox 3 or PhysBox 4 product code adds a reachable consumer for a
  feature currently marked not used;
- direct human review shows a predecessor capability was central to play
  value despite weak code-frequency evidence;
- joint or geometry work violates portable WebGPU cost/capacity gates and a
  narrower interaction feature offers materially better value; or
- the current water checkpoint selects multi-material or smooth-surface work
  as the next explicit product priority.
