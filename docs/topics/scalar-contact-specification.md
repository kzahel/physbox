# Scalar contact specification

Status: S1.2 executable pass equations, analytic baselines, and scoped
multi-step fixture contact regeneration implemented; S12 independently
qualified a coupled authority-structure bridge without changing the selected
scalar equations or stage order, 2026-07-19.

Governing direction:
[one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md).
Shared wire contract:
[solver laboratory contracts](solver-laboratory-contracts.md).
Execution tracker:
[S1 rigid solver laboratory](../tactical/001-rigid-solver-laboratory.md).

## Purpose and boundary

`physbox-lab::scalar` is the executable specification for captured contact
passes. It consumes the same sorted bodies, authored manifolds, cache entries,
and settings that GPU candidates consume, and emits the shared body, cache,
and trace types. It is diagnostic and test-only. It is not a CPU product
backend, alternate engine state, broadphase, or general narrowphase.

The implementation is independent Rust written from the equations recorded
here. The pinned unmodified Box2D v3 source at
`3a4f0da8374af61293a03021c9a0b3ebcfe67948`, especially
`src/contact_solver.c`, `src/solver.c`, and `src/solver.h`, informed pass
decomposition and terminology; no Box2D source is linked or copied.

## Convention and preparation

A manifold normal points from body A toward body B. Its tangent is the right
perpendicular `(normal.y, -normal.x)`. An impulse `P` changes the bodies by:

```text
vA' = vA - inverse_mass_A * P
wA' = wA - inverse_inertia_A * cross(rA, P)
vB' = vB + inverse_mass_B * P
wB' = wB + inverse_inertia_B * cross(rB, P)
```

Here `rA` and `rB` are the authored local anchors rotated by each body's
orientation. For an axis `d`, preparation computes:

```text
k = inverse_mass_A + inverse_mass_B
  + inverse_inertia_A * cross(rA, d)^2
  + inverse_inertia_B * cross(rB, d)^2
effective_mass = if k > 0 then 1 / k else 0
relative_velocity(d) = dot((vB + wB x rB) - (vA + wA x rA), d)
```

Preparation records normal and tangent effective mass plus the initial normal
velocity used later by restitution. Warm starting matches the ordered
`(body A, body B, feature A, feature B)` key and applies the cached accumulated
normal and tangent impulses before a solve pass.

## Accumulated solve

For each declared iteration, all points of a contact solve non-penetration,
then friction. A positive separation uses speculative bias
`separation / substep_dt`; therefore it produces an impulse only when approach
speed can close that gap within the substep. A penetrating point in the bias
pass uses:

```text
velocity_bias = bias_rate * separation / substep_dt
mass_scale = configured mass_coefficient
impulse_scale = configured impulse_coefficient
increment = -normal_mass * (mass_scale * normal_velocity + velocity_bias)
          - impulse_scale * old_normal_impulse
new_normal_impulse = max(old_normal_impulse + increment, 0)
```

The applied impulse is the accumulated difference, never the unclamped
candidate. Tangent accumulation is:

```text
candidate = old_tangent_impulse - tangent_mass * tangent_velocity
limit = friction * new_normal_impulse
new_tangent_impulse = clamp(candidate, -limit, limit)
```

The relaxation pass repeats normal and tangent projection with no penetration
bias, `mass_scale = 1`, and `impulse_scale = 0`. The captured-pass entry point
exposes these equations directly. The analytic multi-step runner places
position integration between biased solve and relaxation and refreshes
separation from the moved body transforms before relaxing.

Restitution is eligible only when material restitution is positive, the
prepared impact velocity is below the negative configured threshold, and the
solve produced a positive normal impulse. It then projects the accumulated
normal impulse using current normal velocity plus
`restitution * initial_normal_velocity`. A speculative contact that generated
no collision impulse cannot bounce.

The existing coefficient values in engine-authored fixtures are laboratory
inputs, not reviewed product defaults or a solver-family selection.

## Trace contract and baselines

Trace order is deterministic: prepare, warm start, each normal/tangent
iteration, relaxation normal/tangent, then restitution. Every record identifies
step, substep, iteration, stage, manifold, and point. Values are sorted by name
and carry both finite `f32` values and exact bits. Comparison stops at the first
different record identity, name, count, or value outside the declared absolute
plus relative band.

Checked analytic baselines currently include:

- a central unit-inverse-mass impact with effective mass `1` and impulse equal
  to approach speed;
- an off-center contact with effective mass exactly `1 / 3` for the declared
  inverse inertia and lever arm;
- Coulomb projection that clamps tangent impulse to `friction * normal`;
- restitution above and below its velocity threshold;
- speculative separation with no closing speed and therefore zero impulse;
- explicit penetration bias, relaxation removal, warm start, mass scale, and
  accumulated-impulse scale records; and
- diagnosed rejection when finite inputs overflow intermediate `f32` math.

Metamorphic gates cover translation, a quarter-turn rotation, swapped manifold
orientation under the canonical normal rule, and total linear/angular momentum
for an isolated frictionless pair. Tests perturb an early trace value and
require the comparator to identify that exact first divergence.

## Scoped fixture contact regeneration

The shared engine scenario registry owns three neutral analytic families:
circle/floor, rectangle/floor, and axis-aligned box-on-box. They carry bodies,
shapes, requested exact step count, and solver settings but no authored
manifolds. The laboratory generator rebuilds manifolds from those shared bodies
at every substep, using stable feature identifiers so accumulated impulses can
warm start across regeneration.

Supported geometry is intentionally exhaustive rather than approximate:

- a circle against an axis-aligned fixed rectangle's top face;
- an axis-aligned or oriented rectangle against an axis-aligned fixed floor,
  with two points when flat and one deepest vertex when rotated; and
- an axis-aligned rectangle pair on the minimum-overlap axis, including
  vertical and side contact.

Circle/circle, general circle/rectangle, oriented box/box, floor edges, SAT
clipping, broadphase, and arbitrary feature persistence are rejected or out of
range; they are not silently approximated. Those belong to P5.2. The fixture
material projection uses geometric-mean friction and maximum restitution only
inside this validation generator. It is not a product material-policy
selection.

An exact fixture step prepares regenerated contacts, integrates gravity,
warm-starts and solves, integrates positions, refreshes separation, relaxes,
applies restitution, and stores the next cache. Sleeping input skips gravity
and integration and is checked for exact no-drift; no sleep or wake policy is
invented here. Authored manifolds and neutral actions are rejected by this
entry point so there is no ambiguous ownership.

At the checked 60 Hz, four-substep laboratory inputs, 180-step circle/floor
settling ends at `y = 0.49999994` with zero vertical velocity. Rectangle/floor
settling remains within `0.00034` of the support height with less than `0.007`
linear residual speed. A 60-step sliding fixture reduces horizontal speed from
`2.0` to approximately `0.00013`. Tests primarily gate deterministic replay,
exact integration-record count, bounded support/velocity, friction reduction,
and sleeping no-drift; these observations do not freeze product settings or
accepted solver quality.

## S9 schedule-differential closure

The [S9.6 decision package](../../evidence/p5.2b-s9.6-schedule-differential-review/README.md)
closes four material comparators and one timing hygiene control without
promoting scalar or GPU math. Contact-major normal/tangent staging (E3), the
single static-contact bias tuple (E4), fixed-anchor analytical separation
refresh (E5), and restitution after all substeps (E6) remain named laboratory
policies only. E2 changes color order without changing a scalar equation.

E5 and E6 are especially important boundaries for this specification: the
selected multi-step execution still refreshes anchors, effective mass, and
separation each substep and still applies restitution after each substep. E6
is byte-inert on the governed restitution-ineligible corpus, which is a
hygiene result rather than authority to reorder the selected trace. E5 reduces
short-stack residuals but materially worsens 16-stack energy, so its changed
preparation recurrence is rejected. The equations, coefficient ownership,
trace identity, and Box2D/invariant precedence above remain authoritative.

## S12 authority-structure laboratory boundary

The [S12 review](../../evidence/p5.2b-s12.6-authority-structure-review/README.md)
uses this specification as endpoint P and proves its independent bridge
bit-exact on all four frozen sources. Endpoint A is a separate versioned
laboratory recurrence derived independently from the pinned authority
structure; it does not redefine `physbox-lab::scalar` or the selected GPU
equations.

The decomposition finds a coupled four-axis measured survivor: outer-step
authority preparation/impulse transport, authority graph-color accumulation,
authority soft coefficients including static `2x`, and one biased plus one
relaxation pass. Restitution timing is unnecessary. No one axis or routed pair
qualifies, and unmeasured triples leave the true minimum bounded at 3-4. The
common authority bridge also uses scalar point-sequential normal-then-friction
order; the selected two-point block normal equation is not an exact
substitute. These are an executable decision target for a separately
authorized GPU probe, not a selected scalar or product-policy change.

## S13 portable arithmetic disposition

The [S13 review](../../evidence/p5.2b-s13.6-exact-gpu-probe-review/README.md)
executes that one exact target on portable GPU arithmetic without changing
this scalar recurrence. Scalar and GPU agree through the first substep to a
few ULPs, then first exceed the frozen `5e-5` implementation band in the
substep-1 biased normal pass. Exact GPU replay does not rescue the response:
depth 11 remains at `0.0304508 m/s`, and depth 16 loses structure.

This leaves the S12 scalar authority endpoint qualified and unchanged while
rejecting it as a directly portable product candidate. A future branch-robust
formulation would intentionally be a new recurrence with authority-band
response ownership; it is not an implementation correction to this
specification and requires a human-authorized study plus new scalar/GPU traces.
No tolerance, selected scalar equation, product coefficient, or oracle
precedence changes by the S13 result.

## S14 branch-robust scalar disposition

The [S14 review](../../evidence/p5.2b-s14.7-branch-robust-review/README.md)
derives and freezes one real-equivalent separation evaluation without changing
the normal equation, predicate, coefficients, or this selected specification.
It removes the retained large-term cancellation and passes local identity,
bound, outward-rounding, metamorphic, finite-state, and exact-replay controls.

The governed rounding gate rejects it at 10/14 rows. All depth-11 lanes pass;
at depth 16 the nearest, uniform-high, step-alternating, and hashed lanes fail
the unchanged energy or positive-run predicates. This is a response failure,
not a mathematical identity, finite-state, support, or error-bound failure.
The conditional GPU port is therefore not admitted. The formula remains
validation-only, the S12 endpoint and this specification remain unchanged,
and another formulation requires a new human decision.

## Reconsideration triggers

Change these equations or ordering only with measured candidate evidence and a
same-change update to scalar, GPU, traces, and this topic. Changing physical
defaults, accepted quality, or numerical bands remains an S1.6 human decision.
If the Box2D authority later conflicts with a mathematical invariant, preserve
the reproducer and stop for review rather than making this specification mimic
the conflict silently.
