# Box3D planar stability reference

Status: exploratory source-reference investigation active, 2026-07-18.
Box3D v0.1.0 is pinned as a clean local reference. It may generate bounded
solver, contact-persistence, and scheduling hypotheses, but it is not a rigid
authority, product dependency, or alternative engine. Box2D remains the rigid
behavior authority and the scalar specification and invariants retain
mathematical precedence.

Execution tracker:
[S7 Box3D planar stability investigation](../tactical/007-box3d-planar-stability-investigation.md).
Related governing topics:
[awake settling and rest stability](awake-settling-and-rest-stability.md),
[contact solver parallelism and scheduling](contact-solver-parallelism.md),
and [performance and scale](performance-and-scale.md).

## Why study Box3D now

PhysBox 5 has a working GPU-resident rigid world and a selected hard/direct
colored manifold-block solver. That product preserves the governed stack
structures across the required hosts but remains visibly active without
sleeping. The bounded soft-settling study ended at S6.5 with no promotable
candidate: the best candidate quieted ordinary fixtures but retained a rising
16-stack energy mode and collapsed on both the hardware-backed AVD and a
physical Android GPU. Human review deferred softness and left the hard/direct
product unchanged.

Box3D is valuable because it is a newly released sibling of Box2D with an
architecture that remains close to Box2D while extending Soft Step to stable
large three-dimensional piles. Its lineage includes an early Rubikon-Lite
base, but its author describes most APIs, data structures, and algorithms as
replaced with Box2D-derived or new work in the
[official announcement](https://box2d.org/posts/2026/06/announcing-box3d/).
This makes it a source of concrete
hypotheses about contact persistence, graph ordering, manifold construction,
and solve staging rather than evidence that a Valve or three-dimensional
engine can be transplanted into PhysBox.

The managed checkout is:

- repository: `https://github.com/erincatto/box3d.git`;
- release: `v0.1.0`;
- revision: `8441b4a06d6d09dcfb0b0f704df4d847d1437b92`;
- license: MIT, copyright 2026 Erin Catto; and
- local path: `reference/box3d`, required clean by `reference/pins.toml`.

No Box3D source is copied into the product. An independent harness may compile
and process-isolate the clean source checkout. Any internal microscope must use
a separate worktree or clone and record the base revision plus patch digest.

## What “planar Box3D” means

Box3D has no separate two-dimensional solver. It has body motion locks, and
its stock `Pyramid2D` sample deliberately restricts a three-dimensional body
to the three degrees of freedom of a 2D rigid body:

| Box3D degree of freedom | Planar setting | PhysBox analogue |
| --- | --- | --- |
| translation X/Y | free | 2D position |
| translation Z | locked | absent |
| rotation X/Y | locked | absent |
| rotation Z | free | scalar body angle |

This is a controlled planar dynamics lane, not exact Box2D equivalence.
Collision remains three-dimensional. Rectangles are extruded boxes, convex
manifolds may contain four points instead of two, friction is a two-axis
tangent-plane constraint, and Box3D also models twist and rolling resistance.
Sphere and box inertias likewise need explicit projection if a comparison is
intended to be quantitative.

The study therefore separates dimension-independent mechanisms from
three-dimensional mechanisms:

- potentially transferable: contact/cache lifetime, feature stability,
  persistent color assignment, static-contact color priority, color execution
  order, warm-start/storage semantics, biased/relaxation pass staging, and
  sleep-off energy behavior;
- not assumed transferable: four-point support geometry, central two-axis
  friction, twist/rolling resistance, full inertia tensors, gyroscopic terms,
  mesh contacts, or a visual result that depends on locked 3D geometry.

Box3D is compared beside Box2D and PhysBox, never substituted for either:

| Lane | Role |
| --- | --- |
| pinned Box2D v3 | exact 2D behavior authority |
| pinned Box3D with planar locks | optional mechanism/reference comparator |
| PhysBox GPU | product implementation under evaluation |

## Stock evidence and its limits

Two stock sources are the initial controls:

- `Pyramid2D` creates a 12-row planar-locked pyramid in the sample application.
  It uses the ordinary world sleep policy unless the harness overrides it.
- `CreateLargePyramid` creates a 90-row, 4,095-body planar pyramid and
  explicitly disables sleeping. The release benchmark advances it for 200
  outer steps at 60 Hz with four substeps.

The release source also notes that an “80 block version” falls after 1,000
steps.
The impressive demonstrations are therefore a reason to measure long-run
awake behavior, not proof of indefinite stability. The stock 200-step timing
run is a build/reproduction control and cannot establish settling, convergence,
or a performance comparison with PhysBox or Box2D.

## Initial hypotheses

The investigation ranks these hypotheses before measurement:

1. **Persistent graph order.** Box3D assigns a graph color when a touching
   contact enters the constraint graph and retains that assignment with the
   contact. PhysBox currently rebuilds its bounded coloring. A stable
   Gauss-Seidel order may matter even when both graphs are valid.
2. **Static-contact priority.** Box3D allocates static constraints from the
   high end of its color range so they execute late and have higher effective
   priority. Support transfer may be sensitive to this order.
3. **Cached contact identity.** Box3D first attempts to rebuild the cached SAT
   face or edge within linear slop, persists exact feature IDs, recycles nearby
   contacts, and uses a stable point-reduction pecking order. This is stronger
   temporal geometry than a frame-local separating-axis tie.
4. **Friction staging.** Box3D's convex lane applies central manifold friction,
   twist friction, and rolling resistance only in the unbiased relaxation
   solve. PhysBox currently runs tangent stages after both biased normals and
   relaxation normals. This may change feedback energy, though its 3D friction
   model prevents direct attribution without a controlled comparator.
5. **Manifold/order coupling.** Box3D solves up to four normal points
   sequentially against immediately updated bodies. PhysBox uses a coupled
   two-point normal block. A difference here would be material architecture
   evidence, not permission to replace the selected product solver.
6. **Presentation versus awake stability.** Sleeping, limited run duration,
   or a favorable locked geometry may explain part of a visual tower claim.
   Sleep-off long runs and exact structural metrics test this first.

The first study does not presume any hypothesis is causal. It is designed to
kill unhelpful explanations cheaply before adding a PhysBox comparator.

## Runtime investigation contract

The clean checkout is configured and built out of tree. Build products and
downloaded dependencies must remain outside `reference/box3d`, and
`npm run references:status` must pass after every reference run.

The first public-API observer independently authors the stock planar sources
without modifying Box3D. It records:

- exact source revision, license, build type, compiler, SIMD/validation flags,
  worker count, fixed step, substeps, sleep and recycling settings;
- initial and observed body transforms, linear/angular velocities, body mass
  and inertia, maximum displacement, fallen/lost-body counts, and structure;
- translational, rotational, and total kinetic energy at predeclared cadence;
- touching contact IDs, shape IDs, manifold/point counts, feature IDs,
  persistence flags, normals, separations, normal impulses, total normal
  impulses, and central friction impulses available through the public API;
- Box3D work counters and complete wall time as context, not as a PhysBox
  performance claim; and
- a fresh atomic result plus a success sentinel.

The frozen initial observation points are steps `0`, `200`, `1,200`, and
`5,000`. The 12-row planar sample records every step after the settling window
so energy trend and feature churn are visible. The 4,095-body source may use
bounded checkpoint observations to avoid turning public getters into the
measured solver cost. Sleeping is disabled for the primary lane; a stock-sleep
lane may be shown only as presentation context.

The first comparison uses boxes only. Circle/sphere projection waits until a
question justifies an explicit inertia policy. A later neutral-scenario adapter
must preserve XY transforms, mass, in-plane inertia, materials, fixed step,
actions, and observation points, while recording a separate projection hash
and rejecting unsupported features instead of approximating them silently.

## Forwarding and stop gates

A process-isolated neutral Box3D comparator is justified only if the stock
sleep-off study establishes a material behavior absent from current PhysBox
evidence and the public metrics identify a dimension-independent hypothesis.
Otherwise the study closes with source findings and no adapter.

An internal microscope is justified only when public body/contact observations
cannot distinguish two surviving explanations. It must answer one named
question, such as retained color order or per-pass energy, in a separate
patch-digested checkout. Microscope output is diagnostic and cannot become an
authority baseline.

No Box3D finding autonomously authorizes:

- a product solver, accumulation, manifold, coloring, friction, or sleep change;
- a new accepted physical default or tolerance;
- changed Box2D or invariant precedence;
- a native-only path, CPU fallback, or Box3D product dependency; or
- bypassing the S6.5d reopening gates for 16-stack energy and Android
  portability.

Any PhysBox comparator returns to human review before implementation if it
changes manifold accumulation, color lifetime/priority, or friction staging.
Those are material architecture or physical-policy decisions even if Box3D
demonstrates them successfully.

## Evidence package

The compact review package should let a reviewer decide whether further work
is warranted without reading either engine. It contains:

- a source-mechanism table with exact Box3D and Box2D locations;
- stock and planar-locked run manifests with clean provenance;
- sleep-off 200/1,200/5,000 structure and energy tables;
- contact persistence, feature churn, point-count, impulse, and friction
  summaries;
- synchronized Box3D/Box2D/PhysBox playback only if a neutral adapter is
  justified;
- minimized or smallest available failing tower configurations;
- rejected hypotheses and the evidence that killed them; and
- at most two proposed transferable experiments, each with an explicit human
  checkpoint before product work.

## Reconsideration triggers

Revisit the role of Box3D only when a new release materially changes the
measured mechanisms, a clean long-run stock result contradicts this record, or
a PhysBox trace exposes a question directly answered by Box3D. Pin changes are
reviewed reference decisions. Do not track Box3D main opportunistically or use
its visual demonstrations as a numerical baseline.
