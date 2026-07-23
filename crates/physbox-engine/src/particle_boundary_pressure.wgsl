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
@group(0) @binding(2) var<storage, read> density_states: array<vec4f>;
@group(0) @binding(3) var<storage, read> body_poses: array<vec4f>;
@group(0) @binding(4) var<storage, read> body_properties: array<vec4f>;
@group(0) @binding(5) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(6) var<storage, read_write> contributions: array<vec4f>;
@group(0) @binding(7) var<storage, read_write> contact_generations: array<u32>;
@group(0) @binding(8) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_RECTANGLE: u32 = 1u << 8u;
const BODY_FLAG_CIRCLE: u32 = 1u << 9u;
const PRESSURE_BODY_BIT: u32 = 1u << 31u;
const OVERFLOW_BODY: u32 = 0xfffffffeu;
const NO_BODY: u32 = 0xffffffffu;

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
fn project_boundary_pressure(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x {
        return;
    }
    contributions[index] = vec4f(0.0, 0.0, 0.0, bitcast<f32>(NO_BODY));
    contact_generations[index] = 0u;
    if atomicLoad(&active_flags[index]) == 0u {
        return;
    }

    var world = positions[index].xy;
    let state = density_states[index];
    var remaining = params.pressure.z;
    var dynamic_contacts = 0u;
    for (var body_index = 0u; body_index < params.counts.y; body_index += 1u) {
        let flags = body_metadata[body_index].y;
        if (flags & BODY_FLAG_LIVE) == 0u {
            continue;
        }
        let pose = body_poses[body_index];
        let local = rotate(world - pose.xy, -pose.z);
        var boundary = LocalBoundary(vec2f(0.0), vec2f(0.0), 1.0e20);
        if (flags & BODY_FLAG_CIRCLE) != 0u {
            boundary = circle_boundary(local, body_properties[body_index].x);
        } else if (flags & BODY_FLAG_RECTANGLE) != 0u {
            boundary = rectangle_boundary(local, body_properties[body_index].xy);
        }
        let ghost_distance = 2.0 * max(boundary.signed_distance, 0.0);
        if ghost_distance >= params.material.y || remaining <= 0.0 {
            continue;
        }
        let weight = clamp(1.0 - ghost_distance / params.material.y, 0.0, 1.0);
        let requested = params.dt_gravity.x * params.dt_gravity.x
            * (state.z * weight + state.w * weight * weight);
        let displacement = min(max(requested, 0.0), remaining);
        if displacement <= 0.0 {
            continue;
        }
        remaining -= displacement;
        let normal = rotate(boundary.normal, pose.z);
        let body_inverse_mass = body_properties[body_index].z;
        let body_inverse_inertia = body_properties[body_index].w;
        if body_inverse_mass > 0.0 || body_inverse_inertia > 0.0 {
            let contact_point = pose.xy + rotate(boundary.point, pose.z);
            let lever = contact_point - pose.xy;
            let rotational = cross(lever, normal);
            let inverse_effective_mass = params.pressure.w + body_inverse_mass
                + body_inverse_inertia * rotational * rotational;
            if inverse_effective_mass <= 0.0 {
                continue;
            }
            let positional_impulse = displacement / inverse_effective_mass;
            world += normal * positional_impulse * params.pressure.w;
            if dynamic_contacts == 0u {
                contributions[index] = vec4f(
                    -normal * positional_impulse * body_inverse_mass,
                    -positional_impulse * body_inverse_inertia * rotational,
                    bitcast<f32>(body_index | PRESSURE_BODY_BIT),
                );
                contact_generations[index] = body_metadata[body_index].x;
            } else {
                contributions[index] = vec4f(0.0, 0.0, 0.0, bitcast<f32>(OVERFLOW_BODY));
                contact_generations[index] = 0u;
            }
            dynamic_contacts += 1u;
        } else {
            world += normal * displacement;
        }
    }
    let previous = positions[index];
    positions[index] = vec4f(world, previous.zw);
}
