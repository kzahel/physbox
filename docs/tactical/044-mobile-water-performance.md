# S44 — Mobile water performance

Status: complete, 2026-11-26. Selected the density and viscosity changes.

## Goal and ownership

Improve the rate at which a physical Pixel 7a advances a coupled water demo
at the reviewed physics settings. Build on [S43](043-liquid-scale-benchmark-and-optimization.md),
the [liquid performance diagnosis](../topics/liquid-dispatch-and-capacity-performance.md)
and the [scale contract](../topics/performance-and-scale.md).
The shared simulation crate owns neighbor search and solver behavior. The
browser is a consumer. Use the existing release showcase and engine-owned
liquid benchmark to measure presentation and fixed-step cost respectively.

The first native trial on clean base `62469ab` skipped density-grid cells
outside particle support. With 20 dynamic bodies and 8,192 particles on
Pixel, the control, candidate and repeat control took 139.8, 121.3 and
147.2 wall seconds for 30 simulated seconds in the paced offscreen lane.
All three retained the full population. This is the starting hypothesis
for a physical-browser comparison.

## Ordered experiments and gates

- [x] Compare release control and density candidate in physical Pixel Chrome.
  Alternate fresh worlds at fixed scene, viewport, rate and quality. Measure
  actual steps per wall second in the showcase and record live counts. Use
  separately timed engine rows to check finite state, capacity and population.
  Keep recording outside the timed trials.
- [x] Run 0, 1, 1,024 and 8,192 live water against one 8,192-slot reserve
  with the same 20 dynamic bodies. Record complete-step P50/P95, CPU
  encode/submit and available GPU timings. Retain unavailable clocks and
  rejected rows. Compare the fixed cost to the cost added by active water.
- [x] Test one further change selected by these measurements. For a spatial
  change, test frozen density or velocity outputs against identical records,
  including grid edges. For a pass-scheduling change, prove resource
  visibility and the absence of new per-step CPU synchronization. Recheck
  the candidate on the actual demo and preserve a control on either side.
- [x] Record the decision and next target. Before product promotion, require
  repeatable end-to-end improvement, exact particle retention, finite state,
  clean capacity and the shared solver, ordinary browser, headless and
  physical Android gates. Capture a labeled visual example separately when it
  helps interpret the visible motion.

## Scope and completion

Preserve the same world, population, fixed rate, substeps, pressure iterations,
coupling, sleep policy, grid extent and pacing. A dropped interaction or
capacity error stops ranking that candidate. Rendering and recording belong
to their existing adapters. Later independent investigations cover rigid
awake-work scaling at 1K/5K and the first loss or contribution overflow at
100 solids plus 16K water. S44 closes when the physical browser and
fixed-reserve evidence identifies which candidate to promote, reject or
revisit and documents what qualification remains.

## Physical Pixel browser results

Chrome 157.0.8103.80 used hardware WebGPU (ARM Valhall) on Android. The
8K showcase authored 6,497 live water particles and 11 bodies, with a
1,050 × 1,673 drawing buffer. Each timed run opened a fresh scene, warmed for
six seconds and counted fixed steps over the following 20 seconds. Android's
thermal status remained zero. The release builds share base `62469ab`.

| Run order | First arm (steps/s) | Second arm (steps/s) |
| --- | --- | --- |
| Control, density, density, control | Control 35.6, 36.1 | Density 45.4, 47.1 |
| Density, combined, combined, density | Density 46.3, 45.0 | Density + viscosity 50.7, 51.1 |

The additional viscosity gather uses a freshly rebuilt grid. Between rebuild
and gather the velocity passes update velocities and body reactions. Particle
positions stay fixed. The pressure correction gather has a different
precondition because boundary pressure can move particles after the rebuild.
For each changed gather, clipping the existing stencil to the particle's
support bounds preserves the accumulated neighbor order. Frozen checks at
0.15 and 0.30 support radii found bit-identical output for both shaders.

At fixed 8,192 particle slots and 20 dynamic bodies plus three walls,
the instrumented browser physics benchmark gave the following control
results. Each row has 16 warmups and 60 separately fenced steps. These
times include asynchronous completion observation. The browser's quantized
stage timestamps did not consistently isolate liquid work.

| Live water | CPU encode/submit P50 | Host completion P50 | Host completion P95 |
| ---: | ---: | ---: | ---: |
| 0 | 8.9 ms | 52.2 ms | 69.9 ms |
| 1 | 9.0 ms | 56.5 ms | 87.5 ms |
| 1,024 | 9.1 ms | 67.3 ms | 85.6 ms |
| 8,192 | 8.8 ms | 76.9 ms | 101.8 ms |

All four rows retained their requested population at clean capacity, and the
engine reported 273 dispatches per step even at zero live water. This points
to a separate investigation of fixed submission and completion cost.

## Decision and qualification

Use the bounded cell search for density and viscosity. Both are measured
independently above. In a final control/combined/combined/control block, the
larger showcase advanced at 15.9 and 15.8 steps/s on control and 23.2 and
23.2 on combined. All four runs retained 13,104 live water and 11 bodies.
Both showcase sizes remain below the selected 60 Hz rate on this phone.

The sustained browser check rendered 960 × 600 offscreen with 20 dynamic
bodies, three walls and exactly 8,192 particles. Both arms ran 16 warmup and
1,800 measured frames with a completion wait per frame. Median frame time
fell from 108.1 to 92.7 ms and P95 from 151.2 to 111.6 ms. Thirty simulated
seconds took 201.0 and 165.1 wall seconds respectively. Both finished with
finite state, valid particle generations and flags, exact retention and clean
capacity. These are single sustained runs per arm.

Two native Radeon 890M runs per arm provided an independent check. With
20 dynamic bodies and 16,384 particles, physics-only host completion P95
fell from 11.06/11.96 ms to 9.03/9.20 ms. At 8,192 particles the respective
values were 7.11/7.99 and 7.08/6.68 ms. The diagonal grid-corner regression,
fast lane including desktop browser WebGPU, production clippy, 605-frame
headless liquid sandbox, ARM64 check and release APK build passed. The Pixel
also passed the native 600-step liquid sandbox using the release native
library in a separately signed test package.

In the combined build, the visible playground with 21 bodies and zero water
ran at 60.0/59.9 steps/s with zero slots and 59.7/59.6 with 8,192 slots.
The 60 Hz ceiling limits this comparison. The expensive empty fenced benchmark
alone gives insufficient reason to prioritize a dry-scene optimization.

## Next experiments

1. Locate the first correctness limit at 100 dynamic solids plus 16,384
   water. Bracket the first lost particle or overflowing contribution buffer
   by step, capture its counters and compare against the passing 8,192-water
   tier. Require exact retention and clean capacity before ranking timings.
2. For the remaining wet-demo cost, measure candidate neighbor checks and
   accepted pairs in the pressure correction gather. Bound motion between
   grid build and gather before testing any narrower stencil. Repeat the
   frozen-state differential and the two physical showcase sizes.
3. Sweep 1,000 and 5,000 rigid bodies with controlled awake fractions.
   Record live and awake counts alongside physics P95 to distinguish sleeping
   capacity from the cost of advancing moving solids.
