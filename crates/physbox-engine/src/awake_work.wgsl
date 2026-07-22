struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

struct SleepParams {
    counts: vec4u,
    thresholds: vec4f,
}

@group(0) @binding(0) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read> live_bodies: array<u32>;
@group(0) @binding(2) var<storage, read_write> sleep_state: array<SleepStateV1>;
@group(0) @binding(3) var<storage, read_write> awake_bodies: array<u32>;
@group(0) @binding(4) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(5) var<storage, read> properties: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> wake_requests: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> wake_frontier: array<atomic<u32>>;
@group(0) @binding(9) var<storage, read_write> control: array<atomic<u32>>;
@group(0) @binding(10) var<uniform> params: SleepParams;

const ACTIVE_BODY_STATE_WORDS: u32 = 20u;
const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_SLEEPING: u32 = 1u << 1u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const SLEEP_FLAG_AWAKE: u32 = 1u << 0u;
const SLEEP_FLAG_ELIGIBLE: u32 = 1u << 1u;
const SLEEP_POLICY_VERSION: u32 = 1u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;
const CAUSE_INITIALIZATION: u32 = 1u << 0u;
const CAUSE_CONFIGURATION: u32 = 1u << 1u;
const CAUSE_GRAB: u32 = 1u << 2u;
const CAUSE_CONSTRUCTION: u32 = 1u << 4u;
const CAUSE_CONTACT: u32 = 1u << 6u;
const CAUSE_IMPACT: u32 = 1u << 7u;
const CAUSE_AUTOMATIC: u32 = 1u << 9u;

fn radial_extent(slot: u32) -> f32 {
    let flags = metadata[slot].y;
    if ((flags & FLAG_CIRCLE) != 0u) {
        return properties[slot].x;
    }
    if ((flags & FLAG_RECTANGLE) != 0u) {
        return length(properties[slot].xy);
    }
    return 0.0;
}

fn farthest_point_speed(slot: u32) -> f32 {
    let motion = motions[slot];
    return length(motion.xy) + abs(motion.z) * radial_extent(slot);
}

@compute @workgroup_size(64)
fn begin_sleep_step(@builtin(global_invocation_id) invocation: vec3u) {
    let live_index = invocation.x;
    if (live_index >= live_bodies[0]) { return; }
    let slot = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
    let identity = metadata[slot];
    if ((identity.y & FLAG_LIVE) == 0u) { return; }
    atomicStore(&wake_frontier[slot], 0u);
    let request = atomicExchange(&wake_requests[slot], 0u);
    let dynamic = (identity.y & FLAG_FIXED) == 0u;
    var state = sleep_state[slot];
    let same_generation = state.identity_flags.x == identity.x &&
        state.provenance.z == SLEEP_STATE_LAYOUT_VERSION;
    let initialized_dynamic = same_generation &&
        (state.identity_flags.y & SLEEP_FLAG_ELIGIBLE) != 0u;
    if (!dynamic) {
        state.identity_flags = vec4u(identity.x, 0u, 0u, 0u);
        state.provenance.y = SLEEP_POLICY_VERSION;
        state.provenance.z = SLEEP_STATE_LAYOUT_VERSION;
        sleep_state[slot] = state;
        metadata[slot].y = identity.y & ~FLAG_SLEEPING;
        return;
    }
    if (!initialized_dynamic) {
        state.identity_flags = vec4u(
            identity.x,
            SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE,
            0u,
            0u,
        );
        state.provenance = vec4u(
            CAUSE_INITIALIZATION | CAUSE_CONSTRUCTION,
            SLEEP_POLICY_VERSION,
            SLEEP_STATE_LAYOUT_VERSION,
            0u,
        );
    }
    if (params.counts.y == 0u) {
        state.identity_flags.y = SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE;
        state.identity_flags.z = 0u;
        metadata[slot].y = identity.y & ~FLAG_SLEEPING;
    } else {
        state.identity_flags.y |= SLEEP_FLAG_ELIGIBLE;
        if ((request & CAUSE_GRAB) != 0u) {
            state.identity_flags.y &= ~SLEEP_FLAG_ELIGIBLE;
        }
        if ((state.identity_flags.y & SLEEP_FLAG_AWAKE) != 0u) {
            atomicOr(&wake_frontier[slot], CAUSE_CONTACT);
        } else if (farthest_point_speed(slot) > params.thresholds.y) {
            atomicOr(&wake_frontier[slot], CAUSE_IMPACT);
        }
        if (request != 0u) {
            atomicOr(&wake_frontier[slot], request);
        }
    }
    state.provenance.y = SLEEP_POLICY_VERSION;
    state.provenance.z = SLEEP_STATE_LAYOUT_VERSION;
    sleep_state[slot] = state;
}

@compute @workgroup_size(64)
fn finalize_wakes(@builtin(global_invocation_id) invocation: vec3u) {
    let live_index = invocation.x;
    if (live_index >= live_bodies[0]) { return; }
    let slot = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
    let identity = metadata[slot];
    if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) { return; }
    var state = sleep_state[slot];
    let causes = atomicLoad(&wake_frontier[slot]);
    if (params.counts.y == 0u) {
        state.identity_flags.y = SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE;
        state.identity_flags.z = 0u;
    } else if (causes != 0u && (state.identity_flags.y & SLEEP_FLAG_AWAKE) == 0u) {
        state.identity_flags.y |= SLEEP_FLAG_AWAKE;
        state.identity_flags.z = 0u;
        state.identity_flags.w = atomicLoad(&control[0]) + 1u;
        state.provenance.x = causes;
    }
    if ((state.identity_flags.y & SLEEP_FLAG_AWAKE) != 0u) {
        metadata[slot].y = identity.y & ~FLAG_SLEEPING;
    } else {
        metadata[slot].y = identity.y | FLAG_SLEEPING;
    }
    sleep_state[slot] = state;
}

fn write_awake_bodies() {
    let previous_awake_count = awake_bodies[0];
    let live_count = live_bodies[0];
    var awake_count = 0u;
    for (var live_index = 0u; live_index < live_count; live_index += 1u) {
        let slot = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
        let identity = metadata[slot];
        if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) { continue; }
        let state = sleep_state[slot];
        if (state.identity_flags.x == identity.x &&
            state.provenance.z == SLEEP_STATE_LAYOUT_VERSION &&
            (state.identity_flags.y & SLEEP_FLAG_AWAKE) != 0u) {
            awake_bodies[ACTIVE_BODY_STATE_WORDS + awake_count] = slot;
            awake_count += 1u;
        }
    }
    for (var stale = awake_count; stale < previous_awake_count; stale += 1u) {
        awake_bodies[ACTIVE_BODY_STATE_WORDS + stale] = 0u;
    }
    awake_bodies[0] = awake_count;
    awake_bodies[1] = awake_count;
    awake_bodies[2] = 0u;
    awake_bodies[3] += 1u;
    awake_bodies[4] = (awake_count + 63u) / 64u;
    awake_bodies[5] = 1u;
    awake_bodies[6] = 1u;
    awake_bodies[7] = (awake_count + 255u) / 256u;
    awake_bodies[8] = 1u;
    awake_bodies[9] = 1u;
    awake_bodies[10] = (awake_count * 32u + 63u) / 64u;
    awake_bodies[11] = 1u;
    awake_bodies[12] = 1u;
    awake_bodies[13] = 6u;
    awake_bodies[14] = awake_count;
    awake_bodies[15] = 0u;
    awake_bodies[16] = 0u;
    atomicStore(&control[2], awake_count);
    atomicStore(&control[3], live_bodies[1] - awake_count);
}

@compute @workgroup_size(1)
fn compact_awake_bodies() {
    atomicAdd(&control[0], 1u);
    write_awake_bodies();
}

@compute @workgroup_size(1)
fn refresh_awake_bodies() {
    write_awake_bodies();
}

@compute @workgroup_size(64)
fn admit_sleeping_bodies(@builtin(global_invocation_id) invocation: vec3u) {
    let live_index = invocation.x;
    if (live_index >= live_bodies[0]) { return; }
    let slot = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
    let identity = metadata[slot];
    if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) { return; }
    var state = sleep_state[slot];
    if (state.identity_flags.x != identity.x ||
        state.provenance.z != SLEEP_STATE_LAYOUT_VERSION ||
        (state.identity_flags.y & SLEEP_FLAG_AWAKE) == 0u) {
        return;
    }
    if (params.counts.y == 0u ||
        (state.identity_flags.y & SLEEP_FLAG_ELIGIBLE) == 0u) {
        state.identity_flags.z = 0u;
        sleep_state[slot] = state;
        return;
    }
    if (farthest_point_speed(slot) > params.thresholds.x) {
        state.identity_flags.z = 0u;
        sleep_state[slot] = state;
        return;
    }
    let quiet_steps = state.identity_flags.z + 1u;
    state.identity_flags.z = quiet_steps;
    if (quiet_steps >= params.counts.z) {
        state.identity_flags.y &= ~SLEEP_FLAG_AWAKE;
        state.identity_flags.w = atomicLoad(&control[0]);
        state.provenance.x = CAUSE_AUTOMATIC;
        motions[slot] = vec4f(0.0);
        metadata[slot].y = identity.y | FLAG_SLEEPING;
    }
    sleep_state[slot] = state;
}
