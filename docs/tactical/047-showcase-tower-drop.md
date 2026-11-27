# S47 — Replayable tower drop

Status: complete locally, 2026-11-27. Both presets passed the Pixel shot.

Give the public showcase a repeatable tower impact with at least 30 visible
frames per second during the splash on a physical Pixel 7a. Compare the same
tower, tank, camera and release at roughly 13K and 32K live water particles.
The showcase owns staging and surface shading. The simulation library owns
the opt-in high-count water profile. The standard solver and scale benchmark
keep their existing settings. Production deployment is outside this study.

1. [x] Author two selectable sizes. Pause at the release pose and restore it
   on Reset. Keep the ordinary drag tool available.
2. [x] Measure the standard water path on the first shot. Try the S46 solver
   settings and compare release, impact, rebound and settling on the phone.
3. [x] Tune the tower and neighborhood grid against the slowest window, review
   a separate canvas recording, then verify a settled-body drag and replay.
4. [x] Record matched Pixel results, footage and the maintained validation gates.

Build the release showcase and use a fresh output directory for evidence:

```sh
node scripts/build-showcase-web.mjs --release --output target/tower
node scripts/showcase-tower-pixel.mjs target/tower /tmp/tower 13k time
node scripts/showcase-tower-pixel.mjs target/tower /tmp/tower 32k time
node scripts/showcase-tower-pixel.mjs target/tower /tmp/tower 32k film
```

Each run opens its own foreground Chrome tab over USB on the attached Android
device. Timing runs measure rendered showcase frames and admitted fixed steps
over four windows after release, with boundaries at steps 0, 30, 80, 150 and
240. Film runs continue through step 480. The canvas recorder incurs extra
GPU work, so each timing run disables it.
Completion requires replayable scenes, both populations intact through the
shot, and a measured disposition of the 32K target.

## Physical Pixel 7a results

Chrome 157, ARM Valhall WebGPU, 1,050 × 1,673 canvas, 60 Hz fixed step.
The two presets use ten dynamic blocks, three fixed tank pieces and 64 rigid
slots. The pool fills the same world-space rectangle with spacings of 0.1575
and 0.1 respectively. The surface combines nearby live particles into a
half-resolution thickness texture. All counts are measured simulation slots.

| Shot and water profile | Release FPS | Impact FPS | Rebound FPS | Settling FPS |
| --- | ---: | ---: | ---: | ---: |
| 13,122 standard, one control | 53.0 | 49.2 | 21.9 | 23.7 |
| 13,122 selected, two runs | 56.2–58.1 | 60.0 | 60.0 | 60.0 |
| 32,640 standard, one rejected control | 34.0 | 17.9 | 7.0 | 8.2 |
| 32,640 selected, two runs | 54.5–56.3 | 58.8–60.0 | 36.8–37.5 | 39.7–40.0 |

The controls use the final tower, camera, tank and surface renderer. The
32K standard run finished with 32,598 live particles, so its timings describe
a rejected run. Both selected runs at each size admitted one physics step per
rendered frame throughout the measured windows. At 32K the 95th-percentile
interval was 16.7–16.8 ms at impact and 50 ms in rebound. These are short
foreground scene measurements. The target is a phase average, and individual
frames can still exceed 33 ms.

The first four-block-wide tower opened a broad vertical cavity. A two-block-wide
leaning tower, taller walls and a 192 × 204 grid improved the 32K rebound from
26 to 33 FPS in preliminary trials. The grid preserves the 32 × 34 world-space
extent with a cell width of one sixth. The visual profile uses one fluid
substep and one pressure iteration, with pressure strengths of 350/450.
Raising water gravity from 0.12 to 0.30 dropped the rebound below 28 FPS.

Extending the shot to eight simulated seconds exposed excessive spray. By step
480 the undamped version had lost 927 particles. The selected version keeps
0.12 water gravity and applies velocity retention of exp(−0.9 × substep
seconds) at integration. The wave folds back over the fallen blocks. Both
480-step captures retained their exact starting counts. A pointer constraint
held a visible block for at least 15 steps after each capture, released, and
Reset restored the tower for a second drop.

The local evidence directory is `evidence/2026-11-27-tower/`. The
`standard/` and `damped/` subdirectories contain per-frame timing rows,
run IDs, revision, dirty state, WASM digest, canvas dimensions and live
counts. The standalone HTML embeds the selected release WASM. Desktop
Chromium opened the file, released and reset each tier. The two MP4s crop
the unused portrait margins from the separate Pixel canvas recordings.

Validation passed the maintained fast lane (57.46 s), production Clippy,
showcase WASM Clippy with package scope, showcase library tests and packaged
browser smoke. A 605-frame liquid sandbox passed on Radeon. ARM64 check and
the release native library build passed. The Pixel also passed the native
liquid sandbox using the same native library in a temporary validation package
because its installed product app uses a different signing key.

For the next pass, compare fluid/solid contact projection and neighbor
correction specifically in steps 80–150. Keep both live populations and the
480-step return wave as checks while reducing the 32K rebound's 50 ms tail.
