# Box3D planar observer

This process-isolated validation tool independently authors the two pinned
Box3D pyramid sources through Box3D's public API. It is not linked into any
PhysBox product crate, is not an authority, and does not provide a CPU physics
fallback.

The CMake configure step rejects a Box3D checkout unless it is clean and at
`8441b4a06d6d09dcfb0b0f704df4d847d1437b92`. Configure and build out of tree:

```sh
cmake -S tools/box3d-planar-observer \
  -B /tmp/physbox5-box3d-planar-observer \
  -DCMAKE_BUILD_TYPE=Release \
  -DPHYSBOX5_BOX3D_SOURCE="$PWD/reference/box3d"
cmake --build /tmp/physbox5-box3d-planar-observer \
  --target box3d_planar_observer --parallel
```

Run either frozen source twice for exactly 5,000 steps; the wrapper validates
both artifacts and publishes output only after they match byte-for-byte:

```sh
node tools/box3d-planar-observer/run-observer.mjs \
  --binary /tmp/physbox5-box3d-planar-observer/bin/box3d_planar_observer \
  --scenario pyramid2d-12 --steps 5000 --output /tmp/pyramid2d-12.json
node tools/box3d-planar-observer/run-observer.mjs \
  --binary /tmp/physbox5-box3d-planar-observer/bin/box3d_planar_observer \
  --scenario large-pyramid-90 --steps 5000 --output /tmp/large-pyramid-90.json
```

The contract is 60 Hz, four substeps, one worker, sleeping disabled, warm
starting enabled, and stock contact recycling. `pyramid2d-12` has the stock
linear-Z and angular-X/Y locks. `large-pyramid-90` is only *authored* one box
deep: the stock benchmark leaves all three translational and rotational axes
free, so the observer measures its out-of-plane motion rather than calling it
a 2D mode. Step 0, 200, 1,200, and 5,000 are full body/contact checkpoints.
`pyramid2d-12` also emits aggregate samples for every step from 900 onward. A
shorter step count is accepted only for harness smoke tests; it does not
satisfy the S7.3 evidence contract.

The predeclared interpretations are deliberately descriptive and strict:

- `nearRest` requires maximum linear speed and maximum angular edge speed to
  both be at or below Box3D's default 0.05 m/s sleep threshold;
- `toppled` means orientation departure exceeds 45 degrees from the authored
  pose; it is descriptive for cubes and does not alone fail structure;
- `displaced` means center displacement exceeds one authored full box width
  (2 m for `pyramid2d-12`, 1 m for `large-pyramid-90`);
- `lost` means a center falls below -1 m or exits the stock ground's horizontal
  extent;
- `uprightStructure` requires zero lost bodies, at least 70% initial height,
  no more than 1.5x X-footprint expansion, and—for the unlocked large
  source—no center more than one box width out of its authored plane.

These labels do not change Box2D authority, PhysBox tolerances, or product
quality policy. Kinetic energy uses public mass and local rotational inertia;
because the planar source locks angular X/Y and linear Z, its rotational term
is `0.5 * Izz * wz^2`. Public contact data are counted once from shape A (or
from the dynamic side of a static/dynamic pair) and report point persistence,
separation, accumulated normal impulse, and central friction impulse. Box3D's
3D four-point manifolds and two-axis/twist/rolling friction remain visible as
3D mechanisms, not claimed 2D parity.
