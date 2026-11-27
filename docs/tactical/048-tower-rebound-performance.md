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
2. [ ] Measure contact projection, neighbor correction and rendering on the
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
