# Awake settling and rest stability

Status: S6.5, S9, S10, S11, S14, and S15 are closed as negative results; S16
is an active measurement-only authority-event-structure decomposition.
S12's
coupled soft scalar authority endpoint qualifies, S13 rejects its one exact
GPU execution candidate on frozen response quality, and S14 rejects its sole
branch-robust formulation at the scalar robustness gate, 2026-07-19.
S11's measured rewind correction space is exhausted with no survivor. The
comparator-engine trigger remains recorded but no comparator is authorized.
The S6.5d soft candidate and all S9/S10/S11 material candidates are rejected;
restitution timing is byte-inert on the governed controls.
Hard/direct manifold-block i2 remains the product. S15 reaches its human
checkpoint with no two-float candidate admitted and returns an authority-
event-structure decomposition addendum for possible human authorization. No
candidate is promoted or authorized for GPU or cross-host expansion.
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

S6.5 review package, 2026-07-17: the clean
[decision report](../../evidence/p5.2b-s6.5-settling-review/README.md) pins every
S6.4 input and presents current hard/direct, the combined candidate, and clean
Box2D at synchronized steps 600, 900, and 1,200. It records the candidate's
9/11 Metal hard gates, zero settled churn, `0.02324 m/s` worst accepted-window
speed, 7.917 ms 1K Metal P50, severe 16-stack rising-energy mode, minimized
four-body mixed-pile energy failure, and 5/6 candidate cross-host structure
captures. The recommendation is to keep the candidate as the laboratory
leader but hold promotion while the AVD candidate-16 collapse is minimized
and explained. This is the mandatory human checkpoint; it does not authorize
a coefficient, identity, quality-band, or product-default change.

S6.5 human decision, 2026-07-17: accept the recommendation. Keep
`prior-contact-hysteresis / 30 Hz / damping 10 / static×1` as the laboratory
leader, hold promotion, and diagnose/minimize the AVD divergence. The first
diagnostic compares the AVD collapse with the retained 16-stack and four-body
rising-energy modes to determine whether Vulkan merely crosses the same
feedback threshold earlier or exposes a distinct stateful-identity defect.
Reversible laboratory instrumentation and corrections are authorized. A
coefficient retune, wider identity band, changed solver/scheduler, tolerance
or gate change, portability exception, or product promotion still requires
human review.

S6.5a factorial attribution, 2026-07-17: clean direct shared controls show the
16-stack hard/direct policy centered on Metal, Chrome, and AVD. Prior-contact
identity alone collapses on all three; softness alone collapses on Metal/AVD
and is strongly leaning on Chrome. The combined candidate restores the stack
on Metal/Chrome but still collapses on AVD. This rules out either component as
an independently safe fix and rules out a simple interaction-only failure:
the apparent desktop win is a backend-sensitive cancellation between two
individually unstable changes. The
[checked package](../../evidence/p5.2b-s6.5a-contact-attribution/README.md)
keeps promotion blocked.

The bounded S6.5a trace contract uses the same generated-contact body storage,
construction graph, selected solver/scheduler, and shared scenario builders as
the product world. It records every exact step with a one-outer-step command
submission cadence matching interactive execution, but performs no CPU mapping
until step 1,200 completes. A native adapter runs that contract on Metal and a
native Android adapter runs it on Vulkan; both consume the engine-owned four
factorial policies and the permanent minimized mixed-pile source. This is
diagnostic instrumentation rather than another engine or an interactive
readback dependency.

The clean bounded traces resolve the first S6.5a causal question. Candidate
body values differ across Metal and Vulkan from step 1, but their contact
feature topology and cache-key sets remain identical through step 796. By then
the AVD has already completed 298 consecutive energy increases under the
frozen S6.2 epsilon, beginning at step 499; its first identity transition is
step 797 and structural loss follows at step 892. The permanent four-dynamic-
body reproducer independently rises for all 300 settled intervals on both
backends with zero settled feature churn and zero warm-cache misses, while the
AVD reaches 168.59× Metal's last-quartile peak energy and 12.97× its residual
speed. The
[checked trace package](../../evidence/p5.2b-s6.5a-backend-traces/README.md)
therefore attributes the AVD collapse to the shared contact-feedback
instability crossing its threshold much earlier on Vulkan. The later identity
transition may amplify the collapse but does not initiate it. This result does
not authorize coefficient retuning or promotion; the next gate is the stack-
depth threshold and smallest structural reproducer.

The exact depth sweep establishes that threshold. On Metal the combined
candidate retains centered-stack structure at every depth from 8 through 16.
On AVD Vulkan, depths 8 through 14 retain structure through step 1,200, depth
15 first loses it at step 1,057, and depth 16 first loses it at step 892. The
[checked minimum](../../evidence/p5.2b-s6.5a-depth-minimum/README.md) retains
depth 15 and an exact replay as the smallest governed structural reproducer,
with depth 14 as the immediate control. The control is already energy-unstable:
359 consecutive increases precede its first feature transition. Depth 15 has
311 such increases before its first feature transition, then loses structure
108 steps later. This reinforces the causal order—small-signal feedback,
identity transition, structural loss—and prevents a correction from claiming
success merely because depth 14 remains upright for 1,200 steps.

The first reversible correction is rejected. Reducing the combined candidate
from two contact iterations per substep to one changes no coefficient or
solver/scheduler architecture, but it introduces a Metal depth-16 structure
loss at step 1,150 and an AVD depth-14 loss at 1,047. Existing AVD depth-15 and
depth-16 failures advance from 1,057/892 to 874/779. The permanent four-body
case still rises for all 300 settled observations on both backends with zero
cache misses. The [checked `i1` package](../../evidence/p5.2b-s6.5a-soft-i1/README.md)
therefore rejects the hypothesis that merely repeating an otherwise stable
soft recurrence causes the failure. The next bounded correction must preserve
the selected `i2` constraint work while isolating its soft-feedback term; this
result does not authorize a formulation or product-default change.

That isolation comparator is also rejected. Retaining `i2` but applying the
soft tuple only on biased iteration one produces new Metal structure loss at
depths 15/16 (steps 696/604) and AVD loss at depths 14/15/16 (377/280/247).
The four-body mode remains identity-clean and energy-unstable on both backends,
including all 300 settled increases on AVD. The
[checked soft-first package](../../evidence/p5.2b-s6.5a-soft-first/README.md)
rules out a staged soft/hard iteration as a correction. The remaining bounded
formulation question is whether soft feedback interacts specifically with the
coupled manifold-block normal solve; evaluating an existing point-normal lab
lane does not authorize changing the selected solver family.

The existing point-control bundle is rejected too. It retains the exact frozen
soft source and `i2`, but necessarily crosses both normal/color unit and the
historical serial scheduler because speculative parallel coloring is qualified
only for manifold units. The
[checked point package](../../evidence/p5.2b-s6.5a-point-control/README.md)
attests that limitation. It creates a Metal depth-16 structure loss at step
399 and advances AVD depth-15/16 losses to steps 290/130. The permanent
four-body energy envelope rises on Metal, AVD, and a physical Pixel 7a; Metal
and Pixel retain zero warm-cache misses. The Pixel depth-14 stack follows
Metal's quiet result rather than the AVD's 300-increase amplification, making
the worst amplification MoltenVK-specific without making the formulation safe.

S6.5a has therefore exhausted its bounded existing corrections without a
forwarded candidate. Promotion remains held. The recommended next diagnostic
is per-pass energy accounting for the permanent four-body source across the
scalar recurrence, GPU point/block executions, and pinned Box2D authority. A
GPU/scalar implementation divergence can be corrected inside the selected
family; agreement that conflicts with the awake energy gate requires human
review before changing accumulation/formulation, physical policy, or scope.

S6.5b authorization, 2026-07-18: accept that recommendation and keep the soft
candidate, single-iteration, staged-feedback, and point-control lanes
unpromoted. Instrument the permanent four-body reproducer at identical
operation boundaries in the selected GPU manifold-block recurrence and a
shader-faithful scalar recurrence. Metal is the first decision lane. AVD
Vulkan enters only after the common pass that begins the retained energy mode
is localized, so MoltenVK amplification is not confused with the root signal.

Accounting precision is part of the diagnostic contract. Captured body,
contact, and accumulated-impulse values remain the actual `f32` values used by
the shader. The scalar mirror rounds at the same state-changing operation
boundaries. Every pass reports both strict ordered-`f32` kinetic energy and a
compensated wider accumulation over those same `f32` body values. Energy-sum
disagreement alone cannot establish an implementation defect: the autonomous
fix branch requires a body, impulse, or per-contact delta that diverges beyond
the declared exact/ULP-bounded comparison. Pinned unmodified Box2D remains the
world-behavior authority; it is not relabeled as an exact owner of PhysBox's
private pass order. A patch-digested microscope may be created separately only
if public authority state cannot resolve a localized ambiguity.

The branch is fixed before results are measured. If GPU state diverges from
the scalar recurrence, an attributable implementation defect may be fixed
autonomously inside the selected manifold-block family, followed by the frozen
matrix. If GPU and scalar agree on the energy-producing recurrence and the
Box2D behavior target remains quiet, stop for human review before changing
formulation, accumulation order, coefficients, identity policy, solver or
scheduler architecture, or the awake energy gate. Sleeping, wider tolerances,
and refreshed baselines cannot hide either result.

S6.5b pass-trace checkpoint, 2026-07-18: a native-only bounded diagnostic now
captures the actual generated-contact body and prepared-contact `f32` state at
56 logical state-changing boundaries in one selected outer step. Every
colored boundary encloses the complete ordered 32-color dispatch sequence;
this is pass accounting, not a claim that each color is a separate physical
stage. All copies remain GPU ordered and one map occurs only after the target
step and pair-cache store. The split diagnostic target is bit-exact with the
ordinary unsplit graph in the maintained Metal test. At that checkpoint the
scalar recurrence and measured attribution were still open, and the product
remained unchanged.

The version-1 scalar mirror now covers every body-changing pass in the selected
colored manifold-block schedule using `f32` state and the captured compacted
color order. Contact prepare/refresh boundaries prove body-state identity and
then synchronize to actual prepared GPU `f32` inputs, avoiding false
divergence from CPU-versus-GPU trigonometry. The report retains strict
ordered-`f32` and compensated wider kinetic-energy accounts together with raw
bit, absolute, and ULP state differences. The small two-step contract fixture
has only a one-ULP first difference; this observation is not a newly accepted
numerical tolerance. At that checkpoint the permanent settled-window Metal
attribution was still open, and all candidates remained unpromoted.

S6.5b decision, 2026-07-18: the clean
[matched-precision package](../../evidence/p5.2b-s6.5b-pass-energy/README.md)
captures step 900 of the permanent four-dynamic-body reproducer at all 56
logical pass boundaries. GPU kinetic energy rises `1.9601904193100975e-8` and
the scalar pass sum predicts `1.9598340244749856e-8`; their difference is
`3.563948351118385e-12`, or `0.01818%` of the GPU rise. Every one of the eight
normal-relaxation passes injects energy in both recurrences. Their totals are
`2.7251938749029784e-6` on Metal and `2.7251939285323583e-6` in scalar Rust.
The largest absolute state-component difference is `1.2677629e-7`; large ULP
counts belong to near-zero tangent impulses and are retained rather than
misused as a physical tolerance.

Fresh process-isolated unmodified Box2D `3a4f0da...`, with sleeping disabled,
stays near `8.4e-13` kinetic energy and changes only
`3.6376413761573665e-15` from step 899 to 900. The candidate recurrence is
therefore millions of times more active than the declared rigid behavior
target, while GPU and scalar agree on the energy-producing update. This does
not support an autonomous GPU implementation fix. Per the frozen branch, work
stops for review before changing relaxation formulation, accumulation order,
coefficients, identity, solver/scheduler architecture, or the awake energy
gate. The AVD per-pass overlay was not run because it follows, rather than
precedes, that stop branch. No AVD pass is inferred. Hard/direct manifold-block
i2 remains the selected product; the soft/prior candidate and every S6.5a
correction remain unpromoted.

S6.5c authorization, 2026-07-18: test relaxation policy without changing the
product. Arm 1 omits only normal relaxation for soft contacts. Arm 2 retains
normal relaxation but, for penetrating contacts, combines zero velocity bias
with the selected soft mass and accumulated-impulse coefficients; positive-
separation speculative contacts retain their existing branch. Arm 3 changes
only normal relaxation to point-sequential accumulation while retaining the
biased manifold-block solves, and is allowed only if arms 1 and 2 leave block
coupling attribution ambiguous. Arm 1 energy success accompanied by drop
overshoot or bounce is a quality rejection, not an arm-3 trigger.

Pinned Box2D corrects the original rationale for arm 2. In both its scalar
overflow and SIMD contact paths, `useBias = false` selects zero bias,
`massScale = 1`, and `impulseScale = 0`; it does not preserve contact softness
during relaxation. Arm 2 is therefore a measured PhysBox formulation
comparator, not an authority-aligned implementation. The current hard-relax
equation already has the authority-shaped coefficients, while its interaction
with PhysBox's coupled block solve and colored order remains under test.

The survival rule is frozen before measurements. On the permanent four-body
reproducer, steps 900–1,200 must satisfy the existing S6.2 energy-envelope
gate: `1e-12` absolute epsilon, `1e-5` relative epsilon, fewer than five
consecutive material increases, and no first-to-last-quartile envelope rise.
At step 900, every normal-relaxation pass that exists must be neutral or a sink
under the same per-boundary epsilon in matched GPU and scalar accounting; an
omitted arm records absence. A survivor may not add a Metal structural loss in
the governed 4-, 8-, or 16-stack matrix relative to the current soft/prior
candidate. Endpoint success without that per-pass signature, or structure
success bought with a frozen-band regression, is rejection.

Only an arm surviving those cheapest kills reaches seeded drop fixtures and
the full Metal matrix. At most one leader then reaches the hardware-backed
AVD and available physical hardware. Its AVD trace must include one matched-
boundary overlay showing whether MoltenVK's amplification collapses with the
common mode. All arms, including a conditional arm not run, remain named in
the evidence. Promotion returns to human review with the full matrix and does
not follow automatically from factorial ranking.

Arm 1 result, 2026-07-18: omitting soft normal relaxation is rejected by the
permanent four-body gate. The settled energy envelope rises 5.84× with a full
300-observation positive run, and residual linear speed reaches
`0.09048607 m/s`. The policy-attested trace has 48 logical boundaries, no
normal-relaxation pass, and all eight tangent-relaxation passes. This proves
that normal-relaxation omission alone cannot remove the feedback mode; it does
not exonerate or condemn block coupling in a retained relaxation equation.
Arm 2 therefore proceeds, while Arm 1 stops before stack and drop gates. See
the [verified package](../../evidence/p5.2b-s6.5c-arm1-no-normal-relax/README.md).

Arm 2 result, 2026-07-18: the soft-coefficient normal-relaxation equation is
rejected by both predeclared gates. Although maximum linear speed improves to
`0.01083445 m/s`, the settled envelope rises 1.273× with a 299-observation
positive run. Every one of the eight normal-relaxation passes injects energy
in GPU and scalar recurrences, totaling `0.00034883377174823997` on Metal;
their close state agreement excludes a hidden GPU-only update. Arm 1 shows the
overall mode persists without block normal relaxation, while Arm 2 shows a
retained block relaxation remains an injector. The conditional point-
sequential Arm 3 is therefore triggered to isolate block coupling. See the
[verified package](../../evidence/p5.2b-s6.5c-arm2-soft-coefficient-relax/README.md).

Arm 3 result, 2026-07-18: point-sequential normal relaxation is rejected by
all three cheapest gates. The settled energy envelope rises 5.83× with a full
300-observation positive run, while the `0.02844868 m/s` linear and
angular-edge residuals exceed the frozen `0.025 m/s` rest band. All eight
normal-relaxation passes inject in both matched recurrences, totaling
`2.2802872491491276e-6` on Metal; maximum GPU/scalar component difference is
only `4.2957254e-8`. Thus changing the two-point relaxation coupling does not
remove the common formulation-level injector. See the
[verified package](../../evidence/p5.2b-s6.5c-arm3-point-sequential-relax/README.md).

No arm survives, so the conditional drop, stack, AVD matched-boundary, and
physical-device lanes are not run. This is a no-survivor disposition, not an
inferred platform pass or an omitted gap. The authorized hypothesis space is
exhausted; work now packages the comparative record and returns to human
review before any new relaxation equation, accumulation change, coefficient
change, tolerance change, or promotion.

S6.5c checkpoint, 2026-07-18: the
[comparative review package](../../evidence/p5.2b-s6.5c-relaxation-factorial-review/README.md)
verifies every arm package and records no survivor. Its recommendation is not
to promote the quietest failing arm. Continue to select hard/direct manifold-
block i2, and require a human decision before deriving a new coupled soft-
contact recurrence. A useful successor must first have a scalar analytic or
discrete-energy justification that makes each normal-relaxation boundary
neutral or dissipative; endpoint cancellation alone is insufficient. It must
then pass the existing four-body energy and rest bands before reaching Metal
stacks, AVD, physical hardware, or playback.

S6.5d authorization, 2026-07-18: derive from the clean pinned Box2D v3
`3a4f0da...` soft-step recurrence rather than from a free-form empirical
toggle. The authority control is an independent transcription of the public
equation and records the pin and MIT provenance; copied source is neither
needed nor permitted. Box2D already executes contact constraints sequentially
by graph color, with disjoint contacts parallel inside one color. The scalar
study must therefore preserve the captured compacted PhysBox color order and
separately identify any effect from PhysBox's coupled two-point block update,
normal/tangent stage separation, or other schedule difference. A sequential-
only energy argument is insufficient.

Before any GPU port, a scalar candidate must have a discrete-energy argument
at every colored normal-relaxation boundary. It must also preserve support
transfer and penetration resolution on both the permanent four-body
reproducer and the existing two-step contract fixture. Neutrality obtained by
removing correction, zeroing impulses, excessive damping, or allowing sag is
a rejection rather than survival. The frozen S6.2 energy/rest bands and the
existing structural, penetration, and replay contracts do not change.

The phase is bounded to one derived candidate. First replay the authority
recurrence as a control under the captured order; derive a coupled term only
where that comparison proves it necessary. If no recurrence is both stable
under colored order and scalar-predicts quality retention, stop for the
physical-policy decision without editing the GPU path. If one scalar candidate
survives, port that candidate alone as an explicitly named laboratory policy.
It must pass the permanent four-body energy/rest gate and matched pass trace,
then introduce no new loss in the Metal 4/8/16 stack matrix. Only then run the
realistic drops, one AVD matched-pass overlay, and available physical hardware.
The AVD overlay must show the MoltenVK amplification collapsing with the common
defect. Every result returns to human review; S6.6 promotion remains
unauthorized.

The matched-pass trace is a recurrence diagnostic, not an indirect-dispatch
benchmark. Its version-3 contract replays the selected compacted color lists
and their exact color order with bounded direct launches over color capacity,
then maps only after the final boundary. This distinction is explicit because
MoltenVK drops indirect work when snapshot copies split the target solve into
many compute passes even though the same indirect schedule executes in the
ordinary one-pass solver. The failure reproduces for hard, point-sequential,
and projected-relaxation controls, so it is instrumentation-specific rather
than candidate-specific. A direct-replay hard control matches the scalar
boundary within one `f32` ULP. Matched pass traces continue to use the selected
per-color indirect control so each logical color remains observable. Product
execution may combine those same colors into the selected small-world island
dispatch at capacities through 64; larger performance lanes retain per-color
indirect dispatch. The two forms are exact for body/cache/work and do not alter
this recurrence evidence.

S6.5d authority-control checkpoint, 2026-07-18: the existing independent
`b2MakeSoft` transcription is joined by an explicit scalar point-normal update
covering the pinned overflow and SIMD colored contact equations. Analytic and
metamorphic tests freeze the speculative, soft-biased, hard-relaxation, and
unilateral-clamp branches. For non-positive separation, the hard relaxation
update is the constrained one-dimensional kinetic projection and is
dissipative. For positive separation, pinned Box2D retains `separation / h` as
a speculative target even with `useBias = false`; a zero-velocity analytic
case proves that branch can inject energy. This is an authority fact, not yet
a PhysBox candidate or a claim that Box2D's complete schedule is unstable.

The matched S6.5c trace identifies the practical trigger: one point in a
two-point resting manifold is separated by roughly `9.7e-6 m`, so the
speculative relaxation target is about `0.00233 m/s`. The resulting support-
impulse reduction creates the measured body-pair kick. The contact has already
passed the post-integration refresh boundary, so stale prepared geometry is
not the explanation. The remaining scalar question is whether a coupled
zero-target kinetic projection retains the biased pass's penetration and
support work on both frozen fixtures.

S6.5d scalar-candidate checkpoint, 2026-07-18: the one bounded candidate is
the coupled unilateral zero-target kinetic projection during normal
relaxation. For a manifold impulse change `delta`, current normal velocity
`v`, and block inverse effective mass `K`, the exact kinetic-energy change is
`v^T delta + 1/2 delta^T K delta`. The candidate minimizes this convex
quadratic subject to `oldImpulse + delta >= 0`. The no-op `delta = 0` is
feasible, so the selected minimum is non-positive. Manifolds in one color are
body-disjoint, making their changes additive; colors execute sequentially, so
each sees the preceding color's state and the non-increase argument composes
over the captured schedule. The two-point active-set enumeration covers both,
first-only, second-only, and neither-active solutions. Biased normal passes,
soft coefficients, friction, and restitution are unchanged.

The typed report replays both the pinned point authority and the candidate
from the same captured `f32` pre-pass state at all eight normal-relaxation
boundaries. On the two-step contract trace the candidate has no injecting
manifold, preserves identical geometry, and discards no supported point that
the authority would retain. At permanent reproducer step 900, the point
authority injects `2.094123958471181e-6` total with one injecting manifold in
every boundary. The candidate injects in zero manifolds and removes
`1.6434270572867685e-6`; all seven supported points remain supported. Total
normal impulse stays near `0.2927` at every boundary and separation extrema
are unchanged. This satisfies the predeclared scalar energy, penetration, and
support-transfer gate. It authorizes exactly this candidate for the single GPU
port; it is not product promotion or evidence that the multi-step Metal gate
will pass.

S6.5d laboratory-port checkpoint, 2026-07-18: policy code `3` ports exactly the
scalar-qualified coupled zero-target projection to normal relaxation. The
shader bypasses speculative and penetration targets only for that named
laboratory policy; the biased stages and every other pass are unchanged.
Every product constructor still selects policy code `0`, hard/direct. The
two-step candidate trace replays through the scalar mirror, including the
versioned policy attestation.

The first uncommitted-source Metal kill passes. The permanent reproducer
reaches the rest band by step 601, with settled maximum linear speed
`1.7262822e-6 m/s`, angular-edge speed `7.3337867e-7 m/s`, zero unexplained
churn, zero warm-cache misses, a maximum positive-energy run of one, and no
capacity failure. At step 900 all eight GPU normal-relaxation boundaries are
sinks totaling `-2.0920214104119448e-11`; the scalar total is
`-2.0920214088975228e-11`. Per-relaxation component differences are at most
`1.080025e-12`. This clears the cheap implementation/energy kill and permits a
fresh clean Metal 4/8/16 structural matrix. It does not yet clear that matrix,
the realistic drops, cross-host forwarding, or promotion.

S6.5d AVD overlay-instrument checkpoint, 2026-07-18: the first Android
snapshot attempt exposed the instrumentation issue above, not a candidate
recurrence divergence. With the version-3 direct replay, candidate step 900
has eight normal-relaxation sinks totaling `-0.0015984423944470935` on GPU
versus `-0.0015984423985654773` in scalar, `4.656613e-8` maximum normal-pass
state difference, and no injecting manifold. This validates recurrence
agreement on the captured AVD state; it does not waive the long AVD structural
gate or claim that MoltenVK amplification has collapsed.

S6.5d decision checkpoint, 2026-07-18: the
[verified review package](../../evidence/p5.2b-s6.5d-energy-projection-review/README.md)
rejects the single candidate on deep-stack energy and portability. Metal is
excellent on the permanent four-body source, 4/8 stacks, and all seven
realistic fixtures. Its 16-stack remains upright and below the awake speed
band, but energy rises for all 300 settled observations. The AVD 16-stack
loses structure at step 945 and the physical Pixel 7a loses it at step 1001;
the same physical Pixel passes the permanent four-body energy gate, separating
the common depth limit from the AVD's stronger small-pile amplification. The
unchanged hard/direct product control remains upright on both Metal and AVD.

The AVD matched replay agrees with scalar at every recorded pass, and all
candidate normal-relaxation boundaries are sinks. This is therefore not an
attributable shader recurrence defect that can be fixed inside the authorized
branch. No candidate is promoted. Reopening requires human selection of a new
physical policy or deferral of softness; S6.6 remains unauthorized.

S6.5d human decision, 2026-07-18: review accepts that recommendation. The
candidate is rejected, soft settling is deferred, and the hard/direct
manifold-block i2 product remains unchanged. This closes S6.5 as a measured
negative result rather than satisfying the awake-settling gate. No second
recurrence, feedback-composition study, or compromise to deep-stack quality is
authorized.

The failed candidate relocates the unresolved injector. Its projected normal
relaxation is neutral or dissipative at every measured colored boundary on
Metal and AVD, while the S6.5c omission arm still rose by `5.84x` without any
normal-relaxation pass and the candidate Metal 16-stack still rose for all 300
settled observations. Energy therefore enters through one or more remaining
pass families or their composition; the soft biased normal/support-transfer
path is a hypothesis, not an established attribution. Any future soft
formulation proposal must begin with matched-precision, per-pass energy
attribution on the governed 16-stack, including the biased-normal, tangent,
integration, and support-transfer boundaries. The permanent four-body
reproducer alone is no longer sufficient evidence.

Reopening this deferred direction requires one package that removes the
16-stack rising-energy mode, holds the candidate 16-stack through step 1,200
on both the hardware-backed AVD and available physical Android hardware, keeps
the AVD permanent reproducer below five consecutive positive-energy
observations, and preserves the same structural outcomes on Metal, AVD, and
physical Android. It may not widen tolerances, change oracle precedence, drop
portability, or use sleeping to conceal awake motion.

S9 authorization, 2026-07-18: human review authorizes the bounded settling
schedule-differential program tracked by
[tactical 009](../tactical/009-settling-schedule-differential.md). It first
satisfies the attribution prerequisite recorded above — matched-precision
per-pass energy accounting on the governed 16-stack, including biased-normal,
tangent, integration, and support-transfer boundaries — for the hard/direct
product itself, then measures unpromoted schedule arms derived from verified
pinned-authority differences: static-late contact ordering, contact-major
fused normal/tangent solves, a single declared static-stiff-hard bias value,
and conditional fixed-anchor-refresh and restitution-timing arms. The program
stays inside the hard/direct manifold-block family, does not reopen the
deferred soft direction, changes no product default, and returns any winner
to human review under this topic's existing completion gate.

S9.2 attribution, 2026-07-18: the clean
[matched per-pass package](../../evidence/p5.2b-s9.2-pass-attribution/README.md)
localizes the hard/direct 16-stack injector across settled steps 900, 1000,
and 1100. Warm start is the only material non-gravity injecting pass family at
all three phases; biased normal and relaxation are net sinks, while tangent
and refresh/non-gravity-integration positive energy are zero. Exact scalar
replay stays within `4.66598e-7` maximum absolute difference against the
frozen `2e-5` contract. This routes static-late E2 first because it changes
colored support-transfer order including warm start, leaves fused E3 and
static-stiff E4 mandatory under the authorized program, and does not activate
fixed-anchor E5 or require a formulation stop. Product settings remain
unchanged.

S9 E2 disposition, 2026-07-18: the clean
[static-late full-corpus package](../../evidence/p5.2b-s9.4-arm-static-late/README.md)
discards the first routed arm. Shallow behavior is materially quieter, but
16-stack last-quartile energy regresses `62.11%`, its settled quartile
envelope rises, and only the 4-stack clears the paired 25% speed-and-energy
threshold. The 32-stack first loss also moves from step 29 to 23. The arm is
structurally disqualified when its declared four-color static partition
atomically exhausts the unchanged solver-color capacity on the accepted
mixed-pile fixture and both 100-body buckets. Exact replay and minimized
prefixes establish a measured arm failure, not a product regression. E2 is
not eligible for combinations; mandatory E3 and E4 continue independently.

S9 E3 disposition, 2026-07-18: the clean
[fused full-corpus package](../../evidence/p5.2b-s9.4-arm-fused-normal-tangent/README.md)
also fails the material gate. It is replay- and capacity-clean, and its 32/64
diagnostic structures live substantially longer, but the accepted centered
16-stack loses structure at step 17. The 8-stack exceeds the 5% residual
regression allowance and its energy rises `38.37%`; 16-stack last-quartile
energy rises `13.61%` with a rising envelope. The preserved first-loss prefix
has no execution failure. E3 is discarded independently; E4 remains
mandatory and no E2-containing combination is eligible.

S9 E4 disposition, 2026-07-18: the clean
[static-stiff full-corpus package](../../evidence/p5.2b-s9.4-arm-static-stiff-hard/README.md)
preserves every accepted structure but misses the material gate. The 8-stack
residual regresses `27.28%`, 16-stack energy regresses `30.59%`, and no stack
improves both metrics by 25%. The permanent four-body last-quartile energy
rises more than 13x and its positive run grows to 139. Exact replay, capacity,
and diagnostic first-loss clauses all pass, so this is a quality rejection of
the declared coefficient rather than an implementation failure. Primary-arm
exhaustion activates E5; E6 hygiene follows, with no combination eligible.

S9 E5 disposition, 2026-07-18: the clean
[fixed-anchor full-corpus package](../../evidence/p5.2b-s9.4-arm-fixed-anchor-substep-refresh/README.md)
discards authority-style substep separation refresh. It materially improves
4/8-stack residual speed and energy and lowers 16-stack speed by `39.58%`, but
16-stack last-quartile energy rises `85.26%` to `0.6235986` and its quartile
envelope rises. The 64-stack diagnostic first loss also regresses from step
31 to 29. Exact replay, final-only state, accepted structure, and capacity all
pass; the failure belongs to the changed recurrence, not its implementation.

S9 closure, 2026-07-18: the
[verified decision package](../../evidence/p5.2b-s9.6-schedule-differential-review/README.md)
records zero material survivors. Conditional E6 moves restitution after all
substeps but is byte-identical to selected execution on the 4/8/16 stacks and
both S6.2 drops; fresh clean pinned Box2D rows replay exactly. All combinations
require discarded E2, so none is eligible. No arm enters survivor cost or
cross-host qualification and no platform result is inferred. The schedule-
differential direction is closed with no promotion candidate; the awake
completion gate remains unmet and sleeping remains unauthorized as a mask for
the residual.

S10 authorization, 2026-07-18: human review authorizes the warm-start and
stored-impulse stationarity workstream tracked by
[tactical 010](../tactical/010-warm-start-impulse-stationarity.md). It follows
the S9.2 localization with a sleep-off warm-start on/off differential in both
PhysBox and the pinned authority, settled-window stored-impulse stationarity
measurement referenced to the authority's own measured band, single-step
operator localization, and at most two scalar-first corrections inside the
selected hard/direct family. Warm-start-off lanes are diagnostics and can
never become candidates. The authorization permits one bounded stationarity
re-observation of the existing frozen unpromoted soft-candidate lane solely to
test whether the deferred soft failure shares the hard mode's mechanism; it
does not authorize any new soft recurrence, coefficient, or candidate. If no
correction survives the frozen S9 gates, S10 returns the formulation decision
package against this topic's recorded reopening evidence instead of further
hard-family arms; that reopening remains a human decision.

S10.1 warm-start differential, 2026-07-18: clean four-lane evidence in
`evidence/p5.2b-s10.1-warm-start-differential/` favors the predeclared
non-stationary-stored-impulse route. On the selected 16-stack, disabling warm
starting reduces settled maximum linear speed from `0.70852673` to
`0.000023823235` (`0.9999663` removed) and last-quartile maximum energy from
`0.3366095` to `2.307887e-9` (`1.0` removed), while preserving structure,
capacity, exact replay, and final instrumentation equality. Fresh warm-on
summaries equal the S9.1 pins exactly. The authority warm-on lane is also
exactly replayable and bounded (`0.0005379421` maximum settled speed on the
16-stack); its warm-start-off lane instead loses the 8- and 16-stack
structures and remains context only. This routes S10.2 toward measured
stored-impulse stationarity against the authority band; it does not promote a
warm-start-off product lane or yet prove which stored-impulse operator is at
fault.

S10.2 stored-impulse stationarity, 2026-07-18: clean exact-replay evidence in
`evidence/p5.2b-s10.2-impulse-stationarity/` signs hypothesis 1. All three
manifold classes are oscillating against the warm-on authority band. PhysBox
floor-support manifold normal amplitude is `1.246864803135395` versus
`0.00025260448455810547` (`4936.0359x`), and point amplitude is
`0.623433031141758` versus `0.0006193220615386963` (`1006.6379x`). Interior
support is `2793.9682x`/`1162.0351x` authority at the manifold/point levels,
records `46` active-set flips against exact authority zero, and reaches full
redistribution `1.0` against `0.0020854763602072557`. Top support repeats the
signature with `50` flips against zero. Tangent maxima remain below authority,
so the dominant mode is accumulated normal-impulse non-stationarity rather
than excessive tangent impulse. The local speed/amplitude correlation is
weak across interior tracks (`0.018146322`) despite the large impulse bands,
whereas authority is `0.730651603`; this warns against treating instantaneous
speed alone as the operator signature. S10.3 must now identify the first
single-step composition divergence and preserve the smallest depth at which
this normal/active-set/redistribution signature appears.

S10.3 mechanism and unification, 2026-07-18: clean evidence in
`evidence/p5.2b-s10.3-mechanism/` names
`inter-substep-stale-persistence-reseed`. On the exact step-904 operator, all
three inter-substep reset-mode-1 refreshes overwrite the current solved
impulse vector with the first prepared outer-step vector, and every warm start
is followed by non-fixed biased-normal work. The governed sweep replays
exactly and selects the centered four-body stack as the first harmful
structure: depth 2 remains at `8.5901223e-7 m/s` with a nonincreasing energy
envelope, while depth 4 reaches `0.03872548 m/s` and a rising
`0.0017579256` last-quartile maximum. At that minimum failure the refreshes
discard `0.00020720600332838917` impulse L1 despite zero active-set flips and
only `8.945056e-8` redistribution. The derived hard correction must therefore
retain current within-step impulses while refreshing anchors and effective
masses; it must preserve unilateral response, penetration correction,
deterministic colored support transfer, refreshed geometry, and final
persistence.

The one authorized re-observation of the frozen S6.4 soft 16-stack preserves
its rising-energy mode and exceeds the floor/interior authority point-normal
bands by `1.8396131x`/`2.2314258x`, with tangent below authority. It does not
reproduce the hard lane's manifold-total or active-set/redistribution
signature: the corresponding manifold amplitudes and redistribution stay
below authority and active-set flips remain zero. Hypothesis 3 is therefore a
partial point-normal unification, not evidence that the hard stale-reseed
operator is the complete soft-failure mechanism. S10.4 is routed to the
bounded warm-start/bias composition space; a separate active-set/tie rule has
no root-mechanism support in the minimum reproducer.

S10.4 correction disposition, 2026-07-18: the verified
[retained-refresh package](../../evidence/p5.2b-s10.4-candidate-retained-refresh/README.md)
closes both candidate slots without a survivor. Retaining just-solved impulses
through full anchor/effective-mass refresh passes the scalar stationarity and
quality gate and changes only the three inter-substep reset words on GPU. It
is exact-replay, dispatch-form exact, and structurally clean across every
accepted full-corpus scene. It materially improves the 4/8 stacks, but the
16-stack last-quartile energy regresses `29.24%` to `0.43503416` with a rising
envelope, and the permanent four-body source remains at `0.04818385 m/s` with
positive run 199. The changed 64-stack trajectory reaches 33 conflicting
solver units at step 924 and is atomically rejected by the unchanged 32-color
capacity. Retained refresh is discarded by the frozen S9 material gate. The
active-set/tie slot remains `not-routed` because the minimum mechanism has no
active-set flip and negligible redistribution. No product/default/tolerance/
capacity/oracle contract changes.

S10.5 formulation decision, 2026-07-18: the
[verified package](../../evidence/p5.2b-s10.5-formulation-decision/README.md)
recommends human authorization of a bounded soft-specific formulation
reopening while the hard/direct manifold-block i2 product remains selected.
This is not promotion of the rejected S6.5d lane: the fresh soft observation
still rises for 300 settled observations and shares only the point-normal
symptom, not the hard manifold-total/active-set/redistribution mechanism. A
new study would begin with matched soft 16-stack attribution across warm
start, biased normal, colored support transfer, refresh, and persistence;
derive any recurrence from that soft mechanism; and treat the 16-stack as the
primary gate. It must remove the rising-energy mode, stay inside the
`0.025 m/s` awake bands with positive runs under 5, preserve the 16-stack
through step 1,200 on AVD and available physical Android, keep the AVD
permanent reproducer below 5 positive observations, and preserve structures
on Metal/AVD/physical Android without sleeping or weakened contracts. The
alternative is continued deferral with the measured hard residual. This is
the recorded human fork; the package authorizes neither implementation nor
promotion.

S10.6 review and human decision, 2026-07-19: the verified
[S10.6 review](../../evidence/p5.2b-s10.6-stationarity-review/README.md)
closes S10 complete with no hard survivor and one recommendation — take the
formulation decision. Human review takes the recommended option: the
hard/direct manifold-block i2 product remains selected, and a bounded
soft-specific formulation study is authorized under the recorded reopening
gate above. The deferred comparator-engine question stays deferred: a
Rapier2D or other comparator lane is reconsidered only if the authorized soft
study itself closes negative, and then requires its own explicit decision in
this topic. One measured caution is carried forward as a successor gate: the
retained-refresh candidate passed its full-length scalar lane on the
permanent four-body reproducer (`0.006659909 m/s`, positive run 3) yet failed
the same scene on the GPU corpus (`0.04818385 m/s`, positive run 199), so a
soft candidate must pass an early GPU falsification on the 16-stack and the
permanent reproducer before any wider corpus investment; a scalar-only pass
is never a qualification signal.

S11 authorization, 2026-07-19: human review authorizes the bounded soft
contact formulation study tracked by
[tactical 011](../tactical/011-soft-contact-formulation.md). It begins with
matched-precision per-pass and impulse-operator attribution on the frozen
unpromoted soft 16-stack — warm start, biased normal, colored support
transfer, refresh, and persistence treated as one stationarity problem —
derives at most two simultaneous scalar-first corrections under each frozen
mechanism verdict only from the measured soft mechanism, subjects each single
named unpromoted GPU port to the early
16-stack/permanent-reproducer falsification before corpus investment, and
qualifies any survivor on Metal, the hardware-backed AVD, and available
physical Android independently under the recorded reopening gate. The
16-stack is the primary derivation gate; the permanent four-body reproducer
is required evidence but cannot qualify a candidate alone. Candidate count is
not a closure argument: another candidate after a discarded pair requires new
attribution and a versioned mechanism addendum, while closure requires that no
unresolved mechanism-supported correction remains inside the recorded
ownership constraints. No product default changes; any winner returns to
human promotion review.

S11.1 attribution result, 2026-07-19: the clean
[soft pass package](../../evidence/p5.2b-s11.1-soft-pass-attribution/README.md)
routes the governed 16-stack to hypothesis 1, but only as a composition. Over
settled steps 900–1200 its compensated kinetic energy grows by
`4.342633906687518e-5` with positive run 300. `normal-biased` is the only
consistent non-gravity material injector (`+5.953993913608667e-7`, positive
on all 301 steps and at steps 900/1000/1100), while normal relaxation is a
sink and the gravity/warm-start pair leaves a larger
`+4.526201848124244e-5` support-cancellation deficit. The permanent four-body
context reverses the soft-normal signs: biased normal is a sink and normal
relaxation is the consistent injector. S11.2 therefore owns a
scene-dependent soft-normal recurrence study across bias, relaxation,
gravity-to-warm support cancellation, and the persistence handoff; it may not
infer a bias-only correction from the primary family label. All observation,
window, and anchor lanes replay exactly with clean capacity and structure;
energy accounting reconciles within the frozen bands. Product/default/
tolerance/oracle contracts remain unchanged.

S11.2 mechanism and minimizer, 2026-07-19: the clean
[soft operator package](../../evidence/p5.2b-s11.2-soft-mechanism/README.md)
names `depth-triggered-inter-substep-soft-seed-rewind`. Across the complete
depth catalog and permanent four-body context, every step-1099 final contact
hands its slot and impulse bit-exactly to step 1100, excluding an outer
persistence loss. Inside the next outer step, all three later-substep
refreshes restore the first prepared impulse and discard the correction
accumulated by the preceding substep. The primary normal-biased maps are all
oscillatory and the normal-relaxation maps all contractive. The frozen rising
signature first appears at depth 11, is below the predicate at depths 12–14,
and reappears at 15/16; the exact shared depth-11 source hash is
`9c02153e801e463f5e693f2187ee8ca497fce817a52933676916c7b614644658`.
At depth 16 the refreshes discard `1.925067298338412e-4` impulse L1 even
though all 32 handoff points are exact. The four-body lane carries the same
rewind schedule but reverses the bias/relaxation energy signs, confirming the
composition route rather than bias attenuation. A correction must therefore
refresh geometry/effective mass while retaining the current outer-step
impulse and must preserve unilateral response, penetration correction,
deterministic colored support transfer, final persistence, exact replay and
capacity, and soft impact response without sag. S11.3 first evaluates the
existing isolated retained-impulse refresh policy under soft-specific scalar
and early-GPU gates; the discarded S10 hard result is evidence, not a
transferred verdict. Product/default/tolerance/oracle contracts remain
unchanged.

S11.3 candidate-1 disposition, 2026-07-19: the verified
[retained-refresh scalar package](../../evidence/p5.2b-s11.3-candidate-retained-refresh/README.md)
discards raw within-step impulse retention before any GPU port. The local
soft complement argument passes 720 analytic samples with only the bounded
serialized-`f32` endpoint residual, and the two-step soft response remains
exact. In the full recurrence, depth 11 falls from `0.17856713` to
`0.00850862 m/s` but retains a rising settled energy envelope and worsens
penetration from `0.01701543` to `0.02646441 m`. The scalar 16-stack is
structurally invalid in both control and candidate and candidate penetration
worsens; the four-body candidate changes the final active support-point count.
All control/candidate lanes replay exactly. Per the frozen scalar-first rule,
`gpuPortAdmitted=false` and no early-GPU, corpus, or cross-host result is
inferred. Candidate 1 therefore rules out simple raw retention as the complete
soft correction but does not close the measured mechanism. Any candidate 2
must derive a distinct soft-consistent impulse transport through refresh;
attenuation, coefficient retuning, or a weakened penetration/structure gate
has no authorization. Product/default/tolerance/oracle contracts remain
unchanged.

S11.3 candidate-2 disposition, 2026-07-19: candidate 1's measured basis/state
composition and the pinned authority's exact once-per-outer-step preparation
schedule route the zero-parameter `fixed-anchor-soft-impulse-transport`
candidate. The verified
[fixed-anchor scalar package](../../evidence/p5.2b-s11.3-candidate-fixed-anchor-transport/README.md)
discards it before any GPU port. It passes the permanent four-body lane and
reduces depth-11 maximum linear speed `80.12%` while improving depth-11
penetration `18.56%`, but the exact two-step body response changes, depth 11
has positive energy run 6, and the 16-stack loses one final active support
point while maximum penetration rises from `0.03863653` to `0.09379554 m`.
Every lane remains finite and replays exactly. The scalar-first rule therefore
sets `gpuPortAdmitted=false`; no GPU or cross-host result is inferred.
Together the two candidates cover the measured rewind correction with either
regenerated or fixed bases. Coefficient re-selection, empirical attenuation,
a new accumulation architecture, and gate weakening remain outside the
authorized space, so no unresolved mechanism-supported correction remains.
S11 advances to its closure decision package; satisfying the recorded
comparator reconsideration trigger does not itself authorize a comparator or
change the hard/direct product. Product/default/tolerance/oracle contracts
remain unchanged.

S11.4 closure decision, 2026-07-19: the verified
[closure package](../../evidence/p5.2b-s11.4-soft-closure-decision/README.md)
binds and re-runs every S11.1–S11.3 evidence verifier. The two dispositions
cover current-impulse transport through both regenerated and fixed contact
bases, and neither passes scalar admission. No unresolved correction remains
for the measured rewind mechanism inside S11's frozen formulation,
accumulation, and quality ownership; no GPU or cross-host result is inferred.
The recommendation is to close this soft route as a measured negative, retain
the hard/direct product pending a separate decision, and return the recorded
comparator-engine fork to human review. The comparator reconsideration trigger
is satisfied but is not self-authorizing. A new accumulation architecture or
coefficient/contact-policy re-selection likewise requires new measured scope;
acceptance-band weakening remains forbidden. Product/default/tolerance/oracle
contracts remain unchanged.

S11.5 final review, 2026-07-19: the verified
[review package](../../evidence/p5.2b-s11.5-soft-formulation-review/README.md)
recursively verifies the closure decision and all attribution, mechanism, and
candidate inputs. Its specific recommendation is to take the measured soft-
route closure, retain hard/direct manifold-block i2 pending a separate human
decision, and explicitly decide whether to authorize a comparator-engine
study or a separately scoped new solver architecture. The comparator trigger
is satisfied but no comparator is authorized or added. S11 is complete; the
selected product is unchanged and is not reclassified as an awake-settling
solution. Product/default/tolerance/oracle contracts remain unchanged.

S12 human fork resolution, 2026-07-19: human review declines to authorize the
comparator-engine study and instead authorizes the bounded
[authority solve-structure decomposition](../tactical/012-authority-structure-decomposition.md).
An independent Rust scalar bridge first reproduces the existing S11 scalar
endpoint exactly and then must qualify the pinned Box2D structure endpoint
against fresh awake native authority results. It crosses exactly five
source-derived axes — preparation/anchors/impulse transport, solve order,
static/dynamic coefficient schedule, biased/relaxation composition, and
restitution timing — through ten mandatory bidirectional single-axis hybrids
and at most three mechanically routed two-axis interactions. For this
laboratory only, native authority response bands own acceptance instead of
exact current PhysBox response; that is an explicit experimental target
change, not a relaxation or product baseline refresh. The study stops at a
decision package identifying the minimal sufficient measured structure and
whether it maps exactly to the selected colored-parallel GPU architecture.
No product, authority precedence, source pin, comparator, or physical default
changes by this authorization.

S12 gate-consistency resolution, 2026-07-19, before qualifying evidence: the
first unretained bridge diagnostic showed that the draft absolute energy rule
rejected native Box2D itself on both governed stacks (depth 11 has a positive
run of 5 and both depths have rising last/first energy quartiles). The native
public trace also exposes contact records only during the settled window, so
an all-step scalar penetration maximum has no native counterpart. The durable
S12 gate now compares settled speed to `2 * native + 2e-5`, settled energy
growth and positive-run to a no-worse-than-native band, and settled
penetration to native plus `2e-4 m`. The two-step source gates response only;
the permanent mixed pile remains finite/structure/product-ceiling context;
depth 11 and depth 16 own qualification. This implements the human-authorized
authority-band ownership instead of requiring the authority to pass a stricter
PhysBox-authored absolute predicate. It changes no product quality ceiling,
physical default, numerical product tolerance, oracle precedence, or source
pin. No clean S12 endpoint or ablation evidence was accepted under the
discarded draft.

S12 endpoint qualification, 2026-07-19: the verified
[endpoint package](../../evidence/p5.2b-s12.2-authority-endpoints/README.md)
proves the bridge P endpoint bit-exact to S11 on all four frozen sources and
reproduces its known failures. The independently implemented A endpoint
qualifies against fresh, exactly replayed native Box2D on both governed stacks.
Depth-11 settled maximum speed is `6.9533e-6 m/s` versus native
`5.9432e-6`; depth 16 is `5.6202e-4` versus native `5.3794e-4`. Both retain
the native support structure, penetration/height bands, positive-run and
energy bands, finite state, and the unchanged product ceiling while awake.

S12 decomposition and minimization, 2026-07-19: all ten mandatory
bidirectional ablations replay exactly. Preparation/anchors/impulse transport,
authority-colored accumulation, authority dynamic/static coefficients, and
one-biased/one-relaxation composition are each necessary on at least one
governed source; none is individually sufficient. Restitution timing is
unnecessary. None of the three precommitted pairs qualifies: preparation plus
accumulation reaches `3.4080e-4 m/s` on depth 16 but loses support structure;
preparation plus coefficients preserves structure but remains at
`0.010768 m/s`; preparation plus composition loses structure at `8.2683
m/s`. `A-X5` is therefore the smallest qualified measured structure. The true
minimum is bounded at 3-4 because triples were outside the frozen matrix; no
unplanned factorial was opened.

S12 final review, 2026-07-19: the recursive
[decision package](../../evidence/p5.2b-s12.6-authority-structure-review/README.md)
records that soft contact remains promising as a coupled authority structure.
S11's local rewind corrections were insufficient; that did not rule out the
coupled route. Static GPU analysis finds the exact X1-X4 structure portable,
GPU-resident, bounded, and storage-compatible, but not expressible by the
selected speculative-color/manifold-block policy unchanged. One minimal exact
unpromoted GPU probe is recommended next. It must preserve the frozen authority
bands, exact replay, forced-overflow capacity, and measured 32/256/1K cost,
then stop before cross-host expansion or product promotion. S12 itself is
complete; no comparator, GPU probe, product default, tolerance, or authority
precedence changed.

S13 human fork resolution, 2026-07-19: human review authorizes the exact
unpromoted GPU probe recommended by S12.6 and recorded in
[S13](../tactical/013-authority-structure-gpu-probe.md). The probe contains
exactly X1-X4, the common normal-then-friction point-sequential recurrence,
and current PhysBox X5 restitution timing. It compares depth 11 and depth 16
to the already frozen authority bands, replays exactly, fails closed on true
capacity overflow, exercises one source-faithful forced-overflow fixture, and
measures 32/256/1K work, allocation, and GPU complete-step time. No approximate
selected-kernel port, comparator, wider scalar or coefficient sweep, product
selection, default change, tolerance change, or authority-precedence change is
authorized. The workstream stops at a new human checkpoint before cross-host
candidate qualification or product promotion.

S13 exact GPU disposition, 2026-07-19: the recursive
[review package](../../evidence/p5.2b-s13.6-exact-gpu-probe-review/README.md)
rejects the sole X1-X4/current-X5 candidate by the unchanged frozen authority
quality gates. Implementation and every retained governed stream replay
exactly. The focused GPU/scalar path first leaves the `5e-5` implementation
band in the substep-1 biased normal pass and reaches a two-step maximum state
difference of `9.26018e-4`. Depth 11 preserves native support topology but
settles at `0.0304508 m/s`; depth 16 loses stack order and support topology.

This is not a broad rejection of soft contact. S12's independently qualified
coupled soft scalar endpoint remains evidence that the response mechanism is
viable. S13 instead establishes that an exact source-faithful recurrence is
too branch-sensitive under the measured portable GPU arithmetic. Execution
is not the blocker: overflow-first scheduling, fail-closed capacity, and exact
replay qualify, and the measured candidate p95 is `1.258/3.273/5.820 ms` at
32/256/1K paired manifolds with no hard constraint. Hard/direct stays selected
because the exact soft GPU response fails. A portable branch-robust
authority-band formulation would be a new study requiring human authorization;
it may not be smuggled in as an implementation correction, tolerance change,
coefficient sweep, approximate selected-kernel port, or comparator.

S14 human fork resolution, 2026-07-19: human review declines S13 promotion and
authorizes the bounded
[branch-robust authority-band formulation study](../tactical/014-branch-robust-authority-band-formulation.md).
It starts from the retained S13.2 first divergence, identifies the exact
near-zero separation expression and branch amplification, and admits exactly
one real-equivalent cancellation-robust evaluation only after a written
binary32 forward-error argument. Empirical damping, deadbands, branch
hysteresis, coefficient changes or sweeps, comparator engines, higher-
precision product state, and band changes are forbidden.

The scalar gate is frozen before governed results: nearest, two outward bound
endpoints, and four deterministic alternating/hashed endpoint schedules must
all pass the unchanged depth-11 and depth-16 native authority bands with exact
replay. Only then may the candidate rerun the S13 portable GPU contract
unchanged, including the same governed bands, 21-manifold overflow-first
fixture, one-below-demand fail-closed fixture, and 32/256/1K timing matrix.
Hard/direct remains selected. S14 stops before cross-host qualification or
promotion and cannot reinterpret a clean negative by changing the formula arm
or a frozen gate.

S14 measured disposition, 2026-07-19: the recursive
[review package](../../evidence/p5.2b-s14.7-branch-robust-review/README.md)
rejects `authority-band-cancellation-robust-v1` at the frozen scalar-first
gate. The retained S13 branch split is explained exactly: the original
separation combines `0.25` and `-0.24999994`, reaching zero on the GPU while
the scalar reaches one positive binary32 ULP and selects the other biased-
normal equation. The real-equivalent displacement formulation reduces the
retained forward bound from `1.49032e-7` to `2.98230e-8`, with a wholly
positive local interval, and all local controls pass.

The full robustness result is nevertheless 10/14. Every depth-11 lane passes.
At depth 16, nearest, uniform-high, and hashed-sign-v1 fail only the unchanged
energy-growth predicate; step-alternating fails only the positive-energy-run
predicate. All rows replay exactly and remain finite, structurally valid,
support-compatible, and inside speed, height, penetration, and arithmetic-
bound gates. Because nearest itself fails, this is not merely an overly broad
endpoint stress. The scalar gate denies GPU admission; S14 performs no GPU,
capacity, timing, cross-host, or promotion run.

This rejects one formulation, not soft contact as a family. S12's coupled
scalar endpoint remains promising, and S14 brings the depth-16 response near
the authority band without crossing every stationary predicate. Hard/direct
remains selected. Any further soft formulation is a new human-authorized
study; it may not reuse S14 to change coefficients, bands, authority, product
selection, or the frozen perturbation result.

S15 human fork resolution, 2026-07-20: human review rejects
`authority-band-cancellation-robust-v1` and authorizes the bounded
[exact-recurrence precision-margin tactical](../tactical/015-exact-recurrence-precision-margin.md).
It first runs the original S12 cancellation DAG alone in binary64 and,
separately, the complete exact scalar solve in binary64 at depths 11 and 16.
Both attribution and precision-limit lanes must satisfy the unchanged native
authority predicates. It then measures a fixed depth-16 absolute-perturbation
lattice under the seven already frozen sign schedules. This is measurement of
response margin, not a coefficient or band sweep.

Only a still 4/4 precision result and a study margin strictly greater than a
conservative error-free-transformation bound may admit one candidate:
`authority-exact-separation-two-float-v1`, the original expression and branch
with two-float intermediates only on the diagnosed cancellation path. That
candidate must pass the unchanged fourteen-row scalar perturbation gate before
the unchanged S13 GPU response, replay, overflow, fail-closed capacity, and
32/256/1K timing contract can run. A precision failure or insufficient margin
returns a decomposition-addendum fork with no candidate. Reformulation arms,
coefficient sweeps, band changes, comparators, approximate ports, cross-host
qualification, and promotion remain outside authorization. Hard/direct stays
selected throughout.

S15 measured disposition, 2026-07-20: the recursive
[precision-margin review](../../evidence/p5.2b-s15.7-precision-margin-review/README.md)
closes the arithmetic route with no candidate. The precision matrix qualifies
only 1/4 governed rows. Evaluating only the original cancellation path in
binary64 remains structurally still at depth 16 but fails the unchanged depth-
11 energy-growth predicate. The independent complete binary64 solve loses the
declared stack and native support topology at both depths. Exact S12 binary32
qualification is therefore a precision-specific trajectory, not the still
precision limit of the recurrence.

The separately required depth-16 amplitude curve replays exactly. Its worst
all-passing prefix is `2^-40 m`; substep- and step-alternating schedules first
fail at `2^-39 m`, and every schedule later re-enters passing islands. These
re-entries do not extend the frozen prefix margin. A non-governed optimized-
profile control also changes the zero-amplitude trajectory enough to fail the
debug-lineage baseline, reinforcing the arithmetic sensitivity without
changing the governed curve.

Because 4/4 precision stillness is mandatory, no candidate-specific EFT proof,
two-float implementation, scalar candidate gate, GPU response/capacity/timing,
cross-host qualification, or promotion is admitted. The one recommendation is
the proposed measurement-only `authority-event-structure-decomposition-
addendum-v1`: localize and ablate which contact-admissibility, identity,
separation/speculative, unilateral active-set, cache, or support-graph decision
selects the still trajectory across precisions. That addendum has zero product
candidates and is not authorized by S15 closure. Hard/direct remains selected;
soft contact as a family is not globally ruled out.

S16 human fork resolution, 2026-07-20: human review authorizes
`authority-event-structure-decomposition-addendum-v1` exactly as recorded by
the S15.7 review and tracked in
[S16](../tactical/016-authority-event-structure-decomposition.md). The study
aligns the exact-S12 binary32, cancellation-path binary64, complete-solve
binary64, and retained native-public event streams on depths 11 and 16. It may
route only six declared discrete families: contact admission/point count,
reference/feature identity, separation/speculative branch, unilateral active
set, cache/impulse ownership, and support-graph color/order.

Continuous equations, coefficients, sources, projections, authority bands,
and values remain native to each scalar lane. Bidirectional replay may select
only locally available payloads from immutable endpoint event tokens; it may
not copy continuous state between precisions. The study has zero product
candidates. It stops after A1 if the first causal divergence has no declared
owner, after the A3 minimal-event-structure disposition before A4, and before
any product candidate or GPU execution probe. A4 is an authorized static
representability question held behind the A3 human checkpoint. Hard/direct
remains selected and no product/default/tolerance/oracle decision changes.

S16 A1 first-event disposition, 2026-07-20: the verified
[event-divergence package](../../evidence/p5.2b-s16.2-first-event-divergence/README.md)
records exact replay for all three scalar endpoints at depths 11 and 16. Every
pairwise scalar comparison first differs on the declared
separation-positive/speculative-branch token. Exact binary32 versus the
cancellation-path binary64 lane first splits at step 1, substep 2,
relaxation, pair 4–5 point 1; exact binary32 versus full binary64 first splits
at step 1, substep 1, relaxation, pair 8–9 point 0. In each case the first
downstream impulse delta occurs at the same event. Retained native public
manifold/cache observations are digest-bound; unavailable native branch,
active-set, and color internals are not inferred. This assigns the first
divergence to an authorized family and admits A2 without claiming that the
family is sufficient or changing any product, recurrence, coefficient, band,
or authority decision.

S16 A2 event-owner replay, 2026-07-20: the verified
[bidirectional replay package](../../evidence/p5.2b-s16.3-event-owner-replay/README.md)
contains all 24 single-family rows across both directions and depths. Every
row replays its fail-closed result exactly. Each immutable owner eventually
requests a choice absent from the receiving trajectory: active-set payload at
step 1, cache ownership as early as step 2, identity or exact normal-event key
around steps 99–102, and contact/order ownership around steps 106–119. No
continuous value is copied and no missing choice is invented. Because no
`f64<-f32` arm completes to improve the full-binary64 first failure, the
individually causal family set is empty. This is negative evidence for
single-family discrete ownership, not a product or authority change.

S16 A3 minimal-structure disposition, 2026-07-20: the verified
[minimal event package](../../evidence/p5.2b-s16.4-minimal-event-structure/README.md)
returns `no-declared-routed-event-structure`. A2's individually causal family
set is empty, so the only bounded subset is the unrouted full-binary64 control.
It replays exactly but fails structure, authority response, and
penetration/support at both governed depths. There is no larger subset A3 may
open without adding a noncausal family or an undeclared interaction search.
The workstream is stopped at its mandatory human checkpoint. A4 static GPU
representability remains authorized but held; no product candidate, GPU
mapping or execution, source copy, new family, band change, or recurrence
change is admitted.

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
