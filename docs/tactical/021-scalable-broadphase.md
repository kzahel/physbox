# S21: scalable broadphase

Status: active next workstream, 2026-07-21. S20 is complete. This is the
executable tracker for the S5.5 bakeoff and S5.6 integration milestones; the
older S5 document remains the umbrella history.

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

- [ ] Freeze canonical AABB input/output, work, amplification, duplicate,
  capacity, overflow, and first-missing-pair records.
- [ ] Add randomized and adversarial uniform/skewed-size, sparse-motion,
  packed, mixed, rain, bucket, giant-static, high-speed-crossing, and complete-
  overlap snapshots.
- [ ] Keep everyday checks small; place count/seed sweeps in the focused
  broadphase lane.

### S21.2 — sorted sweep-and-prune

- [ ] Implement portable GPU endpoint/key generation, stable sort, interval
  enumeration, canonical pair output, and atomic overflow.
- [ ] Prove exact pair completeness and record active-window amplification,
  dispatches, allocation, and timing.

### S21.3 — multi-cell grid

- [ ] Implement AABB/cell counts, prefix scan, scatter, cell-key sort, pair
  enumeration, canonical sort/unique, and bounded global statics.
- [ ] Prove exact pair completeness and record duplicate amplification,
  maximum occupancy, dispatches, allocation, and timing.

### S21.4 — comparative evidence and selection

- [ ] Run qualified Metal and browser matrices at applicable 32, 256, 1K,
  3K, 10K, 30K, and 100K counts with complete workload/capacity context.
- [ ] Admit LBVH only through an explicit measured addendum.
- [ ] Return one compact recommendation and rejected-alternative record for
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
