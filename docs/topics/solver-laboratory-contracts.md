# Solver laboratory contracts

Status: version 1 implemented, 2026-07-14.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Execution tracker:
[S1 rigid solver laboratory](../tactical/001-rigid-solver-laboratory.md).

## Decision

`physbox-contracts` is the schema-only boundary shared by the product engine,
the scalar laboratory, native oracle processes, comparison/minimization, and
performance tooling. It contains serializable data and validation only: no
`wgpu`, solver implementation, process runner, Box2D symbol, or platform code.

Version 1 defines bodies, authored manifolds and contact points, contact-cache
impulses, solver settings, neutral actions, scenarios, oracle requests and
results, pass traces, work/timing metrics, capacity records, failures, and
complete run/build/runtime provenance.

## Canonical bytes and hashes

Canonical payloads are compact UTF-8 JSON emitted from Rust structs in declared
field order. Identifier-bearing vectors and named settings must be sorted and
unique. Hashed payloads admit finite numbers only and contain no object maps
whose iteration order could vary.

The source scenario hash is SHA-256 over the canonical `ScenarioV1` bytes. The
projection hash is SHA-256 over the exact `OracleRequestV1` bytes written to the
native process. The projection hash is recorded in the result rather than
inside the request, avoiding a self-referential payload.

## Versioning

Every top-level scenario and native request/result carries schema version `1`.
Readers reject unknown versions; they do not reinterpret them using the latest
layout. An incompatible field or meaning change requires a new version and a
recorded migration. Additive internal Rust helpers do not change the wire
version.

## Validity

Validation rejects non-finite values, negative inverse mass/inertia or
friction, restitution outside `[0, 1]`, invalid shape dimensions, non-unit
manifold normals, manifolds outside one or two points, negative accumulated
normal impulse, mismatched trace bits, unsorted identifiers/settings, invalid
timing percentiles, inconsistent capacity high-water, and malformed hashes.

Process freshness, atomic result creation, timeout, pin checks, and the success
sentinel are runner responsibilities built on this schema in the next slice.

## Reconsideration triggers

Reconsider JSON only when trace volume measurably dominates validation time or
storage. Any denser encoding must preserve a human-inspectable manifest,
stable hashes, finite-number enforcement, and explicit version negotiation.
