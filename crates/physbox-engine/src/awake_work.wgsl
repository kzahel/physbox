struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read> live_bodies: array<u32>;
@group(0) @binding(2) var<storage, read_write> sleep_state: array<SleepStateV1>;
@group(0) @binding(3) var<storage, read_write> awake_bodies: array<u32>;

const ACTIVE_BODY_STATE_WORDS: u32 = 20u;
const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const SLEEP_FLAG_AWAKE: u32 = 1u << 0u;
const SLEEP_FLAG_ELIGIBLE: u32 = 1u << 1u;
const SLEEP_POLICY_VERSION: u32 = 1u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;
const CAUSE_INITIALIZATION: u32 = 1u << 0u;

@compute @workgroup_size(1)
fn compact_awake_bodies() {
    let previous_awake_count = awake_bodies[0];
    let live_count = live_bodies[0];
    var awake_count = 0u;
    for (var live_index = 0u; live_index < live_count; live_index += 1u) {
        let slot = live_bodies[ACTIVE_BODY_STATE_WORDS + live_index];
        let identity = metadata[slot];
        if ((identity.y & FLAG_LIVE) == 0u) { continue; }
        let dynamic = (identity.y & FLAG_FIXED) == 0u;
        let prior = sleep_state[slot];
        let same_generation = prior.identity_flags.x == identity.x &&
            prior.provenance.z == SLEEP_STATE_LAYOUT_VERSION;
        let causes = select(CAUSE_INITIALIZATION, prior.provenance.x, same_generation);
        let flags = select(0u, SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE, dynamic);
        sleep_state[slot].identity_flags = vec4u(identity.x, flags, 0u, 0u);
        sleep_state[slot].provenance = vec4u(
            causes,
            SLEEP_POLICY_VERSION,
            SLEEP_STATE_LAYOUT_VERSION,
            0u,
        );
        if (dynamic) {
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
}
