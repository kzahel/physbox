struct IntegrationParams {
    dt_gravity: vec4f,
    counts: vec4u,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read> properties: array<vec4f>;
@group(0) @binding(3) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(4) var<uniform> params: IntegrationParams;
@group(0) @binding(5) var<storage, read> active_bodies: array<u32>;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_SLEEPING: u32 = 1u << 1u;
const FLAG_FIXED: u32 = 1u << 2u;

@compute @workgroup_size(64)
fn integrate_bodies(@builtin(global_invocation_id) invocation: vec3u) {
    let active_index = invocation.x;
    if (active_index >= active_bodies[0]) {
        return;
    }
    let index = active_bodies[20u + active_index];
    let flags = metadata[index].y;
    if ((flags & FLAG_LIVE) == 0u || (flags & (FLAG_SLEEPING | FLAG_FIXED)) != 0u) {
        return;
    }
    let inverse_mass = properties[index].z;
    if (inverse_mass <= 0.0) {
        return;
    }
    let dt = params.dt_gravity.x;
    var motion = motions[index];
    motion.x += params.dt_gravity.y * dt;
    motion.y += params.dt_gravity.z * dt;
    var pose = poses[index];
    pose.x += motion.x * dt;
    pose.y += motion.y * dt;
    pose.z += motion.z * dt;
    motions[index] = motion;
    poses[index] = pose;
}
