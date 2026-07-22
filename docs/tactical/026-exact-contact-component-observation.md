# S26 exact contact-component observation

Status: active, 2026-07-22. Human review authorizes only stopping point 1 from
[GPU sleep-island connectivity research](../topics/gpu-sleep-island-connectivity.md):
an observation-only exact GPU component rebuild with CPU-reference validation
and measured cost. It must not affect wake, admission, solving, rendering, or
the product sleeping default.

## Goal

Measure whether exact connected components of the current dynamic-body contact
graph can be rebuilt correctly, deterministically, and cheaply enough to
justify a later sleep-island decision. The stopping result is evidence, not a
product candidate.

## Exclusions and ownership

- Keep S23's K=4 wake and admission behavior byte-for-byte authoritative.
- Do not reduce wake causes or readiness by component, add an exact-island
  behavior switch, change solver/default configuration, or resume S23.7.
- Reuse the engine-owned generated-contact projection. The probe may observe
  one edge per committed dynamic-dynamic manifold, but it may not create a
  second scene, contact generator, or platform implementation.
- Product graphs allocate and dispatch no component-probe resources. Only an
  explicit validation/laboratory construction enables the probe.
- CPU traversal is test/evidence tooling only and may read back after a bounded
  run; no interactive frame depends on GPU results returning to the CPU.

## Frozen observation contract

- Vertices are current live non-fixed body slots.
- An edge is one current committed dynamic-dynamic manifold. Scalar point 1 of
  a two-point manifold is not a second edge, and a fixed floor never connects
  independent piles.
- The exact diagnostic label is the minimum live body slot in the component.
- GPU construction uses monotonic minimum-root hooking plus pointer shortcut
  passes under a capacity-derived finite phase bound. It contains no unbounded
  in-shader spin or host convergence loop.
- An independent final GPU check reports invalid roots and any observed edge
  whose endpoint labels differ. Either count invalidates the sample; capacity
  or convergence failure may not silently produce partial labels.
- Report dynamic vertices, unique edges, component count, largest component,
  executed and last-changing phases, hook/shortcut work, dispatches, parent and
  scratch bytes, and component-stage GPU P50/P95/P99.

## Ordered delivery slices

### S26.1 — contract and scalar proof

- [x] Freeze the graph, label, exclusion, failure, and evidence contracts.
- [ ] Add exhaustive small-graph and adversarial chain/star/pile-model tests
  for the fixed phase bound and deterministic minimum labels.
- [ ] Record the bound beside the implementation and fail closed if capacity
  arithmetic cannot represent it.

Gate: exhaustive connected and disconnected graphs through the practical
small-graph limit match a CPU breadth/depth traversal exactly.

### S26.2 — opt-in GPU observer

- [ ] Add an opt-in generated-contact stage that initializes parents, performs
  bounded hook/shortcut passes, flattens labels, accumulates component sizes,
  and validates roots/edges.
- [ ] Expose compact diagnostic readback and timestamp scope without routing
  any value into sleep, solve, or render decisions.
- [ ] Prove through construction tests that the ordinary product graph has no
  observer allocation or dispatch.

Gate: exact replay, zero invalid roots, and zero cross-label contact edges.

### S26.3 — CPU differential and fixtures

- [ ] Compare GPU labels with an independent CPU traversal after bounded runs.
- [ ] Cover isolated bodies, chains, two piles sharing a fixed floor, a poured
  pile, an arch/bridge, component split/removal, and contact churn.
- [ ] Report the first divergent body or edge and fail the command on any
  mismatch, overflow, stale generation, or nondeterminism.

Gate: every admitted fixture and repeated run matches exact minimum labels.

### S26.4 — cost evidence and stopping report

- [ ] Measure P50/P95/P99 observer GPU time, full physics time, dispatch count,
  allocated bytes, phase usage, and atomic-work proxies at representative
  256/1K/10K capacities and applicable pile/sleeping-city rows.
- [ ] Run proportional engine, clippy, browser, headless, ARM64, release APK,
  and available Android hardware validation required by the touched boundary.
- [ ] Record correctness, cost, limitations, and a recommendation, then stop
  before component reductions or a behavior switch.

Gate: evidence is provenance-complete and separates reserved capacity from
live work. A correct but expensive result remains a valid observation outcome.

## Completion condition

S26 completes when the observation-only GPU labels match the independent CPU
reference on the admitted fixtures, timing and resource costs are recorded,
ordinary product behavior remains unchanged, and the repository returns to a
human decision. Completion does not authorize stopping points 2–5 from the
research topic.
