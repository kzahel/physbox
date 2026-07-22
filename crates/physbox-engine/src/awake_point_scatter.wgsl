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
@group(0) @binding(9) var<storage, read_write> solved_prepared: array<PreparedPoint>;

fn invalid_prepared_point() -> PreparedPoint {
    var point: PreparedPoint;
    point.ids = vec4u(0xffffffffu);
    point.normal = vec2f(0.0);
    point.tangent = vec2f(0.0);
    point.anchor_a = vec2f(0.0);
    point.anchor_b = vec2f(0.0);
    point.data0 = vec4f(0.0);
    point.data1 = vec4f(0.0);
    point.data2 = vec4f(0.0);
    return point;
}

@compute @workgroup_size(64)
fn scatter_prepared_points(@builtin(global_invocation_id) invocation: vec3u) {
    let awake_index = invocation.x;
    if (awake_index >= awake_state[2]) { return; }
    persistence_prepared[awake_to_source[awake_index]] = solved_prepared[awake_index];
}

@compute @workgroup_size(1)
fn clear_stale_prepared_points() {
    for (var index = awake_state[2]; index < awake_state[0]; index += 1u) {
        solved_prepared[index] = invalid_prepared_point();
    }
}
