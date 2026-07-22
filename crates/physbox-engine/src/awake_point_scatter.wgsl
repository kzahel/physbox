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

@group(0) @binding(5) var<storage, read> awake_state: array<u32>;
@group(0) @binding(6) var<storage, read> awake_to_source: array<u32>;
@group(0) @binding(7) var<storage, read_write> persistence_prepared: array<PreparedPoint>;
@group(0) @binding(9) var<storage, read> solved_prepared: array<PreparedPoint>;

@compute @workgroup_size(64)
fn scatter_prepared_points(@builtin(global_invocation_id) invocation: vec3u) {
    let awake_index = invocation.x;
    if (awake_index >= awake_state[2]) { return; }
    persistence_prepared[awake_to_source[awake_index]] = solved_prepared[awake_index];
}
