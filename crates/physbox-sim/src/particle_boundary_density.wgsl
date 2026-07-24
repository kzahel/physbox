struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> density_states: array<vec4f>;
@group(0) @binding(3) var<storage, read> body_poses: array<vec4f>;
@group(0) @binding(4) var<storage, read> body_properties: array<vec4f>;
@group(0) @binding(5) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(6) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(7) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_RECTANGLE: u32 = 1u << 8u;
const BODY_FLAG_CIRCLE: u32 = 1u << 9u;
const COUNTER_BOUNDARY_DENSITY_CONTACTS: u32 = 19u;

fn rotate(value: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * value.x - s * value.y, s * value.x + c * value.y);
}

fn circle_signed_distance(local: vec2f, radius: f32) -> f32 {
    return length(local) - radius;
}

fn rectangle_signed_distance(local: vec2f, half: vec2f) -> f32 {
    let outside = max(abs(local) - half, vec2f(0.0));
    let outside_distance = length(outside);
    let inside_distance = min(max(abs(local).x - half.x, abs(local).y - half.y), 0.0);
    return outside_distance + inside_distance;
}

@compute @workgroup_size(64)
fn add_boundary_density(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let position = positions[index].xy;
    var state = density_states[index];
    var contact_count = 0u;
    for (var body_index = 0u; body_index < params.counts.y; body_index += 1u) {
        let flags = body_metadata[body_index].y;
        if (flags & BODY_FLAG_LIVE) == 0u {
            continue;
        }
        let pose = body_poses[body_index];
        let local = rotate(position - pose.xy, -pose.z);
        var signed_distance = 1.0e20;
        if (flags & BODY_FLAG_CIRCLE) != 0u {
            signed_distance = circle_signed_distance(local, body_properties[body_index].x);
        } else if (flags & BODY_FLAG_RECTANGLE) != 0u {
            signed_distance = rectangle_signed_distance(local, body_properties[body_index].xy);
        }
        let ghost_distance = 2.0 * max(signed_distance, 0.0);
        if ghost_distance >= params.material.y {
            continue;
        }
        let weight = clamp(1.0 - ghost_distance / params.material.y, 0.0, 1.0);
        let weight_squared = weight * weight;
        state.x += weight_squared;
        state.y += weight_squared * weight;
        contact_count += 1u;
    }
    state.z = params.pressure.x * max(state.x - params.material.z, 0.0);
    state.w = params.pressure.y * state.y;
    density_states[index] = state;
    if contact_count > 0u {
        atomicAdd(&counters[COUNTER_BOUNDARY_DENSITY_CONTACTS], contact_count);
    }
}
