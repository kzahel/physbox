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

struct PreparedPoint {
    ids: vec4u,
    normal: vec2f,
    tangent: vec2f,
    anchor_a: vec2f,
    anchor_b: vec2f,
    data0: vec4f,
    data1: vec4f,
    data2: vec4f,
}

struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

@group(0) @binding(0) var<storage, read> source_points: array<PointInput>;
@group(0) @binding(1) var<storage, read> source_state: array<u32>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read> sleep_state: array<SleepStateV1>;
@group(0) @binding(4) var<storage, read_write> awake_points: array<PointInput>;
@group(0) @binding(5) var<storage, read_write> awake_state: array<u32>;
@group(0) @binding(6) var<storage, read_write> awake_to_source: array<u32>;
@group(0) @binding(7) var<storage, read_write> persistence_prepared: array<PreparedPoint>;
@group(0) @binding(8) var<uniform> params: vec4u;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const SLEEP_FLAG_AWAKE: u32 = 1u << 0u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;

fn endpoint_awake(slot: u32) -> bool {
    if (slot >= params.y) { return false; }
    let identity = metadata[slot];
    if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) { return false; }
    let state = sleep_state[slot];
    return state.identity_flags.x == identity.x &&
        state.provenance.z == SLEEP_STATE_LAYOUT_VERSION &&
        (state.identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
}

fn cache_only_prepared(point: PointInput) -> PreparedPoint {
    var prepared: PreparedPoint;
    prepared.ids = point.ids;
    prepared.normal = vec2f(0.0);
    prepared.tangent = vec2f(0.0);
    prepared.anchor_a = vec2f(0.0);
    prepared.anchor_b = vec2f(0.0);
    prepared.data0 = vec4f(0.0);
    prepared.data1 = vec4f(0.0, 0.0, point.cache.x, point.cache.y);
    prepared.data2 = vec4f(0.0);
    return prepared;
}

@compute @workgroup_size(1)
fn compact_awake_points() {
    var awake_count = 0u;
    // Stable pair slots are addressed as pair_index * 2 and are intentionally
    // sparse when a preceding manifold owns one point. Scan the bounded solver
    // capacity and reject invalid sentinels instead of treating the packed
    // narrow-phase point count as a dense solver-input prefix.
    for (var index = 0u; index < params.x; index += 1u) {
        let point = source_points[index];
        persistence_prepared[index] = cache_only_prepared(point);
        if (point.ids.x >= params.y || point.ids.y >= params.y) { continue; }
        if (!endpoint_awake(point.ids.x) && !endpoint_awake(point.ids.y)) { continue; }
        awake_points[awake_count] = point;
        awake_to_source[awake_count] = index;
        awake_count += 1u;
    }
    awake_state[0] = params.x;
    awake_state[1] = awake_count;
    awake_state[2] = awake_count;
    awake_state[3] = source_state[3];
    awake_state[4] = (awake_count + 63u) / 64u;
    awake_state[5] = 1u;
    awake_state[6] = 1u;
    awake_state[7] = (awake_count + 255u) / 256u;
    awake_state[8] = 1u;
    awake_state[9] = 1u;
    awake_state[10] = (awake_count * 2u + 63u) / 64u;
    awake_state[11] = 1u;
    awake_state[12] = 1u;
    awake_state[13] = (awake_count * 2u + 255u) / 256u;
    awake_state[14] = 1u;
    awake_state[15] = 1u;
}
