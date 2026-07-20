# PhysBox laboratory commands

`physbox-lab` is opt-in validation tooling, not a product host. Depending on
this package enables `physbox-engine/solver-lab`, which exposes native
experiment runners while keeping those APIs out of production package builds.

Cargo automatic binary discovery is disabled. The maintained commands are:

- `physbox-lab`: neutral scenarios and randomized corpus entry point;
- `physbox-oracle-fixture` and `physbox-synchronized-playback`: oracle
  projection and playback;
- `physbox-solver-evidence` and `physbox-scheduler-benchmark`: selected-solver
  evidence and scheduling cost;
- `physbox-settling-backend-trace`: current settling diagnostics; and
- `physbox-s17-product-relative`: the retained S17 runner reused by the S18
  coefficient loop and any later promotion review.

The other files under `src/bin/` are closed-study source history. They are
deliberately absent from Cargo metadata, workspace tests, and clippy. To
reopen one, first obtain the technical authorization required by its tactical,
then add an explicit `[[bin]]` entry here with `test = false`. Do not restore
automatic discovery.
