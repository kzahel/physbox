# S8: Physical mobile frame pacing

Status: complete, 2026-07-18. The shared small-world product path sustains the
physical Pixel 7a presentation and physics cadence gates, and the lane is now
repeatable rather than a subjective phone observation.

Governing topics:
[performance and scale](../topics/performance-and-scale.md),
[simulation time and overload](../topics/simulation-time-and-overload.md), and
[contact solver parallelism](../topics/contact-solver-parallelism.md).

## Goal

Reproduce the reported mobile stutter, distinguish reporting overhead from GPU
and clock pacing, repair the smallest controlling causes without changing
physics, and add a physical-device regression lane that independently measures
presented frames and fixed-step progress.

## Exclusions and ownership constraints

- Keep one shared engine, frame path, renderer, fixed delta, and overload
  policy; Android supplies timestamps and artifacts only.
- Do not add a CPU physics fallback, host-owned scheduler, catch-up step,
  interpolation path, reduced physics rate, or platform-specific solver.
- Do not change color assignment/order, solver equations, physical
  coefficients, tolerances, capacity rejection, or oracle precedence.
- Do not generalize a two-body phone result into a 3K-10K mobile scale claim.
- Preserve the scalable per-color schedule wherever the island reserve loses
  the measured crossover; a small-scene repair may not silently regress 1K.
- The browser and native adapters share the repaired engine path. This slice
  does not claim a separately automated physical Chrome lane.

## Ordered delivery slices

### S8.1 — reproduce and isolate

- [x] Compare bootstrap with frame markers off and all on.
- [x] Measure SurfaceFlinger presentation independently of application logs.
- [x] Run the shared free-flight scene as a low-dispatch device control.
- [x] Record dispatches, allocated bytes, presented cadence, effective scale,
  and slipped time.

Gate: the result identifies whether the display, reporting, shared GPU graph,
or fixed-step admission controls the failure without assuming from visual
stutter.

Outcome: Pixel 7a bootstrap was `13.51 FPS` markers-off and `13.31 FPS`
markers-all, while free flight was `60.07 FPS`. Reporting was secondary; the
1,434-dispatch product floor was controlling.

### S8.2 — bounded repair and crossover

- [x] Evaluate the predeclared workgroup-per-island reserve on the same phone.
- [x] Compare the reserve with the selected scalable schedule on Metal at 32
  and 1K bodies.
- [x] Select by declared capacity only where evidence favors the reserve.
- [x] Prove exact body, cache, and work results across both dispatch forms.

Gate: the Pixel bootstrap meets 60 Hz presentation, the scalable 1K schedule
does not change, and no physics or capacity contract changes.

Outcome: capacities through 64 use island dispatch, reducing bootstrap from
1,434 to 195 dispatches. Larger capacities retain per-color indirect because
the island diagnostic regressed generated 1K GPU P50 from `8.781` to `21.884
ms`.

### S8.3 — clock phase and permanent guard

- [x] Reproduce nominal-60 callbacks arriving just below fixed `dt`.
- [x] Add bounded signed phase tolerance while retaining zero-or-one steps and
  permanent slip for genuine multi-step overload.
- [x] Add deterministic clock and dispatch-selection regression tests.
- [x] Add a physical Android lane with a passing device control, marker-off
  presentation percentiles, marker-derived physics scale, provenance, and a
  success sentinel.

Gate: `npm run android:pacing-device` requires at least 55 presented FPS, at
most 20 ms presentation P95, at least 0.90 effective simulation scale, the
bootstrap scene contract, and a simultaneously healthy free-flight control.

Outcome: the Pixel 7a records `60.06 FPS`, `16.75 ms` presentation P95,
`59.38` fixed steps/s, `0.990` effective scale, and `0.067 s` slip during the
six-second all-marker window. The lane restores the default bootstrap launch
when it exits.

## Completion condition

Complete when the physical Pixel lane passes from a release Rust build, shared
exact/capacity tests prove the adaptive dispatch result, native/browser/Android
build gates pass, the pacing decisions and measured crossover are recorded in
their owning topics, and no platform adapter owns engine policy.

Validation commands:

```sh
npm run android:pacing-device
npm run test:android-tools
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown -p physbox-web
cargo check --target aarch64-linux-android -p physbox-android
npm run check:boundaries
npm run test:web
```
