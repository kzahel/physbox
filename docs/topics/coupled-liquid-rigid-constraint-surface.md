# Coupled liquid/rigid constraint surface

Status: decided entry direction, 2026-07-23; implementation is active under
the approved [S31](../tactical/031-coupled-liquid-sandbox.md) plan.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Performance and capacity:
[performance and scale](performance-and-scale.md).
Pacing:
[simulation time and overload](simulation-time-and-overload.md).
Current product boundary:
[product capability status](product-capability-status.md).
Sleeping and waking:
[GPU sleeping and waking](gpu-sleeping-and-waking.md).
Rigid-only comparison:
[interactive Box2D comparison](interactive-box2d-comparison.md).
Reference and oracle map:
[local references](../../reference/README.md).

## Decision

PhysBox 5 is ready to begin a bounded liquid and sandbox phase on top of the
current GPU rigid foundation. This is a product-priority decision, not a
retroactive claim that every P5.4 rigid-parity or performance gate passed.

The accepted entry baseline is:

- one authoritative GPU-resident PhysBox body world on every maintained host;
- implemented circle/rectangle collision, persistence, response, construction,
  grab, scalable SAP, active-work compaction, and optional GPU sleeping;
- the selected soft-coupled 60/4/i1 default with an honest eight-box awake
  quality claim and demonstrated 14-box structural survival;
- the retained 12/14 quietness failures, 16-stack loss, missing comprehensive
  rigid parity, and missing correctness-qualified CPU/GPU crossover; and
- sleeping off plus local-4 grouping as compatibility defaults, with local-8
  and exact grouping available for explicit review.

Those limitations remain open rigid debt. Liquid must not hide, rename, or
weaken them. They no longer block starting the next material and interaction
work because the current foundation is useful and sufficiently understood for
the product to learn more from coupled behavior than from another unbounded
rigid-solver research loop.

The first new material is water. It exists only in the authoritative PhysBox
GPU world and interacts only with the PhysBox GPU rigid solver. The
default-enabled Box2D shadow remains a rigid-only comparison feature; it is not
a second product direction and receives no particle state, fluid contacts,
fluid forces, or fluid-tool actions.

## Product outcome for the first phase

The first accepted coupled sandbox should let a person:

- pour or paint water into a visible container;
- erase particles and build a fixed floor, wall, or ramp;
- place, grab, drop, and throw PhysBox circles and rectangles through water;
- observe displacement, floating or sinking, off-center torque, wave impact,
  and fluid-triggered waking;
- switch to the rigid-only Box2D review view without suggesting that Box2D is
  simulating or interacting with the liquid; and
- run the same behavior through desktop, browser, headless, and Android
  adapters with one engine-owned scenario, tool, render, and report contract.

Coherence and interaction quality matter before headline particle count.
A useful, responsive 4K water scene with correct same-step response is more
valuable than a 128K scene with delayed or one-way coupling.

## One authoritative state and step graph

The CPU may own authored particle regions, material/tool choices, bounded
commands, save data, and delayed compact diagnostics. Particle positions,
velocities, neighborhood data, density/constraint state, particle/body
contacts, and every value that advances body or particle mechanics remain on
the GPU.

The target dependency graph is:

```text
shared input and authored commands
              |
              v
GPU body + particle command application
              |
              v
body and particle prediction
              |
              +--> rigid SAP / manifolds / persistence
              |
              `--> particle neighborhood construction
                         |
                         v
        bounded coupled rigid/fluid iterations
        - fluid density and pressure
        - current body boundary state
        - particle/body non-penetration
        - equal-and-opposite body translation and torque
                         |
                         v
          velocity reconstruction and wake roots
                         |
                         v
             sleep decision and direct rendering
```

No mapped counter, impulse packet, collider snapshot, convergence result, or
foreign-engine state may be required to encode the next interactive frame.
Bounded validation may map complete results only after its final requested
step.

The complete world follows the selected outer fixed `dt`. Interactive frames
still execute zero or one outer step and permanently slip excess wall time.
Particles, emitters, coupled iterations, wake decisions, and world-space
effects advance in simulated time; they do not independently catch up from
wall time.

## Same-step coupling invariant

A particle/body contact is one coupled constraint over current GPU state. Its
effective mass includes particle translation, body translation, and body
rotation. The response must:

- move or impulse the particle and body in opposite directions during the same
  outer fixed step;
- include the contact lever arm so an off-center load creates correctly
  directed angular response;
- use the body's current surface velocity at the contact point;
- contribute dynamic boundaries to fluid density/pressure iterations rather
  than treating them as unchanged walls until a final projection pass;
- preserve finite state and bounded work under high contact concentration; and
- make every overflow or capacity failure explicit and invalidating.

The exact first pass order and whether a fluid iteration is paired with each
rigid substep are S31 implementation decisions, but the candidate set is
bounded. S31 may compare at most:

1. one coupled fluid iteration after each rigid substep; and
2. one rigid substep batch followed by a bounded coupled fluid batch inside
   the same outer step.

The selected schedule must pass the same scalar invariants and coupled
behavior fixtures. A third schedule requires a measured failure that neither
declared candidate can explain.

Portable accumulation should reuse PhysBox's GPU-owned work construction and
deterministic body gathering. Particle/body contacts emit bounded
equal-and-opposite records, group body contributions on the GPU, and apply
linear and angular response without requiring `atomic<f32>` or CPU reduction.
A bounded fixed-point atomic path may be a diagnostic comparator with written
range/error bounds, but it is not automatically the product path.

### Selected boundary-density representation

The first coupled implementation represents a body surface near a particle as
one mirrored boundary sample. Its kernel distance is twice the nonnegative
analytic signed distance to the circle or rectangle, so a particle one radius
from a surface sees the same weight as a same-material particle mirrored
across that surface. Fixed and dynamic samples contribute to density and near
density before each pressure correction.

Boundary pressure then uses the scalar material pressure at that weight and
the same effective-mass terms as direct particle/body contact: particle
translation, body translation, and the contact lever arm against body inverse
inertia. The resulting bounded positional impulse moves the particle and body
in opposite directions before later iterations consume the new pose. One
particle may target one dynamic body per pass; a second dynamic target records
an invalidating overflow rather than dropping an unreported reaction.

This is independently implemented from the scalar contract. Original
LiquidFun's boundary contact weight and pressure transfer were consulted as
algorithm references at pinned revision
`7f20402173fd143a3988c921bc384459c6a858f2`; its CPU contact buffers,
iteration order, and body application path were not copied. PhysBox 4's clean
revision `92f745eb85908243c69080a4fa4dba24f05c22cf` was rechecked only to
confirm that its delayed CPU reaction remains excluded.

## Sleeping and fluid wake behavior

Sleeping bodies remain live fluid boundaries. A qualifying particle/body
contact or accumulated fluid load creates a generation-checked GPU wake root
before the body response must be solved. The existing local-4, local-8, or
exact rigid contact grouping then propagates that root according to the
selected policy.

The first coupled contract requires:

- sleeping-off and pre-first-fluid-contact behavior to retain existing rigid
  results;
- a sleeping body struck or supported by material fluid work to wake without a
  CPU event;
- an ordinary static hydrostatic load not to create a permanent wake/sleep
  cycle;
- erased/reused bodies and particle slots not to inherit stale wake/contact
  state; and
- the awake mask rule to remain intact: fluid or sleep cannot turn an existing
  sleep-off rigid failure into a rigid pass.

Sleeping remains off by default for the first water milestone. S31 qualifies
fluid wake behavior with sleeping explicitly enabled but does not promote the
sleeping default.

## Rigid-only Box2D comparison boundary

The interactive Box2D shadow does not gain a particle system.

When Box2D is visible:

- only the shared rigid scenario projection, rigid settings, and stable-ID
  rigid actions reach the shadow;
- PhysBox water is not rendered over Box2D bodies;
- water, particle erase, and other fluid-specific tools are hidden or
  unavailable;
- the engine-rendered HUD states that this is a rigid-only comparison and that
  PhysBox fluid is hidden; and
- no Box2D transform, contact, sleep state, or response feeds PhysBox fluid.

The authoritative PhysBox world may continue advancing offscreen, including
its particles, while the comparator is visible. Mirrored rigid actions still
reach both rigid worlds, so returning to PhysBox reveals its current coherent
rigid/liquid state rather than a state copied from Box2D.

The process-isolated Box2D executable remains the rigid authority. Coupled
acceptance instead uses the native PhysBox 3 sidecar as primary authority and
original LiquidFun as secondary evidence. Neither is added to the interactive
product.

Comparator-enabled frames remain ineligible for PhysBox performance claims,
whether or not liquid is visible. Performance lanes compile the comparator out
or prove it absent.

## First material, capacity, and neighborhood boundary

The first product material is water only. Goop, sand, foam, spray, surface
tension variants, multiphase behavior, and temperature are deferred until the
water corpus passes.

The initial useful scene targets approximately 4,096 active particles inside
an explicit 32K particle-slot capacity. These are provisional workstream
bounds, not final defaults or a 32K-at-60 claim. Particle capacity is distinct
from body capacity and reports allocation, active count, neighborhood/contact
high-water, overflow, work, and timing separately.

PhysBox 4's fixed 128×80 grid with 128 particle indices per cell is an
inspectable bounded baseline. It may support the first 4K implementation only
when overflow fails closed and remains visible. It is not inherited as the
scalable product answer. Before a larger capacity or scale claim, S31 must
measure whether compact sorted cell records/ranges using the existing
scan/radix infrastructure are required.

Particle slot allocation, erasure, grid records, density scratch, correction
scratch, particle/body contacts, body-contribution edges, and render work all
have explicit fixed or derived capacities. No stage silently truncates a
neighborhood, contact list, or command batch.

## PhysBox 4 reuse and rejection boundary

The planning source snapshot is the clean PhysBox 4 revision
`92f745eb85908243c69080a4fa4dba24f05c22cf`. It is evidence and selective
source material, not a module, scene, or compatibility dependency.

Candidates for selective adaptation include:

- the 24-byte particle state and GPU free-slot lifecycle concepts;
- integration, density/lambda, PBF/XPBD correction, XSPH-style velocity
  smoothing, and water material constants;
- bounded particle brush/erase command behavior;
- fixed-grid construction and diagnostics as the first bounded baseline;
- analytic circle/rectangle particle-boundary math;
- direct particle rendering, material/debug colors, and offscreen capture
  patterns;
- the drag-to-create fixed `Surface` gesture and procedural preview; and
- numerical fixtures, debug views, work counters, and direct-render captures.

Every close source port records the exact source file and revision in the
implementation commit and preserves any applicable license. Prefer adapting
the documented equations and tests to PhysBox 5 contracts over copying whole
modules.

The following do not transfer:

- `physbox-rigid`, Rapier, CPU rigid stepping, or collider snapshot uploads;
- fixed-step catch-up or independent wall-time material behavior;
- fluid reaction packets, counter-driven mechanical readback, delayed host
  impulse application, contact-scaled drag, terminal-speed clamps, or
  passivity rules built to stabilize that loop;
- PhysBox 4 scene/API compatibility or platform-specific semantics; and
- the assumption that its planned smooth-liquid renderer already exists.

PhysBox 4 implemented direct particle rendering and debug views. Its
reduced-resolution density/thickness surface renderer remained planned work,
so smooth water is a new PhysBox 5 rendering feature rather than an import.

## Tools, UI, and rendering

All semantic interaction remains in shared Rust and all product UI remains
engine-rendered. Browser JavaScript stays a raw input/display leaf.

The first coupled sandbox adds:

- a water brush whose held emission is bounded by admitted simulated steps and
  cannot burst after overload;
- particle erase integrated with the existing deliberate erase policy;
- a drag-preview-release `Surface` tool that authors a thin fixed rectangle for
  floors, walls, ramps, and containers;
- direct particle rendering from GPU state with a coherent water/body/UI
  palette and legible active-tool state; and
- engine-rendered fluid capacity, overflow, work, and pacing diagnostics.

Existing camera, two-touch navigation, body grab, circle/rectangle placement,
menu ownership, and input cancellation semantics remain the base. A visual
refresh may selectively adapt PhysBox 4 glyph, outline, palette, and procedural
render ideas, but it must preserve current mechanics and keep the comparator
label unmistakable.

Reduced-resolution density/thickness water rendering is a later S31 slice only
after direct rendering and mechanics pass. It must be optional presentation,
consume current GPU particle state, preserve a direct/debug mode, and stay
inside the browser/Android frame budget. It cannot feed physics.

Persistent selection/property editing, arbitrary save/load, undo/redo,
freehand terrain, joints, additional rigid shapes, and the full PhysBox 4 tool
catalog remain separate breadth. The coupled scenario/result schema comes
first so later authored documents do not freeze an obsolete particle format.

## Oracle and acceptance corpus

The native PhysBox 3 particle sidecar plus its pinned Box2D v3 source is the
primary coupled-behavior authority. Original LiquidFun/Box2D 2.3 is a separate
secondary native process. A small fresh PhysBox 3 web corpus calibrates the
sidecar and subjective feel; it is not routine CI.

The first canonical sources are:

1. contained water settling against fixed rectangles;
2. a dynamic rectangle displacing a known water volume;
3. matched light/default/heavy bodies that float, settle, or sink;
4. an off-center wave or jet that produces correctly directed torque;
5. a dam break striking a free body and moving boundary;
6. a grabbed body entering, moving through, and leaving the water; and
7. a sleeping body woken by fluid load without a recurring wake cycle.

Each source is authored once in shared Rust and projected independently to the
native sidecar, original LiquidFun where supported, and the PhysBox GPU
runner. An unsupported oracle feature fails with a named projection result
rather than an approximation.

Acceptance records at least:

- particle count and finite state;
- density error and correction work;
- particle/body contact and contribution counts;
- displacement and body immersion;
- body translation, rotation, linear/angular velocity, and wake transitions;
- momentum/energy exchange and equal-and-opposite residual;
- escaped/killed particles and every overflow;
- exact replay within a configuration;
- complete adapter, source, scenario, configuration, and dirty-state
  provenance; and
- synchronized direct/debug captures for human review.

Exact trajectory equality with LiquidFun is not required. Invariants and the
scalar coupled specification own mathematical correctness; the PhysBox 3
sidecar owns the selected behavior bands; original LiquidFun can expose
disagreement but cannot overrule that authority silently.

## Performance and cross-host contract

Coupled work extends the existing work vector with:

- live/active particle slots;
- emitted/erased/dropped commands;
- occupied grid cells or sorted cell records;
- neighbor checks and accepted neighbors;
- fluid constraints and iterations;
- particle/body contacts and body-contribution edges;
- awake bodies reached by fluid;
- density, correction, coupling, and render time;
- allocation/high-water by particle buffer class; and
- effective interactive scale and slipped wall time.

The first performance claim is deliberately modest: a useful default water
scene must remain interactive on the ordinary Browser WebGPU path and execute
correctly on physical Android. Larger particle counts are diagnostic until
correctness-qualified curves exist. Slow-time behavior is reported honestly
and never converted into a sustained-rate claim.

Every physics/layout/pass change runs the ordinary production gates, clippy,
fresh headless coupled evidence, and focused performance probes. Cross-host
mechanics or visible UI changes additionally require Browser WebGPU, ARM64,
release APK, and a physical Android run when one is available. Human review
has explicitly authorized S31 to substitute a hardware-backed AVD whenever no
device is available; the evidence must name the substitution and adapter, use
the shared Android path, and shut the emulator down afterward.

## Deferred breadth

The first coupled milestone does not include:

- multi-material water/goop/sand behavior;
- smooth-surface rendering as a mechanics prerequisite;
- 128K/512K/1M product claims;
- capsules, polygons, chains, terrain, joints, or CCD;
- Box2D particles, fluid forces, or interactive LiquidFun comparison;
- native-only algorithms or required float atomics; or
- a CPU fluid or rigid fallback.

## Reconsideration triggers

Revisit this direction if:

- neither bounded coupled schedule can pass the scalar equal-and-opposite and
  off-center torque fixtures;
- the 4K direct-render scene cannot remain useful on Browser WebGPU or on the
  physical-or-explicitly-authorized-AVD Android lane without changing behavior
  per host;
- the fixed-grid baseline overflows or dominates cost in ordinary water
  scenes, triggering a measured sorted-cell implementation;
- fluid wake roots create repeated sleep cycles or require CPU observation;
- the native sidecar cannot be calibrated against the PhysBox 3 product with
  a documented projection; or
- the Box2D rigid-only view cannot remain unmistakable without materially
  perturbing the authoritative PhysBox frame.

Do not respond by adding delayed readback, a CPU fallback, per-host mechanics,
silent capacity loss, or hidden damping. Return the failing invariant,
schedule, neighborhood representation, or product bound to explicit review.
