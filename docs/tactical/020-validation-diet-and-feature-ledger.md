# S20: validation diet and feature ledger

Status: complete, 2026-07-21. Human review accepts the soft solver direction
and now has a measured 7.31-second warm everyday native/browser feedback loop.
Scalable broadphase work may begin under S21.

Governing topics:
[build, validation, and evidence health](../topics/build-validation-and-evidence-health.md),
[product capability status](../topics/product-capability-status.md), and
[performance and scale](../topics/performance-and-scale.md). Successor:
[S21 scalable broadphase](021-scalable-broadphase.md).

## Goal

Make the normal development loop fast, explicit, and proportional to change
risk. Preserve high-signal product, GPU, and browser checks; move exhaustive
configuration matrices and historical research into opt-in lanes; provide one
targeted performance-smoke command; and publish a compact comprehensive ledger
of implemented, limited, missing, and deferred engine capabilities.

## Exclusions

- No physics equation, product default, tolerance, scenario outcome, buffer
  layout, or shader change.
- No broadphase candidate implementation or selection; S21 starts afterward.
- No deletion of historical experiments, oracle coverage, or reviewed
  evidence.
- No claim that a fast smoke timing is a portable performance baseline.
- No weakening of exact replay, capacity, freshness, or provenance gates in
  the lifecycle lanes where they apply.

## Ownership constraints

- `production-validation` owns the maintained per-change Rust surface.
  Exhaustive matrices remain executable without that feature or through the
  directly affected package/target.
- The everyday lane must execute the real native GPU path and one browser
  WebGPU path, not CPU substitutes.
- Cross-platform work is sampled by risk: one browser and one available
  physical Android device are sufficient for a release/default change unless
  a failure or Android-specific question requires an AVD comparator.
- Performance smoke reports finite timing and exact work/capacity facts with a
  generous wall-time budget. Hardware-specific regression claims require the
  governed benchmark/evidence lane.
- A failed candidate stops expensive qualification after the cheapest
  authoritative blocking result unless human review explicitly requests more
  characterization.

## Ordered delivery

### S20.1 — capability and validation contracts

- [x] Add one durable current-capability ledger with stable feature IDs and
  implemented/limited/not-implemented/deferred status.
- [x] Declare everyday, targeted, pre-push, platform, audit, and promotion
  tiers with explicit ownership and budgets.
- [x] Make S21 the next executable physics workstream.

Gate: current product state and missing foundational features can be understood
without reading completed solver tacticals.

### S20.2 — production GPU test diet

- [x] Retain min/default/max soft resolution replay in production validation
  and move the full 12-arm matrix to the opt-in audit lane.
- [x] Retain one maximum-resolution grab/construction boundary plus the full
  default reset/grab script; move the duplicated six-arm interaction matrix to
  the opt-in lane.
- [x] Keep 30/60/120 rate replay, deterministic soft floor, readback-boundary,
  capacity, and shared-host interaction checks.

Gate: production validation still crosses configuration boundaries and the
real GPU interaction path while exhaustive research combinations run only when
their owning code changes.

### S20.3 — everyday and performance-smoke commands

- [x] Add `npm run test:fast` for format, maintained Rust tests, fast GPU
  performance smoke, tooling tests, platform boundaries, and browser WebGPU.
- [x] Enforce a 120-second hard wall budget and emit per-lane timing markers.
- [x] Add `npm run test:perf:smoke` as a targeted filter over the body-pipeline
  timing and exact soft-floor work/capacity tests already run once inside
  `test:fast`, not as an additional everyday sublane.
- [x] Add concise browser-smoke output for the everyday lane while retaining
  complete JSON provenance on disk.

Gate: one command produces a fresh native/browser result and a machine-readable
timing summary; the focused performance command completes in seconds on the
named development host.

### S20.4 — proportional cross-host policy

- [x] Replace the monolithic ordinary gate list with everyday and risk-based
  additions in `AGENTS.md`, README, and the validation topic.
- [x] Reserve APK/device/AVD, full workspace, oracle matrices, and long
  600/1,200-step evidence for their declared lifecycle triggers.
- [x] Preserve a direct command for every retained opt-in lane.

Gate: a small shared edit does not imply an APK, physical device, AVD, full
workspace, and evidence campaign; an intentional default or portability change
still receives one representative real-device qualification.

### S20.5 — measurement and closure

- [x] Measure warm `test:fast`, focused performance smoke, production Rust,
  browser smoke, and the retained exhaustive matrices separately.
- [x] Run formatting, production tests/clippy, WASM, Android compile, headless,
  boundaries, browser, and targeted script/tool tests for this cleanup.
- [x] Record before/after production matrix scope and measured wall time, then
  mark S20 complete without running unrelated historical evidence.

Gate: warm everyday validation is below 30 seconds on the named M4 Pro and the
hard budget is at most two minutes; product behavior/configuration hashes are
unchanged; S21 may start.

## Completion condition

S20 completes when the comprehensive capability ledger is current, ordinary
GPU matrices are minimal but boundary-complete, `test:fast` passes within its
budget with native and browser GPU execution, focused performance facts are
visible, cross-host triggers are proportional, exhaustive lanes remain
invocable, and the product scene/configuration contracts are unchanged.

## Closure evidence

Measured on the Apple M4 Pro/Metal development host with a warm target tree:

| Lane | Before | After |
| --- | ---: | ---: |
| Maintained production Rust command | 6.44 s | 2.18 s |
| Engine tests inside that command | 5.83 s | 2.07 s |
| Ordinary browser smoke | 4.03 s | 1.91 s inside the final fast run |
| Complete `test:fast` | not present | 7.31 s |
| Focused two-probe performance smoke | not present | 1.42 s |
| Retained exhaustive resolution/interaction matrices | ordinary production coverage | 5.26 s test / 7.09 s command, opt-in |

Production coverage changed from all 12 resolution combinations plus six
duplicated grab/construction combinations to three min/default/max resolution
anchors plus one maximum-resolution interaction boundary. The exhaustive
tests remain directly invocable and pass. The unchanged default reset/grab,
30/60/120 Hz, deterministic soft-floor, capacity, and readback-boundary tests
remain in production.

Final `test:fast` run
`b7395b40-5639-4ef0-9f36-501e68f85920` passes format, 133 maintained Rust
tests, both performance-smoke probes, 16 tool tests, platform boundaries, and
Chrome WebGPU. Its browser and fresh Metal headless reports retain canonical
soft configuration hash
`42c9e256d6c684148edd79ca18ddb7f569b53e745edfef0592842ce8ca783e9f`,
68 dispatches, `1,234,468` solver bytes, exact clean capacity, and the same
scene hashes. Production clippy with warnings denied and the ARM64 Android
compile check also pass. The fresh headless artifacts are under
`/tmp/physbox5-s20-smoke.cVvLM5/`. No APK, device, AVD, full workspace, or
historical evidence sweep ran because this cleanup changes no product runtime,
physics, shader, buffer, Android adapter, or pixels.
