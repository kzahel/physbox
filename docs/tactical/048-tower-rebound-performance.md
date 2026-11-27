# S48 — Tower rebound performance

Status: complete locally, 2026-11-27.

Reduce visible stutters during steps 80–150 of the S47 tower drop on the Pixel
7a. Keep the tower, camera, 60 Hz clock and water populations fixed. The
showcase owns drawing and the simulation library owns contact and neighbor
work. This experiment excludes deployment and changes to standard water
settings. It follows the [performance contract](../topics/performance-and-scale.md)
and the liquid preview in the [bring-up plan](../bringup-plan.md).

1. [x] Reproduce the 32K control with the S47 release. The first run at
   `5b61767` gave 36.2 FPS and 50.1 ms p95 during rebound.
2. [x] Measure contact projection, neighbor correction and rendering on the
   physical GPU. Keep instrumented samples separate from cadence runs.
3. [x] Try a change to the measured bottleneck. Compare fresh, uninstrumented
   runs with the same control. Check both populations and film the return
   wave through step 480.
4. [x] Record phase timings, stutter counts, visual checks, validation gates
   and the disposition. Keep a candidate only if the repeated Pixel timing
   and visual checks support it. A measured rejection also closes the study.

Use the S47 Pixel runner with `time` for cadence, `film` for capture and
`profile` for pass timings. Local evidence lives in
`evidence/2026-11-27-rebound/`.

## GPU attribution

On the physical ARM Valhall adapter, both the adapter and release device
exposed WebGPU timestamp queries. Profile mode attaches pass-boundary writes
to the benchmark tab every fifth step from 80 through 145, then reads the
results asynchronously. The ordinary timing and film modes use the original
release encoder. Repeat with the same build used for the control:

```sh
node scripts/showcase-tower-pixel.mjs target/tower-damped /tmp/tower-profile 32k profile
```

The GPU started later passes before earlier passes completed. Summing
begin-to-end durations would double-count queued work. Mean differences
between adjacent pass completion timestamps over 14 sampled steps were
4.46 ms for neighbor correction gather, 4.31 ms for viscosity gather and
6.89 ms for the two density estimates combined. Solid projection after
integration and correction together accounted for 0.59 ms. Boundary
pressure projection added 0.30 ms and both boundary density passes added
0.49 ms. These are completion intervals within the recorded queue,
including any intervening unmarked work. Use uninstrumented runs for
visible cadence. Raw timestamp pairs are in `profile-control/`.

## Decision and Pixel results

Run density, correction and viscosity gathers in the order of the live slots
already packed by grid scatter. Adjacent GPU lanes now tend to visit the same
cells. Each lane still writes to its particle's original slot. The prefix
ends at the final cell's offset plus occupancy. Inactive outputs are cleared
by slot, including when a particle escapes during this step. No extra grid
buffer or dispatch is needed. Correction reads the current position after
boundary pressure and retains its full neighbor stencil.

| 32,640 live water | Runs | Rebound FPS | Rebound p95 (ms) |
| --- | ---: | ---: | ---: |
| S47 control | 2 | 36.2–36.8 | 50.1 |
| Drop the per-neighbor live flag read | 3 | 45.6–46.2 | 33.4 |
| Also read cell counts through a read-only binding | 2 | 45.2–45.7 | 33.5–49.9 |
| Cache positions in widened grid records | 3 | 47.7–48.3 | 33.4–33.5 |
| Select spatial invocation order with original four-byte grid records | 3 | 60.0 | 16.7–16.8 |

Each of the last three trials builds on removing the redundant flag read.
The read-only binding did not improve cadence. Caching positions added
12 bytes per slot. Ordering work by cell gave the larger gain with the
original allocation. An initial spatial-order prototype reused a shader-local
name and the first GPU count snapshot read zero. The runner rejected that
artifact in `grid-order/`. The corrected prototype and the selected build
kept their full counts.

On the selected GPU profile, the correction gather averaged 0.72 ms,
viscosity 0.68 ms and both density gathers combined 1.18 ms. The two solid
contact projections averaged 0.65 ms. The two controls had 7 and 10 rebound
frame intervals over 42 ms. All 210 intervals across three selected runs
were at most 16.8 ms. Impact and settling averaged 60.0 FPS in those runs.
Cold release averaged 51.4–56.3 FPS compared with 58.1 FPS in the controls.
The phase contains only 30 steps and includes first-use work.

The 13,122 tier also delivered 60.0 FPS from impact through settling in two
runs. Separate 480-step Pixel recordings of both tiers show the cavity fill
over the blocks, with every population sample at the starting count. Both
post-capture block grabs, releases and resets passed. The MP4s are in the
evidence root. The instrumented and recorded runs are excluded from the FPS
table. The selected release WASM SHA-256 is
`3edffa789a8d8e3ce9d71a3c89d760abc8369699a43147487d8c291234895726`.

The maintained fast lane passed in 52.64 s, including the partial-workgroup
regression with a killed low slot and two survivors in the last grid cell.
Production Clippy, the 605-frame Radeon liquid sandbox, packaged showcase
browser smoke, ARM64 check and ARM64 release build passed. Native Pixel
validation passed on Mali-G710 in a temporary application ID using the release
library. That package was removed after capture.

The next count experiment is a separate 48K tower tier. Review the current
32,768-slot product bound and buffers first, then repeat the same Pixel
windows, count checks and return-wave capture.
