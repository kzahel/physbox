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
    floats2: vec4f,
    uints0: vec4u,
    uints1: vec4u,
};

struct BlockSolution {
    impulses: vec2f,
    active_set: u32,
};

struct NormalEquation {
    base: f32,
    impulse_scale: f32,
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
@group(0) @binding(9) var<storage, read_write> color_dispatches: array<atomic<u32>>;

var<workgroup> edge_radix_digits: array<u32, 256>;
var<workgroup> edge_radix_histogram: array<atomic<u32>, 16>;

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

fn point_incidence(body_index: u32) -> f32 {
    var count = atomicLoad(&topology[body_index * 2u + 1u]);
    if (params.uints1.z != 0u) {
        let start = atomicLoad(&topology[params.uints0.z + body_index * 2u]);
        let end = atomicLoad(&topology[params.uints0.z + body_index * 2u + 1u]);
        count = end - start;
    }
    return f32(max(count, 1u));
}

fn manifold_incidence(body_index: u32) -> f32 {
    return f32(max(atomicLoad(&topology[params.uints1.w + body_index]), 1u));
}

fn preparation_incidence(body_index: u32) -> f32 {
    if (params.uints1.y == 1u) { return point_incidence(body_index); }
    if (params.uints1.y == 3u) { return manifold_incidence(body_index); }
    return 1.0;
}

fn effective_mass(body_a: u32, body_b: u32, anchor_a: vec2f,
                  anchor_b: vec2f, axis: vec2f,
                  incidence_a: f32, incidence_b: f32) -> f32 {
    let property_a = properties[body_a];
    let property_b = properties[body_b];
    let cross_a = cross(anchor_a, axis);
    let cross_b = cross(anchor_b, axis);
    let inverse = incidence_a * property_a.z + incidence_b * property_b.z +
        incidence_a * property_a.w * cross_a * cross_a +
        incidence_b * property_b.w * cross_b * cross_b;
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

fn invalid_prepared_point() -> PreparedPoint {
    var output: PreparedPoint;
    output.ids = vec4u(0xffffffffu);
    output.normal = vec2f(0.0);
    output.tangent = vec2f(0.0);
    output.anchor_a = vec2f(0.0);
    output.anchor_b = vec2f(0.0);
    output.data0 = vec4f(0.0);
    output.data1 = vec4f(0.0);
    output.data2 = vec4f(0.0);
    return output;
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

@compute @workgroup_size(64)
fn count_manifolds(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    if (!valid_point(point) || point.ids.w != 0u) { return; }
    atomicAdd(&topology[params.uints0.z + point.ids.x], 1u);
    atomicAdd(&topology[params.uints0.z + point.ids.y], 1u);
}

@compute @workgroup_size(1)
fn color_contacts(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    let compact = params.uints1.w != 0u;
    let counts_base = params.uints1.y + 1u;
    let offsets_base = counts_base + params.uints1.z;
    let cursors_base = offsets_base + params.uints1.z + 1u;
    let indices_base = cursors_base + params.uints1.z;
    var color_count = 0u;
    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point)) {
            atomicStore(&topology[params.uints0.z + point_index], 0xffffffffu);
            continue;
        }
        let used = atomicLoad(&topology[params.uints0.w + point.ids.x]) |
                   atomicLoad(&topology[params.uints0.w + point.ids.y]);
        var color = 0u;
        while (color < params.uints1.z && (used & (1u << color)) != 0u) {
            color += 1u;
        }
        if (color == params.uints1.z) {
            atomicStore(&topology[params.uints0.z + point_index], 0xffffffffu);
            atomicStore(&topology[params.uints1.x], 1u);
        } else {
            let color_bit = 1u << color;
            atomicStore(&topology[params.uints0.z + point_index], color);
            atomicOr(&topology[params.uints0.w + point.ids.x], color_bit);
            atomicOr(&topology[params.uints0.w + point.ids.y], color_bit);
            if (compact) {
                atomicAdd(&topology[counts_base + color], 1u);
            }
            color_count = max(color_count, color + 1u);
        }
    }
    atomicStore(&topology[params.uints1.y], color_count);
    if (!compact) { return; }

    let overflowed = atomicLoad(&topology[params.uints1.x]) != 0u;
    var running = 0u;
    for (var color = 0u; color < params.uints1.z; color += 1u) {
        let count = atomicLoad(&topology[counts_base + color]);
        atomicStore(&topology[offsets_base + color], running);
        atomicStore(&color_dispatches[color * 3u],
                    select(count + 63u, 0u, overflowed) / 64u);
        atomicStore(&color_dispatches[color * 3u + 1u], 1u);
        atomicStore(&color_dispatches[color * 3u + 2u], 1u);
        running += count;
    }
    atomicStore(&topology[offsets_base + params.uints1.z], running);
    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        let color = atomicLoad(&topology[params.uints0.z + point_index]);
        if (color < params.uints1.z) {
            let slot = atomicAdd(&topology[cursors_base + color], 1u);
            let offset = atomicLoad(&topology[offsets_base + color]);
            atomicStore(&topology[indices_base + offset + slot], point_index);
        }
    }
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
fn build_stable_edges(@builtin(global_invocation_id) id: vec3u) {
    let scheduled_index = id.x;
    if (scheduled_index >= params.uints0.y) { return; }
    let point_index = select(
        scheduled_index,
        params.uints0.y - 1u - scheduled_index,
        params.uints1.y != 0u);
    let point = point_inputs[point_index];
    let edge_a = point_index * 2u;
    let edge_b = edge_a + 1u;
    var body_a = 0xffffffffu;
    var body_b = 0xffffffffu;
    if (valid_point(point)) {
        body_a = point.ids.x;
        body_b = point.ids.y;
    }
    atomicStore(&topology[params.uints0.z + edge_a * 2u], body_a);
    atomicStore(&topology[params.uints0.z + edge_a * 2u + 1u], edge_a);
    atomicStore(&topology[params.uints0.z + edge_b * 2u], body_b);
    atomicStore(&topology[params.uints0.z + edge_b * 2u + 1u], edge_b);
}

@compute @workgroup_size(256)
fn edge_radix_histograms(@builtin(global_invocation_id) global: vec3u,
                         @builtin(local_invocation_id) local: vec3u,
                         @builtin(workgroup_id) group_id: vec3u) {
    if (local.x < 16u) {
        atomicStore(&edge_radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    if (global.x < params.uints0.x) {
        let key = atomicLoad(&topology[params.uints0.z + global.x * 2u]);
        let digit = (key >> params.uints1.y) & 15u;
        atomicAdd(&edge_radix_histogram[digit], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        atomicStore(
            &topology[params.uints1.x + group_id.x * 16u + local.x],
            atomicLoad(&edge_radix_histogram[local.x]));
    }
}

@compute @workgroup_size(1)
fn edge_radix_prefix(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    var running = 0u;
    for (var digit = 0u; digit < 16u; digit++) {
        for (var block = 0u; block < params.uints0.y; block++) {
            let index = params.uints1.x + block * 16u + digit;
            let count = atomicLoad(&topology[index]);
            atomicStore(&topology[index], running);
            running += count;
        }
    }
}

@compute @workgroup_size(256)
fn edge_radix_scatter(@builtin(global_invocation_id) global: vec3u,
                      @builtin(local_invocation_id) local: vec3u,
                      @builtin(workgroup_id) group_id: vec3u) {
    var digit = 16u;
    if (global.x < params.uints0.x) {
        let key = atomicLoad(&topology[params.uints0.z + global.x * 2u]);
        digit = (key >> params.uints1.y) & 15u;
    }
    edge_radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= params.uints0.x) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, edge_radix_digits[lane] == digit);
    }
    let output = atomicLoad(
        &topology[params.uints1.x + group_id.x * 16u + digit]) + local_rank;
    let key = atomicLoad(&topology[params.uints0.z + global.x * 2u]);
    let contribution = atomicLoad(&topology[params.uints0.z + global.x * 2u + 1u]);
    atomicStore(&topology[params.uints0.w + output * 2u], key);
    atomicStore(&topology[params.uints0.w + output * 2u + 1u], contribution);
}

@compute @workgroup_size(64)
fn build_edge_ranges(@builtin(global_invocation_id) id: vec3u) {
    let edge_index = id.x;
    if (edge_index >= params.uints0.x) { return; }
    let body = atomicLoad(&topology[params.uints0.z + edge_index * 2u]);
    if (body >= params.uints0.y) { return; }
    if (edge_index == 0u ||
        atomicLoad(&topology[params.uints0.z + (edge_index - 1u) * 2u]) != body) {
        atomicStore(&topology[params.uints0.w + body * 2u], edge_index);
    }
    if (edge_index + 1u == params.uints0.x ||
        atomicLoad(&topology[params.uints0.z + (edge_index + 1u) * 2u]) != body) {
        atomicStore(&topology[params.uints0.w + body * 2u + 1u], edge_index + 1u);
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
    let incidence_a = preparation_incidence(input.ids.x);
    let incidence_b = preparation_incidence(input.ids.y);
    var point = previous;
    point.ids = input.ids;
    point.normal = input.normal;
    point.tangent = tangent;
    point.anchor_a = anchor_a;
    point.anchor_b = anchor_b;
    point.data0 = vec4f(select(retained_initial_velocity, initial_velocity, reset),
        effective_mass(input.ids.x, input.ids.y, anchor_a, anchor_b, input.normal,
            incidence_a, incidence_b),
        effective_mass(input.ids.x, input.ids.y, anchor_a, anchor_b, tangent,
            select(1.0, incidence_a, params.uints1.y == 1u),
            select(1.0, incidence_b, params.uints1.y == 1u)),
        dot((pose_b.xy + anchor_b) - (pose_a.xy + anchor_a), input.normal));
    if (reset) {
        point.data1 = vec4f(input.material.y, input.material.z, input.cache);
        point.data2 = vec4f(0.0, incidence_a, incidence_b, f32(params.uints1.y));
    }
    return point;
}

@compute @workgroup_size(64)
fn prepare(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) {
        prepared_points[id.x] = invalid_prepared_point();
        return;
    }
    var empty: PreparedPoint;
    var point = refreshed_point(point_inputs[id.x], empty, true);
    if (params.uints1.x == 0u) {
        point.data1.z = 0.0;
        point.data1.w = 0.0;
    }
    prepared_points[id.x] = point;
}

@compute @workgroup_size(64)
fn refresh_contacts(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    if (!valid_point(point_inputs[id.x])) {
        prepared_points[id.x] = invalid_prepared_point();
        return;
    }
    var point = refreshed_point(
        point_inputs[id.x], prepared_points[id.x], params.uints0.w != 0u);
    if (params.uints0.w != 0u && params.uints1.x == 0u) {
        point.data1.z = 0.0;
        point.data1.w = 0.0;
    }
    prepared_points[id.x] = point;
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
        if (params.floats2.x > 0.0) {
            velocity_bias = max(velocity_bias, -params.floats2.x);
        }
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

fn normal_equation(point: PreparedPoint) -> NormalEquation {
    let normal_velocity = relative_velocity(point, point.normal);
    var velocity_bias = 0.0;
    var mass_scale = 1.0;
    var output: NormalEquation;
    output.impulse_scale = 0.0;
    if (point.data0.w > 0.0) {
        velocity_bias = point.data0.w / params.floats0.x;
    } else if (params.uints1.w != 0u) {
        velocity_bias = params.floats0.y * point.data0.w / params.floats0.x;
        if (params.floats2.x > 0.0) {
            velocity_bias = max(velocity_bias, -params.floats2.x);
        }
        mass_scale = params.floats0.z;
        output.impulse_scale = params.floats0.w;
    }
    output.base = mass_scale * normal_velocity + velocity_bias;
    return output;
}

fn block_matrix_entry(left: PreparedPoint, right: PreparedPoint,
                      incidence_a: f32, incidence_b: f32) -> f32 {
    let property_a = properties[left.ids.x];
    let property_b = properties[left.ids.y];
    return incidence_a * property_a.z + incidence_b * property_b.z +
        incidence_a * property_a.w * cross(left.anchor_a, left.normal) *
            cross(right.anchor_a, right.normal) +
        incidence_b * property_b.w * cross(left.anchor_b, left.normal) *
            cross(right.anchor_b, right.normal);
}

fn solve_normal_block(matrix: vec4f, base: vec2f, old: vec2f,
                      impulse_scale: vec2f,
                      point_count: u32) -> BlockSolution {
    var output: BlockSolution;
    output.impulses = old;
    output.active_set = 5u;
    if (point_count == 1u) {
        let mass = select(0.0, 1.0 / matrix.x, matrix.x > 0.0);
        output.impulses = vec2f(max(old.x - mass * base.x -
            impulse_scale.x * old.x, 0.0), 0.0);
        output.active_set = 4u;
        return output;
    }

    let scaled_old = impulse_scale * old;
    let rhs = base + vec2f(
        matrix.x * scaled_old.x + matrix.y * scaled_old.y,
        matrix.z * scaled_old.x + matrix.w * scaled_old.y);
    let determinant = matrix.x * matrix.w - matrix.y * matrix.z;
    if (determinant > 0.0) {
        let delta = vec2f(
            (-matrix.w * rhs.x + matrix.y * rhs.y) / determinant,
            (matrix.z * rhs.x - matrix.x * rhs.y) / determinant);
        let candidate = old + delta;
        if (all(candidate >= vec2f(0.0))) {
            output.impulses = candidate;
            output.active_set = 0u;
            return output;
        }
    }

    if (matrix.x > 0.0) {
        let delta1 = -old.y;
        let delta0 = -(rhs.x + matrix.y * delta1) / matrix.x;
        let candidate0 = old.x + delta0;
        let inactive_velocity = rhs.y + matrix.z * delta0 + matrix.w * delta1;
        if (candidate0 >= 0.0 && inactive_velocity >= 0.0) {
            output.impulses = vec2f(candidate0, 0.0);
            output.active_set = 1u;
            return output;
        }
    }

    if (matrix.w > 0.0) {
        let delta0 = -old.x;
        let delta1 = -(rhs.y + matrix.z * delta0) / matrix.w;
        let candidate1 = old.y + delta1;
        let inactive_velocity = rhs.x + matrix.x * delta0 + matrix.y * delta1;
        if (candidate1 >= 0.0 && inactive_velocity >= 0.0) {
            output.impulses = vec2f(0.0, candidate1);
            output.active_set = 2u;
            return output;
        }
    }

    let delta = -old;
    let velocity = rhs + vec2f(
        matrix.x * delta.x + matrix.y * delta.y,
        matrix.z * delta.x + matrix.w * delta.y);
    if (all(velocity >= vec2f(0.0))) {
        output.impulses = vec2f(0.0);
        output.active_set = 3u;
    }
    return output;
}

fn store_block_trace(point_index: u32, matrix: vec4f, active_set: u32) {
    contributions[point_index * 2u].padding = bitcast<u32>(matrix.x);
    contributions[point_index * 2u + 1u].padding = bitcast<u32>(matrix.y);
    if (point_index + 1u < params.uints0.y) {
        contributions[(point_index + 1u) * 2u].padding = bitcast<u32>(matrix.w);
        contributions[(point_index + 1u) * 2u + 1u].padding = active_set;
    }
}

@compute @workgroup_size(64)
fn emit_normal_block(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= params.uints0.y) { return; }
    let input0 = point_inputs[id.x];
    if (!valid_point(input0) || input0.ids.w != 0u) { return; }
    var point0 = prepared_points[id.x];
    var point1 = point0;
    var point_count = 1u;
    if (id.x + 1u < params.uints0.y) {
        let input1 = point_inputs[id.x + 1u];
        if (valid_point(input1) && input1.ids.x == input0.ids.x &&
                input1.ids.y == input0.ids.y && input1.ids.z == input0.ids.z &&
                input1.ids.w == 1u) {
            point1 = prepared_points[id.x + 1u];
            point_count = 2u;
        }
    }

    let block_mass_split = u32(params.floats2.y) == 3u;
    var incidence_a = 1.0;
    var incidence_b = 1.0;
    if (block_mass_split) {
        incidence_a = f32(max(atomicLoad(
            &topology[params.uints0.w + point0.ids.x]), 1u));
        incidence_b = f32(max(atomicLoad(
            &topology[params.uints0.w + point0.ids.y]), 1u));
    }
    let k11 = block_matrix_entry(point0, point0, incidence_a, incidence_b);
    var matrix = vec4f(k11, 0.0, 0.0, 0.0);
    if (point_count == 2u) {
        let k12 = block_matrix_entry(point0, point1, incidence_a, incidence_b);
        let k21 = block_matrix_entry(point1, point0, incidence_a, incidence_b);
        let k22 = block_matrix_entry(point1, point1, incidence_a, incidence_b);
        matrix = vec4f(k11, k12, k21, k22);
    }
    let old = vec2f(point0.data1.z, select(0.0, point1.data1.z, point_count == 2u));
    let equation0 = normal_equation(point0);
    let equation1 = normal_equation(point1);
    let base = vec2f(equation0.base,
        select(0.0, equation1.base, point_count == 2u));
    let impulse_scale = vec2f(equation0.impulse_scale,
        select(0.0, equation1.impulse_scale, point_count == 2u));
    let solution = solve_normal_block(matrix, base, old, impulse_scale, point_count);
    store_block_trace(id.x, matrix, solution.active_set);

    let applied0 = solution.impulses.x - old.x;
    point0.data1.z = solution.impulses.x;
    point0.data2.x += max(applied0, 0.0);
    point0.data2.w = f32(solution.active_set);
    prepared_points[id.x] = point0;
    emit_pair(id.x, point0, point0.normal * applied0);
    if (point_count == 2u) {
        let applied1 = solution.impulses.y - old.y;
        point1.data1.z = solution.impulses.y;
        point1.data2.x += max(applied1, 0.0);
        point1.data2.w = f32(solution.active_set);
        prepared_points[id.x + 1u] = point1;
        emit_pair(id.x + 1u, point1, point1.normal * applied1);
    }
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
    var start = atomicLoad(&topology[body_index * 2u]);
    var end = start + atomicLoad(&topology[body_index * 2u + 1u]);
    if (params.uints0.z != 0u) {
        start = atomicLoad(&topology[params.uints0.w + body_index * 2u]);
        end = atomicLoad(&topology[params.uints0.w + body_index * 2u + 1u]);
    }
    for (var index = start; index < end; index += 1u) {
        var contribution_index = atomicLoad(&topology[params.uints1.w + index]);
        if (params.uints0.z != 0u) {
            contribution_index = atomicLoad(&topology[params.uints1.z + index * 2u + 1u]);
        }
        let contribution = contributions[contribution_index];
        motion = apply_impulse(body_index, motion, contribution.impulse, contribution.anchor);
    }
    motions_destination[body_index] = motion;
}

@compute @workgroup_size(64)
fn colored_stage(@builtin(global_invocation_id) id: vec3u) {
    var point_index = id.x;
    if (params.uints0.x != 0u) {
        let count = atomicLoad(&topology[params.uints0.x + params.uints0.w]);
        if (id.x >= count) { return; }
        let offset = atomicLoad(&topology[params.uints1.z + params.uints0.w]);
        point_index = atomicLoad(&topology[params.uints0.z + offset + id.x]);
    } else if (point_index >= params.uints0.y ||
               atomicLoad(&topology[params.uints0.z + point_index]) != params.uints0.w) {
        return;
    }
    if (!valid_point(point_inputs[point_index])) { return; }
    var point = prepared_points[point_index];
    var motion_a = motions_source[point.ids.x];
    var motion_b = motions_source[point.ids.y];
    let stage = params.uints1.y;
    var impulse = vec2f(0.0);
    if (stage == 1u) {
        impulse = point.normal * point.data1.z + point.tangent * point.data1.w;
    } else if (stage == 2u || stage == 4u) {
        let normal_velocity = relative_velocity(point, point.normal);
        var velocity_bias = 0.0;
        var mass_scale = 1.0;
        var impulse_scale = 0.0;
        if (point.data0.w > 0.0) {
            velocity_bias = point.data0.w / params.floats0.x;
        } else if (params.uints1.w != 0u) {
            velocity_bias = params.floats0.y * point.data0.w / params.floats0.x;
            if (params.floats2.x > 0.0) {
                velocity_bias = max(velocity_bias, -params.floats2.x);
            }
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
    } else if (stage == 3u || stage == 5u) {
        let tangent_velocity = relative_velocity(point, point.tangent);
        let old_impulse = point.data1.w;
        let limit = point.data1.x * point.data1.z;
        point.data1.w = clamp(old_impulse - point.data0.z * tangent_velocity, -limit, limit);
        impulse = point.tangent * (point.data1.w - old_impulse);
    } else if (stage == 6u) {
        let normal_velocity = relative_velocity(point, point.normal);
        let old_impulse = point.data1.z;
        let eligible = point.data1.y > 0.0 && point.data0.x < -params.floats1.x &&
                         point.data2.x > 0.0;
        if (eligible) {
            let candidate = -point.data0.y * (normal_velocity + point.data1.y * point.data0.x);
            point.data1.z = max(old_impulse + candidate, 0.0);
        }
        impulse = point.normal * (point.data1.z - old_impulse);
    }
    motion_a = apply_impulse(point.ids.x, motion_a, -impulse, point.anchor_a);
    motion_b = apply_impulse(point.ids.y, motion_b, impulse, point.anchor_b);
    emit_pair(point_index, point, impulse);
    prepared_points[point_index] = point;
    motions_source[point.ids.x] = motion_a;
    motions_source[point.ids.y] = motion_b;
}
