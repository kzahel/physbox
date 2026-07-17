# Awake settling and rest stability

Status: active measurement workstream, 2026-07-17. Human review authorized S6.1
through S6.4 and accepted the bounded parallel scheduler as the baseline.
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
