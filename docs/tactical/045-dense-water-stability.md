# S45 — Dense water stability

Status: complete, 2026-11-26. First failures isolated and full-window benchmark check qualified.

## Goal and scope

Locate the first correctness failure with 100 dynamic solids and 16,384 water
particles, compare with 8,192 water, and identify a reproducible next physics
experiment. The shared engine owns scenario construction and the benchmark.
The simulation crate owns reaction storage, grid bounds and particle lifetime.
Use the reviewed S44 density and viscosity shaders at base `62469ab`. Keep the
v2 fixture, three fixed walls, fixed step, standard coupling, substeps and
pressure iterations. Exclude changes to wall height, kill plane, material and
performance targets. Failed rows are ineligible for speed comparisons.

## Ordered work and gates

- [x] Reproduce the sustained run using the uninstrumented benchmark. Record
  failures and retain the 8K control. Use one continuous trajectory for first
  events because independent GPU runs diverge.
- [x] Snapshot particle and body state after every outer step in a diagnostic
  build. Identify the first failing pass, particle, target IDs and occupancy.
  Check the first lost particle's trajectory against the wall and grid bounds.
- [x] Reproduce the two failure mechanisms independently, then run two
  diagnostic eight-slot trials at identical solver settings.
- [x] Qualify the benchmark's full-window particle check against a passing
  row and an actual early failure. Record the outcome and next experiment.

Completion requires a decision on the eight-slot candidate and reproducible
evidence for a solver or fixture investigation. Product solver promotion
requires exact retention, finite state, zero overflow, same-step reaction and
the cross-host gates in the [performance contract](../topics/performance-and-scale.md).

## What failed first

The ordinary full-frame run advanced 16 warmup plus 1,800 measured steps at
960 × 600. Radeon 890M finished 42 particles short, with 10 body-reaction
overflows in the final step. Pixel 7a native Vulkan finished 87 short with
zero overflow in the *final step*. Both reported finite state. Separate
Radeon diagnostic runs sampled all steps, including warmup. Steps are
one-based from world creation.

| Radeon diagnostic run | First reaction overflow | Steps with overflow | First particle loss | Total lost |
| --- | ---: | ---: | ---: | ---: |
| 100 solids + 8,192 water, four slots | none | 0 | none | 0 |
| 100 solids + 16,384 water, four slots A | 99 | 434 | 326 | 113 |
| Same, four slots B | 88 | 20 | 316 | 93 |
| Same, eight slots A | none | 0 | 316 | 60 |
| Same, eight slots B | none | 0 | 325 | 52 |
| 100 solids + 8,192 water, eight slots A and B (each) | none | 0 | none | 0 |

At step 98 of trace A all particles were present and all flags clear. At
step 99 the **boundary pressure** gather for particle 7635 needed five
distinct dynamic targets in one pass (body slots 39, 40, 52, 65 and 78).
The representation holds four. Four overflow events occurred that outer
step while all 16,384 particles remained live. The existing one-particle
four/five-target GPU fixture independently reproduced the boundary.

Particle 16072 was the first loss in trace A. It rose above the tank, crossed
the left wall above its top (wall top y = 7.3), reached y ≈ 12.8 and later
crossed the grid edge x = −16 at (−16.021, −7.433). The kill plane is y = −10.
The step recorded one grid escape and zero reaction overflows. Starting at
step 204 with that particle's observed position and reconstructed velocity,
one water particle and the three fixed walls reproduced the exit at step 326,
within 0.003 units on each axis. This isolates the flight after ejection. The
vertical speed inferred from its final substep displacement went from −12.56
at step 88 to +6.01 at step 96, before the first overflow in this world.
At steps 88, 92 and 96 its nearest rigid surface was 1.87, 1.71 and 2.04
units away, respectively, versus a water support radius of 0.15. These
step-end snapshots localize the reversal but do not identify the pass that
produced it. Trace B first lost a particle beyond the right wall at
(11.96, −10.04), below the kill plane (the escape flag was clear).

## Decision and follow-up

Reject doubling the production reaction allocation. It removes the observed
reaction overflows in two trials and costs another 1.25 MiB of GPU storage
at 16,384 slots. Both trials still lose particles. Long-run counts vary with
GPU neighbor ordering, so the smaller loss totals do not establish an
improvement in retention.

At 100 solids plus 8,192 water, one four-slot Radeon trace was clean for all
1,816 steps. Another Radeon explicit-step row overflowed at step 70 and a
sustained Pixel row at step 66. Both rejected rows ended with all 8,192
particles and clear final flags. At 16,384 water, the updated gate also
rejected a Radeon row at step 108 and a Pixel row at step 98 while their
final validation still passed. This invalidates a general 100+8K correctness
claim for the four-slot solver.

The eight-slot arm at 100 solids plus 8,192 water retained the exact
population with zero overflows in two full Radeon diagnostic windows. A
physical Pixel native full-frame run with the same candidate passed the
whole-window gate as well, with 1,800 measured frames following 16 warmup
steps. Its fenced frame P95 was 96.2 ms at 960 × 600, above the 16.67 ms
objective. This makes 8K a candidate for focused follow-up. It requires
640 KiB of extra per-particle reaction storage at this reserve. A selective
overflow path would need an end-to-end comparison and a same-step momentum
check before promotion.

The benchmark now copies active count and overflow flags into eight bytes of
GPU history per outer step. A single final readback covers warmup and measured
steps and identifies the first bad step even if the final flags cleared.
Interactive advancement uses the existing frame path. Historical artifacts
only certify fields that their benchmark actually sampled. The new history
check passed at 20 solids and 8,192 water on Radeon and Pixel (160 steps each)
and desktop WebGPU (76 steps). The transient-error regression and 605-frame
headless smoke passed. The ordinary fast lane passed in 71 s using one test
thread and the Radeon Vulkan driver after two native test-process crashes
with the default driver selection. Production and affected benchmark clippy
passed with warnings denied.

Next, trace the velocity of particles in the upward jet through integration,
boundary projection, each pressure correction and velocity reconstruction.
Keep a short history so the first escaping particle can be followed backward
within the same trajectory. Start with steps 88–96 from trace A; use each
repeat’s own reversal interval. Compare the launch interval against matched 20-
and 100-solid runs and check the fluid/body impulse balance at the pass where
the excess upward speed first appears. A candidate must clear both the
overflow and exact-retention gates before measuring its speed.
