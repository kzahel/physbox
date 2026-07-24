# physbox-sim

`physbox-sim` is the non-published, GPU-resident mechanical library extracted
from PhysBox 5. It owns rigid and particle compute state, fixed-step command
encoding, stable generational handles, supported quality configuration, a
same-device render view, and explicitly requested asynchronous observations.
It does not own a WGPU adapter, queue submission, a surface, camera/input/UI
policy, scenario presets, validation reports, or Box2D.

Local consumers use the current checkout:

```toml
[dependencies]
physbox-sim = { path = "../physbox5/crates/physbox-sim" }
wgpu = "25"
```

The path is deliberately unversioned and `publish = false`; record the
PhysBox 5 Git revision and dirty state for reproducible builds. The caller
creates the `wgpu::Device`, `Queue`, command encoder, and render targets.
`World::step` records exactly one requested outer fixed step into the caller's
encoder and never submits it.

The supported surface is `World`, definitions and handles, `QualityDef`, the
fixed-step clock, bounded counters, `RenderView`, and snapshot requests.
`RenderView` is read-only and same-device. A snapshot copy must be submitted
before `SnapshotRequest::begin_map`; completion remains asynchronous until the
caller polls its device. Interactive advancement never depends on that
readback.

Initial rigid definitions support arbitrary validated circles and rectangles.
Shape density normally derives mass and inertia; `MassPropertiesDef` provides
an explicit Box2D-style initial mass-data override when an authored world must
preserve exact inverse mass and inertia. `BodyDef::with_sleeping` can author an
initial sleeper, subject to the world's sleeping configuration.
Creation after the first step currently uses the existing bounded GPU
construction path and therefore accepts the standard construction circle or
rectangle, standard material, derived mass, awake state, and zero initial
rotation/velocity. Liquids and neutral grab/query commands are mechanically
present but do not yet have a supported public authoring façade. The
`engine-internal` feature and
`__private` module are compatibility plumbing for PhysBox 5 itself, not a
consumer API.

Run the surface-free public proof with:

```sh
cargo test -p physbox-sim --test public_world
```
