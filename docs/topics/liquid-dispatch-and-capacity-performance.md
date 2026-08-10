# Liquid dispatch and capacity performance

Status: active diagnosed performance issue and measured optimization program,
updated 2026-08-10. Physical Pixel
7a evidence identifies a reserved-body-capacity scheduling cliff as the
primary cause of the reported mobile liquid slowdown. The 92-dispatch particle
graph is a separate material cost at the ordinary 64-body tier. No remediation
for either performance issue has yet been selected or implemented; the
two-target contribution change below is a correctness-capacity fix.

Governing performance contract:
[performance and scale](performance-and-scale.md).
Coupled behavior and architecture:
[coupled liquid/rigid constraint surface](coupled-liquid-rigid-constraint-surface.md).
Unified product direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Pacing:
[simulation time and overload](simulation-time-and-overload.md).
Current implementation workstream:
[S43 liquid scale benchmark and optimization](../tactical/043-liquid-scale-benchmark-and-optimization.md),
following the implemented-preview
[S31 coupled liquid sandbox](../tactical/031-coupled-liquid-sandbox.md).

S43 activation, 2026-08-10: human direction authorizes a benchmark-first
optimization program around raw water plus 20- and 100-dynamic-solid scenes.
The two solid tiers deliberately bracket the current 64-slot scheduling
boundary: the 20-solid scene is expected to use a 32-slot world, while 100
dynamics plus fixed boundaries requires approximately 128 slots. The initial
target hierarchy is 20+8K as the ordinary gate, 20+16K as a strong demo,
100+8K as the headline, 100+16K as stretch, and 32K as a capacity frontier
unless a complete qualified row actually sustains its declared rate.

The work first creates one shared physics-only and full-frame measurement
surface across native, Browser WebGPU, and standalone Android-native Vulkan.
Only then may it select among live-work rigid routing, fewer compute passes,
active-particle indirect execution, spatial particle/body candidates,
bounded contribution accumulation, or conservative grid reuse. The fixed
delta, particle substeps/iterations, same-step coupling invariant, sleeping
default, and no-catch-up policy do not change to make a timing row pass.

## Conclusion

Two independent costs were conflated in the initial mobile observation:

1. A world reserving more than 64 rigid-body slots selects the scalable
   per-color rigid schedule even when only a few bodies are live. On the
   physical Pixel 7a, changing only the reserve from 64 to 256 raised the
   exact-sleep bootstrap graph from 175 to 553 rigid dispatches and reduced
   interactive simulation from 59.43 to 35.20 outer steps per wall second.
   The 1,000-slot row was similar at 561 dispatches and 34.68 steps/s.
2. At the correct 64-body tier, the current liquid implementation still
   declares 92 particle dispatches per outer step. A native GPU timestamp
   sample attributed approximately 10.08 ms to liquid compute and 6.40 ms to
   rigid compute in the ordinary 3,952-particle scene. Direct body, particle,
   and UI rendering together were under 1 ms in that sample.

The first issue explains why a scene showing only a few rigid bodies can run
badly with `BODY 3/1000`: reserved body capacity currently changes dispatch
topology, not just allocation. The second issue explains why reducing the
visible water population does not remove all remaining cost: the graph still
exists, and most particle kernels dispatch over particle capacity rather than
the compact live list.

The 64-body preset remains the correct ordinary default and does not reduce
the independent 8,192-particle reserve. Selecting 64 is a useful immediate
workaround, not closure of either scaling problem.

## Why PhysBox 5 is materially different from PhysBox 4

PhysBox 4 and PhysBox 5 both simulate particles on the GPU, but they do not
offer the same coupling semantics.

| Concern | PhysBox 4 | PhysBox 5 |
| --- | --- | --- |
| Rigid authority | CPU Rapier world | PhysBox GPU rigid world |
| Body state consumed by fluid | CPU rigid snapshots uploaded before particle work | Current GPU body state already resident |
| Fluid reaction to bodies | Quantized integer impulse accumulators copied through an asynchronous 1 KiB readback | GPU contribution records reduced and applied directly to GPU body state |
| Reaction latency | Deliberately one to two steps late | Same outer fixed step |
| Feedback during fluid solve | Later pressure iterations do not observe the CPU body's reaction | Later coupled iterations consume the updated body pose and motion |
| Reaction content | Delayed velocity impulse, drag, and terminal-speed policy | Equal-and-opposite positional pressure/non-penetration, translation, torque, reconstructed velocity, and wake roots |
| Default particle schedule | One 60 Hz particle step with four correction iterations | Two 120 Hz particle substeps inside one 60 Hz outer step, with two pressure iterations each |
| Default grid policy | Fixed-bin clear/bin once per step | Compact clear/count/scan/scatter before every pressure iteration and final estimate |
| Ordinary compute dispatches | Approximately 21 per fixed step, excluding optional tool and render work | 92 particle dispatches per outer step, excluding rigid solving, tools, and render work |

The PhysBox 4 host explicitly takes fluid impulses completed by an earlier GPU
frame, applies drag and impulse to Rapier on the CPU, advances Rapier, uploads
new collider snapshots, and then encodes the next particle step. This is an
efficient split architecture, but it is not one contemporaneous coupled
physics state.

PhysBox 5 deliberately rejected that split. A particle/body contact must move
the particle and body in opposite directions during the current outer step,
include the lever arm and body inertia, update sleeping/wake state without CPU
observation, and expose the changed body to later fluid iterations. This is
the principal reason it needs more synchronization boundaries and body
gathers.

Therefore, “the extra dispatches apply forces to rigid bodies” is directionally
correct but incomplete. The implementation usually applies constraint
corrections rather than conventional accumulated forces, and only part of the
92-dispatch graph exists for rigid response. The rest comes from a more
conservative grid policy, two particle substeps, compact active-state
maintenance, and separate portable kernels.

### Source basis

The comparison uses clean PhysBox 4 revision
`92f745eb85908243c69080a4fa4dba24f05c22cf`. Its relevant paths are:

- `crates/physbox-engine/src/host.rs`: asynchronous reaction consumption,
  CPU rigid advance, and snapshot upload order;
- `crates/physbox-engine/src/solver.rs`: the linear particle schedule and
  default one-grid-build policy;
- `crates/physbox-engine/src/colliders_compute.wgsl`: fixed-point atomic fluid
  reaction accumulation;
- `crates/physbox-engine/src/colliders.rs`: the compact asynchronous reaction
  readback; and
- `crates/physbox-engine/src/rigid_scene.rs`: CPU drag, impulse, torque, and
  terminal-speed application.

The current PhysBox 5 sources are:

- `crates/physbox-engine/src/particle_world.rs`: the 92-dispatch schedule,
  compact grid rebuild, capacity-sized kernels, and tiled body gathers;
- `crates/physbox-engine/src/particle_boundary*.wgsl`: direct
  non-penetration, density, and pressure response;
- `crates/physbox-engine/src/particle_body_reduce.wgsl`: portable tiled
  contribution reduction; and
- `crates/physbox-engine/src/particle_body_gather.wgsl`: body pose/velocity
  application and fluid wake requests.

`npm run references:status` passed on 2026-07-23. The PhysBox 4 sibling had an
empty worktree at the recorded revision.

## Composition of the 92-dispatch liquid graph

The current ordinary liquid scenario uses two particle substeps and two
pressure iterations per substep. The implementation declares:

```text
1 + substeps * (4 + pressure_iterations * 14 + 12) + 3
= 1 + 2 * (4 + 2 * 14 + 12) + 3
= 92 dispatches per outer step
```

The stages are:

| Scope | Dispatches | Work |
| --- | ---: | --- |
| Once per outer step | 4 | Begin-step bookkeeping plus final active-list clear, compact, and indirect-argument finalize |
| Before pressure, per substep | 4 | Particle integration followed by particle/body projection, tiled reduction, and body application |
| Each pressure iteration | 14 | Four-pass compact grid rebuild; particle density; body-boundary density; boundary-pressure producer/reduce/apply; particle correction gather/apply; direct boundary producer/reduce/apply |
| After pressure, per substep | 12 | Four-pass final grid rebuild; final particle and boundary density; velocity reconstruction; body-surface velocity producer/reduce/apply; viscosity gather/apply |

The body-coupling part is substantial. Each substep contains six
producer/reduce/apply triplets:

- direct boundary response after integration;
- boundary-pressure response for each of two pressure iterations;
- direct boundary response after each of two particle corrections; and
- final body-surface velocity response.

Those triplets account for 18 dispatches per substep, or 36 of the 92
dispatches. They exist because portable WebGPU does not provide the
unrestricted floating-point atomics that would let arbitrary particles safely
accumulate translation and torque directly into a body. PhysBox instead emits
bounded records, reduces them in 64-particle tiles, and runs a per-body apply
pass.

That still leaves 56 dispatches not classified as body-reaction triplets. The
largest single part is spatial-grid construction: two pressure iterations
plus one final estimate require three rebuilds per substep. At four
dispatches per compact rebuild and two substeps, grid construction contributes
24 dispatches. PhysBox 4 normally uses one two-dispatch fixed-bin build for the
whole outer step.

PhysBox 4's reuse is cheaper because it accepts a bounded approximation:
individual correction movement is clamped below one cell, so it keeps the old
bins for later iterations and accepts that near-range entrants may be absent
until the next step. Its fixed bins can also overflow a cell. PhysBox 5's
compact count/scan/scatter representation removed that occupancy limit and
qualified the 32K capacity frontier, but the current implementation also
rebuilds much more frequently. Compact representation and rebuild frequency
are separable choices; retaining compact ranges does not require proving that
all six current rebuilds are necessary.

## Why few visible particles can remain expensive

The particle system is allocated when a scenario creates it. Once present,
the graph and its pipelines do not need to be rebuilt merely because live
particle count changes. Dynamic execution can be driven by GPU-written
indirect arguments without any CPU readback, but the current implementation
uses that path only for grid count/scatter and direct rendering.

Integration, density, correction, body-boundary, viscosity, active compaction,
and most other stages use capacity-sized direct dispatches. Inactive slots
return early in WGSL, but the dispatch, synchronization boundary, workgroups,
and slot checks still occur. Consequently:

- erasing particles does not reduce the declared 92 dispatches;
- an 8,192-slot liquid world with few live particles still launches most
  kernels for 8,192 slots; and
- `START` in the HUD is the authored initial population, not an asynchronous
  live-count claim.

The physical test attempted a held-Erase control and improved from 57.53 to
58.15 steps/s with sleeping off, but the interaction path continued to issue
brush commands and the final image still contained substantial water. That
row is directional only. A controlled zero/one/1K/4K-live sweep at fixed
8,192 capacity is still required before assigning a measured empty-world
cost.

## Why reserved body capacity also increases liquid cost

Each body reaction gather creates partials over:

```text
body_capacity * ceil(particle_capacity / 64)
```

The reduction dispatch is two-dimensional over particle tiles and reserved
body slots; the final gather also dispatches over reserved bodies. Raising the
body reserve from 64 to 1,000 therefore increases all 12 body gathers in the
outer liquid step even when only a few bodies are live or receive fluid
contributions.

This is separate from the rigid scheduling cliff. At more than 64 reserved
bodies, `GpuSolverSchedulingPolicyV1::selected_for_body_capacity` also switches
the rigid solver from `WorkgroupPerIsland` to `PerColorIndirect`. Active-work
compaction keeps many buffer traversals proportional to live work, but it does
not make these two capacity-selected dispatch graphs identical.

Earlier S22 evidence showed approximately flat sparse-capacity GPU time on the
named RADV desktop adapter after active-work compaction. The physical Pixel
result does not reverse that correctness or work-compaction result. It exposes
a portable latency dimension that the desktop result did not bound:
hundreds of small dispatches are comparatively expensive on this mobile
Vulkan stack even when their shader work is sparse.

## Physical Pixel 7a evidence

The 2026-07-23 matrix used:

- PhysBox 5 revision `48a7ebf139294efad9afc51c2666db4e1d1d3151`
  with an initially clean worktree;
- ARM64 release Android application with `box2d-comparator` compiled out;
- APK SHA-256
  `63a37bbffdd227298b1f74fe19c6c46b31fd7a8e9510033df71d9ace4a04067e`;
- physical Google Pixel 7a, Android Vulkan, Mali-G710;
- six-second interactive slow-clock observation windows; and
- the selected 60 Hz outer step, four rigid substeps, and ordinary
  3,952-active/8,192-capacity liquid scene.

The device ended at 31.5 degrees Celsius, with no temperature evidence that
thermal throttling controlled the result.

### Interactive pacing

| Scene | Body reserve | Sleep | Rigid dispatches | Steps/s | Effective scale | Slipped wall time |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| Bootstrap | 64 | exact/on | 175 | 59.43 | 0.990 | 0.050 s |
| Bootstrap | 64 | off | 137 | 59.16 | 0.986 | 0.083 s |
| Bootstrap | 256 | exact/on | 553 | 35.20 | 0.587 | 2.517 s |
| Bootstrap | 1,000 | exact/on | 561 | 34.68 | 0.578 | 2.583 s |
| Bootstrap | 1,000 | off | 507 | 37.62 | 0.627 | 2.283 s |
| Liquid | 64 | exact/on | 175 | 52.46 | 0.874 | 0.767 s |
| Liquid | 64 | off | 137 | 57.53 | 0.959 | 0.250 s |
| Liquid | 256 | exact/on | 553 | 23.52 | 0.392 | 3.700 s |
| Liquid | 1,000 | exact/on | 561 | 21.92 | 0.365 | 3.850 s |
| Liquid | 1,000 | off | 507 | 22.27 | 0.371 | 3.800 s |

Exact sleeping is a secondary cost near the 64-slot deadline. Disabling it
raises the liquid row from 52.46 to 57.53 steps/s, but it does not rescue the
256- or 1,000-slot schedule. Sleeping behavior should therefore remain
independently measurable, but it is not the primary capacity-cliff fix.

### Asynchronous GPU-stage samples

These are individual delayed timestamp samples from the displayed diagnostic,
not a P50/P95 distribution:

| Scene | Rigid | Liquid | Body render | Particle render | UI | Total GPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Bootstrap, 64 exact | 10.66 ms | 0 | 0.69 ms | 0.01 ms | 0.33 ms | 11.70 ms |
| Bootstrap, 256 exact | 25.64 ms | 0 | — | — | — | 26.24 ms |
| Bootstrap, 1K exact | 30.11 ms | 0 | 0.38 ms | 0.01 ms | 0.19 ms | 30.69 ms |
| Liquid, 64 exact | 6.40 ms | 10.08 ms | 0.33 ms | 0.44 ms | 0.21 ms | 17.46 ms |
| Liquid, 1K exact | 28.50 ms | 17.45 ms | 0.37 ms | 0.50 ms | 0.22 ms | 47.03 ms |

This rules out direct particle rendering as the principal native bottleneck in
the captured configuration. It does not rule out fill-rate cost at larger
viewports, smooth rendering, or other devices.

### Browser WebGPU control

The same physical device loaded the deployed shared browser host:

| Liquid scene | Body reserve | Effective simulation scale | Total GPU |
| --- | ---: | ---: | ---: |
| 64 exact/on | approximately 1.01 | 14.88 ms |
| 1K exact/on | approximately 0.51 | 28.31 ms |

The browser's total GPU timing agreed with pacing. Its intermediate timestamp
buckets did not: liquid compute appeared near zero while the body-render
bucket absorbed compute work. Per-stage BrowserWebGPU attribution on this
Mali/browser path is therefore not accepted until the timestamp-boundary
implementation is corrected. Total GPU time remains useful.

## Current decisions

Correctness-capacity update, 2026-07-24: a minimized submerged-circle failure
required two generation-checked body-contribution records per particle rather
than one. The selected primary/secondary planar layout keeps ordinary primary
reads contiguous and consults the secondary plane only when the primary record
declares it. It retains the complete 92-dispatch graph. At the 8,192-slot
product reserve it raises declared particle allocation from the current
clean-HEAD `889,988` bytes to `1,053,828` bytes (`+163,840`, `+18.4%`).
Same-machine control/candidate/control timing was bimodal but showed no
significant host regression: the later aggregated host P50/P95 was
`13.561/18.865 ms` for control and `12.987/19.273 ms` for the candidate. This
does not remediate or materially worsen the independently diagnosed dispatch
cost, and it creates no new throughput claim.

1. Keep 64 rigid-body slots as the ordinary cross-host default. Particle
   capacity remains independent.
2. Do not weaken or silently delay the same-step equal-and-opposite coupling
   invariant to recover PhysBox 4's dispatch count.
3. Treat the greater-than-64 rigid scheduling cliff as the first optimization
   target for small live worlds.
4. Treat the 92-dispatch particle graph and capacity-sized inactive work as a
   separate optimization program.
5. Do not attribute the problem to direct water rendering, exact sleeping
   alone, or “GPU fluid” generically.
6. Do not claim that PhysBox 4 and PhysBox 5 have equivalent behavior merely
   because they show similar water and bodies. PhysBox 4 is a useful
   performance and visual reference, but its reaction is delayed and
   differently stabilized.

## Candidate optimization order

No candidate is selected merely by this diagnosis. A tactical implementation
should measure the following in order:

1. **Live-work rigid schedule routing.** Select the bounded small-world
   dispatch form from live island/work size, with a fail-closed scalable
   fallback, rather than from reserved body capacity alone. A three-body world
   must not pay the general 1K dispatch floor merely because 1K slots are
   available.
2. **Active/contributing body gathers.** Build or reuse a GPU-resident compact
   list of dynamic bodies that can receive fluid work. Size reduction and
   apply work from active/contributing bodies rather than every reserved body.
3. **Active-particle indirect execution.** Move safe capacity-sized particle
   kernels onto the GPU-written live list/indirect count, and make an empty
   particle world execute zero meaningful work without a CPU count readback.
4. **Grid-rebuild reuse.** Compare the current six compact rebuilds with
   bounded reuse across pressure iterations or substeps. Preserve the compact
   no-cell-cap representation; measure neighbor misses, density error, escape,
   and coupled behavior before accepting fewer rebuilds.
5. **Coupling fusion or accumulation alternative.** Evaluate producer/reduce/
   apply fusion and a bounded fixed-point atomic comparator. Any atomic path
   needs written range, precision, overflow, contention, torque, wake, and
   portability evidence. Same-step body application still requires an apply
   boundary even if contribution accumulation becomes cheaper.
6. **Substep/iteration schedule.** Only after the structural reductions,
   compare one versus two particle substeps or another pressure schedule under
   the existing behavior corpus. A lower-quality setting is not a performance
   fix.
7. **Sleeping overhead.** Optimize exact grouping after the capacity and
   liquid graph costs are isolated. Retain sleeping-on/exact as the reviewed
   default unless a separate behavior/default decision changes it.
8. **Browser timing attribution.** Correct stage boundaries so BrowserWebGPU
   can distinguish liquid compute from body rendering on physical mobile
   hardware.

## Required validation for a fix

A candidate must retain:

- same-step equal-and-opposite particle/body translation and torque;
- current-body feedback during the remaining coupled iterations;
- fluid wake roots and sleeping correctness;
- finite state and explicit fail-closed capacity/overflow behavior;
- the selected compact-grid capacity frontier unless a replacement qualifies;
- one shared portable WebGPU path across native, browser, headless, and
  Android; and
- the interactive no-catch-up pacing contract.

Performance evidence must include:

- physical Pixel 7a 64/256/1K body-reserve rows with the same small live body
  set;
- 0/1/1K/approximately-4K active particles at a fixed 8,192 reserve;
- 64/256/1K body reserves at fixed particle work;
- exact-sleep and explicit sleep-off attribution rows;
- end-to-end physics and full-frame timing with render stages separated;
- dispatches, workgroups, live work, allocation, high-water, and overflow;
- native Android and BrowserWebGPU totals on the same physical device; and
- the coupled scalar, behavior, wake, capacity, and cross-host gates required
  by the governing topics.

PhysBox 4 may be rerun as a latency-different reference. Its 21-dispatch result
is not an acceptance target unless equivalent coupling behavior is first
defined and demonstrated.

## Alternatives not selected

### Restore PhysBox 4's delayed CPU impulse loop

Rejected by the current unified-world direction. It would make CPU Rapier a
product authority, require collider uploads and reaction readback, and remove
current-iteration body feedback. Reopening it is an engine-direction decision,
not a routine performance optimization.

### Force every world onto the 64-body schedule

Not selected. The small-world dispatch form was retained only through its
qualified bound; using it blindly for a dense 1K scene would trade a visible
mobile latency bug for an unqualified scale or watchdog risk. Dynamic routing
needs a safe fallback.

### Disable exact sleeping by default

Not selected. Sleep-off improves the 64-slot liquid row but leaves the larger
capacity rows far below real time and removes reviewed behavior and eventual
awake-work savings. It remains a useful attribution control.

### Optimize particle rendering first

Not selected by this evidence. Direct particle rendering measured about
0.44–0.50 ms while compute dominated. A later smooth-surface renderer must
still carry its own fill-rate and mobile budget.

## Reconsideration triggers

Revisit these decisions if:

- a live-work small-world route cannot preserve the scalable schedule's
  capacity, correctness, or watchdog bounds;
- controlled active-particle sweeps show that capacity-sized kernels are not a
  material cost on physical mobile hardware;
- grid reuse violates neighbor completeness or materially changes accepted
  coupled behavior;
- a bounded portable atomic accumulation path demonstrably preserves range,
  precision, torque, wake, and capacity while reducing complete-step cost;
- direct human comparison concludes that same-step feedback is not worth its
  measured cost and explicitly reopens the unified-world invariant; or
- a physical-device matrix shows the 64 threshold is not the operative cliff
  on another required mobile adapter.

Do not respond to a failed optimization by hiding slow time, adding catch-up,
introducing simulation-dependent CPU readback, making mechanics platform
specific, or reporting a smaller reserve as a solved scale result.
