struct Body {
    position: vec2f,
    angle: f32,
    inverse_mass: f32,
    linear_velocity: vec2f,
    angular_velocity: f32,
    inverse_inertia: f32,
};

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

struct Trace {
    identity: vec4u,
    values0: vec4f,
    values1: vec4f,
    tail: vec4u,
};

struct PassParams {
    floats0: vec4f,
    floats1: vec4f,
    uints0: vec4u,
    uints1: vec4u,
};

@group(0) @binding(0) var<storage, read_write> bodies_source: array<Body>;
@group(0) @binding(1) var<storage, read_write> bodies_destination: array<Body>;
@group(0) @binding(2) var<storage, read> point_inputs: array<PointInput>;
@group(0) @binding(3) var<storage, read_write> prepared_points: array<PreparedPoint>;
@group(0) @binding(4) var<storage, read_write> contributions: array<Contribution>;
@group(0) @binding(5) var<storage, read_write> topology: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> traces: array<Trace>;
@group(0) @binding(7) var<uniform> params: PassParams;

fn rotate(vector: vec2f, angle: f32) -> vec2f {
    let pair = vec2f(cos(angle), sin(angle));
    return vec2f(pair.x * vector.x - pair.y * vector.y,
                 pair.y * vector.x + pair.x * vector.y);
}

fn cross(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

fn point_velocity(body: Body, anchor: vec2f) -> vec2f {
    return body.linear_velocity + vec2f(-body.angular_velocity * anchor.y,
                                        body.angular_velocity * anchor.x);
}

fn relative_velocity(body_a: Body, body_b: Body, point: PreparedPoint, axis: vec2f) -> f32 {
    return dot(point_velocity(body_b, point.anchor_b) -
               point_velocity(body_a, point.anchor_a), axis);
}

fn effective_mass(body_a: Body, body_b: Body, anchor_a: vec2f,
                  anchor_b: vec2f, axis: vec2f) -> f32 {
    let cross_a = cross(anchor_a, axis);
    let cross_b = cross(anchor_b, axis);
    let inverse = body_a.inverse_mass + body_b.inverse_mass +
        body_a.inverse_inertia * cross_a * cross_a +
        body_b.inverse_inertia * cross_b * cross_b;
    return select(0.0, 1.0 / inverse, inverse > 0.0);
}

fn write_trace(index: u32, stage: u32, iteration: u32, point: PreparedPoint,
               values0: vec4f, values1: vec4f, value_count: u32) {
    if (params.floats1.w < 0.0) { return; }
    traces[index].identity = vec4u(stage, iteration, point.ids.z, point.ids.w);
    traces[index].values0 = values0;
    traces[index].values1 = values1;
    traces[index].tail = vec4u(value_count, u32(params.floats1.w), 0u, 0u);
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

fn apply_impulse(body: Body, signed_impulse: vec2f, anchor: vec2f) -> Body {
    var output = body;
    output.linear_velocity += signed_impulse * body.inverse_mass;
    output.angular_velocity += body.inverse_inertia * cross(anchor, signed_impulse);
    return output;
}

@compute @workgroup_size(64)
fn clear_topology(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints1.w) { return; }
    atomicStore(&topology[id.x], 0u);
}

@compute @workgroup_size(64)
fn count_edges(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
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
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    let slot_a = atomicAdd(&topology[params.uints0.w + point.ids.x], 1u);
    let slot_b = atomicAdd(&topology[params.uints0.w + point.ids.y], 1u);
    atomicStore(&topology[params.uints0.z + slot_a], point_index * 2u);
    atomicStore(&topology[params.uints0.z + slot_b], point_index * 2u + 1u);
}

@compute @workgroup_size(1)
fn color_contacts(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    var color_count = 0u;
    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        let point = point_inputs[point_index];
        let used = atomicLoad(&topology[params.uints1.y + point.ids.x]) |
                   atomicLoad(&topology[params.uints1.y + point.ids.y]);
        var color = 0u;
        while (color < 32u && (used & (1u << color)) != 0u) {
            color += 1u;
        }
        if (color == 32u) {
            atomicStore(&topology[params.uints1.x + point_index], 0xffffffffu);
            atomicStore(&topology[params.uints1.z], 1u);
        } else {
            let color_bit = 1u << color;
            atomicStore(&topology[params.uints1.x + point_index], color);
            atomicOr(&topology[params.uints1.y + point.ids.x], color_bit);
            atomicOr(&topology[params.uints1.y + point.ids.y], color_bit);
            color_count = max(color_count, color + 1u);
        }
    }
    atomicStore(&topology[params.uints1.z + 1u], color_count);
}

@compute @workgroup_size(64)
fn integrate_velocity(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x) { return; }
    var body = bodies_source[body_index];
    if (body.inverse_mass > 0.0) {
        body.linear_velocity += params.floats1.yz * params.floats0.x;
    }
    bodies_source[body_index] = body;
}

@compute @workgroup_size(64)
fn integrate_position(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x) { return; }
    var body = bodies_source[body_index];
    if (body.inverse_mass > 0.0) {
        body.position += body.linear_velocity * params.floats0.x;
        body.angle += body.angular_velocity * params.floats0.x;
    }
    bodies_source[body_index] = body;
}

@compute @workgroup_size(64)
fn prepare(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let input = point_inputs[point_index];
    let body_a = bodies_source[input.ids.x];
    let body_b = bodies_source[input.ids.y];
    let anchor_a = rotate(input.local_anchor_a, body_a.angle);
    let anchor_b = rotate(input.local_anchor_b, body_b.angle);
    let tangent = vec2f(input.normal.y, -input.normal.x);
    let initial_velocity = dot(point_velocity(body_b, anchor_b) -
                               point_velocity(body_a, anchor_a), input.normal);
    var point: PreparedPoint;
    point.ids = input.ids;
    point.normal = input.normal;
    point.tangent = tangent;
    point.anchor_a = anchor_a;
    point.anchor_b = anchor_b;
    point.data0 = vec4f(initial_velocity,
        effective_mass(body_a, body_b, anchor_a, anchor_b, input.normal),
        effective_mass(body_a, body_b, anchor_a, anchor_b, tangent),
        input.material.x);
    point.data1 = vec4f(input.material.y, input.material.z, input.cache.x, input.cache.y);
    point.data2 = vec4f(0.0);
    prepared_points[point_index] = point;
    write_trace(params.uints1.z + point_index, 0u, 0u, point,
        vec4f(anchor_a.x, anchor_a.y, anchor_b.x, anchor_b.y),
        vec4f(initial_velocity, point.data0.y, point.data0.w, point.data0.z), 8u);
}

@compute @workgroup_size(64)
fn refresh_contacts(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let input = point_inputs[point_index];
    let body_a = bodies_source[input.ids.x];
    let body_b = bodies_source[input.ids.y];
    let anchor_a = rotate(input.local_anchor_a, body_a.angle);
    let anchor_b = rotate(input.local_anchor_b, body_b.angle);
    let tangent = vec2f(input.normal.y, -input.normal.x);
    let initial_velocity = dot(point_velocity(body_b, anchor_b) -
                               point_velocity(body_a, anchor_a), input.normal);
    var point = prepared_points[point_index];
    point.normal = input.normal;
    point.tangent = tangent;
    point.anchor_a = anchor_a;
    point.anchor_b = anchor_b;
    let reset_impulses = params.uints0.w != 0u;
    let retained_initial_velocity = point.data0.x;
    point.data0 = vec4f(select(retained_initial_velocity, initial_velocity, reset_impulses),
        effective_mass(body_a, body_b, anchor_a, anchor_b, input.normal),
        effective_mass(body_a, body_b, anchor_a, anchor_b, tangent),
        dot((body_b.position + anchor_b) - (body_a.position + anchor_a), input.normal));
    if (reset_impulses) {
        point.data1.z = 0.0;
        point.data1.w = 0.0;
        point.data2 = vec4f(0.0);
    }
    prepared_points[point_index] = point;
    if (params.uints0.z != 0u) {
        write_trace(params.uints1.z + point_index, 0u, 0u, point,
            vec4f(anchor_a.x, anchor_a.y, anchor_b.x, anchor_b.y),
            vec4f(initial_velocity, point.data0.y, point.data0.w, point.data0.z), 8u);
    }
}

@compute @workgroup_size(64)
fn emit_warm(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = prepared_points[point_index];
    let impulse = point.normal * point.data1.z + point.tangent * point.data1.w;
    emit_pair(point_index, point, impulse);
    write_trace(params.uints1.z + point_index, 1u, 0u, point,
        vec4f(impulse.x, impulse.y, point.data1.z, point.data1.w), vec4f(0.0), 4u);
}

@compute @workgroup_size(64)
fn emit_normal(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    var point = prepared_points[point_index];
    let body_a = bodies_source[point.ids.x];
    let body_b = bodies_source[point.ids.y];
    let normal_velocity = relative_velocity(body_a, body_b, point, point.normal);
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
    prepared_points[point_index] = point;
    emit_pair(point_index, point, point.normal * applied);
    write_trace(params.uints1.z + point_index, params.uints1.y, params.uints1.x, point,
        vec4f(applied, impulse_scale, mass_scale, point.data1.z),
        vec4f(normal_velocity, velocity_bias, 0.0, 0.0), 6u);
}

@compute @workgroup_size(64)
fn emit_tangent(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    var point = prepared_points[point_index];
    let body_a = bodies_source[point.ids.x];
    let body_b = bodies_source[point.ids.y];
    let tangent_velocity = relative_velocity(body_a, body_b, point, point.tangent);
    let old_impulse = point.data1.w;
    let limit = point.data1.x * point.data1.z;
    point.data1.w = clamp(old_impulse - point.data0.z * tangent_velocity, -limit, limit);
    let applied = point.data1.w - old_impulse;
    prepared_points[point_index] = point;
    emit_pair(point_index, point, point.tangent * applied);
    write_trace(params.uints1.z + point_index, params.uints1.y, params.uints1.x, point,
        vec4f(applied, limit, point.data1.w, tangent_velocity), vec4f(0.0), 4u);
}

@compute @workgroup_size(64)
fn emit_restitution(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    var point = prepared_points[point_index];
    let body_a = bodies_source[point.ids.x];
    let body_b = bodies_source[point.ids.y];
    let normal_velocity = relative_velocity(body_a, body_b, point, point.normal);
    let old_impulse = point.data1.z;
    let eligible = point.data1.y > 0.0 && point.data0.x < -params.floats1.x &&
                     point.data2.x > 0.0;
    if (eligible) {
        let candidate = -point.data0.y * (normal_velocity + point.data1.y * point.data0.x);
        point.data1.z = max(old_impulse + candidate, 0.0);
    }
    let applied = point.data1.z - old_impulse;
    prepared_points[point_index] = point;
    emit_pair(point_index, point, point.normal * applied);
    write_trace(params.uints1.z + point_index, 6u, 0u, point,
        vec4f(applied, select(0.0, 1.0, eligible), point.data0.x, point.data1.z),
        vec4f(normal_velocity, 0.0, 0.0, 0.0), 5u);
}

@compute @workgroup_size(64)
fn gather(@builtin(global_invocation_id) id: vec3u) {
    let body_index = id.x;
    if (body_index >= params.uints0.x) { return; }
    var body = bodies_source[body_index];
    let start = atomicLoad(&topology[body_index * 2u]);
    let count = atomicLoad(&topology[body_index * 2u + 1u]);
    for (var index = 0u; index < count; index += 1u) {
        let contribution_index = atomicLoad(&topology[params.uints1.w + start + index]);
        let contribution = contributions[contribution_index];
        body = apply_impulse(body, contribution.impulse, contribution.anchor);
    }
    bodies_destination[body_index] = body;
}

@compute @workgroup_size(64)
fn colored_stage(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y ||
        atomicLoad(&topology[params.uints0.z + point_index]) != params.uints0.w) { return; }
    var point = prepared_points[point_index];
    var body_a = bodies_source[point.ids.x];
    var body_b = bodies_source[point.ids.y];
    let stage = params.uints1.y;
    var impulse = vec2f(0.0);
    if (stage == 1u) {
        impulse = point.normal * point.data1.z + point.tangent * point.data1.w;
        write_trace(params.uints1.z + point_index, 1u, 0u, point,
            vec4f(impulse.x, impulse.y, point.data1.z, point.data1.w), vec4f(0.0), 4u);
    } else if (stage == 2u || stage == 4u) {
        let normal_velocity = relative_velocity(body_a, body_b, point, point.normal);
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
        impulse = point.normal * applied;
        write_trace(params.uints1.z + point_index, stage, params.uints1.x, point,
            vec4f(applied, impulse_scale, mass_scale, point.data1.z),
            vec4f(normal_velocity, velocity_bias, 0.0, 0.0), 6u);
    } else if (stage == 3u || stage == 5u) {
        let tangent_velocity = relative_velocity(body_a, body_b, point, point.tangent);
        let old_impulse = point.data1.w;
        let limit = point.data1.x * point.data1.z;
        point.data1.w = clamp(old_impulse - point.data0.z * tangent_velocity, -limit, limit);
        let applied = point.data1.w - old_impulse;
        impulse = point.tangent * applied;
        write_trace(params.uints1.z + point_index, stage, params.uints1.x, point,
            vec4f(applied, limit, point.data1.w, tangent_velocity), vec4f(0.0), 4u);
    } else if (stage == 6u) {
        let normal_velocity = relative_velocity(body_a, body_b, point, point.normal);
        let old_impulse = point.data1.z;
        let eligible = point.data1.y > 0.0 && point.data0.x < -params.floats1.x &&
                         point.data2.x > 0.0;
        if (eligible) {
            let candidate = -point.data0.y * (normal_velocity + point.data1.y * point.data0.x);
            point.data1.z = max(old_impulse + candidate, 0.0);
        }
        let applied = point.data1.z - old_impulse;
        impulse = point.normal * applied;
        write_trace(params.uints1.z + point_index, 6u, 0u, point,
            vec4f(applied, select(0.0, 1.0, eligible), point.data0.x, point.data1.z),
            vec4f(normal_velocity, 0.0, 0.0, 0.0), 5u);
    }
    body_a = apply_impulse(body_a, -impulse, point.anchor_a);
    body_b = apply_impulse(body_b, impulse, point.anchor_b);
    prepared_points[point_index] = point;
    bodies_source[point.ids.x] = body_a;
    bodies_source[point.ids.y] = body_b;
}
