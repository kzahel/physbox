# Platform architecture

PhysBox 5 has one shared engine. Desktop, web, Android, and headless are host
adapters, not alternate implementations.

`physbox-engine` owns fixed stepping, the
[slow-time overload policy](topics/simulation-time-and-overload.md), GPU state,
compute/render pipelines, shaders, input semantics, scenarios, reports, and
scene contracts.
`physbox-launch` owns every option name/default/parser and host projection.
`physbox-shell` translates raw `winit` lifecycle and pointer events into the
neutral frame contract. Apps own only their event loop, target/surface,
argument transport, logging, packaging, and presentation.

```text
host lifecycle + raw events
          |
          v
FrameInput --------> EngineHost::render_frame --------> FrameReport
                         |                  |
                         v                  v
                  shared compute       shared render
                         \__________________/
                              GPU state
```

Surface and offscreen renderers construct different frame targets around that
same boundary. The browser DOM is a fullscreen canvas plus a startup-fatal
message only. Cross-host validation scenarios are Rust-owned; Playwright and
ADB may prove that one raw event crosses the adapter and then consume the
shared result.

Interactive hosts forward elapsed wall time but never catch up locally. The
shared clock executes zero or one fixed outer step per interactive engine
frame and permanently slips excess whole-step wall time, so the simulated world
slows under load. Explicit headless validation may request a declared number of
steps independent of wall time; that is an engine-owned execution mode rather
than a platform scheduler.

No app crate may contain WGSL, pipeline/pass encoding, engine settings,
physics structures, scenario expectations, or numeric scene behavior.
