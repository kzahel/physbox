struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

struct PointInput {
    ids: vec4u,
    normal: vec2f,
    local_anchor_a: vec2f,
    local_anchor_b: vec2f,
    padding0: vec2f,
    material: vec4f,
    cache: vec2f,
    generations: vec2f,
}

struct Params {
    // body capacity, point capacity, stable confirmation steps, reserved
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read> live_bodies: array<u32>;
@group(0) @binding(2) var<storage, read> sleep_state: array<SleepStateV1>;
@group(0) @binding(3) var<storage, read> awake_bodies: array<u32>;
@group(0) @binding(4) var<storage, read_write> control: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read> points: array<PointInput>;
@group(0) @binding(6) var<storage, read_write> point_state: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> metrics: array<atomic<u32>>;
@group(0) @binding(8) var<uniform> params: Params;

const ACTIVE_BODY_STATE_WORDS: u32 = 20u;
const VERSION: u32 = 1u;
const INVALID: u32 = 0xffffffffu;
const FLAG_LIVE: u32 = 1u;
const FLAG_FIXED: u32 = 4u;
const SLEEP_FLAG_AWAKE: u32 = 1u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;

fn is_dynamic(body: u32) -> bool {
    return body < params.counts.x &&
        (metadata[body].y & (FLAG_LIVE | FLAG_FIXED)) == FLAG_LIVE;
}

fn matching_state(body: u32) -> bool {
    return is_dynamic(body) &&
        sleep_state[body].identity_flags.x == metadata[body].x &&
        sleep_state[body].provenance.z == SLEEP_STATE_LAYOUT_VERSION;
}

fn is_awake(body: u32) -> bool {
    return matching_state(body) &&
        (sleep_state[body].identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
}

@compute @workgroup_size(1)
fn prepare_metrics() {
    atomicStore(&metrics[0], VERSION);
    for (var word = 2u; word <= 6u; word += 1u) {
        atomicStore(&metrics[word], 0u);
    }
    atomicStore(&metrics[12], 0u);
    atomicStore(&metrics[13], params.counts.z);
}

@compute @workgroup_size(64)
fn count_body_metrics(@builtin(global_invocation_id) invocation: vec3u) {
    let live_index = invocation.x;
    if (live_index >= live_bodies[0]) { return; }
    let body = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
    if (!matching_state(body)) { return; }
    atomicAdd(&metrics[12], 1u);
    let state = sleep_state[body];
    let awake = (state.identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
    if (state.identity_flags.w == atomicLoad(&control[0])) {
        if (awake) {
            atomicAdd(&metrics[5], 1u);
        } else {
            atomicAdd(&metrics[4], 1u);
        }
    }
}

@compute @workgroup_size(64)
fn count_mixed_contact_cuts(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= atomicLoad(&point_state[2])) { return; }
    let point = points[point_index];
    if (point.ids.w != 0u || point.ids.x == INVALID || point.ids.y == INVALID ||
        point.ids.x == point.ids.y || !matching_state(point.ids.x) ||
        !matching_state(point.ids.y)) {
        return;
    }
    let generations = bitcast<vec2u>(point.generations);
    if (metadata[point.ids.x].x != generations.x ||
        metadata[point.ids.y].x != generations.y) {
        return;
    }
    if (is_awake(point.ids.x) != is_awake(point.ids.y)) {
        atomicAdd(&metrics[6], 1u);
    }
}

@compute @workgroup_size(1)
fn finalize_metrics() {
    let step = atomicLoad(&control[0]);
    let awake = awake_bodies[0];
    let dynamic = atomicLoad(&metrics[12]);
    atomicStore(&metrics[1], step);
    atomicStore(&metrics[2], awake);
    atomicStore(&metrics[3], dynamic - min(dynamic, awake));

    let previous_low = atomicLoad(&metrics[10]);
    let next_low = previous_low + awake;
    atomicStore(&metrics[10], next_low);
    if (next_low < previous_low) {
        let previous_high = atomicAdd(&metrics[11], 1u);
        if (previous_high == INVALID) {
            atomicStore(&metrics[14], 1u);
        }
    }

    if (dynamic > 0u && awake == 0u) {
        let previous_streak = atomicLoad(&metrics[9]);
        if (previous_streak == 0u) {
            atomicStore(&metrics[8], step);
        }
        let streak = min(previous_streak + 1u, params.counts.z);
        atomicStore(&metrics[9], streak);
        if (streak >= params.counts.z && atomicLoad(&metrics[7]) == INVALID) {
            atomicStore(&metrics[7], atomicLoad(&metrics[8]));
        }
    } else {
        atomicStore(&metrics[8], INVALID);
        atomicStore(&metrics[9], 0u);
    }
}
