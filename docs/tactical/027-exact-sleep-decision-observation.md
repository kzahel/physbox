# S27 exact sleep-decision observation

Status: active, 2026-07-22. Human review authorizes stopping point 2 from
[GPU sleep-island connectivity research](../topics/gpu-sleep-island-connectivity.md):
reduce wake causes and post-solve readiness by the exact S26 component labels
beside the unchanged K=4 sleeper, compare decisions, and stop before any exact
decision affects product behavior.

## Goal

Measure when and how much exact whole-component wake and admission decisions
differ from the current four-contact-hop approximation. Include time-evolved
centered towers and poured piles so the result answers the observed recurring
wake-wave question rather than only static connectivity cost.

## Exclusions and ownership

- K=4 remains the sole applied wake and admission policy. Exact results may be
  copied to laboratory readback but may not drive sleep state, awake work,
  solving, rendering, the lab menu, configuration, or defaults.
- Reuse S26's exact labels and the engine-owned sleep frontiers at their actual
  schedule points. Do not invent CPU contacts, a second scene, or a second
  sleeper.
- Do not implement an exact-island behavior switch, raise K, add persistent
  island state, optimize dirty splits, resume S23.7, or alter solver quality.
- CPU traversal and replay checks are bounded evidence tooling only. No
  interactive frame may depend on GPU results returning to the CPU.
- A disagreement is evidence, not an automatic promotion or rejection. Record
  both broader exact wake work and any K-only partial admission.

## Frozen comparison contract

- Build exact minimum-slot labels from the current committed
  dynamic-dynamic contact manifolds after persistence and before wake
  propagation, using the proven S26 phase bound and validity checks.
- After all ordinary wake roots are seeded, OR their causes by exact root.
  After K=4 propagation, compare signaled bodies and projected newly-woken
  sleeping bodies. K-only wake membership is invalid; exact-only membership
  measures the bounded-radius gap.
- After solve and ordinary local readiness evaluation, OR admission blockers
  by exact root. After K=4 blocker propagation and before applied admission,
  compare blocked bodies and locally-ready bodies that each policy would
  admit. Exact-only admission is invalid; K-only admission measures a partial
  component transition.
- Record per-step component/edge shape, local roots/blockers, K and exact
  decision counts, directional disagreements, awake/sleeping population, and
  projected awake-work deltas. Preserve enough compact GPU output for an
  independent CPU reduction check after the bounded run.
- Trace at least a small centered tower through sleep, a larger centered tower,
  and a gravity-poured pile through settling. The scalar long-path control must
  expose both comparison directions; the integrated GPU control must expose a
  real beyond-K wake without applying it.

## Ordered delivery slices

### S27.1 — contract and scalar differential

- [x] Freeze the schedule points, count definitions, invalid directions, and
  trace rows.
- [x] Add scalar component-reduction tests for wake and admission, including
  paths shorter than, equal to, and longer than K=4.

Gate: scalar exact and K=4 decisions match within four hops and expose the
declared exact-only wake / K-only admission directions beyond four hops.

### S27.2 — opt-in GPU reductions

- [x] Extend the laboratory observer with root aggregation and comparison
  passes at the frozen pre-wake and pre-admission schedule points.
- [x] Emit compact per-step statistics and prove ordinary product graph
  allocation, dispatch, and applied sleep state remain unchanged.
- [x] Differential-check GPU reductions against an independent CPU replay and
  fail closed on invalid roots, cross-label edges, or impossible comparison
  directions.

Gate: every admitted fixture replays and matches the CPU reduction exactly;
the applied K=4 outcome is byte-identical with and without observation.

### S27.3 — time-evolved traces and cost

- [x] Run the synthetic long path, centered tower controls, and a poured-pile
  settling trace long enough to cover admission and subsequent wake activity.
- [x] Report disagreement incidence and duration, cohort sizes, projected
  extra/fewer awake bodies, observer GPU P50/P95/P99, dispatches, and bytes.
- [ ] Run proportional engine, clippy, browser, headless, ARM64, release APK,
  and available Android hardware validation required by the touched boundary.

Gate: fresh provenance-complete evidence distinguishes behavioral benefit from
added component cost and from solver motion that correctly blocks an island.

### S27.4 — stopping report

- [x] Update the governing topic with findings, limitations, and one of:
  retain K=4, authorize a later behavior candidate, or first authorize a
  persistent-cost experiment.
- [ ] Stop before exact decisions are applied and return to human review.

## Completion condition

S27 completes when exact root reductions are measured beside K=4 on synthetic
and time-evolved structures, GPU results match an independent CPU replay,
incremental cost and projected work are recorded, applied product behavior is
unchanged, and the repository stops at the behavior-switch decision point.

## Completed observation

Implementation revision `98045db8bbff076661520b79bf3bca93056f3b16`
owns the clean evidence. A laboratory-only graph schedule rebuilds the S26
exact labels after persistence, aggregates ordinary wake roots before K=4,
compares after K=4, then repeats the aggregation/comparison for post-solve
blockers before ordinary admission. Only observer buffers are writable from
the new passes. A separate ordinary graph replay proves final pose, motion,
metadata, sleep state, and awake-list words remain byte-identical.

The durable command is:

```sh
npm run benchmark:sleep-decisions -- \
  --output /tmp/physbox5-s27-sleep-decisions-98045db.json \
  --warmup 1 --samples 5
```

It emits `PHYSBOX5_VALIDATION_SUCCESS`; the 3,761,788-byte artifact has
SHA-256
`8c9e26cfe7ba671a6ae5717bae871cd880596cfcf4232f4f8621ca22cde4eedc`.
All five rows and all 3,601 longitudinal steps pass exact labels, final GPU
root/edge validation, independent CPU root reduction, impossible-direction
checks, and applied-state replay on AMD Radeon 890M RADV Vulkan.

| Row | Exact-only wake signal | Actual/projected transition difference | Observer GPU P50 / P95 | Physics P50 / observer share |
| --- | ---: | ---: | ---: | ---: |
| sleeping path 8, step 1 | 3 bodies | 3 exact-only new wakes | 0.268 / 0.280 ms | 1.346 ms / 19.92% |
| centered tower 2, 600 steps | none | none; K=4 and exact sleep both bodies at step 30 | 0.260 / 0.847 ms | 1.178 ms / 22.07% |
| centered tower 8, 600 steps | 10 body-events / 10 steps | none; every body was already awake | 0.251 / 0.478 ms | 1.537 ms / 16.34% |
| centered tower 14, 1,200 steps | 1,484 body-events / 301 steps; peak 8 | none; every body was already awake | 0.258 / 0.440 ms | 1.710 ms / 15.09% |
| poured pile 100, 1,200 steps | 9,962 body-events / 836 steps; peak 82 | none; every body was already awake | 0.394 / 0.616 ms | 8.344 ms / 4.73% |

Five retained samples make nearest-rank P95 and P99 the same maximum and are a
bounded decision sample, not a release distribution. Exact reductions add
four dispatches and cost only 0.007–0.008 ms P50. The exact component rebuild
dominates the 33-dispatch 64-slot and 41-dispatch 256-slot observer. Fixed
working storage is 5,632 and 11,264 bytes. The much larger 0.88–5.45 MB row
allocations are longitudinal body/stat trace retention and are not a projected
product working set.

The evolved fixtures contain no K-only partial admission and no exact-only new
wake. The 8- and 14-body towers remain fully awake because all bodies retain
local blockers; the poured pile remains 100% awake, has 99 local blockers at
step 1,200, and creates new wake roots on that final step. Exact connectivity
therefore detects broader signal cohorts but does not change the transition
that limits those scenes. The result retains K=4 and does not admit stopping
point 3. A future behavior candidate first needs a deterministic fixture that
reproduces an actual mixed transition, or new solver/readiness evidence.
