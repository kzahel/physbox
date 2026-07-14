# Simulation time and overload

Status: decided default contract, 2026-07-14. Interactive slow-clock admission
is implemented; exact-count headless execution remains the next implementation
slice described below.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Related measurement contract:
[performance and scale](performance-and-scale.md).

## Decision

PhysBox 5 preserves a fixed outer physics delta, normally `1 / 60` simulation
seconds, but does not try to keep simulation time synchronized with wall time
when an interactive host falls behind. The default response to overload is for
the entire simulated world to run in slow motion.

An interactive engine frame may encode at most one outer fixed physics step.
Elapsed wall time can make that step eligible, but cannot create a backlog of
future work. Excess whole-step wall time slips immediately, is reported, and
is never repaid by later catch-up frames.

This is a product choice, not merely a safety cap:

- no interactive frame encodes two or more outer steps to recover wall time;
- no missed step remains as time debt;
- recovery means returning to the requested simulation rate from the current
  state, not racing forward to the wall-clock timeline;
- overload never enlarges the selected fixed delta; and
- solver substeps inside one accepted outer step remain allowed and are not
  catch-up.

The intended failure mode is honest, coherent slow motion. PhysBox prefers
that over a burst of simulation work, stale input, large presentation jumps,
or a catch-up spiral.

## Fixed step does not imply catch-up

These are separate properties:

- **Fixed delta:** every accepted outer step advances simulation time by the
  same `dt`, normally `1 / 60` second.
- **Interactive pacing:** wall time determines when at most one step becomes
  eligible in a rendered engine frame.
- **Effective time scale:** completed simulation seconds divided by active wall
  seconds. It falls below the requested scale when frames are too slow.
- **Catch-up:** encoding multiple outer steps in one interactive frame to repay
  wall-clock debt. PhysBox 5 does not do this by default and initially should
  not expose it as a product mode.
- **Solver substeps:** smaller internal intervals used by Soft Step/TGS or
  another selected solver within one outer fixed step. They are part of solver
  quality and cost, not scheduler catch-up.
- **Explicit stepping:** a validation or benchmark runner intentionally asks
  for a declared number of steps independent of wall time. This is not
  catch-up either.

Keeping these terms distinct prevents an optimization or diagnostic from
silently changing numerical behavior.

## Interactive clock behavior

The shared engine owns the clock behind `EngineHost::render_frame`. Platform
adapters provide elapsed wall time and lifecycle facts; they do not maintain
an accumulator, choose an overload mode, or issue private physics steps.

Conceptually, for requested time scale `s <= 1`, fixed delta `dt`, and active
wall interval `wall_dt`:

```text
accumulator += admitted(wall_dt) * s

if accumulator < dt:
    execute 0 steps
else:
    execute exactly 1 step of size dt
    accumulator -= dt
    slip every remaining whole dt
    retain only the fractional remainder below dt
```

The slipped amount is diagnostic history, not debt. A later fast frame cannot
consume it.

At the default 60 Hz physics rate and requested scale 1:

| Presented engine cadence | Interactive step pattern | Approximate effective simulation scale |
| ---: | --- | ---: |
| 120 Hz | alternating zero and one step | 1.0x |
| 60 Hz | one step per frame | 1.0x |
| 50 Hz | one step per frame, with excess fractional time eventually slipped | 0.83x |
| 30 Hz | one step per frame | 0.5x |
| 15 Hz | one step per frame | 0.25x |
| one 100 ms stall | one step, roughly five steps of wall time slipped | slow for that interval |

The exact rolling scale comes from completed steps and active wall time rather
than an instantaneous reciprocal frame-rate estimate.

A future manual Time Scale control remains a requested ceiling. At 0.5x on a
healthy 60 Hz host, the engine normally alternates zero and one step. Under
load, the effective scale may fall below 0.5x; an automatic policy may never
exceed the user's request in an attempt to catch up.

## Slow motion applies to the world

Everything whose meaning belongs to the simulated world advances from fixed
step count or simulation time:

- rigid and particle integration;
- contact, joint, fluid, and material solvers;
- sleeping and waking timers;
- scripted or kinematic world motion;
- emitters, fuses, projectiles, and machines;
- world-space animation and effects whose behavior interacts with physics; and
- held-tool actions that intentionally produce work per simulation step.

These systems do not consult wall time independently to remain apparently
real-time while physics slows. Otherwise a ten-second wall-clock fuse could
expire after only five simulated seconds, or an emitter could inject twice as
much material per simulated second during 0.5x overload.

Host lifecycle, input collection, pointer sampling, menus, accessibility,
logging, and non-world UI feedback may remain wall-time responsive. Input is
applied at the next actual fixed-step boundary; slipped wall time does not
replay a command several times or create fictional intermediate input. Where a
continuous gesture produces many host samples before the next step, the shared
input policy coalesces or interprets those samples without inventing catch-up
steps.

## Suspend, resume, and long stalls

Suspension is not overload and does not create physics debt. On hidden,
suspended, surface-lost, or resumed transitions, the shared clock rebases its
wall origin and discards the inactive gap. The next interactive frame may
execute at most one ordinary fixed step.

Record lifecycle-discarded wall time separately from active overload slip. A
large elapsed value caused by debugging, a breakpoint, clock discontinuity, or
host scheduling stall is likewise bounded and labeled; it never produces a
multi-step recovery batch.

Pause and manual single-step remain explicit simulation controls:

- paused frames execute zero steps and do not accumulate debt;
- one single-step request advances exactly one fixed `dt`; and
- unpausing resumes from the current state without compensating for paused wall
  time.

## Presentation remains a separate concern

High-refresh presentation may produce zero-step frames. Low or deliberately
reduced presentation cadence naturally lowers the maximum interactive
simulation cadence because the engine will not batch multiple outer steps into
one presentation frame.

Interpolation may later make high-refresh or fractional-step presentation
smoother, but it must not extrapolate the world to a state that has not been
simulated, conceal slipped time, or become an alternate clock. The displayed
world always derives from completed GPU simulation states.

The no-catch-up policy bounds newly encoded work per engine frame but does not
by itself prove that the GPU queue cannot grow. Interactive submission and
surface pacing must also retain a bounded in-flight policy or asynchronous
backlog signal. The engine must not queue future simulated time merely because
the API accepts submissions faster than the GPU completes them.

## Validation and benchmark modes

Interactive pacing and deterministic execution are distinct modes of the same
shared simulation, not separate physics implementations.

### Interactive slow-clock mode

- wall paced;
- zero or one outer fixed step per engine frame;
- excess whole-step wall time slips permanently;
- complete diagnostics report effective scale and slip; and
- used by desktop, browser, Android, and ordinary interactive headless hosts.

### Explicit-step mode

- the shared scenario runner requests an exact step count;
- machine speed does not change simulated time or results;
- a bounded headless runner may encode or submit several intentionally
  requested steps for validation or throughput measurement; and
- there is no wall-time accumulator or claim that the run kept up in real
  time.

This exception is deliberate: an offline request to simulate 600 fixed steps
is authored work, not repayment of interactive wall debt.

Performance benchmarks must not use slow-time pacing to hide insufficient
throughput. Solver-only, collision-only, and raw end-to-end scopes use explicit
steps and report time per step. A full interactive-frame benchmark additionally
reports effective simulation scale and slipped time. A scene running smoothly
at 0.5x is a valid product degradation result, not evidence that it sustains
60 Hz physics.

## Required diagnostics

The shared frame/result schema should expose at least:

- selected fixed-step Hz and `dt`;
- requested time scale;
- completed outer steps this frame, constrained to zero or one interactively;
- cumulative completed steps and simulated seconds;
- admitted active wall time;
- current-window and cumulative overload-slipped wall seconds;
- lifecycle/clamp-discarded wall seconds as a separate value;
- effective simulation seconds per active wall second over a rolling window;
- zero-step and one-step frame counts;
- presented frame cadence and full-frame time; and
- GPU timing or bounded asynchronous backlog evidence where supported.

Use `slipped wall time` or `effective simulation scale`, not an ambiguous
`dropped steps` counter. No physics work existed for the slipped interval, and
the engine will never execute it later.

Fresh artifacts record the pacing mode. Cross-host timing tests cover 120, 60,
50, 30, and 15 Hz input sequences, long stalls, pause/single-step, suspend and
resume, and requested time scales below one. Structural tests assert that an
interactive frame never produces more than one outer step and never retains a
whole-step backlog.

## PhysBox 4 evidence and selected lesson

PhysBox 4's
[performance, catch-up, and load-modes topic](https://github.com/kzahel/physbox4/blob/main/docs/topics/performance-catch-up-modes.md)
recorded the exploratory path to this decision. Its scheduler could encode up
to eight 60 Hz fixed steps in one rendered frame. Dense mobile-web scenes that
looked unstable at 1x became stable when manual Time Scale reduced wall-time
admission. The document identified a feedback loop in which overload caused
more steps to become due, catch-up encoded more complete GPU solver work, and
queue/presentation pressure grew further.

That investigation also exposed the deeper split-world problem. Catch-up did
not create the architectural CPU-rigid/GPU-fluid delay, but it amplified
batching, presentation jumps, packet age, and the difficulty of reasoning
about delayed reactions. PhysBox 5 removes the separate mechanical worlds and
also chooses the simpler overload behavior rather than carrying forward an
eight-step catch-up mechanism.

The old topic explored adaptive presentation profiles, automatic controllers,
quality reduction, and optional strict real-time modes. Those remain evidence,
not inherited requirements. PhysBox 5 initially selects fixed-delta slow time
with no interactive catch-up and measures it before considering more policy.

## Current implementation status

The shared engine clock now admits zero or one interactive step, keeps only a
fractional remainder below one fixed `dt`, records excess whole-step time as
permanent overload slip, and accounts for lifecycle/clamp discard separately.
Deterministic tests cover the required cadence, stall, time-scale, pause,
single-step, and suspend/resume sequences. The 250 ms discontinuity bound is
engine-owned rather than applied by a platform adapter.

Headless capture still feeds nominal wall intervals through that interactive
mode. The next slice adds the distinct exact-count request before accepting
P5.0a performance evidence; it will reuse the same outer-step encoding rather
than adding a platform scheduler.

## Reconsideration triggers

Revisit this decision only with measured product evidence and an explicit
topic update. Relevant triggers include a future networked or externally
synchronized mode, a product interaction that demonstrably requires wall-time
alignment, or a platform whose presentation model cannot support the shared
contract.

Any future real-time catch-up mode must be opt-in, retain a hard work and
in-flight bound, remain outside correctness baselines, report its debt and
bursts explicitly, and prove that it improves the intended experience without
reintroducing stale input, queue spirals, or cross-material timing artifacts.
It must never silently become the default.
