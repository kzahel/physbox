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
results (including public narrowphase manifolds), synchronized playback
tracks/samples, pass traces, work/timing metrics, capacity records, failures,
and complete run/build/runtime provenance.

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
sentinel are runner responsibilities. The runner rejects unknown versions and
incomplete native provenance before a result can be accepted.

## Native process protocol

Each adapter is launched in a unique run directory with:

```text
ADAPTER [adapter options] --request oracle-request.json \
  --result oracle-result.partial.json
```

The request is atomically written before launch. The adapter parses that exact
file, writes one canonical result to the supplied staging path, closes it, and
prints an exact `PHYSBOX5_ORACLE_SUCCESS` line only after its work succeeds.
Adapter diagnostics go to captured stdout and stderr logs; they are never
parsed as result data.

The sentinel alone never accepts a result. The shared runner applies a bounded
timeout, kills and waits for an overrun process, validates the result schema,
then matches all of the following against the current request and the
preflight-verified oracle source:

- run id in both the result and provenance;
- source scenario SHA-256;
- projection SHA-256 over the exact request bytes;
- source revision, dirty state, and optional patch digest; and
- an absent structured oracle failure.

Only after those checks does the runner atomically rename the staging result
to `oracle-result.json` and sync the run directory. Valid but unacceptable
payloads become `oracle-result.rejected.json`; malformed or interrupted writes
remain visibly partial. Neither can be mistaken for accepted evidence.

Run ids are portable filename components and never reuse an existing run
directory. A pre-existing directory is a stale-result failure, even if the old
payload would otherwise validate. Clean authority checkouts are verified by
exact Git revision plus an empty tracked/untracked status before launch.

The transport fixture is a subprocess used only to prove the runner. Its
deliberately stale, wrong-hash, wrong-pin, unsupported, partial,
missing-sentinel, and timeout modes are not an oracle or a behavioral
baseline.

## Reconsideration triggers

Reconsider JSON only when trace volume measurably dominates validation time or
storage. Any denser encoding must preserve a human-inspectable manifest,
stable hashes, finite-number enforcement, and explicit version negotiation.

## Scenario ownership and replay

The scenario registry and deterministic parameterized generators live in
`physbox-engine`; `physbox-lab` calls that registry and never maintains a
second fixture definition. The current authored one-contact and chain families
use a versioned fixed integer generator whose seed and complete canonical
scenario are recorded. Replaying the same `(family, parameters, seed)` must
produce byte-identical scenario JSON.

The engine also owns the neutral circle/floor, rectangle/floor, and
box-on-box analytic fixture families. They contain no private contact data;
the scoped scalar generator projects their shared body state into regenerated
manifolds. General collision detection remains deferred rather than becoming a
second fixture source.

The laboratory independently checks finite vectors, equal-and-opposite
contributions, nonnegative normal impulse, Coulomb tangent projection,
body-label swap symmetry, and capacity validity. Tests include deliberately
broken responses for every invariant so a vacuous checker cannot pass.

P5.0a failure automation, 2026-07-14: the deterministic corpus applies exact
scalar trace comparison only to isolated authored contacts, where execution
order is shared. Multi-contact candidates are checked for canonical replay,
finite state, projected impulses, capacity, and paired Jacobi contributions;
their different parallel schedules are not incorrectly compared with a
sequential scalar ordering. A failing case is reduced only while the original
failure signature persists, across manifolds, actions, bodies, and requested
step windows. Injected failures prove deterministic reduction, and a clean run
publishes no invented reproducer.

## Synchronized playback

`PlaybackArtifactV1` is the shared presentation contract for scalar/oracle and
later GPU/oracle evidence. A track embeds the complete engine-owned source
scenario and its canonical hash. Its ordered samples pair body states at an
identical completed fixed-step count and record scalar trace count/hash, oracle
scenario/projection hashes, full oracle provenance, the preserved process
artifact path, objective maximum position/angle/velocity deltas, and dynamic
sleep-state agreement.

Playback is evidence, not an implicit tolerance. S1.3 reports the scalar and
Box2D differences without declaring either trajectory delta an acceptance
band. Scalar mathematical/pass invariants retain their precedence; reviewed
solver-quality bands are established from the S1.4/S1.5 candidate matrix.
