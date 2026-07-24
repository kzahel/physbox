# physbox-render

`physbox-render` is the optional, non-published renderer for
`physbox-sim::RenderView`. It creates body, grab, particle, and optional
PhysBox-background pipelines for a caller-selected target format. The caller
owns the WGPU device and queue, texture or OpenXR image, encoder, submission,
view transform, load/clear choice, composition order, and presentation.

```toml
[dependencies]
physbox-render = { path = "../physbox5/crates/physbox-render" }
physbox-sim = { path = "../physbox5/crates/physbox-sim" }
```

Construct `PhysicsRenderer` after the world has been stepped once, using the
world's same-device `RenderView`. `PhysicsRenderer::encode` writes uniforms
through the caller's queue and records render work into the caller's encoder.
It does not create a surface, submit, present, drive simulation, or perform a
mechanical readback. `Background::Load` preserves an existing composition;
`Transparent` is suitable for a physics plane composed by an XR host.

