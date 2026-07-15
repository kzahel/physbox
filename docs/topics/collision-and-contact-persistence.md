# Collision and contact persistence

Status: S3.6 manual checkpoint; contact representation and physical policy
remain provisional, 2026-07-15.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Execution tracker:
[S3 small-world collision and contact persistence](../tactical/003-small-world-collision-persistence.md).

## Current decision

P5.2 keeps the accepted 64-byte `BodyCoreV1` unchanged. A centered analytic
shape uses `properties.xy`: rectangles store `(half_extent_x, half_extent_y)`
and circles repeat `(radius, radius)`. Metadata bit 8 remains rectangle
(`256`); bit 9 is circle (`512`). Exactly one analytic shape bit is present for
each live P5.2 body. The properties and metadata lanes remain 16 bytes each,
and the body plus existing command records remain 64 and 48 bytes respectively.

This is an inline encoding for the first centered circle/rectangle scope, not
a universal shape allocation. General polygons, compounds, per-shape
materials, capsules that outgrow the two-word payload, and joints still require
separate versioned buffers. No P5.2 decision may reinterpret a stable body
field to avoid that later allocation.

## Canonical identity and features

Candidate pairs are lexicographically ordered by full generational `BodyId`;
`body_a < body_b` is mandatory. A generation is part of every AABB, pair, and
persistent key, so a reused slot cannot inherit an old contact.

The provisional analytic feature convention is:

- a circle has one feature, `circle:0`;
- rectangle vertices 0 through 3 are lower-left, lower-right, upper-right,
  upper-left in counterclockwise order; and
- rectangle faces 0 through 3 are bottom, right, top, left, following the same
  counterclockwise winding.

A persistent point key is `(body_a, body_b, feature_a, feature_b)`. Manifold
points are sorted and unique by that key. Circle/circle and circle/face pairs
therefore have one key; clipped face contacts retain a vertex feature on at
least one participant so their two points remain distinct.

This convention is not frozen at S3.1. S3.3 compares topology and geometric
correspondence directly with clean pinned Box2D. Box2D's packed public point ID
is authority output, not a product storage format. A measured disagreement may
revise the provisional mapping before S3.6; it may not be hidden by widening a
tolerance or attaching a frame-local ordinal to a persistent key.

## Versioned record surface

Collision layout version 1 declares these portable, 16-byte-aligned GPU
records:

| Record | Stride | Contract meaning |
| --- | ---: | --- |
| body AABB | 32 B | generational identity plus finite lower/upper bounds |
| candidate pair | 16 B | canonical full identities for A and B |
| capacity counter | 16 B | attempted/committed/overflow staging counters |
| cache value | 16 B | accumulated normal/tangent impulse and last-seen step |
| manifold | 48 B | pair, normal/material, and canonical point range |
| contact point | 48 B | local anchors, world point/separation, and two features |
| persistent key | 32 B | canonical pair plus geometric features |
| work counters | 128 B | complete pair/narrowphase/cache/dispatch accounting |

`CollisionArtifactV1` is the canonical bounded-validation envelope. It carries
schema and collision-layout versions, run/source/projection identity, complete
provenance, analytic encodings, AABBs, pairs, manifolds, persistent cache,
work, capacities, first-divergence traces, and structured failure data.
Variable JSON provenance, trace, and failure records are not misrepresented as
fixed GPU structs.

Capacity records state stride, capacity, attempts, committed elements,
allocation, high-water, overflow, and acceptance. A non-overflowing stage
commits every attempt. An overflowing stage commits zero, marks the artifact
unaccepted, and requires a structured capacity failure. Prefix truncation is
invalid even if the truncated prefix looks canonical.

## Shared sources and neutral projection

The engine owns four deterministic P5.2 sources: circle/circle,
circle/rectangle, oriented rectangle/rectangle, and a six-body mixed small
world. They contain no authored manifolds, cache, or actions. The neutral
Box2D narrowphase projection is produced from those sources, forces one
narrowphase evaluation step, and changes the projection ID explicitly.
Authored manifolds, cache, or actions are rejected instead of approximated or
silently omitted.

The canonical hashes and byte table are checked in the
[S3.1 evidence package](../../evidence/p5.2-s3.1/README.md). The generator is
deterministic and the scenario tests pin all source and projection hashes.

## Complete candidate baseline

S3.2 establishes an eight-dispatch GPU baseline: clear stage state, compute
world AABBs, classify every unordered slot pair, prefix-scan flags in blocks,
scan block totals, add block offsets, finalize counts/capacity, and scatter
canonical pairs. Fixed/fixed and AABB-separated live pairs are diagnosed
separately. Slot holes are excluded by live generation metadata. Candidate
order follows the triangular `(body_a, body_b)` enumeration and therefore the
full generational body order.

No mapped count schedules the scan or scatter. All dispatches use declared
slot/pair capacities, while bounded native validation maps results only after
the graph completes. Candidate overflow atomically commits zero records and
invalidates the result; it never exposes a prefix as a successful set.

The [S3.2 evidence](../../evidence/p5.2-s3.2/README.md) has exact set agreement
against independent scalar brute-force enumeration across 324 governed,
randomized, touching, rotated-thin, coincident, and fixed-only cases. Maximum
AABB absolute error is `2.3841858e-7` under the unchanged `2e-5` comparison
bound. The candidate, flag, offset, scratch, scan, and work allocations report
attempts, committed records, bytes, high-water, and acceptance independently.

This stage intentionally allocates flags, offsets, and scratch for
`N * (N - 1) / 2` slot pairs. It is the persistent completeness oracle for
small worlds, not a scalable broadphase choice. S3.3 consumes the same
canonical candidate records for narrowphase; P5.3 later compares SAP, grid,
and any justified LBVH against this baseline.

## Provisional reference-axis tie convention

Rectangle SAT normally uses Box2D's direct maximum-separation reference
selection. One deliberately narrow adversarial class uses the same explicit
coherence bias in scalar Rust and portable WGSL: identical rectangles whose
centers are within `0.1` and whose two best axes are both deeper than `0.05`.
There canonical body A remains the reference unless B's separation is
materially better under relative factor `0.98` and absolute bias `0.001`. The
rule does not change SAT rejection; it prevents CPU/GPU floating-point
contraction from alternating between nearly equivalent axes in a deeply
coincident initial state and therefore changing persistent features.

The expanded 64-seed GPU corpus exposed the need for this rule in deeply
coincident, nearly parallel equal rectangles. A diagnostic comparison of the
same degenerate lane found four cases in 16 seeds where unmodified Box2D chose
the other valid reference axis. Those cases are not treated as geometry rows
under a wider tolerance: direct authority geometry keeps the unchanged
`2e-5` bound on non-degenerate randomized pairs and exact governed ties, while
deep coincidence remains a product stability/invariant lane. This provisional
departure and its effect on persisted features must be reviewed explicitly at
S3.6 before the contact surface is frozen.

## Validation

S3.1 validation covers schema round trips, exact byte claims, nonzero
generations, canonical pair/manifold/cache ordering, finite AABBs and contact
values, normalized normals, feature ranges, stable hashes, missing/stale
identity, shape-flag mismatch, unsupported projection data, inconsistent work
accounting, and silent or partial capacity overflow.

The [S3.3 package](../../evidence/p5.2-s3.3/README.md) records 324 exact
scalar/GPU topology and feature comparisons, 100 fresh process-isolated
Box2D public-collision runs, four compact governed playbacks, grouped capacity
rejection, and deterministic minimization. Maximum GPU numerical error was
`1.2516975e-6`; maximum authority geometry error was `3.2782555e-7` under the
unchanged `2e-5` bound.

The S3.5 product frame now consumes generated contacts and sorted warm starts
directly. Nine behavior tracks replay exactly, cached impulses remain
projected, and 128 randomized cases have zero replay differences or accepted
overflow. Floor, friction, and restitution endpoints are close to Box2D, while
the mixed stack differs materially (`0.560` maximum position and `0.974`
maximum velocity) and remains a quality-review item rather than a widened
band.

Two minimized failures remain explicit. A six-body coincident spawn injects
`26891.44` mechanical energy in one step even though canonical incidence makes
the outcome deterministic. A two-body closing gap crosses during an outer
step but has no initial AABB overlap, so the current graph emits no contact and
ends at `0.00507` penetration while Box2D responds. S3.6 must choose a
depenetration/spawn policy and a portable speculative distance plus AABB
expansion; neither physical default is selected by the implementation.

## Pinned PhysX organization study

The [S3.6 PhysX source study](../../evidence/p5.2-s3.6/physx-gpu-organization-study.md)
confirms that the current persistence sorter already uses four-bit digits. Its
64 passes arise from globally sorting the complete eight-word semantic point
key. PhysX instead organizes active collision pairs around stable manager
slots and bounded local persistent manifolds, and its particle path maintains
rigid-oriented and particle-oriented contact orderings before a segmented
rigid-reaction reduction.

For the proposed S3.5a bakeoff, this makes stable pair-slot two-point
persistence the lead cache lane and a stable body-edge index view plus
offsets/segmented gather the lead adjacency lane. The complete semantic key
remains canonical acceptance truth; local feature equality and pair
generation must be verified before any impulse transfer. Alternate orderings
are compact views over one canonical payload so later particle/rigid reactions
can add a consumer without a second engine.

These are source-informed recommendations, not accepted representation
decisions. PhysX remains a 3D CUDA organizational reference. Its CPU-owned
pair/partition management, host-known solve loops, count readback, and partial
overflow behavior are incompatible with the product contract and will not be
copied.

## Reconsideration triggers

Reconsider the provisional feature or record layout before S3.6 if direct
Box2D comparison cannot establish stable correspondence, clipped points need
identity beyond the two geometric features, sort/persistence measurement shows
a material bandwidth problem, or a required WebGPU adapter cannot bind the
declared records. Stop for human review if resolving such evidence would
change the accepted body core, solver accumulation architecture, oracle
precedence, numerical tolerance, platform portability, atomic failure policy,
or require simulation-dependent readback.
