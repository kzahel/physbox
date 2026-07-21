# S21: scalable broadphase

Status: awaiting human selection, 2026-07-21. The clean candidate bakeoff
recommends full-sort one-axis SAP; the product still uses all-pairs. This is
the executable tracker for the S5.5 bakeoff and S5.6 integration milestones;
the older S5 document remains the umbrella history.

Governing topics:
[one GPU-resident 2D physics world](../topics/physbox5-unified-gpu-physics.md#broadphase-development-contract),
[performance and scale](../topics/performance-and-scale.md),
[collision and contact persistence](../topics/collision-and-contact-persistence.md),
and [product capability status](../topics/product-capability-status.md).
Predecessor: [S20 validation diet](020-validation-diet-and-feature-ledger.md).

## Goal

Replace quadratic product candidate generation with one measured portable GPU
broadphase while retaining brute force as the bounded completeness oracle.
Compare sorted one-axis sweep-and-prune and a multi-cell grid on the same
canonical pair contract, select from correctness and complete cost, integrate
only the selected winner, and publish honest small-through-scalable body
curves.

## Exclusions

- No solver equation, default physics configuration, sleep policy, shape set,
  CCD, joint, or fluid change.
- No CPU product broadphase or simulation-dependent readback.
- No LBVH unless SAP and grid measurements leave a specific unresolved scale
  or size-variation problem.
- No 30K/100K claim for dense quadratic physical-contact scenes.

## Ownership constraints

- All candidates consume the same GPU body/AABB state and emit the same sorted
  canonical generational pair contract.
- Brute force owns pair completeness for every bounded corpus case. A false
  positive costs work; one false negative rejects the complete candidate run.
- Count, prefix-scan, scatter/sort, unique, and output capacity fail atomically;
  no overflow may silently drop a pair.
- Large fixed shapes use a bounded global-static treatment rather than
  determining grid cell size or multiplying cell records without bound.
- Candidate physics rankings use trajectory-independent snapshots so solver
  motion cannot favor a broadphase. Product-scene timing begins after
  correctness qualification.
- Cross-host qualification uses Metal and browser for candidate portability;
  one physical Android lane qualifies the selected winner, not every rejected
  arm, unless a platform distinction appears.

## Ordered delivery

### S21.1 — canonical corpus and brute-force authority

- [x] Freeze canonical AABB input/output, work, amplification, duplicate,
  capacity, overflow, and first-missing-pair records.
- [x] Add randomized and adversarial uniform/skewed-size, sparse-motion,
  packed, mixed, rain, bucket, giant-static, high-speed-crossing, and complete-
  overlap snapshots.
- [x] Keep everyday checks small; place count/seed sweeps in the focused
  broadphase lane.

### S21.2 — sorted sweep-and-prune

- [x] Implement portable GPU endpoint/key generation, stable sort, interval
  enumeration, canonical pair output, and atomic overflow.
- [x] Prove exact pair completeness and record active-window amplification,
  dispatches, allocation, and timing.

### S21.3 — multi-cell grid

- [x] Implement AABB/cell counts, prefix scan, scatter, cell-key sort, pair
  enumeration, canonical sort/unique, and bounded global statics.
- [x] Prove exact pair completeness and record duplicate amplification,
  maximum occupancy, dispatches, allocation, and timing.

### S21.4 — comparative evidence and selection

- [x] Run qualified Metal and browser matrices at applicable 32, 256, 1K,
  3K, 10K, 30K, and 100K counts with complete workload/capacity context.
- [x] Admit LBVH only through an explicit measured addendum. No addendum was
  opened because SAP resolved the measured selection problem.
- [x] Return one compact recommendation and rejected-alternative record for
  human selection.

Gate: selection requires zero false negatives, atomic capacity behavior, exact
replay, portable WebGPU execution, and lower complete candidate-stage cost than
brute force on the scale families it claims to improve.

### S21.5 — winner integration and product scale

- [ ] Integrate only the selected algorithm into the shared product graph;
  retain brute force as a bounded validation option without automatic fallback.
- [ ] Re-run collision-only, complete physics, and full-frame curves plus
  effective simulation scale/slip and memory high-water.
- [ ] Qualify the integrated winner on browser and one physical Android device.

## Completion condition

S21 completes when one portable GPU broadphase is selected and integrated,
brute force remains a bounded exact oracle, all candidate and product lanes are
complete/capacity-safe/replayable, scale claims distinguish sparse from dense
worlds, and the feature ledger advances `SCALE-2` without changing the solver
or silently accepting a fallback.

## Evidence and selection checkpoint

The candidate implementation is deliberately outside the product graph. The
shared contract now owns versioned AABB snapshots, canonical generational
pairs, work/amplification records, capacity failure, replay, and first-pair
differences. The focused corpus contains all ten required families. CPU brute
force is the bounded exact authority; the existing GPU all-pairs path is also
measured through its portable dispatch boundary.

The two GPU-resident candidates are executable from the same immutable input:

- full-sort one-axis SAP uses an eight-pass lower-X sort, exact active-window
  filtering, bounded atomic emission, indirect sizing, and a ten-pass
  canonical pair sort (58 dispatches);
- the multi-cell grid uses count/scan/scatter, bounded global fixed shapes,
  indirect cell and emission sizing, cell and pair sort/unique, and atomic
  capacity rejection (73 dispatches).

`physbox-broadphase-evidence` is the native focused lane. It retains raw timing
series plus compact pair digests instead of multi-gigabyte stress outputs.
`npm run benchmark:broadphase-browser` executes the identical candidate WGSL
and capacities through Browser WebGPU. Browser completion maps the candidate's
own capacity counters, rejects overflow, and runs every submission inside a
validation error scope. Neither lane integrates a winner by itself.

### Clean provenance

Both artifacts bind clean source revision
`2f51e1576a05742077d742e9e608470cdce56a01`:

- `/tmp/physbox5-broadphase-metal-clean-20260721-r3.json`, SHA-256
  `c1e017e824f03554627db936709c29f30f1df81f04717a2bfd7828ab83fd6956`:
  Apple M4 Pro Metal, five warmups, twenty samples, all ten families at every
  applicable standard count, 64 accepted rows;
- `/tmp/physbox5-browser-broadphase-clean-20260721-r2.json`, SHA-256
  `9fa10d6c3fdf26660e2c9bb3a9db0baadbc68d3ef0ed6716944cc1153fefaeda`:
  Chrome 150 Browser WebGPU, two warmups, five samples, four representative
  families at all seven counts, 28 completed capacity-clean rows.

The native lane uses the CPU exact oracle through 1K and the current GPU
all-pairs stage through 2K. All 64 SAP/grid rows replay exactly, contain zero
missing or unexpected pairs, match each other, and remain capacity-clean.
Focused negative tests separately prove atomic candidate and grid-emission
overflow. The product all-pairs baseline is validated against the GPU AABBs it
actually generates after body projection; SAP and grid are validated against
the pre-projection canonical AABB snapshot.

### Complete candidate cost

The table shows native GPU timestamp P50 at the 100K frontier. Pair count is
the exact overlap count; SAP active and grid duplicate are the candidate's
principal amplification records.

| Family | Pairs | SAP ms | Grid ms | SAP max active | Grid duplicates |
| --- | ---: | ---: | ---: | ---: | ---: |
| Uniform random | 9,482 | 6.85 | 12.67 | 315 | 3,587 |
| Skewed size | 92,230 | 13.78 | 42.69 | 2,216 | 37,085 |
| Sparse moving grid | 0 | 6.52 | 11.59 | 315 | 0 |
| Packed | 398,104 | 32.65 | 49.46 | 631 | 180,536 |
| Mixed aspect | 299,526 | 27.05 | 41.01 | 1,159 | 104,518 |
| Rain | 48,399 | 28.40 | 38.15 | 99,999 | 18,561 |
| Bucket | 398,409 | 55.97 | 68.94 | 99,998 | 130,815 |
| Giant static | 99,999 | 47.14 | 55.71 | 99,999 | 0 |
| High-speed crossing | 1,000,130 | 92.49 | 320.98 | 4,411 | 2,167,426 |

SAP wins all 27 native rows at 10K, 30K, and 100K. At 100K the fixed evidence
capacity allocates 82.80 MiB for SAP versus 323.97 MiB for grid; at 30K the
figures are 24.84 MiB and 97.20 MiB. SAP uses 58 dispatch/synchronization
passes versus grid's 73. The grid wins ten of 64 native rows, all at 3K or
below where fixed GPU overhead dominates. The existing all-pairs stage remains
lower overhead in most 1K rows, but it cannot represent the 3K dispatch within
portable per-dimension limits and its quadratic memory is already 12.28 MiB at
1K. This evidence does not authorize an automatic small-count fallback.

Browser capacity-readback P50 at 100K confirms executable portability rather
than serving as a substitute correctness oracle:

| Family | SAP ms | Grid ms |
| --- | ---: | ---: |
| Sparse moving grid | 6.78 | 13.23 |
| Packed | 22.29 | 32.55 |
| Giant static | 48.69 | 47.60 |
| High-speed crossing | 68.13 | 238.59 |

The browser grid's 2.2% giant-static advantage does not offset SAP's native
win on that row, four-times-lower fixed allocation, fewer passes, and wins on
every other measured scale family. The earlier detached browser fence is not
evidence: mapping candidate capacity exposed and led to correction of an
illegal simultaneous writable-storage/indirect usage before these clean runs.

### Recommendation and rejected alternative

Recommend **full-sort one-axis SAP** for human selection. It satisfies the
correctness, exact-replay, atomic-capacity, and portable-execution gates; wins
every 10K-through-100K native comparison; needs no cell-size/global-static
tuning; and is materially smaller and simpler than the grid.

Reject the multi-cell grid for product integration. Keep it as focused lab
evidence only: its small-count and one browser giant-static wins are real, but
its duplicate amplification, four capacity classes, 4x allocation, 73 passes,
and 1.18x-to-3.47x native cost at 100K make it the inferior complete product
choice.

Do not open an LBVH addendum now. Reconsider only if the integrated SAP misses
the declared collision-only/product frame envelope on representative adapters,
or later product scenes demonstrate a specific sparse multiscale or active-
window problem that this corpus did not resolve.

Human selection remains the gate. No broadphase candidate, fallback, solver
setting, or physics default has changed in the product graph.
