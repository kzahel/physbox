# Rigid quality targets

Status: decided current acceptance contract with one qualified custom
configuration, 2026-07-22. This topic owns the awake rigid stack hierarchy for
new implementation and promotion work. The ordinary soft-coupled default
misses the native 12-stack quiet observation; a bounded eight-substep custom
configuration now passes 2/4/8/12 and has matching browser/physical-Android
product-path evidence. Human review retains the four-substep default and
accepts an 8-box awake quality claim plus demonstrated 14-box structural
survival as the current limitation; the 12-stack target itself is unchanged.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Behavior authority:
[Box2D rigid authority](box2d-rigid-authority.md).
Settling evidence:
[awake settling and rest stability](awake-settling-and-rest-stability.md).
Sleeping boundary:
[GPU sleeping and waking](gpu-sleeping-and-waking.md).
Cost and scale claims:
[performance and scale](performance-and-scale.md).
Execution tracker:
[S24 current rigid quality qualification](../tactical/024-current-rigid-quality-qualification.md)
and the authorized
[S25 bounded configuration follow-up](../tactical/025-bounded-awake-configuration-qualification.md).

## Decision

New rigid acceptance uses this explicit centered-stack hierarchy:

| Dynamic rectangles | Role | Consequence |
| ---: | --- | --- |
| 2, 4, and 8 | Ordinary correctness and regression foundation | All must preserve their declared structure. Passing only these shallow cases does not close the product-quality gate. |
| 12 | Blocking awake product-quality target | The selected product configuration must pass this sleep-off fixture before comprehensive rigid acceptance or liquid work can be claimed. |
| 14 | Stretch target | Always report it in a promotion package, but failure alone does not veto an otherwise qualified 12-stack product result. |
| 16 | Box2D-frontier diagnostic | Retain it for comparison, first-loss analysis, penetration, drift, and solver research. It is not a blocking product gate. |
| 17 and 32 | Failure-boundary diagnostics | Use them to characterize a solver or architecture change, not as ordinary product acceptance gates. |

Twelve is deliberately deeper than the 8-stack foundation while leaving
meaningful margin below the matched reference cliff. Fourteen keeps pressure
on near-frontier support propagation without making that frontier the minimum
portable product. This balance can be revised only from product and cost
evidence, not from a candidate's convenience.

These are discrete scenario roles, not percentages. A 12-stack is not
described as "75% of Box2D," because stack behavior is nonlinear and the
measured 15-to-17 transition is a cliff rather than a smooth quality scale.

The blocking target is explicitly **awake**. Sleeping is disabled in the
acceptance lane, every dynamic body remains awake, and sleep-assisted
structural survival can never satisfy the 2/4/8 foundation or 12-stack gate.
Sleep-enabled runs remain important product-behavior evidence, but any
different outcome is labeled as masking the awake result.

## Reference fixture contract

The hierarchy uses the existing engine-owned centered rectangle stack:

- identical dynamic rectangles, 0.7 m wide by 0.25 m high;
- centered, upright, initially touching, with the shared exact mass and
  inertia projection and friction 0.5;
- a fixed horizontal support, gravity, warm starting, and no authored motion;
- 1,200 outer steps for the ordinary result; and
- the selected product configuration, with sleeping disabled, for PhysBox
  qualification.

`centered_stack_structure` remains the structural predicate. The complete run
must remain finite and capacity-clean, preserve body order and the connected
support chain, keep every rectangle within the upright range, and avoid
escape. Structural survival is necessary but not sufficient: settled speed,
angular-edge speed, penetration, drift, stack height, energy behavior, replay,
and complete cost remain reported under their owning contracts.

A product acceptance claim requires fresh 2/4/8/12 sleep-off results for the
selected default on a named native GPU, the ordinary browser WebGPU lane, and
a physical Android device. The 14- and 16-stack results accompany the package
as stretch and diagnostic evidence. A routine code-change gate may exercise a
smaller subset; that narrower cadence cannot be presented as milestone
qualification.

## Current selected-default checkpoint

S24 ran a clean native blocking discriminator at revision
`66d85dde3449897dcda9ec80de46f8a0aa245992` on the AMD Radeon 890M Graphics
RADV/Vulkan adapter. The selected soft-coupled 60/4/i1, contact 30 Hz,
damping 10, static-contact x2 recurrence ran sleeping off for exactly 1,200
outer steps per stack under the existing `0.025000 m/s` quiet limit:

| Stack | Structural result | Last-quarter maximum linear speed | Verdict |
| ---: | --- | ---: | --- |
| 2 | pass | `0.001747 m/s` | foundation pass |
| 4 | pass | `0.007952 m/s` | foundation pass |
| 8 | pass | `0.021373 m/s` | foundation pass |
| 12 | pass | `0.029767 m/s` | **blocking quiet failure** |
| 14 | pass | `0.028236 m/s` | reported stretch quiet failure |
| 16 | first structural loss at step 463 | `0.009305 m/s` after loss | reported frontier failure; post-loss quiet value is not a settling pass |

All rows remained finite and capacity-valid. The result demonstrates why
structure alone is insufficient: the 12-stack looks upright but retains more
motion than the accepted awake quiet band. Per S24's predeclared stop, no
durable product package, browser row, or physical-Android row was produced.
Those missing hosts are neither passes nor inferred failures.

This blocked product qualification without changing the target, threshold,
solver, or default. At that checkpoint S23.7 remained held until human review
authorized an awake solver/configuration follow-up and a fresh native 12-stack
attempt. The exact commands and full disposition are recorded in S24.

Human review subsequently authorizes S25 as that bounded configuration-only
follow-up. It freezes six evidence-driven rows, retains this topic's bands and
sleep-off rule, and leaves the default unchanged unless a later human decision
promotes a fully qualified finalist.

## Qualified custom-finalist checkpoint

S25 selected exactly one custom configuration without changing the solver
family or equation: soft-coupled 60 Hz, eight substeps, one biased iteration,
contact 30 Hz/damping 10/static x2, sleeping off. Clean revision
`c71eabc1c3d756c19edd75114aa38d0b8dbeae4d` projects configuration hash
`87dfec84f20c6adfab96618809e79fbc65da83220f81c34b6477ec12db9ed6b2`
through the same canonical profile and centered-stack source used by the
shared host.

| Stack | Structure | Settled linear | Angular edge | Result |
| ---: | --- | ---: | ---: | --- |
| 2 | pass | `0.000957 m/s` | `0.000672 m/s` | foundation pass |
| 4 | pass | `0.004333 m/s` | `0.001736 m/s` | foundation pass |
| 8 | pass | `0.010482 m/s` | `0.002915 m/s` | foundation pass |
| 12 | pass | `0.017233 m/s` | `0.005768 m/s` | **blocking awake pass** |
| 14 | pass | `0.014716 m/s` | `0.006246 m/s` | stretch pass |
| 16 | first loss at step 840 | `1.868055 m/s` after loss | `1.783489 m/s` after loss | reported frontier failure |

Every row is finite/capacity-clean and exactly replays. All four blockers
pass the default-relative penetration/height guard. Depths 2 and 4 regress the
energy-envelope boolean while ending at materially lower absolute energy than
the four-substep controls; this remains a disclosed limitation, not a hidden
pass field.

The eight-substep observation-series wall cost is `4.514/4.515/4.472/4.887
ms` per blocker step, versus `3.928/3.404/3.332/3.411 ms` for same-revision
four-substep controls: approximately 15/33/34/43 percent higher. These
instrumented validation timings expose the cost direction but are not an
uninstrumented frame-rate claim. Browser WebGPU and physical Pixel 7a
Mali-G710 Vulkan complete all six exact shared scenario identities with the
same configuration/scenario hashes, clean capacity, and final captures. The
12 capture remains upright and the 16 capture exposes the diagnostic loss;
native readback continues to own numerical quiet/structure.

This is outcome evidence for a custom setting, not a default change. Human
review compared its quality margin and visibly higher work, declined
promotion, and retained four substeps. The default is described conservatively
as an 8-box awake quality stack. Its 12- and 14-box rows demonstrate upright
1,200-step structural survival but fail quietness; 15 is unmeasured in the
final default run, and 16 loses structure at step 463. This accepts a current
product limitation without moving the 12-stack acceptance target or claiming
that 14 is a full quality pass.

## Why 16 is a diagnostic, not the product gate

The old S5-S18 program repeatedly used 16 as an all-or-nothing solver gate.
That choice was useful for exposing deep support-transfer, ordering,
warm-start, precision, and portability failures, but it accidentally turned a
reference-engine frontier into the assumed minimum product capability.

A fresh 2026-07-22 diagnostic swept the clean pinned Box2D source at
`3a4f0da8374af61293a03021c9a0b3ebcfe67948` through the exact centered fixture.
It used the `world-awake` lane, one worker, 60 Hz, four substeps, warm starting
on, continuous collision on, contact softening off, and sleeping disabled at
both world and body level. Under the shared structural predicate:

- every height from 2 through 16 remained valid for 1,200 steps;
- every height from 17 through 64 failed by step 1,200;
- depth 17 first failed at step 416 and depth 32 at step 246;
- depth 16 remained structurally valid and awake through 100,000 steps; and
- the depth-15/16 horizontal-drift change was nonlinear: approximately
  `0.00831 m` at 15 versus `0.10002 m` at 16 after 1,200 steps.

At 1,200 steps, depth 16 had approximately `0.000158 m/s` maximum speed,
`0.100015 m` maximum horizontal drift, and `0.037722 rad` maximum angle. Its
100,000-step continuation had approximately `0.0000338 m/s` maximum speed,
`0.102181 m` maximum horizontal drift, and `0.038574 rad` maximum angle.

This was a clean decision-calibration source probe, not a newly checked
product acceptance artifact or tolerance. A future qualifying package must
still emit the ordinary run id, hashes, complete settings, and host/source
provenance.

This establishes 16 as the boundary of this exact matched Box2D fixture. It
does **not** establish a universal Box2D tower limit: shape dimensions,
alignment, friction, mass distribution, time step, substeps, solver settings,
and the structural predicate can all move the boundary. Box2D remains the
declared behavior authority without making its most severe passing fixture an
automatic PhysBox product requirement.

## Baseline transition

Before this decision, the historical settling work treated preservation of
the governed 16-stack as a blocking promotion condition. After this decision,
new work uses 12 as the blocking awake stack, 14 as stretch, and 16 as a
non-blocking frontier diagnostic. No solver equation, product configuration,
physical tolerance, oracle source, or authority precedence changes here.

S5-S18 artifacts and tactical checkboxes remain immutable evidence under the
contracts they declared. Their negative 16-stack results still diagnose real
limitations and may motivate future solver work; they no longer veto current
product acceptance by themselves.

The selected soft-coupled 60/4/i1, contact 30 Hz/damping 10/static x2 default
is not qualified under the new hierarchy. S24 supplies its fresh native
2/4/8/12/14/16 discriminator and 12-stack quiet failure. S25 does not rewrite
that result: it qualifies the explicit 60/8/i1 custom alternative across the
native, browser, and physical-Android paths. Its 16-stack loss at step 840
remains diagnostic rather than blocking. Human review declined promotion and
retained the default with the explicit 8-quality/14-structural limitation
above.

## Validation and reporting

Every current qualification package records:

- the complete physics configuration and sleep-off state;
- the result of each 2/4/8/12/14/16 role without omitting a failure;
- first structural-loss step for a failed stack;
- settled speed, angular-edge speed, penetration, drift, height, and energy;
- source, scenario, configuration, and result identity plus exact replay;
- work, dispatches, allocation, capacity high-water, overflow, and timing; and
- native, browser, and physical-Android provenance for the blocking lane.

Performance comparisons use equivalent accepted quality. A faster lane that
fails the 12-stack cannot supply a correctness-qualified performance claim,
and a sleeping lane cannot replace its awake result. Conversely, failing the
14- or 16-stack does not invalidate a performance row whose declared quality
requirement is the passing 12-stack; the stretch/diagnostic failure remains
visible alongside it.

## Reconsideration triggers

Revisit the hierarchy only with an explicit measured decision. Relevant
triggers include:

- fresh matched Box2D evidence moving the exact fixture boundary materially;
- representative product construction showing that 12 is either insufficient
  or needlessly severe for ordinary sandbox structures;
- joints, polygons, compounds, or fluid coupling creating a more relevant
  dependency-depth fixture;
- a selected solver architecture making 14 routine at acceptable portable
  cost, justifying promotion of the stretch target; or
- cross-host evidence showing that the 12-stack does not distinguish healthy
  from unacceptable behavior consistently.

Do not move the gate because a candidate misses it, because sleeping preserves
the structure, or because a larger diagnostic happens to pass on one adapter.
Record the before/after evidence, product benefit, and cost whenever the
hierarchy changes.
