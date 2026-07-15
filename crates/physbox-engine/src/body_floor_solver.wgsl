struct PointInput {
    ids: vec4u,
    normal: vec2f,
    local_anchor_a: vec2f,
    local_anchor_b: vec2f,
    padding0: vec2f,
    material: vec4f,
    cache: vec2f,
    padding1: vec2f,
};

struct PreparedPoint {
    ids: vec4u,
    normal: vec2f,
    tangent: vec2f,
    anchor_a: vec2f,
    anchor_b: vec2f,
    data0: vec4f,
    data1: vec4f,
    data2: vec4f,
};

struct Contribution {
    impulse: vec2f,
    anchor: vec2f,
    body_index: u32,
    point_index: u32,
    side: u32,
    padding: u32,
};

struct PassParams {
    floats0: vec4f,
    floats1: vec4f,
    uints0: vec4u,
    uints1: vec4u,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions_source: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> motions_destination: array<vec4f>;
@group(0) @binding(3) var<storage, read> properties: array<vec4f>;
@group(0) @binding(4) var<storage, read> point_inputs: array<PointInput>;
@group(0) @binding(5) var<storage, read_write> prepared_points: array<PreparedPoint>;
@group(0) @binding(6) var<storage, read_write> contributions: array<Contribution>;
@group(0) @binding(7) var<storage, read_write> topology: array<atomic<u32>>;
@group(0) @binding(8) var<uniform> params: PassParams;

fn rotate(vector: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * vector.x - s * vector.y, s * vector.x + c * vector.y);
}

fn cross(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

fn point_velocity(body_index: u32, anchor: vec2f) -> vec2f {
    let motion = motions_source[body_index];
    return motion.xy + vec2f(-motion.z * anchor.y, motion.z * anchor.x);
}

fn relative_velocity(point: PreparedPoint, axis: vec2f) -> f32 {
    return dot(point_velocity(point.ids.y, point.anchor_b) -
               point_velocity(point.ids.x, point.anchor_a), axis);
}

fn effective_mass(body_a: u32, body_b: u32, anchor_a: vec2f,
                  anchor_b: vec2f, axis: vec2f) -> f32 {
    let property_a = properties[body_a];
    let property_b = properties[body_b];
    let cross_a = cross(anchor_a, axis);
    let cross_b = cross(anchor_b, axis);
    let inverse = property_a.z + property_b.z +
        property_a.w * cross_a * cross_a + property_b.w * cross_b * cross_b;
    return select(0.0, 1.0 / inverse, inverse > 0.0);
}

fn emit_pair(point_index: u32, point: PreparedPoint, impulse: vec2f) {
    contributions[point_index * 2u].impulse = -impulse;
    contributions[point_index * 2u].anchor = point.anchor_a;
    contributions[point_index * 2u].body_index = point.ids.x;
    contributions[point_index * 2u].point_index = point_index;
    contributions[point_index * 2u].side = 0u;
    contributions[point_index * 2u + 1u].impulse = impulse;
    contributions[point_index * 2u + 1u].anchor = point.anchor_b;
    contributions[point_index * 2u + 1u].body_index = point.ids.y;
    contributions[point_index * 2u + 1u].point_index = point_index;
    contributions[point_index * 2u + 1u].side = 1u;
}

fn apply_impulse(body_index: u32, motion: vec4f, signed_impulse: vec2f,
                 anchor: vec2f) -> vec4f {
    let property = properties[body_index];
    var output = motion;
    output.x += signed_impulse.x * property.z;
    output.y += signed_impulse.y * property.z;
    output.z += property.w * cross(anchor, signed_impulse);
    return output;
}

fn valid_point(point: PointInput) -> bool {
    return point.ids.x != 0xffffffffu && point.ids.y != 0xffffffffu;
}

@compute @workgroup_size(64)
fn clear_topology(@builtin(global_invocation_id) id: vec3u) {
    if (id.x < params.uints1.w) {
        atomicStore(&topology[id.x], 0u);
    }
}

@compute @workgroup_size(64)
fn count_edges(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    if (!valid_point(point)) { return; }
    atomicAdd(&topology[point.ids.x * 2u + 1u], 1u);
    atomicAdd(&topology[point.ids.y * 2u + 1u], 1u);
}

@compute @workgroup_size(1)
fn prefix_edges(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    var running = 0u;
    for (var body = 0u; body < params.uints0.x; body += 1u) {
        let count = atomicLoad(&topology[body * 2u + 1u]);
        atomicStore(&topology[body * 2u], running);
        atomicStore(&topology[params.uints0.w + body], running);
        running += count;
    }
}

@compute @workgroup_size(64)
fn scatter_edges(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x) { return; }
    let start = atomicLoad(&topology[body_index * 2u]);
    var written = 0u;
    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point)) { continue; }
        if (point.ids.x == body_index) {
            atomicStore(&topology[params.uints0.z + start + written], point_index * 2u);
            written += 1u;
        }
        if (point.ids.y == body_index) {
            atomicStore(&topology[params.uints0.z + start + written], point_index * 2u + 1u);
            written += 1u;
        }
    }
}

@compute @workgroup_size(64)
fn integrate_velocity(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x || properties[body_index].z <= 0.0) { return; }
    let dt = params.floats0.x;
    var motion = motions_source[body_index];
    motion.x += params.floats1.y * dt;
    motion.y += params.floats1.z * dt;
    motions_source[body_index] = motion;
}

@compute @workgroup_size(64)
fn integrate_position(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x || properties[body_index].z <= 0.0) { return; }
    let motion = motions_source[body_index];
    var pose = poses[body_index];
    pose.x += motion.x * params.floats0.x;
    pose.y += motion.y * params.floats0.x;
    pose.z += motion.z * params.floats0.x;
    poses[body_index] = pose;
}

fn refreshed_point(input: PointInput, previous: PreparedPoint, reset: bool) -> PreparedPoint {
    let pose_a = poses[input.ids.x];
    let pose_b = poses[input.ids.y];
    let anchor_a = rotate(input.local_anchor_a, pose_a.z);
    let anchor_b = rotate(input.local_anchor_b, pose_b.z);
    let tangent = vec2f(input.normal.y, -input.normal.x);
    let initial_velocity = dot(point_velocity(input.ids.y, anchor_b) -
                               point_velocity(input.ids.x, anchor_a), input.normal);
    let retained_initial_velocity = previous.data0.x;
    var point = previous;
    point.ids = input.ids;
    point.normal = input.normal;
    point.tangent = tangent;
    point.anchor_a = anchor_a;
    point.anchor_b = anchor_b;
    point.data0 = vec4f(select(retained_initial_velocity, initial_velocity, reset),
        effective_mass(input.ids.x, input.ids.y, anchor_a, anchor_b, input.normal),
        effective_mass(input.ids.x, input.ids.y, anchor_a, anchor_b, tangent),
        dot((pose_b.xy + anchor_b) - (pose_a.xy + anchor_a), input.normal));
    if (reset) {
        point.data1 = vec4f(input.material.y, input.material.z, input.cache);
        point.data2 = vec4f(0.0);
    }
    return point;
}

@compute @workgroup_size(64)
fn prepare(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) { return; }
    var empty: PreparedPoint;
    prepared_points[id.x] = refreshed_point(point_inputs[id.x], empty, true);
}

@compute @workgroup_size(64)
fn refresh_contacts(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) { return; }
    prepared_points[id.x] = refreshed_point(
        point_inputs[id.x], prepared_points[id.x], params.uints0.w != 0u);
}

@compute @workgroup_size(64)
fn emit_warm(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) { return; }
    let point = prepared_points[id.x];
    emit_pair(id.x, point, point.normal * point.data1.z + point.tangent * point.data1.w);
}

@compute @workgroup_size(64)
fn emit_normal(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) { return; }
    var point = prepared_points[id.x];
    let normal_velocity = relative_velocity(point, point.normal);
    var velocity_bias = 0.0;
    var mass_scale = 1.0;
    var impulse_scale = 0.0;
    if (point.data0.w > 0.0) {
        velocity_bias = point.data0.w / params.floats0.x;
    } else if (params.uints1.w != 0u) {
        velocity_bias = params.floats0.y * point.data0.w / params.floats0.x;
        mass_scale = params.floats0.z;
        impulse_scale = params.floats0.w;
    }
    let old_impulse = point.data1.z;
    let candidate = -point.data0.y * (mass_scale * normal_velocity + velocity_bias) -
                    impulse_scale * old_impulse;
    point.data1.z = max(old_impulse + candidate, 0.0);
    let applied = point.data1.z - old_impulse;
    point.data2.x += max(applied, 0.0);
    prepared_points[id.x] = point;
    emit_pair(id.x, point, point.normal * applied);
}

@compute @workgroup_size(64)
fn emit_tangent(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) { return; }
    var point = prepared_points[id.x];
    let tangent_velocity = relative_velocity(point, point.tangent);
    let old_impulse = point.data1.w;
    let limit = point.data1.x * point.data1.z;
    point.data1.w = clamp(old_impulse - point.data0.z * tangent_velocity, -limit, limit);
    let applied = point.data1.w - old_impulse;
    prepared_points[id.x] = point;
    emit_pair(id.x, point, point.tangent * applied);
}

@compute @workgroup_size(64)
fn emit_restitution(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) { return; }
    var point = prepared_points[id.x];
    let normal_velocity = relative_velocity(point, point.normal);
    let old_impulse = point.data1.z;
    let eligible = point.data1.y > 0.0 && point.data0.x < -params.floats1.x &&
                     point.data2.x > 0.0;
    if (eligible) {
        let candidate = -point.data0.y * (normal_velocity + point.data1.y * point.data0.x);
        point.data1.z = max(old_impulse + candidate, 0.0);
    }
    let applied = point.data1.z - old_impulse;
    prepared_points[id.x] = point;
    emit_pair(id.x, point, point.normal * applied);
}

@compute @workgroup_size(64)
fn gather(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x) { return; }
    var motion = motions_source[body_index];
    let start = atomicLoad(&topology[body_index * 2u]);
    let count = atomicLoad(&topology[body_index * 2u + 1u]);
    for (var index = 0u; index < count; index += 1u) {
        let contribution_index = atomicLoad(&topology[params.uints1.w + start + index]);
        let contribution = contributions[contribution_index];
        motion = apply_impulse(body_index, motion, contribution.impulse, contribution.anchor);
    }
    motions_destination[body_index] = motion;
}
