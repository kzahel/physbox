# Physics configuration and solver profiles

Status: implemented product direction with one qualified custom alternative,
updated 2026-07-22. Human review selected the soft-coupled solver family, and
the shared code still uses its reviewed 60/4/i1 seed as the fresh ordinary
default. Product-owned fixed-step, substep, iteration, contact-response, and
recovery settings form one versioned bounded configuration. S25 now qualifies
60/8/i1 as an explicit sleep-off quality alternative without promoting it.
[S19](../tactical/019-soft-default-and-physics-configuration.md) records the
ordinary default deployment;
[S25](../tactical/025-bounded-awake-configuration-qualification.md) records the
new alternative and its cost.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Related contracts:
[simulation time and overload](simulation-time-and-overload.md),
[rigid quality targets](rigid-quality-targets.md),
[awake settling and rest stability](awake-settling-and-rest-stability.md),
[contact solver parallelism](contact-solver-parallelism.md), and
[performance and scale](performance-and-scale.md).

## Decision

PhysBox is now mature enough to expose a real, bounded physics configuration
instead of treating every accepted value as a source constant. The product
direction is:

- make the existing soft-coupled, point-sequential GPU path the ordinary
  product solver;
- seed its first product configuration from the interactively reviewed lane:
  60 Hz outer fixed rate, four solver substeps, one biased point-sequential
  pass plus relaxation per substep, 30 Hz contact tuning, damping ratio 10,
  and static-contact multiplier 2;
- retain hard/direct manifold-block i2 as a named comparison profile, not an
  automatic fallback and not the continuing product direction;
- replace the historical user-facing `BRANCH ROBUST` label with
  `SOFT COUPLED`; retain `branch-robust` only as a provenance alias while
  stored launch and browser preferences migrate;
- make fixed rate, substeps, solver iterations, and soft-contact coefficients
  explicit engine-owned settings with requested and effective values; and
- provide named presets plus an advanced editor through the shared
  engine-rendered UI and `physbox-launch`, with the same configuration and
  results on every host.

The human selection is a product-quality decision. S19 validated and
characterized it; it did not rerun the old promotion vote or silently return
to hard/direct because one historic candidate gate was not met. Invariants,
finite state, capacity safety, deterministic replay, portable execution, and
honest reporting remain blocking. A failure of one soft configuration may
reject that configuration, but changing the selected family again requires a
new explicit human decision.

## Evidence and accepted trade-off

The soft direction is not an evidence-free preference. Its Box2D-derived
coefficient equation and authority-structure lineage make it mechanically
closer to the rigid authority than hard/direct. On the governed 16-stack, the
60/4 soft lane measured `0.007701 m/s` settled maximum linear speed versus
`0.824075 m/s` for the former hard/direct default, and its complete-step GPU
cost was lower on the recorded 32/256/1K contact vectors.

The current seed is also known to be compliant. Rectangles can visibly overlap
in tall stacks, and the frozen 30/10/staticx2 lane loses the governed 16-stack
at step 407. The 30/10/staticx8 frontier delays loss to step 439 but does not
remove it. This is accepted as the starting product trade-off because direct
interactive review strongly prefers the quiet soft response. It is not
reported as Box2D parity, indefinite tall-tower support, or a final tuning.
Penetration, stack height, first structural-loss step, and subjective review
remain visible configuration characteristics.

The later rigid-quality decision classifies 16 as the matched-Box2D frontier
diagnostic rather than a blocking promotion gate. S24 confirms that the
current 60/4/i1 seed misses the blocking sleep-off 12-stack quiet band.
S25's bounded configuration screen then qualifies the already-supported
60/8/i1 alternative on 2/4/8/12, preserves 14, and reports first 16-stack loss
at step 840. Browser WebGPU and physical Pixel 7a Vulkan complete the same
six scenario/configuration identities. This supplies a real alternative for
human review; it does not silently change the seed.

Historical S6-S18 negative dispositions remain true within their frozen
questions. They explain limitations and preserve useful implementations; they
do not overrule the later human decision to build on the soft family.

## Configuration model

The product configuration separates values whose effects and ownership differ:

| Group | Initial fields | Meaning |
| --- | --- | --- |
| World time | outer fixed-step Hz or canonical `dt` | Amount of simulated time advanced by one accepted outer step |
| Solver resolution | substeps, family-specific biased iterations | Internal constraint work inside one outer step |
| Contact response | contact Hz, damping ratio, static-contact multiplier | Requested soft-contact behavior; effective values depend on substep `dt` and caps |
| Contact recovery | speculative distance, maximum depenetration velocity | Collision anticipation and bounded recovery from overlap; initially reported, not placed in the primary editor |
| Solver family | soft-coupled, hard/direct comparison, later explicit candidates | Discrete recurrence, ordering, preparation, and relaxation policy |
| Sleeping request | off/on request, with separate effective state | Effective on request after S23.3-S23.6; off by default pending the S23.7 promotion decision |

One canonical versioned configuration must project into `EngineConfig` and the
neutral scenario exactly once. `physbox-launch` owns names, parsing, defaults,
help, web mappings, and Android JSON argv. Platform adapters transport the
selection unchanged. They may not own rates, profile defaults, parameter
limits, or rebuild behavior.

`PhysicsConfigurationV2` now owns `fixed_step_hz`, `substeps`, `iterations`,
the contact values projected into `SolverSettingsV1`, and the experimental
sleeping request; there is no second platform-local settings model. Version 2
was required because the serialized configuration gained that field. The scene
contract and validation artifacts record:

- selected profile id and human label;
- outer fixed-step Hz and exact `dt`;
- substeps and internal substep `dt`;
- requested and effective dynamic/static contact Hz;
- biased-iteration and relaxation structure;
- complete contact coefficients and recovery policy;
- passes, dispatches, work, and allocation implied by the configuration; and
- requested/effective sleeping state plus sleep policy/layout versions; and
- configuration version/hash so a screenshot or metric is reproducible.

At the inert S23.2 checkpoint, `--sleeping on|off` and the web query transport
the request, and `MENU → WORLD → SLEEP REQUEST` provides the same
engine-rendered control. It atomically rebuilds the shared configuration,
preserves orthogonal solver/capacity/tool state, persists in the version-3
browser preference, and reports requested and effective values separately.
Effective sleeping was hard-false. This established the representation and
transport control without enabling the feature or making a default decision.

S23.3-S23.6 subsequently made the request effective through the same
configuration and control. Sleeping remains off by default, and final
promotion remains owned by S23.7.

World capacity is a separate engine setting because it changes fixed GPU
storage and broadphase work rather than the solver equation. The shared launch
contract and engine-rendered WORLD page expose only four reviewed presets: 64
(default), 256, 1K, and 10K slots. Applying one atomically rebuilds and restarts
the current scenario while preserving camera and tool state. If the authored
initial body count does not fit the requested preset, the effective capacity
is the next power of two above that count; both requested and effective values
are reported. There is no automatic runtime growth or platform fallback.

Browser world preference is versioned independently as `physbox5.world.v1` so
solver-profile migration cannot silently change storage scale. Non-default
capacity is mirrored into the shared `body-capacity` launch state for
reproducibility. The complete cost and physical-device rationale for the safe
64-slot default live in the
[performance topic](performance-and-scale.md#interactive-world-capacity-policy).

The complete physics preference is `physbox5.physics.v3`. It adds the sleeping
request and migrates version 2 with sleeping off, preserving the ordinary
default while keeping the launch URL reproducible.

## Initial bounds and presets

The first product surface is deliberately bounded rather than an arbitrary
numeric console:

- outer fixed rate: a bounded integer-Hz control, provisionally 15 through
  240, with named 30, 60, and 120 Hz presets;
- substeps: any integer 1 through 8 in the product editor, with up to 16
  retained for explicit laboratory work;
- biased iterations: any integer 1 through 4 in the product editor, with up to
  8 retained for explicit laboratory work; and
- finite positive contact values inside engine-owned limits, always showing
  the effective cap.

The initial profile list is deliberately small:

1. **SOFT COUPLED** — selected product direction, initially 60/4/i1 and
   30/10/staticx2.
2. **HARD DIRECT** — the existing 60/4/i2 product retained as a comparison and
   rollback diagnostic, never selected automatically.

No **SOFT HIGH QUALITY** preset ships yet. The earlier 60 Hz matrix showed no
monotonic winner: 4/i2 delayed the then-governed 16-stack loss while slightly
increasing ordinary-stack motion; 8/i1 and 8/i2 delayed first loss but became
violently active after loss and cost substantially more. S25 asks the narrower
current question and establishes that 8/i1 does clear the 12-stack awake gate,
with 15-43% higher fully observed validation-wall cost across the four blocker
depths. That measured quality/cost trade-off is now ready for human default
review, not automatic preset creation. The existing 30/10/staticx1 response
remains representable as an ordinary custom soft configuration. Additional
presets are admitted only when they offer a distinct, reviewed trade-off; a
grid cell is not a product preset merely because it exists.

## Runtime and pacing semantics

The fixed rate is configurable between runs or atomic world rebuilds, but it
remains fixed while a world advances. Applying a changed rate, profile,
substep count, or iteration count restarts or rebuilds the GPU world at an
explicit boundary. It never changes `dt` halfway through an encoded step.

The zero-or-one interactive outer-step rule is unchanged. A 120 Hz selection
on a host presenting at 60 Hz runs at approximately half real-time rather than
encoding two recovery steps in one frame. Solver substeps remain internal work
and may increase without becoming catch-up. The UI reports effective
simulation scale so a high-quality configuration cannot appear to sustain
real time by silently slowing the world.

The shared `SimulationClock` must be constructed from the selected fixed `dt`;
hardcoded 60 Hz admission, report, and frame parameters are migration defects.
Pause, single-step, lifecycle rebase, permanent overload slip, and explicit-
step validation retain their existing meanings for every configured rate.

## User experience and persistence

The shared canvas menu remains the only interactive settings surface. It must:

- show the active named profile and whether edited values differ from its
  preset;
- expose `SIM HZ`, `SUBSTEPS`, `ITERATIONS`, contact `HZ`, `DAMPING`, and
  `STATIC SCALE` with requested and effective values where they differ;
- explain cost-relevant derived facts compactly: internal substep Hz/pass
  count and whether the current host is sustaining the requested rate;
- apply changes atomically without a page reload and preserve camera/tool
  state when safe;
- restore the versioned local browser preference and migrate the old
  `branch-robust` key without changing the default for a fresh browser; and
- offer one action to restore the selected preset.

The ordinary default and the inspection UI are no longer separate physics
products. Native, browser, Android, and headless all use the same registry and
configuration projection; feature flags may hide experimental presets but may
not change the ordinary soft default.

## Validation and acceptance

Configuration increases the matrix, so validation must use deliberate tiers:

- exhaustive contract/parser/range tests and deterministic clock tests for
  every supported outer rate;
- exact replay and scenario/configuration hash tests for every shipped preset;
- a short cross-product covering outer rates 30/60/120, substeps 2/4/8, and
  iterations 1/2 without claiming every combination is a recommended preset;
- default soft and hard comparison rows on stacks, buckets, impacts, friction,
  restitution, reset/grab, and construction;
- fresh penetration, stack-height, quietness, and first-loss measurements that
  record rather than conceal the accepted tall-stack limitation;
- Metal, browser WebGPU, physical Android, and available AVD execution for the
  ordinary default, plus compile/smoke coverage for every shipped preset; and
- Box2D comparisons with matched outer `dt`, substep count, source state, and
  actions whenever a parity claim is made.

The first soft-default change records measured hard-before/soft-after response,
work, timing, memory, and capacity results in its commit and review artifact.
Subjective review is a required result, not an informal note after objective
metrics.

## Performance accounting

Every configurable result reports both cost per outer step and cost per
simulated second. The latter prevents 30/4, 60/4, 60/8, and 120/4 from being
compared as if they perform equal work over equal simulated time. Reports also
name contact regeneration frequency, internal substep rate, biased and
relaxation passes, dispatches, active contacts, memory high-water, effective
interactive scale, and slipped wall time.

A quality preset is allowed to be more expensive, but it must expose that cost
and must not silently become the basis of a scale or sustained-rate claim.
Scale curves remain correctness- and capacity-gated under the performance
topic.

## Non-goals

- No CPU physics backend, alternate platform solver, or automatic hard-solver
  fallback.
- No adaptive `dt`, variable step within a run, or interactive multi-step
  catch-up.
- No claim that a higher substep or iteration count is monotonically better.
- No hiding soft overlap or structural loss with sleeping, rendering offsets,
  wider tolerances, or omitted diagnostics.
- No general Box2D API compatibility; matching configuration makes comparisons
  meaningful but does not turn PhysBox into a wrapper.
- No unbounded settings UI or platform-specific preference store.

## Reconsideration triggers

Revisit the selected seed values when direct review finds a materially better
shipped preset, a new product scene exposes an unacceptable response, matched
Box2D evidence identifies a specific correctable divergence, or performance
shows that the default cannot sustain a useful envelope on a named target.

Revisit the selected solver family only by explicit human decision with the
candidate available in the shared interactive surface. A failing configuration
must never cause an implicit family switch or platform fallback.
