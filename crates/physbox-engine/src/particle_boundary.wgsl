struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read> body_poses: array<vec4f>;
@group(0) @binding(3) var<storage, read> body_properties: array<vec4f>;
@group(0) @binding(4) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(5) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(6) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_RECTANGLE: u32 = 1u << 8u;
const BODY_FLAG_CIRCLE: u32 = 1u << 9u;
const COUNTER_BODY_CONTACTS: u32 = 6u;

fn rotate(value: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * value.x - s * value.y, s * value.x + c * value.y);
}

fn project_circle(local: vec2f, radius: f32, particle_radius: f32) -> vec2f {
    let distance = length(local);
    let normal = select(vec2f(0.0, 1.0), local / distance, distance > 0.000001);
    let signed_distance = distance - radius;
    return select(vec2f(0.0), normal * (particle_radius - signed_distance),
                  signed_distance <= particle_radius);
}

fn project_rectangle(local: vec2f, half: vec2f, particle_radius: f32) -> vec2f {
    let closest = clamp(local, -half, half);
    let outside = local - closest;
    let outside_distance = length(outside);
    var normal = vec2f(0.0);
    var signed_distance = 0.0;
    if outside_distance > 0.000001 {
        normal = outside / outside_distance;
        signed_distance = outside_distance;
    } else {
        let depth = half - abs(local);
        if depth.x < depth.y {
            normal = vec2f(select(1.0, -1.0, local.x < 0.0), 0.0);
            signed_distance = -depth.x;
        } else {
            normal = vec2f(0.0, select(1.0, -1.0, local.y < 0.0));
            signed_distance = -depth.y;
        }
    }
    return select(vec2f(0.0), normal * (particle_radius - signed_distance),
                  signed_distance <= particle_radius);
}

@compute @workgroup_size(64)
fn project_boundaries(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    var world = positions[index].xy;
    var contacts = 0u;
    for (var body_index = 0u; body_index < params.counts.y; body_index += 1u) {
        let body_flags = body_metadata[body_index].y;
        if (body_flags & BODY_FLAG_LIVE) == 0u {
            continue;
        }
        let pose = body_poses[body_index];
        let local = rotate(world - pose.xy, -pose.z);
        var local_correction = vec2f(0.0);
        if (body_flags & BODY_FLAG_CIRCLE) != 0u {
            local_correction = project_circle(local, body_properties[body_index].x, params.material.x);
        } else if (body_flags & BODY_FLAG_RECTANGLE) != 0u {
            local_correction = project_rectangle(local, body_properties[body_index].xy, params.material.x);
        }
        if dot(local_correction, local_correction) > 0.0 {
            world += rotate(local_correction, pose.z);
            contacts += 1u;
        }
    }
    let previous = positions[index];
    positions[index] = vec4f(world, previous.zw);
    if contacts > 0u {
        atomicAdd(&counters[COUNTER_BODY_CONTACTS], contacts);
    }
}
