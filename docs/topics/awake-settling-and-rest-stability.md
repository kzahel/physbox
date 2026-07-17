# Awake settling and rest stability

Status: mandatory S6.5 review, 2026-07-17. Human review authorized S6.1 through
S6.4 and accepted the bounded parallel scheduler as the baseline. S6.4 is
complete, but its AVD 16-stack result blocks contact-policy promotion.
Awake settling is a prerequisite for broadphase selection and sleeping
acceptance. No contact-identity policy, soft-contact coefficients, penetration
deadband, friction anchor, residual band, or sleep threshold is a product
default by virtue of this document.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Execution tracker:
[S6 awake settling stabilization](../tactical/006-awake-settling-stabilization.md).
Contact scheduling remains owned by
[contact solver parallelism](contact-solver-parallelism.md), contact identity
by [collision and contact persistence](collision-and-contact-persistence.md),
and equivalent-quality timing by
[performance and scale](performance-and-scale.md).

Scheduler prerequisite, 2026-07-17: the clean
[S6.1 promotion package](../../evidence/p5.2b-s6.1-scheduler-promotion/README.md)
qualifies bounded parallel scheduling as the product baseline and preserves
the serial direct control. Settling attribution therefore begins on the
intended product order; policy experiments may still compare the serial lane
but may not silently substitute it.

## The concern

The selected colored manifold-block solver preserves the governed 2-, 4-,
8-, and 16-body structures, but structural survival is not the same as rest.
Deployed review finds centered rectangles visibly bouncing or oscillating long
after impact. Sleeping is not implemented, and must not be used to conceal an
awake solver that continually recreates its own motion.

The clean S5.4c settled windows show a depth-correlated problem under the
current physical settings:

| Fixture | Maximum linear speed | Contact-feature churn |
| --- | ---: | ---: |
| centered 2-stack | `5.01e-7 m/s` | 0 |
| centered 4-stack | `0.0365 m/s` | 20 |
| centered 8-stack | `0.1240 m/s` | 64 |
| centered 16-stack | `0.3348` to `0.4420 m/s` | 140 to 148 |

The 16-stack range covers the accepted serial schedule and the
quality-qualified bounded parallel scheduler. All rows preserve structure;
the table does not claim that feature churn alone causes every residual. The
clean pinned Box2D authority stays in the low-microvelocity band on the
governed awake structures, so the gap remains a quality defect rather than an
expected absence of sleeping.

## What is established

Pair persistence itself is generation-safe. A pair slot cannot transfer state
through erase/reuse, and the S5.3 corpus exercises erase, reuse, and recontact.
Within a live pair, however, `match_cache` transfers an accumulated point
impulse only when the complete feature tuple matches exactly. This is the safe
default: an impulse must not be transferred blindly across a materially
different contact normal or topology.

Rectangle SAT normally selects its reference polygon by direct comparison of
the two maximum separations. The existing relative factor `0.98` and absolute
bias `0.001` apply only to the declared deeply coincident identical-rectangle
case. A nearly tied resting rectangle contact can therefore change reference
ownership or clipped features after small numerical motion. Such a change can
correctly miss the exact warm-start key and force the support contact to
rebuild its impulse. The observed churn and cache rule make this a strong
causal hypothesis, not yet a proven sole cause.

The product already has the major selected solver structure: four substeps,
two colored in-place iterations, coupled two-point manifold normals, warm
start, a relaxation pass, speculative contacts, bounded depenetration, and a
separate restitution pass. Its standard contact coefficients are currently
the hard formulation `bias_rate = 0.2`, `mass_coefficient = 1`, and
`impulse_coefficient = 0`.

The pinned Box2D v3 authority at revision `3a4f0da` does use soft contact
coefficients derived from contact hertz and damping ratio; its defaults are 30
Hz and damping ratio 10, with a stiffer static-contact path. That is a verified
model difference and a required comparator.

S6.4 scalar coefficient contract, 2026-07-17: the independent Rust
specification now reproduces the pinned `src/solver.h` `b2MakeSoft` equations.
For substep duration `h`, angular frequency `w = 2πf`, and damping ratio `z`, it
computes `a1 = 2z + hw`, `a2 = hw·a1`, then `biasRate = w/a1`,
`massScale = a2/(1+a2)`, and `impulseScale = 1/(1+a2)`. Analytic tests preserve
the zero-hertz result, finite-domain rejection, the
`massScale + impulseScale = 1` invariant, and monotonic stiffness response.
Following pinned `src/physics_world.c`, requested contact hertz is capped at
`0.125/h` before the static-contact multiplier is applied. The scalar solve
uses Box2D's `massScale·biasRate·separation` velocity bias. This is provenance
for a reversible comparator only; absent policy data still serializes and
executes the selected `0.2/1/0` hard formulation byte-for-byte.

S6.4 GPU comparator checkpoint, 2026-07-17: the engine derives the same three
coefficients independently from the versioned scenario policy and projects
the soft bias through the existing solver parameter surface. Both the
traceable authored solver shader and the product generated-contact
manifold-block shader select dynamic versus static coefficients from the
authoritative inverse-mass data. No new state, dispatch, allocation, readback,
solver family, or frame path is introduced. Scalar/GPU traces agree across the
bounded hertz/damping/static grid; generated execution replays exactly,
attests the candidate policy, remains capacity-clean, and is rejected by the
selected product constructor. The policy is therefore ready for factorial
measurement but is not a product default.

S6.4 factorial checkpoint, 2026-07-17: the clean
[GPU matrix](../../evidence/p5.2b-s6.4-soft-contact/README.md) measures the hard
control and eight `15/30 Hz × 5/10 damping × static×1/2` settings, then expands
both top softness settings through direct and prior-contact identity over all
11 frozen sources. Retaining both is essential: static×2 wins softness-only,
but composes poorly with temporal identity on the 16-stack. The combined
`30 Hz / damping 10 / static×1 / prior-contact` lane is the objective leader:
9/11 full gates, 11/11 structures, zero settled churn, `0.02324 m/s` worst
linear speed, and `0.00315 m/s` worst angular-edge speed. Every one of 14 fresh
random sources replays, preserves structure/capacity, and meets the final rest
band; impact, friction, restitution, and short-stack probes also pass.

This is a strong but incomplete result. The leader still has a rising-energy
failure on the 16-stack and small mixed pile, with `0.00882 m` worst measured
penetration versus `0.01150 m` for the hard control. It adds one dispatch, no
allocation, and records a 6.538 ms versus 5.984 ms 256-body Metal P50 in this
run. Those failures must be minimized and the candidate must receive fresh
authority/cross-host evidence before S6.5. No product default, physical band,
or oracle interpretation changes here.

S6.4 failure minima, 2026-07-17: deterministic depth sweeping shows that the
leader's rising-energy gate edge begins at 9 bodies but is non-monotonic at 10.
The 9-body difference is in the `7e-10` energy range; the governed 16-stack is
a materially stronger mode, with all 300 settled intervals rising and energy
growing from `4.73e-7` to `1.18e-3`. The small mixed pile reduces in 37 real GPU
attempts from 8 to 4 dynamic bodies while preserving a 300-interval rising run,
exact replay, and capacity. The
[checked minima](../../evidence/p5.2b-s6.4-soft-contact/failures/README.md) are
permanent reconsideration inputs. This evidence does not weaken the energy
gate merely because part of its boundary is small-signal.

S6.4 authority and cost, 2026-07-17: fresh isolated runs against clean pinned
Box2D replay exactly on all 11 frozen sources. Box2D's final 16-stack speed is
`0.000158 m/s`; the leader's `0.02324 m/s` meets the hard product band but is
not authority-level stillness and retains its rising-energy mode. Matched
complete-step Metal timing at 1,024 bodies is 7.917 ms for the leader versus
7.875 ms for hard/direct, with one added dispatch and identical allocation.
Optimized four-worker Box2D is 0.132 ms, leaving a roughly 60× gap. The
[raw authority/cost package](../../evidence/p5.2b-s6.4-soft-contact/authority-performance/README.md)
therefore supports a quality recommendation only, not a CPU crossover or
liquid-headroom claim.

S6.4 cross-host result, 2026-07-17: the shared launch-only hard/candidate 8-
and 16-stack lanes complete on native Metal, Chrome WebGPU, and hardware-backed
AVD Vulkan with clean capacity and the selected scheduler. The candidate adds
one dispatch and no allocation. Visual inspection preserves a portability
failure that the transport sentinel intentionally does not hide: candidate 8
remains centered on every host, Metal/Chrome candidate 16 remains centered,
but the AVD candidate 16-stack is collapsed by captured step 1,203 while its
hard control remains centered. The
[cross-host package](../../evidence/p5.2b-s6.4-settling-cross-host/README.md)
marks this promotion-blocking and leaves the product defaults unchanged. AVD
pacing is not interpreted as cost because unrelated emulator load produced
variable permanent slip; no physical Android device was connected.

## Authority corrections

Two plausible techniques must not be mislabeled as direct Box2D v3 parity:

- The pinned Box2D polygon manifold also uses a direct separation comparison
  for reference selection. It does not apply the old `0.98`/`0.001` bias
  unconditionally at that decision. A deterministic tie bias or true temporal
  hysteresis may still improve PhysBox, but it is a measured product policy.
- Box2D's linear slop is used throughout collision geometry, but the pinned v3
  contact solve does not plainly subtract a five-millimeter penetration
  deadband before soft bias. A rest deadband remains an optional experiment,
  not an assumed missing authority mechanism.

Independent invariants continue to own mathematical correctness and Box2D
continues to own the selected rigid behavior target. A Box2D-versus-invariant
conflict stops for human review; it is not resolved by voting among candidates
or widening a tolerance.

## Measurement contract

Ideal centered stacks remain because they isolate depth and support transfer.
They are joined, not replaced, by deterministic interaction-shaped sources:

- one rectangle dropped nearly flat on the floor at seeded small angles and
  offsets;
- one rectangle dropped onto one already-resting rectangle;
- sequential construction of three to five rectangles rather than a perfect
  pre-authored stack;
- a rectangle sliding to rest under friction; and
- a small seeded mixed pile, with larger buckets retained as diagnostics.

Every source is authored once in shared Rust, projected through the existing
neutral Box2D request, and replayed by native, browser, Android, and headless
hosts without host-authored physics. Baseline, candidate, and authority runs
use the same fixed step, starting state, actions, capture windows, and sleep-off
policy.

Versioned observations add reference owner and face index, point-count
transition, contact-normal change, warm-cache hit/miss reason, time to rest,
and a kinetic-energy envelope to the existing speed, drift, penetration,
separation, impulse-variation, work, capacity, and timing records. A stable
support interval means the same generational pair remains continuously in
contact without input or material normal rotation. Only unexplained identity
changes inside such an interval count against the zero-churn gate; legitimate
impact, rolling, sliding topology, separation, and recontact do not.

S6.2 contract freeze, 2026-07-17: observation version 2 samples every outer
step through step 1200. Steps 1–600 are transient; the accepted settled window
is the inclusive 900–1200 interval. Time to rest requires 60 consecutive
samples at or below both `0.025 m/s` maximum center speed and `0.025 m/s`
maximum angular edge speed. A continuously present gravity-aligned support is
eligible for identity attribution only while its normal changes by at most
`0.5°`, its maximum relative tangent speed is at most `0.025 m/s`, and no
action occurs. It becomes a stable-support interval after 30 consecutive
eligible observations. The energy envelope uses an absolute epsilon of
`1e-12`, a relative epsilon of `1e-5`, compares the maximum of the first and
last settled-window quartiles, and rejects five consecutive positive-energy
intervals. These values are measurement policy, not new solver tolerances or
physical defaults.

The version-2 trace embeds the readable version-1 settling observation and
adds actual GPU manifold and pair-slot-cache state: generational pair,
reference owner and face, prior/current point count, prior/current normal and
feature keys, cache impulses, classified cache status, continuous-support age,
angular edge speed, and aggregate time-to-rest/energy results. Bounded native
validation copies this history in GPU command order and maps it only after the
last requested step. It is not part of the interactive frame path and no CPU
result advances simulation.

The shared S6.2 fixture catalog is frozen as `near-flat-floor-drop`,
`box-on-resting-box-drop`, `sequential-rectangles-3`,
`sequential-rectangles-4`, `sequential-rectangles-5`, `slide-to-rest`, and
`small-mixed-pile`. Each is a seeded neutral `ScenarioV1`, has dense body slots,
contains no authored manifold or cache, uses the selected sleep-off settings,
and projects through the existing Box2D world projection. Sequential sources
encode staggered arrival through deterministic initial height rather than a
host-owned spawn path. The random corpus selects from the same catalog; its
minimizer preserves neutral dense slots while reducing bodies, initial
perturbations, and the failure window.

S6.2 baseline result, 2026-07-17: the clean
[baseline package](../../evidence/p5.2b-s6.2-settling-baseline/README.md)
records 11 canonical per-step GPU traces, 88 isolated pinned-Box2D runs, a
14-seed random corpus, minimized failure evidence, and synchronized playback.
Every GPU source replays exactly, matches its final-only execution, remains
capacity-clean, and preserves structure; every final authority checkpoint
also replays exactly. The 2-stack, near-flat floor drop, and slide-to-rest meet
the full gate. The other eight canonical sources preserve structure but fail
identity churn, speed, energy, or a combination.

The contact-identity attribution is now direct rather than conjectural. In
every settled-window failure, every unexplained feature-set change is also a
reference-owner change. There are no point-count changes and no material
normal changes in those intervals. Each owner flip causes two exact-feature
cache misses on the affected two-point rectangle manifold: the 4-, 8-, and
16-stacks respectively record 148/296, 351/702, and 1,127/2,254 owner
flips/cache misses. Realistic box-on-box, sequential construction, and mixed
pile sources show the same exact relationship. This establishes stable
reference ownership as the first S6.3 comparator question without claiming it
is sufficient: the 8- and 16-stacks and two sequential fixtures also fail the
energy-envelope gate, preserving an independent S6.4 damping question.

The random corpus is 14/14 exact and capacity-clean. Two distinct seeds in
the sequential-five family exceed the final rest-speed band; one reduces in
34 real GPU attempts to the checked permanent reproducer. No candidate result
was run or inferred during this baseline slice.

S6.3 identity result, 2026-07-17: the clean
[contact-identity package](../../evidence/p5.2b-s6.3-contact-identity/README.md)
proves both the causal contribution and the limit of feature coherence. The
selected direct lane has 3,068 settled reference-owner changes and exactly
6,136 resulting cache misses across the frozen corpus. A stateless relative
bias and canonical 1 mm band repair shallow stacks but lock an unsafe answer in
deep chains: the 16-stack loses order at 10.38 and 8.42 m/s. They are rejected.

True prior-contact hysteresis is implemented only as an isolated candidate. It
binary-searches the already-authoritative sorted generational pair records and
retains the prior owner only within a 1 mm separation band, with identical
point count and a compatible normal. It adds no contact state, transfers no
impulse across feature frames, adds one dispatch, and adds no allocation. It
records zero settled owner/feature changes and zero cache misses, brings the
4-stack to `4.59e-6 m/s`, and passes 7/11 hard gates. But the 8-stack remains
at `0.4005 m/s`, the 16-stack narrowly loses order at `0.0256 m/s`, and the
random final-rest score is 12/14 versus direct's 13/14. Identity is therefore
causal but not independently acceptable. Prior-contact hysteresis advances
only into the S6.4 combined factorial; the product policy remains direct.

Canonical impulse correspondence is rejected at this point. The temporal lane
already eliminates the identity misses, so cross-feature impulse transfer has
no residual question to answer and lacks the normal/geometry proof required to
be safe. Reconsider it only for a future polygon clipping case that cannot
maintain coherent identity through the bounded prior-owner rule.

## Ordered hypotheses

Experiments change one mechanism at a time before a combined candidate is
allowed:

1. Attribute reference-owner changes, feature changes, point-count changes,
   normal changes, and warm-cache misses on the new and existing corpus.
2. Compare deterministic near-tie ownership, true prior-contact hysteresis,
   and safe canonical correspondence where normals and geometry permit it.
   Exact replay and generation safety remain mandatory.
3. Compare contact-hertz/damping candidates through the existing coefficient
   surface, including static-contact behavior, without silently copying Box2D
   defaults or changing the selected product settings.
4. Combine only independently beneficial identity and softness candidates and
   measure the interaction.
5. Evaluate a penetration deadband only if bias pumping remains attributable
   after identity and softness work.
6. Evaluate friction anchors only if the remaining failure is tangential drift.
7. Design sleeping only after the awake gate passes.

This order does not preselect a winner. A candidate that looks still because
it sags, drifts, over-penetrates, loses friction, or dissipates legitimate
impact response does not qualify.

## Proposed awake completion gate

The first instrumentation slice freezes the exact observation cadence and
window arithmetic before candidate measurements. The blocking product gate is:

- every accepted structure and seeded drop fixture preserves its declared
  structure and completes without capacity failure;
- settled maximum linear speed is at most `0.025 m/s`, providing a two-times
  margin below Box2D's `0.05 m/s` default sleep threshold;
- maximum angular edge speed, angular speed multiplied by the body's support
  radius, is also at most `0.025 m/s`;
- stable support intervals have zero unexplained reference-owner or feature
  changes and no unexplained warm-cache cold restart;
- the last settled-window kinetic-energy quartile has no larger maximum than
  the first quartile, and no five consecutive observation intervals form a
  positive-energy run;
- penetration, separation, center drift, stack height, friction, restitution,
  and first-impact behavior do not regress their governed bands; and
- replay, source/projection hashes, failure atomicity, one-world ownership,
  WebGPU portability, complete-step cost, and the no-readback frame contract
  remain valid.

Approaching Box2D's low-microvelocity residual is a recorded aspiration rather
than the blocking band. The complete GPU cost is always reported; fitting
under 16.67 ms does not establish a CPU crossover or liquid headroom.

## Sequencing with scheduler, broadphase, and sleeping

The quality-qualified bounded parallel scheduler is the intended measurement
baseline once separately promoted under the S5.4c decision. The serial
full-rebuild scheduler remains a direct comparison lane so a changed
Gauss-Seidel order cannot be confused with a contact-policy improvement.

Broadphase contract work and pair-completeness tests can proceed without
product trajectories. Product-scene churn rankings, end-to-end timing, final
broadphase selection, and integration wait for the awake gate because
persistent jitter changes the candidate-pair workload being compared.
Sleeping and awake-fraction performance wait for the same gate.

## Human decisions and autonomous work

After explicit approval to begin, instrumentation, shared fixtures, reversible
laboratory candidates, failure minimization, exact replay, oracle runs,
cross-host validation, performance accounting, and per-slice commits are
autonomous. The first mandatory review occurs after the identity and softness
factorial, before any candidate becomes a product physical default. That
package must make the change visible without source inspection: synchronized
baseline/candidate/Box2D playback, direct launch controls, settled traces,
costs, minimized failures, and a specific recommendation.

Human review is also required before accepting a penetration deadband,
friction-anchor policy, new quality band, changed authority interpretation, or
sleep thresholds. A materially different solver family, accumulation
architecture, CPU fallback, native-only path, simulation-dependent readback,
or portability loss is an early stop rather than an autonomous extension.

## Reconsideration triggers

Reopen this direction when fresh evidence shows any of the following:

- feature/reference churn is not correlated with warm-cache loss or residual
  motion after direct instrumentation;
- contact softness cannot meet the awake band without unacceptable sag,
  penetration, impact response, or complete-step cost;
- the accepted scheduler changes the best contact policy or produces a
  material quality regression against the serial control;
- realistic seeded drops expose a failure absent from ideal stacks;
- the gate cannot be met inside the selected colored manifold-block family;
  or
- invariant and pinned-Box2D conclusions genuinely conflict.

Do not reconsider by hiding motion with sleeping, weakening the structural or
capacity gates, refreshing baselines to a worse result, or averaging away a
failed seed.
