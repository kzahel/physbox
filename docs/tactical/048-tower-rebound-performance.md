# S48 — Tower rebound performance

Status: active, 2026-11-27.

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
3. [ ] Try a change to the measured bottleneck. Compare fresh, uninstrumented
   runs with the same control. Check both populations and film the return
   wave through step 480.
4. [ ] Record phase timings, stutter counts, visual checks, validation gates
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
