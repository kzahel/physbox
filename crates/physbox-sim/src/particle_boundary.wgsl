struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

struct LocalBoundary {
    normal: vec2f,
    point: vec2f,
    signed_distance: f32,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read> body_poses: array<vec4f>;
@group(0) @binding(3) var<storage, read> body_properties: array<vec4f>;
@group(0) @binding(4) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(5) var<storage, read_write> contributions: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> contact_generations: array<u32>;
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(8) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_RECTANGLE: u32 = 1u << 8u;
const BODY_FLAG_CIRCLE: u32 = 1u << 9u;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_BODY_CONTACTS: u32 = 6u;
const COUNTER_BODY_CONTRIBUTIONS: u32 = 16u;
const COUNTER_BODY_CONTRIBUTION_OVERFLOW: u32 = 18u;
const OVERFLOW_BODY_CONTRIBUTIONS: u32 = 1u << 4u;
const BODY_CONTRIBUTIONS_PER_PARTICLE: u32 = 4u;
const HAS_ADDITIONAL_BODY_BIT: u32 = 1u << 30u;
// This identity is bitcast through a float storage lane. Keep the bit pattern
// finite so every WebGPU WGSL frontend accepts the constant.
const NO_BODY: u32 = 0x7f7fffffu;

fn rotate(value: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * value.x - s * value.y, s * value.x + c * value.y);
}

fn cross(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

fn circle_boundary(local: vec2f, radius: f32) -> LocalBoundary {
    let distance = length(local);
    let normal = select(vec2f(0.0, 1.0), local / distance, distance > 0.000001);
    return LocalBoundary(normal, normal * radius, distance - radius);
}

fn rectangle_boundary(local: vec2f, half: vec2f) -> LocalBoundary {
    let closest = clamp(local, -half, half);
    let outside = local - closest;
    let outside_distance = length(outside);
    if outside_distance > 0.000001 {
        return LocalBoundary(outside / outside_distance, closest, outside_distance);
    }
    let depth = half - abs(local);
    if depth.x < depth.y {
        let normal = vec2f(select(1.0, -1.0, local.x < 0.0), 0.0);
        return LocalBoundary(normal, vec2f(normal.x * half.x, local.y), -depth.x);
    }
    let normal = vec2f(0.0, select(1.0, -1.0, local.y < 0.0));
    return LocalBoundary(normal, vec2f(local.x, normal.y * half.y), -depth.y);
}

@compute @workgroup_size(64)
fn project_boundaries(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x {
        return;
    }
    for (var slot = 0u; slot < BODY_CONTRIBUTIONS_PER_PARTICLE; slot += 1u) {
        let contribution_index = index + slot * params.counts.x;
        contributions[contribution_index] =
            vec4f(0.0, 0.0, 0.0, bitcast<f32>(NO_BODY));
        contact_generations[contribution_index] = 0u;
    }
    if atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    var world = positions[index].xy;
    var contact_count = 0u;
    var dynamic_contacts = 0u;
    var contribution_overflow = false;
    for (var body_index = 0u; body_index < params.counts.y; body_index += 1u) {
        let body_flags = body_metadata[body_index].y;
        if (body_flags & BODY_FLAG_LIVE) == 0u {
            continue;
        }
        let pose = body_poses[body_index];
        let local = rotate(world - pose.xy, -pose.z);
        var boundary = LocalBoundary(vec2f(0.0), vec2f(0.0), 1.0e20);
        if (body_flags & BODY_FLAG_CIRCLE) != 0u {
            boundary = circle_boundary(local, body_properties[body_index].x);
        } else if (body_flags & BODY_FLAG_RECTANGLE) != 0u {
            boundary = rectangle_boundary(local, body_properties[body_index].xy);
        }
        let penetration = params.material.x - boundary.signed_distance;
        if penetration < -0.00001 {
            continue;
        }
        let normal = rotate(boundary.normal, pose.z);
        let body_inverse_mass = body_properties[body_index].z;
        let body_inverse_inertia = body_properties[body_index].w;
        if body_inverse_mass > 0.0 || body_inverse_inertia > 0.0 {
            let contact_point = pose.xy + rotate(boundary.point, pose.z);
            let lever = contact_point - pose.xy;
            let rotational = cross(lever, normal);
            let inverse_effective_mass = params.pressure.w + body_inverse_mass
                + body_inverse_inertia * rotational * rotational;
            if inverse_effective_mass > 0.0 {
                let positional_impulse = max(penetration, 0.0) / inverse_effective_mass;
                world += normal * positional_impulse * params.pressure.w;
                let body_delta = -normal * positional_impulse * body_inverse_mass;
                let angle_delta = -positional_impulse * body_inverse_inertia * rotational;
                if dynamic_contacts < BODY_CONTRIBUTIONS_PER_PARTICLE {
                    let contribution_index = index + dynamic_contacts * params.counts.x;
                    contributions[contribution_index] = vec4f(
                        body_delta,
                        angle_delta,
                        bitcast<f32>(body_index),
                    );
                    contact_generations[contribution_index] = body_metadata[body_index].x;
                } else {
                    contribution_overflow = true;
                }
                dynamic_contacts += 1u;
            }
        } else {
            world += normal * penetration;
        }
        contact_count += 1u;
    }
    if contribution_overflow {
        for (var slot = 0u; slot < BODY_CONTRIBUTIONS_PER_PARTICLE; slot += 1u) {
            let contribution_index = index + slot * params.counts.x;
            contributions[contribution_index] =
                vec4f(0.0, 0.0, 0.0, bitcast<f32>(NO_BODY));
            contact_generations[contribution_index] = 0u;
        }
        atomicAdd(&counters[COUNTER_BODY_CONTRIBUTION_OVERFLOW], 1u);
        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], OVERFLOW_BODY_CONTRIBUTIONS);
    } else if dynamic_contacts > 0u {
        if dynamic_contacts > 1u {
            let first = contributions[index];
            contributions[index] = vec4f(
                first.xyz,
                bitcast<f32>(bitcast<u32>(first.w) | HAS_ADDITIONAL_BODY_BIT),
            );
        }
        atomicAdd(&counters[COUNTER_BODY_CONTRIBUTIONS], dynamic_contacts);
    }
    let previous = positions[index];
    positions[index] = vec4f(world, previous.zw);
    if contact_count > 0u {
        atomicAdd(&counters[COUNTER_BODY_CONTACTS], contact_count);
    }
}
