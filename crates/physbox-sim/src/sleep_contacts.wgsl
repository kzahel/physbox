struct PointInput {
    ids: vec4u,
    normal: vec2f,
    local_anchor_a: vec2f,
    local_anchor_b: vec2f,
    padding0: vec2f,
    material: vec4f,
    cache: vec2f,
    padding1: vec2f,
}

struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

struct SleepParams {
    counts: vec4u,
    thresholds: vec4f,
}

@group(0) @binding(0) var<storage, read> points: array<PointInput>;
@group(0) @binding(1) var<storage, read> point_state: array<u32>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read> motions: array<vec4f>;
@group(0) @binding(4) var<storage, read> poses: array<vec4f>;
@group(0) @binding(5) var<storage, read> sleep_state: array<SleepStateV1>;
@group(0) @binding(6) var<storage, read> live_bodies: array<u32>;
@group(0) @binding(7) var<storage, read_write> source_frontier: array<atomic<u32>>;
@group(0) @binding(8) var<storage, read_write> destination_frontier: array<atomic<u32>>;
@group(0) @binding(9) var<uniform> params: SleepParams;

const ACTIVE_BODY_STATE_WORDS: u32 = 20u;
const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const SLEEP_FLAG_AWAKE: u32 = 1u << 0u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;
const CAUSE_ERASED_NEIGHBOR: u32 = 1u << 5u;
const CAUSE_CONTACT: u32 = 1u << 6u;
const CAUSE_IMPACT: u32 = 1u << 7u;
const CAUSE_PROPAGATION: u32 = 1u << 8u;

fn valid_slot(slot: u32) -> bool {
    return slot < params.counts.x &&
        (metadata[slot].y & (FLAG_LIVE | FLAG_FIXED)) == FLAG_LIVE;
}

fn valid_point(point: PointInput) -> bool {
    return point.ids.x < params.counts.x && point.ids.y < params.counts.x;
}

fn state_matches(slot: u32) -> bool {
    return valid_slot(slot) &&
        sleep_state[slot].identity_flags.x == metadata[slot].x &&
        sleep_state[slot].provenance.z == SLEEP_STATE_LAYOUT_VERSION;
}

fn point_velocity(slot: u32, local_anchor: vec2f) -> vec2f {
    let angle = poses[slot].z;
    let c = cos(angle);
    let s = sin(angle);
    let anchor = vec2f(
        c * local_anchor.x - s * local_anchor.y,
        s * local_anchor.x + c * local_anchor.y,
    );
    let motion = motions[slot];
    return motion.xy + vec2f(-motion.z * anchor.y, motion.z * anchor.x);
}

fn seed_erased_neighbor_point(point_index: u32) {
    let point = points[point_index];
    if (!valid_point(point)) { return; }
    let generations = bitcast<vec2u>(point.padding1);
    let current_a = (metadata[point.ids.x].y & FLAG_LIVE) != 0u &&
        metadata[point.ids.x].x == generations.x;
    let current_b = (metadata[point.ids.y].y & FLAG_LIVE) != 0u &&
        metadata[point.ids.y].x == generations.y;
    if (!current_a && current_b && state_matches(point.ids.y)) {
        atomicOr(&source_frontier[point.ids.y], CAUSE_ERASED_NEIGHBOR);
    }
    if (!current_b && current_a && state_matches(point.ids.x)) {
        atomicOr(&source_frontier[point.ids.x], CAUSE_ERASED_NEIGHBOR);
    }
}

@compute @workgroup_size(64)
fn seed_erased_neighbors(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= point_state[2]) { return; }
    seed_erased_neighbor_point(point_index);
}

@compute @workgroup_size(64)
fn seed_pair_slot_span_erased_neighbors(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    if (pair_index >= point_state[2]) { return; }
    seed_erased_neighbor_point(pair_index * 2u);
    seed_erased_neighbor_point(pair_index * 2u + 1u);
}

fn seed_contact_impact_point(point_index: u32) {
    let point = points[point_index];
    if (!valid_point(point) || !state_matches(point.ids.x) || !state_matches(point.ids.y)) {
        return;
    }
    let awake_a = (sleep_state[point.ids.x].identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
    let awake_b = (sleep_state[point.ids.y].identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
    if (bitcast<u32>(point.padding0.x) != 0u && (awake_a || awake_b)) {
        atomicOr(&source_frontier[point.ids.x], CAUSE_CONTACT);
        atomicOr(&source_frontier[point.ids.y], CAUSE_CONTACT);
    }
    let relative = point_velocity(point.ids.y, point.local_anchor_b) -
        point_velocity(point.ids.x, point.local_anchor_a);
    let closing_speed = -dot(relative, point.normal);
    if (closing_speed > params.thresholds.y) {
        atomicOr(&source_frontier[point.ids.x], CAUSE_IMPACT);
        atomicOr(&source_frontier[point.ids.y], CAUSE_IMPACT);
    }
}

@compute @workgroup_size(64)
fn seed_contact_impacts(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= point_state[2]) { return; }
    seed_contact_impact_point(point_index);
}

@compute @workgroup_size(64)
fn seed_pair_slot_span_contact_impacts(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    if (pair_index >= point_state[2]) { return; }
    seed_contact_impact_point(pair_index * 2u);
    seed_contact_impact_point(pair_index * 2u + 1u);
}

@compute @workgroup_size(64)
fn copy_frontier(@builtin(global_invocation_id) invocation: vec3u) {
    let live_index = invocation.x;
    if (live_index >= live_bodies[0]) { return; }
    let slot = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
    atomicStore(&destination_frontier[slot], atomicLoad(&source_frontier[slot]));
}

fn propagate_point(point_index: u32) {
    let point = points[point_index];
    if (!valid_point(point) || !state_matches(point.ids.x) || !state_matches(point.ids.y)) {
        return;
    }
    let causes_a = atomicLoad(&source_frontier[point.ids.x]);
    let causes_b = atomicLoad(&source_frontier[point.ids.y]);
    if (causes_a != 0u) {
        atomicOr(&destination_frontier[point.ids.y], causes_a | CAUSE_PROPAGATION);
    }
    if (causes_b != 0u) {
        atomicOr(&destination_frontier[point.ids.x], causes_b | CAUSE_PROPAGATION);
    }
}

@compute @workgroup_size(64)
fn propagate_frontier(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= point_state[2]) { return; }
    propagate_point(point_index);
}

@compute @workgroup_size(64)
fn propagate_pair_slot_span_frontier(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    if (pair_index >= point_state[2]) { return; }
    propagate_point(pair_index * 2u);
    propagate_point(pair_index * 2u + 1u);
}
