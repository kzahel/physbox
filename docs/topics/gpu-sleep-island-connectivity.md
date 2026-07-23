# GPU sleep-island connectivity research

Status: S30's configurable behavior switch is complete, 2026-07-23. Exact
labels/reductions now drive an explicit behavior option beside local-4 and
local-8, with matched cost, cycle diagnostics, browser, headless, ARM64/APK,
and authorized AVD evidence. Sleeping remains opt-in and off by default.
Exact fixes the governed 100-body bounded-grouping failure and is the
recommended explicit comparison for that behavior, but local-4 remains the
compatibility default pending broader natural-fixture and adapter evidence.

Governing behavior and current implementation:
[GPU sleeping and waking](gpu-sleeping-and-waking.md). Performance claims and
benchmark validity: [performance and scale](performance-and-scale.md). Awake
quality remains owned by
[awake settling and rest stability](awake-settling-and-rest-stability.md);
exact islands may expose solver motion but may not redefine it as rest. The
rigid behavior authority is [Box2D](box2d-rigid-authority.md), pinned at
`3a4f0da8374af61293a03021c9a0b3ebcfe67948`.

## Current decision

Retain implemented local-4, local-8, and exact as explicit PhysBox
sleep-grouping choices. Interactive same-application Box2D comparison makes
the mixed-transition behavior directly inspectable. Exact connectivity
enforces a whole-current-component invariant but cannot make residual solver
motion qualify as rest.

Local-8 is a diagnostic radius extension, not an island approximation eligible
for promotion. Exact reuses the qualified static component labels and root
reductions. In the clean 1,200-step 100-body poured pile, local-4 and local-8
retain all bodies awake, while exact reaches stable all-body sleep at step 189
and matched-threshold Box2D at step 192. This confirms the causal behavior
interactive review predicted. Exact remains explicit rather than replacing
local-4 from one adapter and natural fixture; complete behavior/cost
provenance and that disposition are in
[S30](../tactical/030-configurable-sleep-grouping.md).

The S27 observer adds four root reduction/comparison dispatches to S26's exact
rebuild: 33 dispatches at a 64-body reserve and 41 at 256, versus the current
nine-dispatch K=4 enabled-path delta. Exact reductions alone cost only
0.007–0.008 ms GPU P50; connectivity dominates. Total observer P50 is
0.251–0.268 ms on the 64-slot active towers and 0.394 ms on the 256-slot
poured pile, respectively 15–22% and 4.73% of the measured unchanged physics
step. The fixed working set is 5,632 and 11,264 bytes; multi-step body/stat
trace storage is validation-only and not included in those figures.

## S27 decision evidence

Clean revision `98045db8bbff076661520b79bf3bca93056f3b16` produced
`/tmp/physbox5-s27-sleep-decisions-98045db.json`, SHA-256
`8c9e26cfe7ba671a6ae5717bae871cd880596cfcf4232f4f8621ca22cde4eedc`,
on AMD Radeon 890M RADV Vulkan. One warmup and five retained timestamp samples
make P95 and P99 the same maximum sample; use the tail values as diagnostic,
not a release-quality distribution. All 3,601 traced steps pass exact root and
edge checks, CPU reduction replay, impossible-direction checks, and
byte-identical applied K=4 replay.

| Row | Horizon | Exact-only wake signal | Changed transition | Observer P50 / P95 | Physics P50 / share |
| --- | ---: | ---: | ---: | ---: | ---: |
| sleeping eight-body path | 1 | 3 bodies | 3 new wakes projected | 0.268 / 0.280 ms | 1.346 ms / 19.92% |
| centered tower 2 | 600 | none | none; both sleep at step 30 | 0.260 / 0.847 ms | 1.178 ms / 22.07% |
| centered tower 8 | 600 | 10 body-events over 10 steps | none; all were awake | 0.251 / 0.478 ms | 1.537 ms / 16.34% |
| centered tower 14 | 1,200 | 1,484 body-events over 301 steps, peak 8 | none; all were awake | 0.258 / 0.440 ms | 1.710 ms / 15.09% |
| poured pile 100 | 1,200 | 9,962 body-events over 836 steps, peak 82 | none; all were awake | 0.394 / 0.616 ms | 8.344 ms / 4.73% |

No row records an invalid K-only wake, K-only blocker, or exact-only
admission. No evolved row records a K-only partial admission. The 8- and
14-body towers retain every body awake through steps 600 and 1,200; their last
local wake roots occur at steps 493 and 1,190. The poured pile retains all 100
bodies awake, has 99 local blockers at step 1,200, and still creates wake roots
on that step. That is direct evidence for solver/readiness limitation in these
fixtures, not evidence that exact islands would settle them.

The counterfactual remains local to each observed K=4 step. Applying a broader
wake could change later motion and readiness, so the trace cannot predict a
long-run exact-island trajectory. Conversely, the absence of any changed
natural transition means there is not yet evidence worth paying that causal
experiment's product-adjacent cost.

### Prior S26 component evidence

The completed observation-only static rebuild remains exact and
capacity-clean. It uses deterministic monotonic min-root hooking, pointer
shortcut dispatch barriers, a fixed `2 * ceil(log2(capacity)) + 1` phase bound,
and independent final root/edge validation. Exact labels and structural counts
replay; atomic work counts are reported as order-dependent contention proxies.

The static path costs 29 dispatches at a 64-body reserve, 37 at 256, 45 at
1,024, and 61 at 10K. Representative GPU P50 is 0.219 ms for an eight-body
chain, 0.299 ms for a 100-body packed pile, 0.577 ms for a 1,000-body packed
pile, 0.361 ms for the matched 1,000-body sleeping city, and 0.489 ms for 200
live bodies under a 10K reserve. Parent, size, statistics, and aligned
parameter storage totals 4,160 bytes at 64, 13,888 bytes at 1,024, and 87,744
bytes at 10K. The complete measurements and provenance are in
[S26](../tactical/026-exact-contact-component-observation.md).
The dense S26 rows intentionally use deterministic initially touching packed
piles; S27 now supplies the separate time-evolved decision trace.

## Observed problem and required invariant

Interactive sleep visualization first exposed an out-of-phase two-body cycle.
The coordinated-admission correction fixed that case by propagating wake roots
and not-ready blockers four contact hops. Larger poured piles can still show a
wave of `!` markers through bodies that appear stationary. That is consistent
with, but does not by itself prove, a component boundary problem: a connected
contact path longer than K can contain both sleeping and awake bodies. Freezing
one side changes the solver support presented to the other side, which can
create a new wake root and move the boundary.

A 2026-07-22 interactive capture provides a smaller human reproducer: one
large dynamic rectangle supporting a five-box tower on a fixed floor. In one
frame the large support is pink `!`, its two nearest boxes are amber quiet
candidates, and the upper three boxes are teal awake. This is a six-dynamic-body
fixture, not a dense-pile-only effect. The capture is qualitative evidence of a
mixed transition; the research fixture must determine whether a sleep
transition in the support precedes a dependent-body wake and must compare the
same interval with sleeping disabled before assigning cause.
The source capture is `Screenshot_20260722-172608-sd.png`, SHA-256
`891ec0b44a26219cae439deb53bff8acdd1dabc451c236c5f6c8779272a303d7`.

An exact sleep island would enforce these invariants at each transition:

- vertices are current live non-fixed bodies;
- an edge is one current touching dynamic-dynamic body pair, not one edge per
  scalar manifold point and not a broadphase-only pair;
- static bodies do not connect otherwise independent structures through a
  common floor;
- all bodies in one connected component share one deterministic component
  label, preferably the minimum current body slot for diagnostics;
- a pre-solve wake root wakes its complete component before awake compaction;
- if any member is not ready after solve, no member of that component is
  admitted to sleep; and
- after admission, no current component edge crosses the awake/asleep cut.

The final point is the behavior K=4 only approximates. Exact grouping does not
guarantee that a pile sleeps. One body with continuing solver motion correctly
keeps the whole component awake. That may make a solver-quality problem more
visible and can increase awake work, but it avoids changing support by
partially freezing one connected structure.

## Current PhysBox mechanism and exact cost

PhysBox keeps generation-checked sleep state per body. Before solve, event
roots from construction, grabs, new contacts, impacts, reset, and erased
support are propagated over current GPU contact points. Persisted resting
contacts are graph edges but are not wake roots. After solve, each unready body
seeds a blocker; the same contact propagation prevents nearby ready bodies
from sleeping. Both traversals are bounded to K=4.

For propagation bound `K`, each traversal contains K live-body frontier-copy
dispatches and K contact-point propagation dispatches. The current generated
contact schedule therefore accounts for sleep work as:

```text
sleeping enabled:  11 + 4K dispatches
sleeping disabled: 10 + 2K dispatches
enabled delta:      1 + 2K dispatches
```

At K=4 the correction added nine enabled-path dispatches, from 145 to 154 in
the focused schedule. Raising K from 4 to 8 would add another 16 dispatches to
the enabled path and eight to the disabled path. It would double the reachable
radius but would still fail on a path longer than eight. Dispatch count is not
GPU time; the body copy scans live bodies and the propagation scan visits live
contact points through indirect work.

Current sleep allocation includes 32 bytes of `SleepStateV1`, four bytes of
awake-list capacity, four bytes of wake requests, and two four-byte frontier
buffers per reserved body, plus small fixed control/parameter buffers. The
frontiers are reused for wake and admission. A component implementation may
reuse some scratch, but it must report incremental rather than merely total
allocation.

### Fresh post-correction baseline

The focused sleeping-city evidence command was rerun before this document was
edited:

```sh
npm run benchmark:sleeping-city -- \
  --output /tmp/physbox5-sleep-island-research-baseline.json
```

The clean source revision was `cb75e656686228bd9fdd2465fa9fada1de6819a7`
on AMD Radeon 890M RADV Vulkan with four warmups and 20 retained native GPU
timestamp samples. The artifact SHA-256 is
`1b399669b29e33db863a25ae907f807568ea6e0c018f0d3372a2dc0c779164e3`.

| Authored awake | Solver bodies | Awake/live points | GPU P50 | GPU P95 |
| ---: | ---: | ---: | ---: | ---: |
| 1% | 10 | 6 / 666 | 5.569 ms | 6.351 ms |
| 10% | 100 | 66 / 666 | 5.775 ms | 6.334 ms |
| 100% | 1,000 | 666 / 666 | 6.230 ms | 6.889 ms |

Every matched row reports 1,523 total dispatches because indirect work size,
not topology, follows the authored awake count. Within this artifact, the 1%
row is 10.6% below the dense row. Its collision-plus-persistence P50 share is
31.8%, persistence alone is 0.85%, and the live/awake point ratio is 111x.
The 1%-awake reserve rows at 256/1K/10K body capacity have P50
`4.319/4.763/4.680 ms` (max/min `1.103`).

This is a current K=4 mechanism baseline, not an island comparison. The scene
authors awake state instead of reaching it through pile dynamics, so it
measures the value of awake work reduction but cannot predict whether exact
islands will reduce or increase awake-body time in a settling structure.

## Pinned Box2D implementation study

The clean source was verified by `npm run references:status` before this
study. The relevant implementation is primarily
[`island.c`](../../reference/box2d/src/island.c),
[`solver.c`](../../reference/box2d/src/solver.c),
[`solver_set.c`](../../reference/box2d/src/solver_set.c), and the serial contact
state transition in
[`physics_world.c`](../../reference/box2d/src/physics_world.c).

### Persistent topology, not a per-step sweep

Box2D maintains an island as a persistent connected component. An island owns
doubly linked lists of bodies, touching contacts, and joints plus counts and a
`constraintRemoveCount`. Static bodies have no island ID. A contact or joint
connected to a static body belongs to the dynamic body's island, but the
static body does not join connectivity. Consequently, two piles resting on one
floor do not become one island through that floor.

Parallel collision only identifies contact-state changes. Worker bitsets are
ORed and the changed contact IDs are processed serially in ascending ID order.
When a current manifold starts touching, Box2D sets the touching flag, wakes a
sleeping solver set if its other endpoint is awake, immediately merges the two
dynamic islands, and then places the contact in the constraint graph. A
persisted touching contact does no island work and is not a recurring wake
event.

Island merge is union-by-size in spirit but not a union-find tree. Box2D keeps
the island with the larger body count, rewrites every body/contact/joint island
ID in the smaller island, concatenates its membership lists, and destroys the
smaller island. The cost is proportional to the smaller island's members and
constraints, with deliberately cache-irregular ID rewrites. This is acceptable
because it is paid on topology additions, not every step.

### Removal is cheap until a split matters

When a touching contact stops or a joint is removed, Box2D unlinks the
constraint and increments `constraintRemoveCount`; it does not immediately
discover whether the island actually split. A dirty island remains a safe
connectivity over-approximation while its bodies are awake.

Body finalization computes a sleep velocity from the farthest-point velocity
and a half-weighted position-correction velocity. Each body's quiet timer is
reset or advanced. If any body has less than the fixed 0.5-second sleep time,
its island bit is marked awake in a per-worker bitset. The bitsets are unioned,
so one unready member keeps the entire island awake.

If a ready body belongs to a dirty island, the worker nominates that island
for splitting. The world deterministically selects one candidate per step,
preferring the longest sleep time and then breaking ties by island ID. The
selected split runs in the next solver window while island structures are not
otherwise touched. Box2D's own source calls the DFS expensive because it
touches substantial memory.

Split allocates arrays proportional to the dirty island's body count and runs
a depth-first traversal over current touching contacts and enabled joints. It
rebuilds every exact component in that stale island, rewrites membership, and
destroys the old island. A dirty island cannot sleep until that split is
complete. Explicitly forcing one awake body asleep performs the required split
synchronously, which the public API documents as potentially expensive and
unintuitive because the whole touching island is affected.

### Whole-island hot/cold storage

After body finalization, Box2D scans awake islands in reverse and moves each
fully ready, clean island to a newly allocated sleeping solver set. Body sims,
touching contacts, joints, and the island record leave the hot awake arrays;
awake body state is destroyed. Waking transfers the complete sleeping set back
to the awake arrays, resets every body timer, restores touching contacts to the
constraint graph, and destroys the sleeping set. Sleeping sets are also merged
by moving the smaller one when a new joint connects two sleepers.

This storage move is a separate optimization from island-correct sleep
semantics. PhysBox currently retains live broadphase, narrowphase, persistence,
and rendering for sleepers while filtering solver work. Adopting exact
component decisions does not imply adopting Box2D's cold contact storage; the
S23.6 evidence explicitly retained live pair processing.

### What transfers to PhysBox

The transferable design is:

1. topology edges arise from contact state, not from every body's quiet state;
2. additions merge components immediately;
3. removals mark topology stale rather than forcing an eager full split;
4. sleep readiness is a component reduction;
5. stale components split only when sleep makes exactness necessary; and
6. static supports belong to constraint processing but not connectivity.

The CPU representation does not transfer directly. Linked membership lists,
serial cache-miss rewrites, arena DFS stacks, and moving structs between CPU
vectors conflict with PhysBox's GPU-resident, capacity-bounded, no-readback
frame path. The product needs the same semantics expressed as buffers,
indirect work, deterministic reductions, and explicit overflow behavior.

## GPU alternatives

Let `N` be live dynamic bodies and `E` be current unique touching
dynamic-dynamic body pairs. One rectangle manifold may have two scalar points,
but connectivity needs one edge; a prototype should avoid paying twice for the
same pair.

| Candidate | Correctness and work | Expected benefit / risk | Research verdict |
| --- | --- | --- | --- |
| Current K=4 dual frontier | Fixed `O(K(N+E))` scans for wake plus admission; exact only within four contact hops. | Small, deterministic fixed radius; already fixes local phase cycles. Can leave a mixed boundary in larger structures. | Retain as control and product behavior during research. |
| Increase K to 8 | Doubles radius and adds 16 enabled-path dispatches over K=4. Still bounded by graph diameter. | May hide the observed pile at one size but does not establish an invariant. | Do not implement as the next step. |
| Naive min-label propagation | Repeatedly scans edges and bodies until labels stop changing; a chain can require `N-1` rounds. | Simple and deterministic, but a 1,000-body chain is a worst-case dispatch/memory-bandwidth failure. | Use only as a tiny CPU/test oracle, not the GPU candidate. |
| Static GPU hook-and-shortcut rebuild | Initialize one parent per body; alternate monotonic root hooking over edges and pointer shortcutting over bodies for a proved fixed phase bound; then reduce wake/blocker flags by root and broadcast. | Exact components and no deletion machinery. Pays full graph work every step even when topology is unchanged; atomic contention is worst in dense piles. | Recommended observation-only first probe and upper cost bound. |
| Persistent GPU components with dirty splits | Union additions into persistent parents; removals mark the stale component; rebuild only dirty near-sleep components from a GPU queue. | Closest to Box2D's amortization and potentially cheap in steady stacks. Deletion, membership, generation reuse, queue capacity, and split latency are substantially harder. | Recommended optimization direction only if the static probe proves value but costs too much. |
| CPU island manager/readback | Ordinary DFS or union-find on CPU. | Easy exact topology but creates a GPU-result dependency in the interactive frame. | Rejected by architecture. |
| Capacity-tier/workgroup islands | Solve small worlds in one workgroup and use another algorithm above a tier. | Fast special case with capacity-dependent behavior. | Rejected; PhysBox requires one semantic algorithm at every tier. |

The classic Shiloach-Vishkin family provides deterministic logarithmic-round
parallel connectivity through hooking and shortcutting, but its PRAM proof is
not automatically a WGSL proof. GPU studies likewise show that linking,
compression, graph structure, and atomic strategy materially change
performance. The PhysBox candidate must therefore specify and test its exact
phase bound rather than assuming `ceil(log2(N))` is sufficient.

WGSL supplies monotonic `atomicMin`, while `atomicCompareExchangeWeak` may
spuriously fail and all atomic built-ins use relaxed memory semantics. A first
candidate should use separate dispatches as global algorithm phases, monotonic
minimum labels for deterministic final roots, and no unbounded retry loop.
Fixed maximum phases may be pre-encoded and later phases disabled through
GPU-written indirect arguments after convergence; convergence must not require
CPU readback.

Primary algorithm/specification reading:

- Shiloach and Vishkin,
  [An O(log n) parallel connectivity algorithm](https://research.ibm.com/publications/an-ologn-parallel-connectivity-algorithm),
  Journal of Algorithms, 1982;
- Sutton et al.,
  [Adaptive Work-Efficient Connected Components on the GPU](https://arxiv.org/abs/1612.01178),
  including atomic hook and multi-jump compression;
- Hong, Dhulipala, and Shun,
  [Exploring the Design Space of Static and Incremental Graph Connectivity Algorithms on GPUs](https://arxiv.org/abs/2008.11839),
  showing that linking/compression choices and graph topology require measured
  selection; and
- the current
  [WGSL atomic specification](https://gpuweb.github.io/gpuweb/wgsl/#atomic-builtin-functions).

## Projected component-stage cost

For a static hook-and-shortcut probe with `P` proved phases, the unavoidable
shape is at least one `N` initialization scan, `P` edge-hook scans, `P` body
shortcut scans, and root aggregation/broadcast scans. Change reduction and
indirect control add further dispatches. In shorthand, it starts near
`O(P(N+E))` traffic and at least `2P+3` full-work dispatches; that is a design
projection, not a measured implementation result.

The smallest new persistent representation is one four-byte parent/component
word per reserved body. A practical static probe is more likely to need parent
plus scratch/change or root-aggregate state. The table brackets one and three
four-byte arrays without claiming a final layout:

| Body capacity | One array | Three arrays |
| ---: | ---: | ---: |
| 256 | 1,024 B | 3,072 B |
| 1,024 | 4,096 B | 12,288 B |
| 10,000 | 40,000 B | 120,000 B |
| 100,000 | 400,000 B | 1,200,000 B |

This memory is modest relative to body/contact storage. The likely risk is
repeated irregular global traffic, synchronization dispatches, and atomic
contention, especially on integrated and mobile GPUs. Static rebuild is a
worst-steady-state cost estimate; persistent dirty splitting trades that cost
for more memory, state transitions, and occasional island-sized spikes.

## Benefit model and break-even question

Exact islands offer three possible benefits:

- **Behavioral:** eliminate mixed awake/asleep contact cuts and the support
  changes caused by partial freezing of a connected structure.
- **Temporal:** replace recurring local wake waves with one component wake and
  one component sleep transition.
- **Performance:** if a connected pile becomes fully quiet, remove the entire
  component from solve work rather than repeatedly waking subsets.

They also impose two costs beyond connectivity computation:

- one marginally moving body retains every body in its component, so exact
  islands can produce more awake solver work than K=4; and
- a large component wake is intentionally broader than a four-hop wake.

The relevant break-even equation is therefore not component-stage time alone:

```text
saved solve/compaction time from lower recurring awake-body time
  > component maintenance time
    + extra solve time from whole-component readiness and wakes
```

Measure the integral of awake bodies and awake constraints over the complete
settle/wake cycle, not only the final asleep count or one frame. A component
algorithm that makes a pile behaviorally honest but never asleep may still be
the right correctness choice, while demonstrating that the scale benefit must
come from improving awake solver quality rather than weakening island
semantics.

## Research scenarios and measurements

The observation probe must share the real persistence/contact projection. At
minimum, run:

- two bodies with intentionally offset quiet counters, retaining the current
  fixed regression;
- the six-dynamic-body support tower from interactive review: one large base
  and five smaller rectangles on a fixed floor, recording the first support
  sleep, the first dependent wake, and the contact edge across each state
  boundary;
- chains at 4, 5, 8, 9, 64, and 1,000 bodies to expose diameter and phase
  bounds;
- two independent piles on one fixed floor, proving static support does not
  merge them;
- poured 100- and 1,000-box piles, including repeated deterministic runs;
- a stone-arch/bridge fixture, where one marginal contact may hold a large
  connected structure awake;
- contact removal that splits one large component into two, plus high-churn
  rain; and
- sleeping-city 1%/10%/100% awake and 256/1K/10K/100K capacity rows.

Record for every applicable row:

- CPU-reference versus GPU component labels and the first divergent body/edge;
- exact replay, component count, maximum component size, phases executed, and
  convergence/overflow state;
- touching edges crossing different component labels and awake/asleep edges,
  both required to be zero for an exact result;
- wake/sleep transition count, cohort sizes, awake-body-time, awake-point-time,
  and first stable sleep step;
- component-stage GPU P50/P95/P99, complete physics time, total dispatches,
  parent/scratch allocation, atomic attempts/contention proxy, and dirty-split
  work; and
- the existing sleep-off mask-rule outcome so island sleep never becomes a
  solver-stability pass.

The exact-component probe is rejected for product integration if labels are
not exact/replay-stable, if any capacity overflow silently degrades to partial
connectivity, or if a fully sleeping connected control is slower end-to-end
than K=4 without a compensating required behavior improvement. If the static
stage is behaviorally successful but too expensive, that is evidence for the
persistent/deferred-split prototype, not for increasing K.

## Staged stopping points

1. **Observation-only static probe — complete:** exact labels and timestamps,
   no effect on sleep behavior. S26 passes and is stopped at human review.
2. **Component reductions — complete:** S27 exact wake/readiness reductions
   match CPU checks beside K=4 and do not affect applied state.
3. **Configurable behavior switch — complete under S30:** local-4, local-8,
   and exact are cross-host product options; exact removes the governed
   100-body wake-cycle failure at measured bounded cost and remains explicit
   pending broader default evidence.
4. **Persistent optimization:** pursue additions plus deferred dirty splits
   only when static measurements demonstrate that maintenance, rather than
   broader whole-island solve work, is the limiting cost.
5. **Promotion:** requires the ordinary browser representative, ARM64 and
   release APK, physical Android behavior/performance, mask-rule disclosure,
   and a new explicit human decision. This research did not itself satisfy
   S23.7; the subsequent 2026-07-23 review closed S23.7 while retaining
   sleeping off and local-4 as defaults.

## Reconsideration triggers

- If the exact static rebuild is already below measurement noise on the named
  desktop, browser, and Android adapters, prefer it over persistent complexity.
- If dense piles show atomic contention or phase traffic dominating, evaluate
  persistent dirty splits and edge sampling/compaction before changing
  semantics.
- If exact islands remove mixed cuts but a pile remains awake, route the result
  to awake solver-quality work; do not weaken component readiness to manufacture
  a sleeping screenshot.
- When joints enter product scope, add enabled dynamic joint edges to the same
  component contract before claiming exact islands.
- Reopen cold sleeping-pair storage only if new mostly-sleeping evidence crosses
  the performance topic's dormant-pair gate; island labels alone do not change
  the S23.6 decision.
