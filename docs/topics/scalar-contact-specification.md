# Scalar contact specification

Status: pass equations and analytic baselines implemented, 2026-07-14;
multi-step fixture contact regeneration is pending in S1.2.

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
bias, `mass_scale = 1`, and `impulse_scale = 0`. The later multi-step fixture
runner will place position integration between biased solve and relaxation;
the current captured-pass entry point exposes the equations without claiming a
complete world step.

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

## Reconsideration triggers

Change these equations or ordering only with measured candidate evidence and a
same-change update to scalar, GPU, traces, and this topic. Changing physical
defaults, accepted quality, or numerical bands remains an S1.6 human decision.
If the Box2D authority later conflicts with a mathematical invariant, preserve
the reproducer and stop for review rather than making this specification mimic
the conflict silently.
