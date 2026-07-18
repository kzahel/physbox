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
    floats3: vec4f,
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
var<workgroup> color_block_histogram: array<atomic<u32>, 32>;
var<workgroup> color_block_assignments: array<u32, 64>;
var<workgroup> color_range_scan: array<u32, 32>;

fn rotate(vector: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * vector.x - s * vector.y, s * vector.x + c * vector.y);
}

fn cross(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

fn point_velocity_from_motion(motion: vec4f, anchor: vec2f) -> vec2f {
    return motion.xy + vec2f(-motion.z * anchor.y, motion.z * anchor.x);
}

fn point_velocity(body_index: u32, anchor: vec2f) -> vec2f {
    return point_velocity_from_motion(motions_source[body_index], anchor);
}

fn relative_velocity(point: PreparedPoint, axis: vec2f) -> f32 {
    return dot(point_velocity(point.ids.y, point.anchor_b) -
               point_velocity(point.ids.x, point.anchor_a), axis);
}

fn relative_velocity_from_motions(point: PreparedPoint, axis: vec2f,
                                  motion_a: vec4f, motion_b: vec4f) -> f32 {
    return dot(point_velocity_from_motion(motion_b, point.anchor_b) -
               point_velocity_from_motion(motion_a, point.anchor_a), axis);
}

fn contact_coefficients(point: PreparedPoint) -> vec3f {
    let static_contact = properties[point.ids.x].z == 0.0 ||
                           properties[point.ids.y].z == 0.0;
    return select(params.floats0.yzw, params.floats3.xyz,
                  params.floats3.w != 0.0 && static_contact);
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
    let compact = (params.uints1.w & 1u) != 0u;
    let manifold_units = (params.uints1.w & 2u) != 0u;
    let parallel_compaction = (params.uints1.w & 4u) != 0u;
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
        if (manifold_units && point.ids.w != 0u) {
            var paired = false;
            if (point_index > 0u) {
                let previous = point_inputs[point_index - 1u];
                paired = valid_point(previous) && previous.ids.x == point.ids.x &&
                    previous.ids.y == point.ids.y && previous.ids.z == point.ids.z &&
                    previous.ids.w == 0u && point.ids.w == 1u;
            }
            if (!paired) {
                atomicStore(&topology[params.uints0.z + point_index], 0xffffffffu);
                atomicStore(&topology[params.uints1.x], 1u);
            }
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
            if (manifold_units && point_index + 1u < params.uints0.y) {
                let next = point_inputs[point_index + 1u];
                if (valid_point(next) && next.ids.x == point.ids.x &&
                        next.ids.y == point.ids.y && next.ids.z == point.ids.z &&
                        next.ids.w == 1u) {
                    atomicStore(&topology[params.uints0.z + point_index + 1u], color);
                }
            }
            atomicOr(&topology[params.uints0.w + point.ids.x], color_bit);
            atomicOr(&topology[params.uints0.w + point.ids.y], color_bit);
            if (compact && !parallel_compaction) {
                atomicAdd(&topology[counts_base + color], 1u);
            }
            color_count = max(color_count, color + 1u);
        }
    }
    atomicStore(&topology[params.uints1.y], color_count);
}

const PARALLEL_COLOR_UNCOLORED: u32 = 0xfffffffeu;
const PARALLEL_COLOR_INVALID: u32 = 0xffffffffu;
const PARALLEL_COLOR_PROPOSAL: u32 = 0x80000000u;

fn parallel_color_seed(point: PointInput) -> u32 {
    var value = point.ids.x * 0x9e3779b9u;
    value ^= point.ids.y * 0x85ebca6bu;
    value ^= bitcast<u32>(point.padding1.x) * 0xc2b2ae35u;
    value ^= bitcast<u32>(point.padding1.y) * 0x27d4eb2fu;
    value ^= value >> 16u;
    value *= 0x7feb352du;
    value ^= value >> 15u;
    return value;
}

fn parallel_contact_is_static(point: PointInput) -> bool {
    let dynamic_a = properties[point.ids.x].z > 0.0 || properties[point.ids.x].w > 0.0;
    let dynamic_b = properties[point.ids.y].z > 0.0 || properties[point.ids.y].w > 0.0;
    return !dynamic_a || !dynamic_b;
}

@compute @workgroup_size(64)
fn initialize_parallel_colors(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    let manifold_units = (params.uints1.w & 2u) != 0u;
    if (!valid_point(point)) {
        atomicStore(&topology[params.uints0.z + point_index], PARALLEL_COLOR_INVALID);
        return;
    }
    atomicStore(&topology[params.uints0.z + point_index], PARALLEL_COLOR_UNCOLORED);
    if (manifold_units && point.ids.w != 0u) {
        var paired = false;
        if (point_index > 0u && point.ids.w == 1u) {
            let previous = point_inputs[point_index - 1u];
            paired = valid_point(previous) && previous.ids.x == point.ids.x &&
                previous.ids.y == point.ids.y && previous.ids.z == point.ids.z &&
                previous.ids.w == 0u;
        }
        if (!paired) {
            atomicStore(&topology[params.uints1.x], 1u);
        }
    }
}

@compute @workgroup_size(64)
fn clear_parallel_color_winners(@builtin(global_invocation_id) id: vec3u) {
    let winner_count = params.uints0.x * params.uints1.z;
    if (id.x >= winner_count) { return; }
    let winners_base = bitcast<u32>(params.floats0.x);
    atomicStore(&topology[winners_base + id.x], 0xffffffffu);
}

@compute @workgroup_size(64)
fn propose_parallel_colors(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    let manifold_units = (params.uints1.w & 2u) != 0u;
    if (!valid_point(point) || (manifold_units && point.ids.w != 0u)) { return; }
    let assignment_index = params.uints0.z + point_index;
    if (atomicLoad(&topology[assignment_index]) != PARALLEL_COLOR_UNCOLORED) { return; }
    let used = atomicLoad(&topology[params.uints0.w + point.ids.x]) |
        atomicLoad(&topology[params.uints0.w + point.ids.y]);
    let seed = parallel_color_seed(point);
    let static_late = (params.uints1.w & 8u) != 0u;
    let static_contact = static_late && parallel_contact_is_static(point);
    let dynamic_candidate_count = select(params.uints1.z, 28u, static_late);
    let candidate_count = select(dynamic_candidate_count, 4u, static_contact);
    let first_color = select(0u, 28u, static_contact);
    let start = seed & 3u;
    var selected = PARALLEL_COLOR_INVALID;
    for (var offset = 0u; offset < candidate_count; offset += 1u) {
        let candidate = (start + offset) % candidate_count;
        let color = select(candidate, 31u - candidate, static_contact);
        if ((used & (1u << color)) == 0u) {
            selected = color;
            break;
        }
    }
    if (selected < first_color || selected >= first_color + candidate_count) {
        atomicStore(&topology[params.uints1.x], 1u);
        return;
    }
    atomicStore(&topology[assignment_index], PARALLEL_COLOR_PROPOSAL | selected);
    let winners_base = bitcast<u32>(params.floats0.x);
    atomicMin(&topology[winners_base + point.ids.x * params.uints1.z + selected], point_index);
    atomicMin(&topology[winners_base + point.ids.y * params.uints1.z + selected], point_index);
}

@compute @workgroup_size(64)
fn resolve_parallel_colors(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    let manifold_units = (params.uints1.w & 2u) != 0u;
    if (!valid_point(point) || (manifold_units && point.ids.w != 0u)) { return; }
    let assignment_index = params.uints0.z + point_index;
    let proposal = atomicLoad(&topology[assignment_index]);
    if ((proposal & PARALLEL_COLOR_PROPOSAL) == 0u ||
            proposal == PARALLEL_COLOR_UNCOLORED ||
            proposal == PARALLEL_COLOR_INVALID) {
        return;
    }
    let color = proposal & 31u;
    let winners_base = bitcast<u32>(params.floats0.x);
    let winner_a = atomicLoad(
        &topology[winners_base + point.ids.x * params.uints1.z + color]);
    let winner_b = atomicLoad(
        &topology[winners_base + point.ids.y * params.uints1.z + color]);
    let stats_base = bitcast<u32>(params.floats0.y);
    if (winner_a == point_index && winner_b == point_index) {
        atomicStore(&topology[assignment_index], color);
        if (manifold_units && point_index + 1u < params.uints0.y) {
            let next = point_inputs[point_index + 1u];
            if (valid_point(next) && next.ids.x == point.ids.x &&
                    next.ids.y == point.ids.y && next.ids.z == point.ids.z &&
                    next.ids.w == 1u) {
                atomicStore(&topology[assignment_index + 1u], color);
            }
        }
        let bit = 1u << color;
        atomicOr(&topology[params.uints0.w + point.ids.x], bit);
        atomicOr(&topology[params.uints0.w + point.ids.y], bit);
        atomicMax(&topology[params.uints1.y], color + 1u);
        atomicMax(&topology[stats_base + 2u], bitcast<u32>(params.floats0.z) + 1u);
    } else {
        atomicStore(&topology[assignment_index], PARALLEL_COLOR_UNCOLORED);
        atomicAdd(&topology[stats_base], 1u);
    }
}

@compute @workgroup_size(64)
fn finalize_parallel_colors(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.y) { return; }
    let point = point_inputs[point_index];
    let manifold_units = (params.uints1.w & 2u) != 0u;
    if (!valid_point(point) || (manifold_units && point.ids.w != 0u)) { return; }
    let color = atomicLoad(&topology[params.uints0.z + point_index]);
    if (color >= params.uints1.z) {
        atomicStore(&topology[params.uints1.x], 1u);
        atomicAdd(&topology[bitcast<u32>(params.floats0.y) + 1u], 1u);
    }
}

fn incremental_identity_hash(point: PointInput, mask: u32) -> u32 {
    var value = point.ids.x * 0x9e3779b9u;
    value ^= point.ids.y * 0x85ebca6bu;
    value ^= bitcast<u32>(point.padding1.x) * 0xc2b2ae35u;
    value ^= bitcast<u32>(point.padding1.y) * 0x27d4eb2fu;
    value ^= value >> 16u;
    return value & mask;
}

fn incremental_identity_matches(base: u32, point: PointInput) -> bool {
    return atomicLoad(&topology[base]) == 0xc0110a1u &&
        atomicLoad(&topology[base + 1u]) == point.ids.x &&
        atomicLoad(&topology[base + 2u]) == point.ids.y &&
        atomicLoad(&topology[base + 3u]) == bitcast<u32>(point.padding1.x) &&
        atomicLoad(&topology[base + 4u]) == bitcast<u32>(point.padding1.y);
}

fn find_incremental_identity(table_base: u32, table_capacity: u32,
                             point: PointInput) -> vec2u {
    let mask = table_capacity - 1u;
    var slot = incremental_identity_hash(point, mask);
    for (var probe = 0u; probe < table_capacity; probe += 1u) {
        let base = table_base + slot * 6u;
        let marker = atomicLoad(&topology[base]);
        if (marker == 0u) { return vec2u(0u); }
        if (incremental_identity_matches(base, point)) {
            return vec2u(1u, atomicLoad(&topology[base + 5u]));
        }
        slot = (slot + 1u) & mask;
    }
    return vec2u(0u);
}

fn insert_incremental_identity(table_base: u32, table_capacity: u32,
                               point: PointInput, color: u32) -> bool {
    let mask = table_capacity - 1u;
    var slot = incremental_identity_hash(point, mask);
    for (var probe = 0u; probe < table_capacity; probe += 1u) {
        let base = table_base + slot * 6u;
        if (atomicLoad(&topology[base]) == 0u) {
            atomicStore(&topology[base], 0xc0110a1u);
            atomicStore(&topology[base + 1u], point.ids.x);
            atomicStore(&topology[base + 2u], point.ids.y);
            atomicStore(&topology[base + 3u], bitcast<u32>(point.padding1.x));
            atomicStore(&topology[base + 4u], bitcast<u32>(point.padding1.y));
            atomicStore(&topology[base + 5u], color);
            return true;
        }
        if (incremental_identity_matches(base, point)) { return false; }
        slot = (slot + 1u) & mask;
    }
    return false;
}

fn paired_manifold_representative(point_index: u32, point: PointInput) -> bool {
    if (point.ids.w == 0u) { return true; }
    if (point_index == 0u || point.ids.w != 1u) { return false; }
    let previous = point_inputs[point_index - 1u];
    return valid_point(previous) && previous.ids.x == point.ids.x &&
        previous.ids.y == point.ids.y && previous.ids.z == point.ids.z &&
        previous.ids.w == 0u;
}

fn store_incremental_color(point_index: u32, point: PointInput, color: u32,
                           color_base: u32, manifold_units: bool) {
    atomicStore(&topology[color_base + point_index], color);
    if (manifold_units && point_index + 1u < params.uints0.y) {
        let next = point_inputs[point_index + 1u];
        if (valid_point(next) && next.ids.x == point.ids.x &&
                next.ids.y == point.ids.y && next.ids.z == point.ids.z &&
                next.ids.w == 1u) {
            atomicStore(&topology[color_base + point_index + 1u], color);
        }
    }
}

@compute @workgroup_size(1)
fn color_contacts_incremental(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    let manifold_units = (params.uints1.w & 2u) != 0u;
    let identity_base = params.uints0.x;
    let color_base = params.uints0.z;
    let mask_base = params.uints0.w;
    let status_index = params.uints1.x;
    let color_count_index = params.uints1.y;
    let counts_base = color_count_index + 1u;
    let offsets_base = counts_base + params.uints1.z;
    let cursors_base = offsets_base + params.uints1.z + 1u;
    var identity_capacity = 1u;
    while (identity_capacity < params.uints0.y * 2u) {
        identity_capacity <<= 1u;
    }
    let maintenance_base = identity_base + identity_capacity * 12u;
    let state_base = maintenance_base + 4u;
    let previous_parity = atomicLoad(&topology[state_base]) & 1u;
    let next_parity = 1u - previous_parity;
    let previous_table = identity_base + previous_parity * identity_capacity * 6u;
    let next_table = identity_base + next_parity * identity_capacity * 6u;
    let previous_count = atomicLoad(&topology[state_base + 1u]);
    let body_count = status_index - mask_base;

    atomicStore(&topology[status_index], 0u);
    atomicStore(&topology[color_count_index], 0u);
    for (var body = 0u; body < body_count; body += 1u) {
        atomicStore(&topology[mask_base + body], 0u);
    }
    for (var color = 0u; color < params.uints1.z; color += 1u) {
        atomicStore(&topology[counts_base + color], 0u);
        atomicStore(&topology[cursors_base + color], 0u);
    }
    for (var count = 0u; count < 4u; count += 1u) {
        atomicStore(&topology[maintenance_base + count], 0u);
    }
    for (var slot = 0u; slot < identity_capacity; slot += 1u) {
        atomicStore(&topology[next_table + slot * 6u], 0u);
    }
    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        atomicStore(&topology[color_base + point_index], 0xffffffffu);
    }

    var color_count = 0u;
    var inserted_count = 0u;
    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point)) { continue; }
        if (manifold_units && point.ids.w != 0u) {
            if (!paired_manifold_representative(point_index, point)) {
                atomicStore(&topology[status_index], 1u);
            }
            continue;
        }
        let previous = find_incremental_identity(previous_table, identity_capacity, point);
        if (previous.x != 0u && previous.y < params.uints1.z) {
            let used = atomicLoad(&topology[mask_base + point.ids.x]) |
                       atomicLoad(&topology[mask_base + point.ids.y]);
            let bit = 1u << previous.y;
            if ((used & bit) == 0u) {
                atomicOr(&topology[mask_base + point.ids.x], bit);
                atomicOr(&topology[mask_base + point.ids.y], bit);
                atomicAdd(&topology[counts_base + previous.y], 1u);
                atomicAdd(&topology[maintenance_base], 1u);
                store_incremental_color(
                    point_index, point, previous.y, color_base, manifold_units);
                if (!insert_incremental_identity(
                        next_table, identity_capacity, point, previous.y)) {
                    atomicStore(&topology[status_index], 1u);
                } else {
                    inserted_count += 1u;
                }
                color_count = max(color_count, previous.y + 1u);
                continue;
            }
        }
    }

    for (var point_index = 0u; point_index < params.uints0.y; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point) || (manifold_units && point.ids.w != 0u) ||
                atomicLoad(&topology[color_base + point_index]) != 0xffffffffu) {
            continue;
        }
        let previous = find_incremental_identity(previous_table, identity_capacity, point);
        let used = atomicLoad(&topology[mask_base + point.ids.x]) |
                   atomicLoad(&topology[mask_base + point.ids.y]);
        var color = 0u;
        while (color < params.uints1.z && (used & (1u << color)) != 0u) {
            color += 1u;
        }
        if (color == params.uints1.z) {
            atomicStore(&topology[status_index], 1u);
        } else {
            let bit = 1u << color;
            atomicOr(&topology[mask_base + point.ids.x], bit);
            atomicOr(&topology[mask_base + point.ids.y], bit);
            atomicAdd(&topology[counts_base + color], 1u);
            if (previous.x == 0u) {
                atomicAdd(&topology[maintenance_base + 1u], 1u);
            } else {
                atomicAdd(&topology[maintenance_base + 2u], 1u);
            }
            store_incremental_color(point_index, point, color, color_base, manifold_units);
            if (!insert_incremental_identity(next_table, identity_capacity, point, color)) {
                atomicStore(&topology[status_index], 1u);
            } else {
                inserted_count += 1u;
            }
            color_count = max(color_count, color + 1u);
        }
    }
    let matched_count = atomicLoad(&topology[maintenance_base]) +
        atomicLoad(&topology[maintenance_base + 2u]);
    var removed_count = 0u;
    if (previous_count >= matched_count) {
        removed_count = previous_count - matched_count;
    }
    atomicStore(&topology[maintenance_base + 3u], removed_count);
    atomicStore(&topology[state_base], next_parity);
    atomicStore(&topology[state_base + 1u], inserted_count);
    atomicStore(&topology[color_count_index], color_count);
}

@compute @workgroup_size(1)
fn compact_colors(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    let manifold_units = (params.uints1.w & 2u) != 0u;
    let counts_base = params.uints1.y + 1u;
    let offsets_base = counts_base + params.uints1.z;
    let cursors_base = offsets_base + params.uints1.z + 1u;
    let indices_base = cursors_base + params.uints1.z;
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
        if (manifold_units && point_inputs[point_index].ids.w != 0u) { continue; }
        let color = atomicLoad(&topology[params.uints0.z + point_index]);
        if (color < params.uints1.z) {
            let slot = atomicAdd(&topology[cursors_base + color], 1u);
            let offset = atomicLoad(&topology[offsets_base + color]);
            atomicStore(&topology[indices_base + offset + slot], point_index);
        }
    }
}

@compute @workgroup_size(64)
fn count_color_blocks(@builtin(global_invocation_id) global: vec3u,
                      @builtin(local_invocation_id) local: vec3u,
                      @builtin(workgroup_id) group: vec3u) {
    let lane = local.x;
    if (lane < 32u) {
        atomicStore(&color_block_histogram[lane], 0u);
    }
    var color = 0xffffffffu;
    let point_index = global.x;
    if (point_index < params.uints0.x) {
        let point = point_inputs[point_index];
        let manifold_units = bitcast<u32>(params.floats0.w) != 0u;
        if (valid_point(point) && (!manifold_units || point.ids.w == 0u)) {
            let assigned = atomicLoad(&topology[params.uints0.y + point_index]);
            if (assigned < 32u) {
                color = assigned;
            }
        }
    }
    color_block_assignments[lane] = color;
    workgroupBarrier();
    if (color < 32u) {
        atomicAdd(&color_block_histogram[color], 1u);
    }
    workgroupBarrier();
    if (lane < 32u) {
        atomicStore(&topology[params.uints0.z + group.x * 32u + lane],
                    atomicLoad(&color_block_histogram[lane]));
    }
    if (color < 32u) {
        var local_rank = 0u;
        for (var previous = 0u; previous < lane; previous += 1u) {
            if (color_block_assignments[previous] == color) {
                local_rank += 1u;
            }
        }
        atomicStore(&topology[params.uints1.x + point_index], local_rank);
    }
}

@compute @workgroup_size(32)
fn prefix_color_blocks(@builtin(global_invocation_id) id: vec3u) {
    let color = id.x;
    var running = 0u;
    let block_count = bitcast<u32>(params.floats0.z);
    for (var block = 0u; block < block_count; block += 1u) {
        let index = block * 32u + color;
        atomicStore(&topology[params.uints0.w + index], running);
        running += atomicLoad(&topology[params.uints0.z + index]);
    }
    atomicStore(&topology[params.uints1.y + color], running);
    atomicStore(&topology[params.uints1.w + color], running);
}

@compute @workgroup_size(32)
fn prefix_color_ranges(@builtin(local_invocation_id) local: vec3u) {
    let color = local.x;
    let count = atomicLoad(&topology[params.uints1.y + color]);
    color_range_scan[color] = count;
    workgroupBarrier();
    var stride = 1u;
    loop {
        if (stride >= 32u) { break; }
        var addition = 0u;
        if (color >= stride) {
            addition = color_range_scan[color - stride];
        }
        workgroupBarrier();
        color_range_scan[color] += addition;
        workgroupBarrier();
        stride <<= 1u;
    }
    let inclusive = color_range_scan[color];
    let exclusive = inclusive - count;
    let total = color_range_scan[31u];
    let status_index = bitcast<u32>(params.floats0.y);
    let overflowed = atomicLoad(&topology[status_index]) != 0u ||
        total > params.uints0.x;
    if (total > params.uints0.x) {
        atomicStore(&topology[status_index], 1u);
    }
    atomicStore(&topology[params.uints1.z + color], exclusive);
    atomicStore(&color_dispatches[color * 3u],
                select(count + 63u, 0u, overflowed) / 64u);
    atomicStore(&color_dispatches[color * 3u + 1u], 1u);
    atomicStore(&color_dispatches[color * 3u + 2u], 1u);
    if (color == 31u) {
        atomicStore(&topology[params.uints1.z + 32u], total);
    }
}

@compute @workgroup_size(64)
fn scatter_color_blocks(@builtin(global_invocation_id) id: vec3u) {
    let point_index = id.x;
    if (point_index >= params.uints0.x) { return; }
    let point = point_inputs[point_index];
    let manifold_units = bitcast<u32>(params.floats0.w) != 0u;
    if (!valid_point(point) || (manifold_units && point.ids.w != 0u)) { return; }
    let color = atomicLoad(&topology[params.uints0.y + point_index]);
    if (color >= 32u) { return; }
    let block = point_index / 64u;
    let block_offset = atomicLoad(
        &topology[params.uints0.w + block * 32u + color]);
    let local_rank = atomicLoad(&topology[params.uints1.x + point_index]);
    let color_offset = atomicLoad(&topology[params.uints1.z + color]);
    let slot = color_offset + block_offset + local_rank;
    let indices_base = bitcast<u32>(params.floats0.x);
    if (slot < params.uints0.x) {
        atomicStore(&topology[indices_base + slot], point_index);
    } else {
        atomicStore(&topology[bitcast<u32>(params.floats0.y)], 1u);
    }
}

fn island_dynamic(body_index: u32) -> bool {
    let property = properties[body_index];
    return property.z > 0.0 || property.w > 0.0;
}

fn island_root(parent_base: u32, body_index: u32) -> u32 {
    var root = body_index;
    loop {
        let parent = atomicLoad(&topology[parent_base + root]);
        if (parent == root || parent == 0xffffffffu) {
            root = parent;
            break;
        }
        root = parent;
    }
    return root;
}

@compute @workgroup_size(1)
fn build_solver_islands(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    let body_count = params.uints0.x;
    let point_count = params.uints0.y;
    let parent_base = params.uints0.z;
    let body_island_base = params.uints0.w;
    let color_base = params.uints1.x;
    let color_status_index = params.uints1.y;
    let bin_counts_base = params.uints1.z;
    let bin_offsets_base = params.uints1.w;
    let roots_base = bitcast<u32>(params.floats0.x);
    let body_counts_base = bitcast<u32>(params.floats0.y);
    let manifold_counts_base = bitcast<u32>(params.floats0.z);
    let bin_cursors_base = bitcast<u32>(params.floats0.w);
    let point_indices_base = bitcast<u32>(params.floats1.x);
    let island_count_index = bitcast<u32>(params.floats1.y);
    let bin_capacity = body_count * 32u;

    for (var body = 0u; body < body_count; body += 1u) {
        atomicStore(&topology[parent_base + body], 0xffffffffu);
        atomicStore(&topology[body_island_base + body], 0xffffffffu);
        atomicStore(&topology[roots_base + body], 0xffffffffu);
        atomicStore(&topology[body_counts_base + body], 0u);
        atomicStore(&topology[manifold_counts_base + body], 0u);
    }
    for (var bin = 0u; bin < bin_capacity; bin += 1u) {
        atomicStore(&topology[bin_counts_base + bin], 0u);
        atomicStore(&topology[bin_offsets_base + bin], 0u);
        atomicStore(&topology[bin_cursors_base + bin], 0u);
    }
    atomicStore(&topology[bin_offsets_base + bin_capacity], 0u);
    atomicStore(&topology[island_count_index], 0u);
    atomicStore(&color_dispatches[96u], 0u);
    atomicStore(&color_dispatches[97u], 1u);
    atomicStore(&color_dispatches[98u], 1u);
    if (atomicLoad(&topology[color_status_index]) != 0u) { return; }

    for (var point_index = 0u; point_index < point_count; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point) || point.ids.w != 0u) { continue; }
        let dynamic_a = island_dynamic(point.ids.x);
        let dynamic_b = island_dynamic(point.ids.y);
        if (!dynamic_a && !dynamic_b) { continue; }
        if (dynamic_a && atomicLoad(&topology[parent_base + point.ids.x]) == 0xffffffffu) {
            atomicStore(&topology[parent_base + point.ids.x], point.ids.x);
        }
        if (dynamic_b && atomicLoad(&topology[parent_base + point.ids.y]) == 0xffffffffu) {
            atomicStore(&topology[parent_base + point.ids.y], point.ids.y);
        }
        if (dynamic_a && dynamic_b) {
            let root_a = island_root(parent_base, point.ids.x);
            let root_b = island_root(parent_base, point.ids.y);
            if (root_a != root_b) {
                let lower = min(root_a, root_b);
                let upper = max(root_a, root_b);
                atomicStore(&topology[parent_base + upper], lower);
            }
        }
    }

    var island_count = 0u;
    for (var body = 0u; body < body_count; body += 1u) {
        if (atomicLoad(&topology[parent_base + body]) == 0xffffffffu) { continue; }
        let root = island_root(parent_base, body);
        atomicStore(&topology[parent_base + body], root);
        if (root == body) {
            atomicStore(&topology[body_island_base + body], island_count);
            atomicStore(&topology[roots_base + island_count], body);
            island_count += 1u;
        }
    }
    for (var body = 0u; body < body_count; body += 1u) {
        let root = atomicLoad(&topology[parent_base + body]);
        if (root == 0xffffffffu) { continue; }
        let island = atomicLoad(&topology[body_island_base + root]);
        atomicStore(&topology[body_island_base + body], island);
        atomicAdd(&topology[body_counts_base + island], 1u);
    }

    for (var point_index = 0u; point_index < point_count; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point) || point.ids.w != 0u) { continue; }
        let color = atomicLoad(&topology[color_base + point_index]);
        if (color >= 32u) { continue; }
        var island = 0xffffffffu;
        if (island_dynamic(point.ids.x)) {
            island = atomicLoad(&topology[body_island_base + point.ids.x]);
        } else if (island_dynamic(point.ids.y)) {
            island = atomicLoad(&topology[body_island_base + point.ids.y]);
        }
        if (island == 0xffffffffu) { continue; }
        atomicAdd(&topology[bin_counts_base + island * 32u + color], 1u);
        atomicAdd(&topology[manifold_counts_base + island], 1u);
    }

    var running = 0u;
    for (var bin = 0u; bin < island_count * 32u; bin += 1u) {
        atomicStore(&topology[bin_offsets_base + bin], running);
        running += atomicLoad(&topology[bin_counts_base + bin]);
    }
    atomicStore(&topology[bin_offsets_base + island_count * 32u], running);
    for (var point_index = 0u; point_index < point_count; point_index += 1u) {
        let point = point_inputs[point_index];
        if (!valid_point(point) || point.ids.w != 0u) { continue; }
        let color = atomicLoad(&topology[color_base + point_index]);
        if (color >= 32u) { continue; }
        var island = 0xffffffffu;
        if (island_dynamic(point.ids.x)) {
            island = atomicLoad(&topology[body_island_base + point.ids.x]);
        } else if (island_dynamic(point.ids.y)) {
            island = atomicLoad(&topology[body_island_base + point.ids.y]);
        }
        if (island == 0xffffffffu) { continue; }
        let bin = island * 32u + color;
        let slot = atomicAdd(&topology[bin_cursors_base + bin], 1u);
        let offset = atomicLoad(&topology[bin_offsets_base + bin]);
        atomicStore(&topology[point_indices_base + offset + slot], point_index);
    }
    atomicStore(&topology[island_count_index], island_count);
    atomicStore(&color_dispatches[96u], island_count);
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
    if (params.uints0.w == 2u) {
        let input = point_inputs[id.x];
        let pose_a = poses[input.ids.x];
        let pose_b = poses[input.ids.y];
        let moving_anchor_a = rotate(input.local_anchor_a, pose_a.z);
        let moving_anchor_b = rotate(input.local_anchor_b, pose_b.z);
        var fixed = prepared_points[id.x];
        fixed.data0.w = dot((pose_b.xy + moving_anchor_b) -
                            (pose_a.xy + moving_anchor_a), fixed.normal);
        prepared_points[id.x] = fixed;
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
    let coefficients = contact_coefficients(point);
    if (point.data0.w > 0.0) {
        velocity_bias = point.data0.w / params.floats0.x;
    } else if (params.uints1.w != 0u) {
        velocity_bias = coefficients.x * point.data0.w / params.floats0.x;
        if (params.floats2.x > 0.0) {
            velocity_bias = max(velocity_bias, -params.floats2.x);
        }
        mass_scale = coefficients.y;
        impulse_scale = coefficients.z;
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

fn normal_equation_from_velocity(point: PreparedPoint,
                                 normal_velocity: f32) -> NormalEquation {
    var velocity_bias = 0.0;
    var mass_scale = 1.0;
    var output: NormalEquation;
    output.impulse_scale = 0.0;
    if (params.floats2.w == 3.0) {
        output.base = normal_velocity;
        return output;
    }
    let coefficients = contact_coefficients(point);
    if (point.data0.w > 0.0) {
        velocity_bias = point.data0.w / params.floats0.x;
    } else if (params.uints1.w != 0u || params.floats2.w == 1.0) {
        if (params.uints1.w != 0u) {
            velocity_bias = coefficients.x * point.data0.w / params.floats0.x;
            if (params.floats2.x > 0.0) {
                velocity_bias = max(velocity_bias, -params.floats2.x);
            }
        }
        mass_scale = coefficients.y;
        output.impulse_scale = coefficients.z;
    }
    output.base = mass_scale * normal_velocity + velocity_bias;
    return output;
}

fn normal_equation(point: PreparedPoint) -> NormalEquation {
    return normal_equation_from_velocity(
        point, relative_velocity(point, point.normal));
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

fn solve_colored_manifold(point_index: u32, preserve_static_motion: bool) {
    let input0 = point_inputs[point_index];
    if (!valid_point(input0) || input0.ids.w != 0u) { return; }
    var point0 = prepared_points[point_index];
    var point1 = point0;
    var point_count = 1u;
    if (point_index + 1u < params.uints0.y) {
        let input1 = point_inputs[point_index + 1u];
        if (valid_point(input1) && input1.ids.x == input0.ids.x &&
                input1.ids.y == input0.ids.y && input1.ids.z == input0.ids.z &&
                input1.ids.w == 1u) {
            point1 = prepared_points[point_index + 1u];
            point_count = 2u;
        }
    }

    var motion_a = motions_source[point0.ids.x];
    var motion_b = motions_source[point0.ids.y];
    let stage = params.uints1.y;
    if (stage == 1u) {
        let impulse0 = point0.normal * point0.data1.z + point0.tangent * point0.data1.w;
        motion_a = apply_impulse(point0.ids.x, motion_a, -impulse0, point0.anchor_a);
        motion_b = apply_impulse(point0.ids.y, motion_b, impulse0, point0.anchor_b);
        emit_pair(point_index, point0, impulse0);
        if (point_count == 2u) {
            let impulse1 = point1.normal * point1.data1.z + point1.tangent * point1.data1.w;
            motion_a = apply_impulse(point1.ids.x, motion_a, -impulse1, point1.anchor_a);
            motion_b = apply_impulse(point1.ids.y, motion_b, impulse1, point1.anchor_b);
            emit_pair(point_index + 1u, point1, impulse1);
        }
    } else if (stage == 2u || stage == 4u || stage == 7u || stage == 8u) {
        let relaxation_stage = stage == 4u || stage == 8u;
        if (relaxation_stage && params.floats2.w == 2.0) {
            let equation0 = normal_equation_from_velocity(
                point0,
                relative_velocity_from_motions(point0, point0.normal, motion_a, motion_b));
            let k11 = block_matrix_entry(point0, point0, 1.0, 1.0);
            let mass0 = select(0.0, 1.0 / k11, k11 > 0.0);
            let old0 = point0.data1.z;
            point0.data1.z = max(old0 - mass0 * equation0.base -
                equation0.impulse_scale * old0, 0.0);
            let applied0 = point0.data1.z - old0;
            point0.data2.x += max(applied0, 0.0);
            point0.data2.w = 4.0;
            let impulse0 = point0.normal * applied0;
            motion_a = apply_impulse(point0.ids.x, motion_a, -impulse0, point0.anchor_a);
            motion_b = apply_impulse(point0.ids.y, motion_b, impulse0, point0.anchor_b);
            emit_pair(point_index, point0, impulse0);
            if (point_count == 2u) {
                let equation1 = normal_equation_from_velocity(
                    point1,
                    relative_velocity_from_motions(
                        point1, point1.normal, motion_a, motion_b));
                let k22 = block_matrix_entry(point1, point1, 1.0, 1.0);
                let mass1 = select(0.0, 1.0 / k22, k22 > 0.0);
                let old1 = point1.data1.z;
                point1.data1.z = max(old1 - mass1 * equation1.base -
                    equation1.impulse_scale * old1, 0.0);
                let applied1 = point1.data1.z - old1;
                point1.data2.x += max(applied1, 0.0);
                point1.data2.w = 4.0;
                let impulse1 = point1.normal * applied1;
                motion_a = apply_impulse(point1.ids.x, motion_a,
                    -impulse1, point1.anchor_a);
                motion_b = apply_impulse(point1.ids.y, motion_b,
                    impulse1, point1.anchor_b);
                emit_pair(point_index + 1u, point1, impulse1);
                store_block_trace(point_index, vec4f(k11, 0.0, 0.0, k22), 4u);
            } else {
                store_block_trace(point_index, vec4f(k11, 0.0, 0.0, 0.0), 4u);
            }
        } else {
            let k11 = block_matrix_entry(point0, point0, 1.0, 1.0);
            var matrix = vec4f(k11, 0.0, 0.0, 0.0);
            if (point_count == 2u) {
                matrix = vec4f(
                    k11,
                    block_matrix_entry(point0, point1, 1.0, 1.0),
                    block_matrix_entry(point1, point0, 1.0, 1.0),
                    block_matrix_entry(point1, point1, 1.0, 1.0));
            }
            let old = vec2f(point0.data1.z,
                select(0.0, point1.data1.z, point_count == 2u));
            let equation0 = normal_equation_from_velocity(point0,
                relative_velocity_from_motions(point0, point0.normal, motion_a, motion_b));
            let equation1 = normal_equation_from_velocity(point1,
                relative_velocity_from_motions(point1, point1.normal, motion_a, motion_b));
            let solution = solve_normal_block(
                matrix,
                vec2f(equation0.base,
                    select(0.0, equation1.base, point_count == 2u)),
                old,
                vec2f(equation0.impulse_scale,
                    select(0.0, equation1.impulse_scale, point_count == 2u)),
                point_count);
            store_block_trace(point_index, matrix, solution.active_set);

            let applied0 = solution.impulses.x - old.x;
            point0.data1.z = solution.impulses.x;
            point0.data2.x += max(applied0, 0.0);
            point0.data2.w = f32(solution.active_set);
            let impulse0 = point0.normal * applied0;
            motion_a = apply_impulse(point0.ids.x, motion_a, -impulse0, point0.anchor_a);
            motion_b = apply_impulse(point0.ids.y, motion_b, impulse0, point0.anchor_b);
            emit_pair(point_index, point0, impulse0);
            if (point_count == 2u) {
                let applied1 = solution.impulses.y - old.y;
                point1.data1.z = solution.impulses.y;
                point1.data2.x += max(applied1, 0.0);
                point1.data2.w = f32(solution.active_set);
                let impulse1 = point1.normal * applied1;
                motion_a = apply_impulse(point1.ids.x, motion_a,
                    -impulse1, point1.anchor_a);
                motion_b = apply_impulse(point1.ids.y, motion_b,
                    impulse1, point1.anchor_b);
                emit_pair(point_index + 1u, point1, impulse1);
            }
        }
        if (stage == 7u || stage == 8u) {
            let tangent_velocity0 = relative_velocity_from_motions(
                point0, point0.tangent, motion_a, motion_b);
            let old0 = point0.data1.w;
            let limit0 = point0.data1.x * point0.data1.z;
            point0.data1.w = clamp(
                old0 - point0.data0.z * tangent_velocity0, -limit0, limit0);
            let impulse0 = point0.tangent * (point0.data1.w - old0);
            motion_a = apply_impulse(
                point0.ids.x, motion_a, -impulse0, point0.anchor_a);
            motion_b = apply_impulse(
                point0.ids.y, motion_b, impulse0, point0.anchor_b);
            emit_pair(point_index, point0, impulse0);
            if (point_count == 2u) {
                let tangent_velocity1 = relative_velocity_from_motions(
                    point1, point1.tangent, motion_a, motion_b);
                let old1 = point1.data1.w;
                let limit1 = point1.data1.x * point1.data1.z;
                point1.data1.w = clamp(
                    old1 - point1.data0.z * tangent_velocity1, -limit1, limit1);
                let impulse1 = point1.tangent * (point1.data1.w - old1);
                motion_a = apply_impulse(
                    point1.ids.x, motion_a, -impulse1, point1.anchor_a);
                motion_b = apply_impulse(
                    point1.ids.y, motion_b, impulse1, point1.anchor_b);
                emit_pair(point_index + 1u, point1, impulse1);
            }
        }
    } else if (stage == 3u || stage == 5u) {
        let tangent_velocity0 = relative_velocity_from_motions(
            point0, point0.tangent, motion_a, motion_b);
        let old0 = point0.data1.w;
        let limit0 = point0.data1.x * point0.data1.z;
        point0.data1.w = clamp(old0 - point0.data0.z * tangent_velocity0, -limit0, limit0);
        let impulse0 = point0.tangent * (point0.data1.w - old0);
        motion_a = apply_impulse(point0.ids.x, motion_a, -impulse0, point0.anchor_a);
        motion_b = apply_impulse(point0.ids.y, motion_b, impulse0, point0.anchor_b);
        emit_pair(point_index, point0, impulse0);
        if (point_count == 2u) {
            let tangent_velocity1 = relative_velocity_from_motions(
                point1, point1.tangent, motion_a, motion_b);
            let old1 = point1.data1.w;
            let limit1 = point1.data1.x * point1.data1.z;
            point1.data1.w = clamp(
                old1 - point1.data0.z * tangent_velocity1, -limit1, limit1);
            let impulse1 = point1.tangent * (point1.data1.w - old1);
            motion_a = apply_impulse(point1.ids.x, motion_a, -impulse1, point1.anchor_a);
            motion_b = apply_impulse(point1.ids.y, motion_b, impulse1, point1.anchor_b);
            emit_pair(point_index + 1u, point1, impulse1);
        }
    } else if (stage == 6u) {
        let normal_velocity0 = relative_velocity_from_motions(
            point0, point0.normal, motion_a, motion_b);
        let old0 = point0.data1.z;
        let eligible0 = point0.data1.y > 0.0 && point0.data0.x < -params.floats1.x &&
            point0.data2.x > 0.0;
        if (eligible0) {
            point0.data1.z = max(old0 - point0.data0.y *
                (normal_velocity0 + point0.data1.y * point0.data0.x), 0.0);
        }
        let impulse0 = point0.normal * (point0.data1.z - old0);
        motion_a = apply_impulse(point0.ids.x, motion_a, -impulse0, point0.anchor_a);
        motion_b = apply_impulse(point0.ids.y, motion_b, impulse0, point0.anchor_b);
        emit_pair(point_index, point0, impulse0);
        if (point_count == 2u) {
            let normal_velocity1 = relative_velocity_from_motions(
                point1, point1.normal, motion_a, motion_b);
            let old1 = point1.data1.z;
            let eligible1 = point1.data1.y > 0.0 &&
                point1.data0.x < -params.floats1.x && point1.data2.x > 0.0;
            if (eligible1) {
                point1.data1.z = max(old1 - point1.data0.y *
                    (normal_velocity1 + point1.data1.y * point1.data0.x), 0.0);
            }
            let impulse1 = point1.normal * (point1.data1.z - old1);
            motion_a = apply_impulse(point1.ids.x, motion_a, -impulse1, point1.anchor_a);
            motion_b = apply_impulse(point1.ids.y, motion_b, impulse1, point1.anchor_b);
            emit_pair(point_index + 1u, point1, impulse1);
        }
    }

    prepared_points[point_index] = point0;
    if (point_count == 2u) {
        prepared_points[point_index + 1u] = point1;
    }
    if (!preserve_static_motion || island_dynamic(point0.ids.x)) {
        motions_source[point0.ids.x] = motion_a;
    }
    if (!preserve_static_motion || island_dynamic(point0.ids.y)) {
        motions_source[point0.ids.y] = motion_b;
    }
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
    if (u32(params.floats2.z) == 1u) {
        solve_colored_manifold(point_index, params.floats1.w != 0.0);
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
        let coefficients = contact_coefficients(point);
        if (point.data0.w > 0.0) {
            velocity_bias = point.data0.w / params.floats0.x;
        } else if (params.uints1.w != 0u) {
            velocity_bias = coefficients.x * point.data0.w / params.floats0.x;
            if (params.floats2.x > 0.0) {
                velocity_bias = max(velocity_bias, -params.floats2.x);
            }
            mass_scale = coefficients.y;
            impulse_scale = coefficients.z;
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

@compute @workgroup_size(64)
fn island_colored_stage(@builtin(local_invocation_id) local_id: vec3u,
                        @builtin(workgroup_id) workgroup_id: vec3u) {
    let island = workgroup_id.x;
    let island_count = atomicLoad(&topology[params.uints0.x]);
    let color_count = atomicLoad(&topology[bitcast<u32>(params.floats2.w)]);
    for (var scheduled_color = 0u; scheduled_color < 32u;
            scheduled_color += 1u) {
        if (island < island_count && scheduled_color < color_count) {
            let color = select(scheduled_color, color_count - 1u - scheduled_color,
                               params.uints1.z != 0u);
            let bin = island * 32u + color;
            let start = atomicLoad(&topology[params.uints0.z + bin]);
            let end = atomicLoad(&topology[params.uints0.z + bin + 1u]);
            for (var slot = start + local_id.x; slot < end; slot += 64u) {
                let point_index = atomicLoad(&topology[params.uints0.w + slot]);
                solve_colored_manifold(point_index, true);
            }
        }
        storageBarrier();
        workgroupBarrier();
    }
}
