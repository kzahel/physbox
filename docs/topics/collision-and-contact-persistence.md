# Collision and contact persistence

Status: S3.1 contracts implemented; GPU collision behavior begins at S3.2,
2026-07-15.

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

## Validation

S3.1 validation covers schema round trips, exact byte claims, nonzero
generations, canonical pair/manifold/cache ordering, finite AABBs and contact
values, normalized normals, feature ranges, stable hashes, missing/stale
identity, shape-flag mismatch, unsupported projection data, inconsistent work
accounting, and silent or partial capacity overflow.

The current product bootstrap still uses its temporary authored fixed-floor
manifold. S3.1 adds no GPU candidate, narrowphase, persistence, or generated
contact solve. S3.2 begins that behavior from the versioned surface above.

## Reconsideration triggers

Reconsider the provisional feature or record layout before S3.6 if direct
Box2D comparison cannot establish stable correspondence, clipped points need
identity beyond the two geometric features, sort/persistence measurement shows
a material bandwidth problem, or a required WebGPU adapter cannot bind the
declared records. Stop for human review if resolving such evidence would
change the accepted body core, solver accumulation architecture, oracle
precedence, numerical tolerance, platform portability, atomic failure policy,
or require simulation-dependent readback.
