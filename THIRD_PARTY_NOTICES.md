# Third-party notices

PhysBox is licensed under the [MIT License](LICENSE). That license applies to
source owned by this repository; it does not replace the licenses of
dependencies, references, tools, or incorporated third-party source.

## Box2D comparator

The optional interactive Box2D comparator contains an explicitly enumerated
redistributable subset of Box2D. Its preserved upstream MIT license is at
[`third_party/box2d-comparator/LICENSE`](third_party/box2d-comparator/LICENSE),
and its exact revision, file inventory, generation record, and artifact digest
are recorded in
[`third_party/box2d-comparator/SOURCE-MANIFEST.txt`](third_party/box2d-comparator/SOURCE-MANIFEST.txt).

## Dependencies and local references

Cargo and npm dependencies remain under their respective upstream licenses.
Their resolved versions are recorded in `Cargo.lock` and `package-lock.json`.

Ignored checkouts under `reference/` and sibling repositories described by
the documentation are development references, not redistributed as part of
this repository. See [`reference/README.md`](reference/README.md) and
[`reference/pins.toml`](reference/pins.toml) for their provenance and license
boundaries.
