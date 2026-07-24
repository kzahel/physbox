# External consumer proof

This non-default workspace package behaves like an independent application. It
depends on `physbox-sim` and `physbox-render`, not `physbox-engine`,
`physbox-launch`, `physbox-ui`, a platform app, a scenario catalog, or the
Box2D comparator.

It creates its own WGPU instance/device/queue, disables one supported sleeping
toggle, constructs a floor and ball through public definitions, records and
submits one fixed step, renders to a caller-owned transparent texture, and
requests a final asynchronous body snapshot. The final bounded readbacks are
proof only and do not advance the world.

```sh
cargo run -p physbox-external-consumer
```

Success prints `PHYSBOX_EXTERNAL_CONSUMER_SUCCESS` with bounded body and pixel
counts.

