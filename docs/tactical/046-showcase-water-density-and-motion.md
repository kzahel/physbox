# S46 — Showcase water density and motion

Status: complete, 2026-11-26. Selected the tapered, two-size local preview.

## Goal and ownership

Choose a high-count water demo by watching the moving water on a physical
Pixel 7a. Rank options by the number of active simulated particles, visible
frame cadence and whether the release, wave, settling and solid interactions
are convincing. The showcase owns the authored scene. The shared simulation
owns integration, pressure and neighbor search. This comparison uses isolated
release builds. The workbench and the standard benchmark remain the references
for their existing settings.

## Experiments

- [x] Give the water block room to collapse sideways in an open tank with
  eight movable solids. Use the same scene, camera, drawing resolution and
  fixed outer step for every solver setting.
- [x] Compare two and one particle substeps, one and two pressure iterations,
  and a smaller neighbor-grid cell at 13,104 live particles. Review matched
  captures of the initial collapse alongside visible timing measurements.
- [x] Use the existing 32,768-slot reserve to try a larger body of water.
  Record the actual live population. Inspect the surface, leaks, solid
  movement and visible multi-frame stalls before choosing a demo.
- [x] Record the selected preview, comparison and next visual target.

A preview can trade physical precision for motion or scale. Count simulated
particles separately from any future cosmetic spray. A visually distracting
collapse, unresponsive interaction or a persistently choppy frame rate counts
against a candidate even when it fits the reserve. This workstream completes
with a runnable preview and a physical-device capture. General solver defaults
and production deployment are outside this trial.

## Pixel observations

Chrome 157 used hardware WebGPU on the Pixel 7a (ARM Valhall, 1,050 × 1,673
drawing buffer). The timed foreground runs used a three-second warmup and at
least 15 measured wall seconds. Live counts and frame and fixed-step deltas
were sampled separately from video capture. The reported FPS is the number
of showcase frames divided by measured wall time.

At 13,104 particles, current S44 settings in the rectangular side-release
scene gave 25.2/25.6 steps/s. One liquid substep gave 29.0/29.0; dropping to
one pressure iteration gave 33.2/33.2. With those settings and a 0.25-unit
neighbor grid, one trial reached 49.0, then 52.3 when the rigid reserve was
set to 64. All these runs had the same eleven bodies. Tightening the grid to
0.20 units and retaining the same physical grid extent gave 55.9/56.7. In
that rectangular scene, 26,166 live particles reached 31.5 steps/s in a
single trial and 32,761 reached 23.2/23.3. Those rates describe the earlier
scene, not the final shaped wave.

For visual review, a trial using pressure strengths of 1120/1040 produced a
speckled, foamy release. A half-resolution thickness pass connected nearby
particles into a visible surface and preserved isolated spray. At that firmer
setting it gave 24.8/24.9 steps/s at 32,761, close to the sprite renderer's
24.9/24.9. Switching that 32K scene to a 30 Hz fixed step produced
10.4 steps/s and finished the timed run with 32,163 live particles. Keep
the 60 Hz fixed interval for the preview.

## Selected preview

The preview uses one fluid substep and one pressure iteration, standard
0.05 particle radius, a 0.20 grid cell, 64 available rigid slots and eight
movable bodies. The crest starts with a tapered front and lateral speed
varying from 2 to 5 world units/s with height. Moderate pressure strengths
(350/450) and larger solids made the interaction easier to follow. A
half-resolution thickness texture shades a continuous surface by height and
motion. The eight solids and three fixed tank pieces remain individually
simulated. Both menu sizes count real liquid particles.

| Tapered preview | Pixel FPS, two runs | Frame interval P95, two runs | Live water range across samples |
| --- | ---: | ---: | ---: |
| 13,104 authored | 56.9 / 56.8 | 33.2 / 33.3 ms | 13,104–13,104 |
| 32,761 authored | 23.8 / 24.2 | 66.5 / 66.5 ms | 32,761–32,761 |

The downloadable HTML embeds the same release WASM used for the Pixel
measurement. Hardware-backed desktop Chromium opened it directly from disk,
switched between the 13K and 32K scenes, added a solid, and reset the scene.
The Pixel videos show separate canvas recordings outside the
timing windows. The surface is stylized and the initial crest briefly holds
its authored shape. These are visual demo results for a physical phone.

The next target is 32K at 30 visible frames/s on Pixel while keeping the
return wave readable. Compare where frame time goes during the release and
after water packs against the floor. The 26K result supplies an intermediate
population for distinguishing cost per live particle from cost due to local
crowding. Keep the two selectable sizes for visual review.
