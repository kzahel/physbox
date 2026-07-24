# External consumer proof

This non-default workspace package behaves like an independent physics-engine
consumer. It depends only on `physbox-sim`, not `physbox-engine`,
`physbox-launch`, `physbox-ui`, renderer or platform crates, a scenario
catalog, or the Box2D comparator.

It creates its own WGPU instance/device/queue, disables one supported sleeping
toggle, and constructs a floor and ball through public definitions. It proves
same-step deferred query/constraint chaining, query misses, pending-command
cancellation, target motion, release, selected-body destruction, complete
capacity-sized asynchronous result delivery, lifecycle reconciliation, and
generational slot reuse. Final physics state is validated through the
canonical `GpuBodyObservation`.

It does not construct a surface, texture, render pipeline, camera, application
tool, or presentation object. Its bounded readbacks are proof only and do not
advance the world.

```sh
cargo run -p physbox-external-consumer
```

Success prints `PHYSBOX_EXTERNAL_CONSUMER_SUCCESS` with bounded body/step
counts, constrained motion, and the reused body generation.
