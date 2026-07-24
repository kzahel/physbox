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

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read> awake_bodies: array<u32>;
@group(0) @binding(2) var<storage, read> awake_points: array<PointInput>;
@group(0) @binding(3) var<storage, read> awake_point_state: array<u32>;
@group(0) @binding(4) var<storage, read_write> solver_bodies: array<u32>;
@group(0) @binding(5) var<uniform> params: vec4u;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const BODY_STATE_WORDS: u32 = 20u;

fn append_fixed(slot: u32, dynamic_count: u32, count: ptr<function, u32>) {
    if (slot >= params.y) { return; }
    let identity = metadata[slot];
    if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) != (FLAG_LIVE | FLAG_FIXED)) { return; }
    for (var index = dynamic_count; index < *count; index += 1u) {
        if (solver_bodies[BODY_STATE_WORDS + index] == slot) { return; }
    }
    solver_bodies[BODY_STATE_WORDS + *count] = slot;
    *count += 1u;
}

@compute @workgroup_size(1)
fn compact_solver_bodies() {
    let dynamic_count = min(awake_bodies[0], params.y);
    var solver_count = dynamic_count;
    for (var index = 0u; index < dynamic_count; index += 1u) {
        solver_bodies[BODY_STATE_WORDS + index] = awake_bodies[BODY_STATE_WORDS + index];
    }
    for (var point_index = 0u; point_index < awake_point_state[2]; point_index += 1u) {
        let point = awake_points[point_index];
        append_fixed(point.ids.x, dynamic_count, &solver_count);
        append_fixed(point.ids.y, dynamic_count, &solver_count);
    }
    for (var index = solver_count; index < solver_bodies[1]; index += 1u) {
        solver_bodies[BODY_STATE_WORDS + index] = 0u;
    }
    solver_bodies[0] = solver_count;
    solver_bodies[1] = solver_count;
    solver_bodies[2] = 0u;
    solver_bodies[3] = 0u;
    solver_bodies[4] = (solver_count + 63u) / 64u;
    solver_bodies[5] = 1u;
    solver_bodies[6] = 1u;
    solver_bodies[7] = (solver_count + 255u) / 256u;
    solver_bodies[8] = 1u;
    solver_bodies[9] = 1u;
    solver_bodies[10] = (solver_count * 32u + 63u) / 64u;
    solver_bodies[11] = 1u;
    solver_bodies[12] = 1u;
    solver_bodies[13] = 6u;
    solver_bodies[14] = solver_count;
    solver_bodies[15] = 0u;
    solver_bodies[16] = 0u;
    solver_bodies[17] = max(1u, (solver_count + 63u) / 64u);
    solver_bodies[18] = 1u;
    solver_bodies[19] = 1u;
}
