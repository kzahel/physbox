# physbox-sim

`physbox-sim` is PhysBox 5's unpublished GPU-resident physics library. It owns
mechanical state, fixed-step command encoding, stable generational handles,
supported quality configuration, canonical GPU physics observations, and
explicit asynchronous snapshots.

It does not expose or own a renderer, rendering surface, camera, target,
visual theme, application scene, validation report, or Box2D comparator.

Local consumers use the current checkout:

```toml
[dependencies]
physbox-sim = { path = "../physbox5/crates/physbox-sim" }
wgpu = "25"
```

The path is deliberately unversioned and `publish = false`; record the exact
PhysBox 5 revision and dirty state for reproducible builds. The caller creates
the `wgpu::Device`, queue, command encoders, and render resources.
`World::step` records exactly one requested outer fixed step into the caller's
encoder and never submits it.

The supported surface includes `World`, definitions and generational handles,
`QualityDef`, supported physical and PhysBox quality controls, the fixed-step
clock, bounded counters, `GpuBodyObservation`, and snapshot requests.

`GpuBodyObservation` is a versioned physics-state projection into a dedicated
same-device buffer. Its canonical records contain body pose, motion, identity,
and public state flags. It contains no shape presentation, draw data, camera,
target, or private solver layout. A consumer owns the shapes it authored and
may use those records in its own renderer or compute work.

A snapshot copy must be submitted before `SnapshotRequest::begin_map`;
completion remains asynchronous until the caller polls its device.
Interactive advancement never depends on that readback.

Initial rigid definitions support validated circles and rectangles. Shape
density normally derives mass and inertia; `MassPropertiesDef` provides an
explicit Box2D-style initial mass-data override. Creation after the first step
currently uses the bounded standard construction path, so its accepted initial
state is narrower. Public particle authoring and particle observation remain
future consumer-driven work.

The `engine-internal` feature and hidden module are unstable first-party
migration plumbing. External consumers must not enable or import them.

Run the surface-free public proof with:

```sh
cargo test -p physbox-sim --test public_world
```
