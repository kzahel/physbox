# Platform architecture

PhysBox 5 has one shared engine. Desktop, web, Android, and headless are host
adapters, not alternate implementations.

`physbox-engine` owns fixed stepping, the
[slow-time overload policy](topics/simulation-time-and-overload.md), GPU state,
compute/render pipelines, shaders, input semantics, scenarios, reports, and
scene contracts.
`physbox-launch` owns every option name/default/parser and host projection.
The versioned product physics configuration follows
[physics configuration and solver profiles](topics/physics-configuration-and-solver-profiles.md):
launch owns its external names/defaults, while the engine owns validation,
application, atomic world rebuild, pacing, effective values, and reporting.
`physbox-shell` translates raw `winit` lifecycle and pointer events into the
neutral frame contract. Apps own only their event loop, target/surface,
argument transport, logging, packaging, and presentation.

Interactive frame markers default to milestone-only emission: the first frame
and each distinct scenario completion or restart are reported once. Desktop
and Android may explicitly request `--frame-markers all` for live validation
probes or `--frame-markers off` for silent frame reporting. Continuous markers
are never the product default; maintained Android automation opts into them
only while it needs current held-grab observations.

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

The authorized S4 workbench adds an engine-rendered shared toolbar, scene menu,
glyph diagnostics, and camera. One empty-world touch is inert; two touches pan
and pinch, right mouse drag pans, wheel zooms around its cursor, and primary
input targets UI or the active world tool. The engine owns gesture arbitration,
screen/world projection, GPU body-hit requests, and bounded construction
commands. Adapters may forward raw pointer, touch, wheel, resize, and lifecycle
facts, but may not own menu contents, scene-selection meaning, camera state,
tool behavior, debug content, or per-host control defaults. Camera, tool, and
ordinary menu state are presentation/input-projection state only. A physics-
configuration action is distinct and explicit: the shared engine validates it
and atomically rebuilds/restarts the one world. No adapter or DOM state may
alter physics, fixed-step admission, or scenario outcomes implicitly.

S4.3 live grab input is a body-free begin/target/end stream. The GPU selects
and retains the generational body-local anchor; the CPU knows only pointer
lifecycle and target positions. While that pointer is active, one shared
compute presentation pass converts current GPU grab/pose state into a bounded
12-vertex tether buffer for the same renderer. This GPU-to-GPU bridge is not a
diagnostic readback, does not gate the next frame, and is owned identically by
surface and offscreen hosts.

Interactive hosts forward elapsed wall time but never catch up locally. The
shared clock executes zero or one selected fixed outer step per interactive
engine frame and permanently slips excess whole-step wall time, so the
simulated world slows under load. The fixed value is configuration, but it
cannot change inside a running step and remains engine-owned. Explicit
headless validation may request a declared number of steps independent of wall
time; that is an engine-owned execution mode rather than a platform scheduler.

No app crate may contain WGSL, pipeline/pass encoding, engine settings,
physics structures, scenario expectations, or numeric scene behavior.
