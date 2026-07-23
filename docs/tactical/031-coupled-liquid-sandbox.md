# S31 — Coupled liquid sandbox

Status: active, approved 2026-07-23. Human review accepts this end-to-end plan
and authorizes continuous implementation plus proportional validation. When no
physical Android device is available, the same shared Android path may be
qualified on a hardware-backed AVD; every AVD run must record that substitution
and shut the emulator down afterward.

Governing topic:
[coupled liquid/rigid constraint surface](../topics/coupled-liquid-rigid-constraint-surface.md).
Authoritative direction:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md).
Performance:
[performance and scale](../topics/performance-and-scale.md).
Pacing:
[simulation time and overload](../topics/simulation-time-and-overload.md).
Platform boundary:
[platform architecture](../platform-architecture.md).
Oracle map:
[local references](../../reference/README.md).

## Goal

Deliver the first useful PhysBox 5 water sandbox end to end: GPU-resident water
particles, bounded neighborhood/density solving, same-step two-way response
with the PhysBox GPU rigid solver, fluid-triggered waking, direct rendering,
water/surface/erase tools, a clear rigid-only Box2D comparison boundary,
native-oracle evidence, and maintained desktop/browser/headless/Android
execution.

The planned completion is deliberately bounded. One water material and one
useful 4K scene must feel coherent before particle-count or tool breadth grows.
Once this plan is approved, its slices may execute continuously without a
manual checkpoint between each passing slice. Stop only for a hard authority
or invariant rejection, an undeclared architecture choice, unavailable
required hardware with no authorized substitute, or the final product/default
review.

## Entry decision and prerequisites

Human direction on 2026-07-23 accepts the current rigid solver and sleeping
foundation as sufficient to begin liquid/UI work while retaining all documented
limitations. This does not pass the default 12-stack quiet gate, establish
comprehensive rigid parity, or establish CPU/GPU crossover.

Before implementation starts:

- [x] Human review approves this exact plan and changes S31 to `active`.
- [x] Start from a clean PhysBox 5 worktree after completed S30.
- [x] Run `npm run references:status` and record the resolved PhysBox 3,
  sidecar, LiquidFun, and Box2D revisions.
- [x] Confirm the PhysBox 4 reference tree is clean at
  `92f745eb85908243c69080a4fa4dba24f05c22cf`.

Entry verification used clean PhysBox 5 base
`ff57b717a2da8bfd6cf88a64764d838670ae794a`. Reference status resolved PhysBox
3 at `b11d73dae7dfe96f2efcba9dc5e3e53d10390867`, its first-party sidecar at
`ee2a63be0a911feeedce8f2972a95972a2b389d4`, LiquidFun at
`7f20402173fd143a3988c921bc384459c6a858f2`, and Box2D v3 at
`3a4f0da8374af61293a03021c9a0b3ebcfe67948`.

Ongoing provenance requirement: every implementation change that copies or
closely ports PhysBox 4 records the exact source file, revision, and applicable
license.

Gate: source/provenance checks pass, the existing ordinary lane is green, and
the approved plan has no unresolved ownership decision.

## Exclusions

- Do not add particles, fluid forces, fluid tools, or a LiquidFun runtime to
  the interactive Box2D shadow.
- Do not import Rapier, `physbox-rigid`, CPU rigid stepping, collider snapshot
  uploads, delayed impulse packets, host-applied feedback, passivity damping,
  terminal-speed clamps, or catch-up scheduling from PhysBox 4.
- Do not add a CPU product fluid backend, platform fallback, host-local
  particle behavior, or simulation-dependent readback.
- Do not add goop, sand, foam, spray, multiphase behavior, temperature,
  arbitrary polygons, capsules, terrain, joints, CCD, undo/redo, or complete
  scene save/load.
- Do not require smooth-surface water rendering for mechanical acceptance.
- Do not claim 32K-at-60, 128K scale, rigid CPU crossover, comprehensive rigid
  parity, or a promoted sleeping default.
- Do not use liquid or sleep-enabled survival to change any retained awake
  rigid verdict.

## Ownership constraints

- `physbox-engine` owns particle state, material passes, coupling, wake roots,
  tools, UI semantics, rendering, shared scenarios, completion, and reports.
- `physbox-contracts` owns versioned neutral particle/material/coupled
  scenario, result, trace, capacity, and oracle schemas.
- `physbox-launch` owns particle-capacity/material option names, defaults,
  bounds, parsing, browser query mappings, and Android JSON argv projection.
- Desktop, browser, headless, and Android remain adapters around
  `EngineHost::render_frame`; no app or authored JavaScript contains particle
  semantics, shader code, numerical defaults, tool policy, or scenario
  expectations.
- The PhysBox GPU world is the only product authority. Box2D is a rigid-only
  review shadow; native PhysBox 3/LiquidFun executables are process-isolated
  oracles.
- Every capacity fails closed. No neighbor, contact, correction,
  body-contribution, command, or diagnostic required for acceptance is silently
  truncated.

## Ordered delivery

### S31.1 — coupled contracts and native oracle foundation

- [x] Add versioned particle state, material, authored-region, tool-command,
  particle/body-contact, capacity/work, trace, metric, and result contracts.
- [x] Extend the shared scenario projection with water regions and coupled
  actions while preserving one authored Rust source.
- [x] Add a scalar executable specification for particle integration, density,
  correction, boundary projection, surface-relative velocity, and one
  equal-and-opposite particle/body contact including rotational effective mass.
- [x] Build a thin process-isolated native PhysBox 3 sidecar adapter against
  its resolved Box2D v3/box2cpp sources.
- [x] Build original LiquidFun/Box2D 2.3 as a separate native secondary
  adapter; do not combine foreign source generations in one process.
- [x] Calibrate the sidecar with a small fresh PhysBox 3 web corpus for
  contained water, displacement, and one dynamic-body interaction.
- [x] Freeze canonical hashes for the first seven coupled sources before GPU
  acceptance uses them.

Contracts/scalar checkpoint: `physbox-contracts::coupled` owns the version-1
water, region, action, state, work, capacity, trace, request/result, analytic
boundary, particle-pair, and equal-and-opposite particle/body contracts.
`physbox-engine::coupled_scenario` authors and freezes the seven source hashes.
Focused contract and engine scenario tests pass; no shader or product runtime
uses these records yet.

PhysBox 3 oracle checkpoint: the exact managed first-party particle branch at
`ef8d9526ae79377da3d610e5e4d3bd227f072cdb` contains sidecar sources unchanged
from their introducing commit
`3884106dd415802d2139d163fca56e65b98645c6`. The new process adapter compiles
those clean sources with Box2D
`3a4f0da8374af61293a03021c9a0b3ebcfe67948` and box2cpp
`45f07785abe6bfeba518fb0256a37dcd050dce32`, preserves fresh request/projection
identity, executes contained-water and displacement sources, rejects unsupported
grab projection deterministically, and emits `PHYSBOX5_ORACLE_SUCCESS`.

Original LiquidFun checkpoint: a second executable compiles the clean
`7f20402173fd143a3988c921bc384459c6a858f2` checkout as static LiquidFun/Box2D
2.3, constructs its own coupled `b2World` and particle system, consumes the
same canonical request/result records, records exact build/runtime settings,
and emits `PHYSBOX5_ORACLE_SUCCESS`. The two native engines share only the
PhysBox-owned projection runner; no foreign source generations or native state
are linked into one process. Focused library and process tests pass for both
executables, including deterministic rejection of the unsupported grab
projection.

PhysBox 3 browser-calibration checkpoint: `npm run
calibrate:physbox3:web` launches the unchanged product through Vite and
Chromium, uses only its engine-rendered toolbar and raw canvas pointer events,
and writes an untracked fresh report plus synchronized screenshots. Fresh run
`physbox3-web-1784805413797` used clean PhysBox 3
`b11d73dae7dfe96f2efcba9dc5e3e53d10390867`, bridge
`ee2a63be0a911feeedce8f2972a95972a2b389d4`, Box2D
`3a4f0da8374af61293a03021c9a0b3ebcfe67948`, and Chromium
149.0.7827.55. The contained, displacement, and dynamic-ball captures each
retained 870 finite render particles and emitted no browser error. Subjective
inspection also records visible side-wall escape in the nominal contained
scene; this is useful calibration behavior, not an acceptance band or a reason
to reproduce the defect.

S31.1 is complete. Both process-isolated native lanes, the scalar contract,
frozen source hashes, and the small product-browser calibration corpus now
satisfy the slice gate. S31.2 is the active delivery slice.

Gate: both native processes consume the shared projection or reject unsupported
features deterministically, emit fresh canonical results with exact source and
build provenance, and print `PHYSBOX5_ORACLE_SUCCESS`. Scalar analytic and
metamorphic tests pass for integration, kernel support, finite density,
equal-and-opposite linear response, and correctly signed torque.

### S31.2 — GPU particle substrate and direct water

- [x] Add generation-safe GPU particle slots, active flags/list, bounded
  authored-region initialization, reset, spawn, erase, kill, and reuse.
- [x] Begin with one water material and an approximately 4,096-particle authored
  scene under an explicit 32K particle-slot capacity.
- [x] Port or independently implement integration, neighborhood construction,
  density/lambda estimation, bounded PBF/XPBD correction, final density
  observation, velocity reconstruction, and XSPH-style smoothing.
- [x] Add fixed circle/rectangle boundary projection using the PhysBox body
  representation; capsules remain unsupported.
- [x] Add explicit grid/neighborhood, density, correction, command, and escaped
  particle diagnostics with fail-closed overflow.
- [ ] Render active particles directly from GPU state in the existing shared
  render frame; add density/contact/debug views without HTML or platform UI.
- [ ] Compare the bounded GPU passes with the scalar specification and retained
  PhysBox 4 water-only numerical/capture evidence.

Temporary boundary: dynamic bodies may be treated as fixed only inside this
slice. That one-way state is an implementation checkpoint, never an accepted
coupled product or a default scene.

GPU-substrate checkpoint: the shared body world now optionally owns bounded
particle SoA storage, active compaction, a fixed-capacity spatial grid,
density/correction/velocity passes, analytic circle/rectangle projection,
generation-safe deterministic commands and reset, and direct indirect
rendering. The product sandbox authors 3,952 water particles in 32,768 slots.
A bounded final headless observer reports finite state, exact active-count
agreement, neighborhood/correction/contact work, escapes, kills, command
drops, and overflow without participating in frame advancement. Fresh
development run `1784807683252532500-2398212` completed 600 steps on RADV
GFX1150 with all 3,952 particles retained, zero overflow/escape/kill/drop,
6,432,680 neighbor checks, 230,722 accepted neighbors, and 1,574 fixed-body
contacts in its final observation window. Browser WebGPU run
`4f3871b3-1e7e-439a-b5a3-4d878ace0ca6` completed the same launch scenario,
direct-rendered the liquid, and exercised a Box2D frame in which the fluid was
absent while the PhysBox world kept advancing.

This implementation is independent PhysBox 5 code. PhysBox 4 files
`crates/physbox-engine/src/particles.rs`,
`crates/physbox-engine/src/pbf_correction.wgsl`,
`crates/physbox-engine/src/particles_render.wgsl`,
`crates/physbox-engine/src/particle_tools.wgsl`, and
`docs/topics/pbf-density.md` were consulted as source references at clean
revision `92f745eb85908243c69080a4fa4dba24f05c22cf`; no PhysBox 4 runtime,
readback coupling, CPU rigid backend, or source file was copied into the
product.

Gate: reset/spawn/erase/reuse replay exactly; fixed-boundary and water-only
metrics remain inside frozen bands; 4K and 32K-capacity rows are finite,
capacity-clean, and direct-rendered on native and Browser WebGPU; ordinary
interactive frames map no particle state.

### S31.3 — same-step PhysBox rigid/liquid coupling

- [ ] Evaluate the two predeclared pass schedules and select from scalar
  invariants, coupled behavior, cost, and portability; do not add a third
  schedule without a measured blocking failure.
- [ ] Make every fluid iteration consume current PhysBox body pose, motion,
  inverse mass, inverse inertia, shape, and surface velocity.
- [x] Include fixed and dynamic boundaries in density/pressure and
  non-penetration work rather than adding reaction only after the final fluid
  projection.
- [x] Emit bounded equal-and-opposite particle/body contribution records,
  group them on the GPU, and apply body translation plus off-center torque
  inside the same outer fixed step.
- [x] Feed updated body motion/pose into the remaining declared coupled
  iterations so response is not a one-step-late boundary snapshot.
- [ ] Create generation-checked fluid wake roots before required body response;
  qualify sleeping off plus local-4/local-8/exact explicitly enabled modes.
- [ ] Preserve rigid contact persistence, grab, construction, reset, erase,
  sleeping, and fail-closed overflow across particle coupling.
- [ ] Add a dependency-graph test proving no particle/body result copy reaches
  CPU code before the next interactive step is encoded.

Gate: the displacement, float/sink, off-center torque, dam-break impact,
grabbed-boundary, and fluid-wake sources pass scalar invariants and justified
native-sidecar bands; a light body neither trampolines nor pins; no host
feedback, delayed packet, drag clamp, or simulation-dependent readback exists.

Coupling-foundation checkpoint: the `rigid-then-coupled-batch` product sandbox
now emits one bounded positional/contact record per particle, reduces records
in stable 64-particle tiles on the GPU, and applies equal-and-opposite body
translation, torque, and surface-relative normal velocity response during the
same outer step. Each later particle iteration reads the updated PhysBox pose;
contact generations are checked against current live body metadata, and a
nonzero response writes the existing GPU fluid-wake cause. The scalar/GPU
centered-contact test bounds linear-momentum residual to `2e-4` with no torque.

The first serial body gather was rejected after a 600-step Vulkan validation
command buffer triggered the driver watchdog. The retained tiled reduction
completed the same 600-step exact run as
`1784809833565460211-2455396`: 3,952/3,952 particles remained active, all
state was finite, overflow/escape/kill/drop stayed zero, 274 final-window
body-contribution edges produced six fluid wake roots, and no interactive
readback was introduced.

Boundary-pressure checkpoint: each pressure iteration now adds current fixed
and dynamic body surfaces as mirrored boundary density samples, recomputes
particle pressure, and emits an effective-mass positional reaction before
ordinary non-penetration. The reaction shares displacement across particle
translation, body translation, and body rotation; two dynamic targets for one
particle fail closed through the existing overflow bit. Scalar and GPU tests
cover ghost-distance weight, equal-and-opposite response, non-penetrating
pressure transfer, centered momentum residual at `2e-4`, and the overflow
fixture. Fresh 600-step run `1784810660622129649-2472207` completed with
3,952/3,952 finite particles, zero capacity failures, 1,722 final-window
boundary-density contacts, 108 boundary-pressure edges, and the mass-0.5
review ball resting at the waterline instead of pinning to the floor or
trampolining out.

These remain foundation checkpoints, not S31.3 acceptance: pass-schedule
review, sleeping-mode qualification, the complete coupled corpus, preserved
interaction/reset behavior, and host dependency proof remain open.

### S31.4 — water, surface, erase, and comparison UI

- [x] Add a shared engine-owned Water tool using the existing touch-first
  press/hold/cancel policy; emission is bounded by admitted simulated steps and
  cannot accumulate a catch-up burst.
- [x] Extend Erase to bounded particle removal while retaining deliberate body
  erase and generation-safe cleanup.
- [x] Add PhysBox 4-inspired drag-preview-release `Surface` authoring backed by
  a thin fixed PhysBox rectangle, preserving two-touch cancellation and camera
  ownership.
- [x] Add engine-rendered particle capacity/work/overflow diagnostics and
  legible tool/material state.
- [x] Hide water and fluid-specific tools while Box2D is visible, hide PhysBox
  water from that renderer, and label the view `BOX2D RIGID ONLY` /
  `FLUID HIDDEN`.
- [x] Keep the authoritative PhysBox rigid/liquid world advancing while hidden
  and continue mirroring only stable-ID rigid actions to Box2D.
- [x] Prove the comparator-off build contains no Box2D runtime while retaining
  the complete PhysBox liquid product.
- [x] Refresh body/particle/UI palette, outlines, and glyphs through the shared
  renderer without changing mechanics or introducing DOM product chrome.

Interaction checkpoint: the shared workbench now exposes Water and Surface
only for the PhysBox liquid sandbox. Water tap/hold writes deterministic
material-0 emission commands into the bounded per-step GPU particle command
range; zero-step frames retain cadence without accumulating a burst. Erase
commits the ordinary generation-safe body intent plus a bounded particle erase
only while PhysBox is visible. The Box2D view therefore cannot mutate hidden
particle storage.

`Surface` uses construction contract v3: a validated drag start/end becomes
one fixed, oriented GPU rectangle with `0.04` world-unit half-thickness.
Preview vertices are written into the existing shared world-space interaction
render path; release commits once, while tap, cancel, second-touch ownership,
and the Box2D view are inert. The continuously advancing comparison shadow
filters this PhysBox-only intent in both visible directions.

The engine-rendered liquid diagnostics now identify initial/capacity particle
counts, declared work per step, fail-closed declared overflow state, no
interactive readback, water material, and current tool. Box2D renders the
explicit `BOX2D RIGID ONLY` and `FLUID HIDDEN` labels. Focused contract, input,
preview, GPU-allocation, and comparator-off checks pass; the existing
continuously-aligned shadow test continues to cover hidden PhysBox
advancement. The release comparator-off package contains no comparator
dependency, path, bridge marker, revision marker, or runtime marker, while its
offscreen liquid frame retains nonzero particles, the declared 32,768-slot /
80-dispatch substrate, same-step rigid coupling, and the no-readback policy.
S31.4 is complete. Clean Browser WebGPU run
`20ef27c4-7e56-44e5-a086-38a02843ca9d` advanced 623 interactive steps,
executed Water, particle Erase, and Surface, retained an aligned rigid-only
Box2D shadow, and reported no overflow or frame-advancement readback.
`npm run test:web:lab` passed as
`e2c8898c-ca89-4378-97ec-fcb1605be1e1`; direct comparator tests and the
feature-off release inspection also pass.

No physical Android device was available. Per the approved substitution,
clean revision `671fff553a4d663e02d78da102ed775c40adc023` passed the ARM64
Android check and release APK build, then run `1784812837-2508497` executed the
same liquid interaction on hardware-backed `jstorrent-tablet`. The Android
report recorded Water and Erase commands plus a placed Surface, finite passing
scene state, and a complete 2560x1600 capture. The AVD used host Vulkan adapter
`AMD Radeon 890M Graphics (RADV GFX1150)` and emitted the mandatory successful
shutdown marker.

The midpoint/angle/fixed-box interaction was independently implemented after
consulting PhysBox 4 `input.rs`, `scene.rs`, and `surface_preview.wgsl` at clean
revision `92f745eb85908243c69080a4fa4dba24f05c22cf`. No PhysBox 4 runtime,
backend, source file, or readback path was copied into PhysBox 5.

Presentation refresh: shared WGSL now renders water with a softer
coverage-blended blue/cyan body and compact highlight, while an active Surface
preview uses a distinct green interaction color before becoming the ordinary
fixed-body style. The existing R8 engine glyph atlas and selected-tool
highlight remain the one UI typography/palette path on every host. PhysBox 4
`particles_render.wgsl` was consulted at the same clean revision for visual
calibration; the PhysBox 5 shader remains an independent implementation.

Gate: shared input tests cover mouse/touch cancellation, held emission, erase,
surface preview/commit, engine switching, hidden fluid tools, and return to the
continuously advanced PhysBox world. `npm run test:web:lab`, direct comparator
tests, feature-on/off builds, browser boundary negatives, and synchronized
captures pass.

### S31.5 — coupled behavior and subjective corpus

- [ ] Run contained-settling, displacement, three-density float/sink,
  off-center torque, dam-break, grabbed-boundary, and sleeping-wake sources
  through the native sidecar and PhysBox GPU result contracts.
- [ ] Run original LiquidFun for every supported projection and retain its
  disagreements as secondary evidence rather than majority voting.
- [ ] Record particle population, density error, fluid RMS motion,
  displacement, immersion, particle/body contacts, body pose/motion, torque,
  wake transitions, momentum/energy exchange, overflow, and exact replay.
- [ ] Minimize the first failure in each metric family and promote its source
  to the permanent corpus.
- [ ] Produce synchronized direct/debug captures and one fresh PhysBox 3 web
  calibration review.
- [ ] Conduct a human feel review of pouring, floating, grab-through-water,
  wave impact, and container construction without changing physics defaults
  inside the review slice.

Gate: every accepted source has fresh provenance, declared authority support,
justified bands, exact configuration identity, finite/capacity-clean PhysBox
state, and visually coherent behavior. A hard sidecar or invariant rejection
stops later performance/default promotion while retaining the failure artifact.

### S31.6 — capacity, performance, and cross-host qualification

- [ ] Extend the performance work vector with active particles, cell/neighborhood
  work, density/correction work, particle/body contacts, body contributions,
  wake roots, render work, allocation/high-water, and every overflow.
- [ ] Measure 1K/4K/16K/32K water-only and applicable coupled rows on a named
  native adapter, stopping a family at its explicit capacity or time bound.
- [ ] Compare the fixed-bin baseline with compact sorted cell records/ranges if
  overflow, occupancy amplification, memory, or stage dominance fires the
  topic's trigger.
- [ ] Preserve direct rendering as the acceptance presentation and measure any
  optional reduced-resolution smooth-water renderer separately.
- [x] Run the ordinary Browser WebGPU product scene and report presentation
  cadence, physics time, effective scale, and slip without labeling slow time
  as sustained 60 Hz.
- [x] Run `npm run test:fast`, warnings-denied production clippy, focused
  particle/coupling tests, native oracle tests, and fresh headless coupled
  artifacts.
- [x] Run the ARM64 Android check, release APK, and the actual shared liquid
  interaction on a physical device when one is available. Human review has
  already authorized a hardware-backed AVD substitute whenever no device is
  available; record the substitution, prove the shared Android path and
  pixels, and shut the emulator down after every run.
- [ ] Prove performance artifacts compile the Box2D comparator out or record
  its runtime absence.

Gate: the 4K useful scene is correct and interactive on Browser WebGPU; a
physical Android device, or the explicitly authorized hardware-backed AVD when
no device is available, executes the same mechanics and pixels; accepted
capacities have complete allocation/high-water records and zero overflow; and
no performance claim includes comparator work or unqualified behavior.

### S31.7 — final disposition and product handoff

- [ ] Update the capability ledger, authoritative direction, performance
  record, README, vision summary, topic index, and this tactical with measured
  implementation state and retained failures.
- [ ] Record whether the 4K water scene, Water/Surface tools, and direct visual
  style become ordinary product defaults or remain explicit preview choices.
- [ ] Record whether a smooth-water renderer, sorted-cell replacement,
  multi-material follow-up, authored save/load, or property editing has a
  measured next-work trigger; do not open all of them automatically.
- [ ] Preserve the eight-quality/14-structural rigid wording, default-off
  sleeping disposition, rigid-only Box2D boundary, and missing CPU/GPU
  crossover unless separate evidence changes them.
- [ ] Return synchronized browser and Android captures plus the complete
  coupled evidence package to human review.

Gate: a human review accepts or rejects the bounded coupled sandbox and makes
every product-default decision explicitly. Deployment or push remains a
separate requested action and must use the exact-pushed-SHA production path.

## Required validation by changed boundary

Every implementation slice runs its directly affected package/target tests.
The proportional minimums are:

- contracts, shared Rust, shaders, passes, tools, or renderer:
  `npm run test:fast` plus production clippy with warnings denied;
- simulation, scenario, report, or offscreen rendering: one fresh headless
  coupled artifact;
- solver/configuration/menu behavior: `npm run test:web:lab`;
- Box2D comparison/UI boundary: comparator crate tests, default feature on,
  explicit feature off, web lab, ARM64, release APK, and Android interaction;
- physics defaults, layouts, pass order, visible cross-host behavior:
  ARM64, release APK, and physical Android validation when available, otherwise
  the already-authorized hardware-backed AVD lane with mandatory shutdown; and
- native sidecar, LiquidFun, or evidence tools: directly affected binary,
  library, integration, and fresh-evidence commands by package/target name.

Do not add `--workspace` to the ordinary lane or run historical exhaustive
matrices merely to repeat unrelated coverage. Long particle/count matrices
belong to S31.5/S31.6 acceptance, not every edit.

## Clean-boundary policy

Each admitted implementation slice must:

1. begin from a clean recorded source state;
2. keep unrelated user changes untouched;
3. update its tactical checkboxes/status in the same change that makes them
   true;
4. commit the passing implementation boundary before producing retained clean
   evidence when commit authorization has been given; and
5. stop on a hard gate before later expensive host/performance lanes.

The approved plan authorizes only the implementation and validation described
here. It does not authorize pushing, deploying, changing unrelated defaults,
or expanding into deferred breadth.

## Completion condition

S31 completes only when one water material is GPU-resident and directly
rendered; the shared Water, Surface, Erase, placement, grab, and camera
interactions work coherently; particle/body contacts apply equal-and-opposite
translation and torque within the same GPU fixed step; fluid wakes the selected
PhysBox sleep policy without CPU mediation; the Box2D shadow remains visibly
rigid-only; native sidecar, secondary LiquidFun, scalar, capacity, native,
browser, headless, and physical-or-authorized-AVD Android gates pass; the
useful 4K scene has honest
performance evidence; and human review records the final product/default
disposition.
