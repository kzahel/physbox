# External consumer proof

This non-default workspace package behaves like an independent physics-engine
consumer. It depends only on `physbox-sim`, not `physbox-engine`,
`physbox-launch`, `physbox-ui`, renderer or platform crates, a scenario
catalog, or the Box2D comparator.

It creates its own WGPU instance/device/queue, disables one supported sleeping
toggle, constructs a floor and ball through public definitions, records and
submits four fixed steps, and requests a final asynchronous body snapshot. It
does not construct a surface, texture, render pipeline, camera, or presentation
object. The final bounded readback is proof only and does not advance the
world.

```sh
cargo run -p physbox-external-consumer
```

Success prints `PHYSBOX_EXTERNAL_CONSUMER_SUCCESS` with bounded body/step
counts and the observed ball height.
