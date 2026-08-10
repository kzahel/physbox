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
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read> contact_generations: array<u32>;
@group(0) @binding(3) var<storage, read_write> contributions: array<vec4f>;
@group(0) @binding(4) var<storage, read> body_poses: array<vec4f>;
@group(0) @binding(5) var<storage, read> body_motions: array<vec4f>;
@group(0) @binding(6) var<storage, read> body_properties: array<vec4f>;
@group(0) @binding(7) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(8) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_FIXED: u32 = 1u << 2u;
const BODY_FLAG_RECTANGLE: u32 = 1u << 8u;
const BODY_FLAG_CIRCLE: u32 = 1u << 9u;
const BODY_CONTRIBUTIONS_PER_PARTICLE: u32 = 4u;
const HAS_ADDITIONAL_BODY_BIT: u32 = 1u << 30u;
const BODY_INDEX_MASK: u32 = 0x3fffffffu;
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
    return LocalBoundary(normal, normal * radius);
}

fn rectangle_boundary(local: vec2f, half: vec2f) -> LocalBoundary {
    let closest = clamp(local, -half, half);
    let outside = local - closest;
    let outside_distance = length(outside);
    if outside_distance > 0.000001 {
        return LocalBoundary(outside / outside_distance, closest);
    }
    let depth = half - abs(local);
    if depth.x < depth.y {
        let normal = vec2f(select(1.0, -1.0, local.x < 0.0), 0.0);
        return LocalBoundary(normal, vec2f(normal.x * half.x, local.y));
    }
    let normal = vec2f(0.0, select(1.0, -1.0, local.y < 0.0));
    return LocalBoundary(normal, vec2f(local.x, normal.y * half.y));
}

@compute @workgroup_size(64)
fn solve_body_velocity(@builtin(global_invocation_id) invocation: vec3u) {
    let particle_index = invocation.x;
    if particle_index >= params.counts.x {
        return;
    }
    let empty_contribution = vec4f(0.0, 0.0, 0.0, bitcast<f32>(NO_BODY));
    let first_prior = contributions[particle_index];
    let first_encoded_body = bitcast<u32>(first_prior.w);
    if first_encoded_body == NO_BODY {
        for (var slot = 0u; slot < BODY_CONTRIBUTIONS_PER_PARTICLE; slot += 1u) {
            contributions[particle_index + slot * params.counts.x] = empty_contribution;
        }
        return;
    }
    let contact_count =
        select(1u, BODY_CONTRIBUTIONS_PER_PARTICLE,
            (first_encoded_body & HAS_ADDITIONAL_BODY_BIT) != 0u);

    var particle_velocity = velocities[particle_index];
    var emitted_contributions = 0u;
    for (var slot = 0u; slot < contact_count; slot += 1u) {
        let prior_index = particle_index + slot * params.counts.x;
        let prior = contributions[prior_index];
        contributions[prior_index] = empty_contribution;
        let body_index = bitcast<u32>(prior.w) & BODY_INDEX_MASK;
        if body_index >= params.counts.y {
            continue;
        }
        let contact_index = particle_index + slot * params.counts.x;
        let identity = body_metadata[body_index];
        if contact_generations[contact_index] == 0u
            || contact_generations[contact_index] != identity.x
            || (identity.y & (BODY_FLAG_LIVE | BODY_FLAG_FIXED)) != BODY_FLAG_LIVE {
            continue;
        }

        let pose = body_poses[body_index];
        let motion = body_motions[body_index];
        let properties = body_properties[body_index];
        let local = rotate(positions[particle_index].xy - pose.xy, -pose.z);
        var boundary = LocalBoundary(vec2f(0.0, 1.0), vec2f(0.0));
        if (identity.y & BODY_FLAG_CIRCLE) != 0u {
            boundary = circle_boundary(local, properties.x);
        } else if (identity.y & BODY_FLAG_RECTANGLE) != 0u {
            boundary = rectangle_boundary(local, properties.xy);
        } else {
            continue;
        }
        let normal = rotate(boundary.normal, pose.z);
        let contact_point = pose.xy + rotate(boundary.point, pose.z);
        let lever = contact_point - pose.xy;
        let surface_velocity = motion.xy + vec2f(
            -motion.z * lever.y,
            motion.z * lever.x,
        );
        let normal_speed = dot(particle_velocity.xy - surface_velocity, normal);
        let rotational = cross(lever, normal);
        let inverse_effective_mass = params.pressure.w + properties.z
            + properties.w * rotational * rotational;
        if normal_speed >= 0.0 || inverse_effective_mass <= 0.0 {
            continue;
        }

        let normal_impulse = -normal_speed / inverse_effective_mass;
        let particle_impulse = normal * normal_impulse;
        let body_impulse = -particle_impulse;
        particle_velocity = vec4f(
            particle_velocity.xy + particle_impulse * params.pressure.w,
            particle_velocity.zw,
        );
        let output_index =
            particle_index + emitted_contributions * params.counts.x;
        contributions[output_index] = vec4f(
            body_impulse * properties.z,
            properties.w * cross(lever, body_impulse),
            bitcast<f32>(body_index),
        );
        emitted_contributions += 1u;
    }
    if emitted_contributions > 1u {
        let first = contributions[particle_index];
        contributions[particle_index] = vec4f(
            first.xyz,
            bitcast<f32>(bitcast<u32>(first.w) | HAS_ADDITIONAL_BODY_BIT),
        );
    }
    velocities[particle_index] = particle_velocity;
}
