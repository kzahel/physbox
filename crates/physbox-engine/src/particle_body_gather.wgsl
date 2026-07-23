struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> partials: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> body_poses: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> body_motions: array<vec4f>;
@group(0) @binding(3) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(4) var<storage, read_write> wake_requests: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(6) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_FIXED: u32 = 1u << 2u;
const COUNTER_FLUID_WAKE_ROOTS: u32 = 17u;
const CAUSE_FLUID: u32 = 1u << 10u;
const TILE_SIZE: u32 = 64u;

fn particle_tile_count() -> u32 {
    return (params.counts.x + TILE_SIZE - 1u) / TILE_SIZE;
}

@compute @workgroup_size(64)
fn gather_body_position_response(@builtin(global_invocation_id) invocation: vec3u) {
    let body_index = invocation.x;
    if body_index >= params.counts.y {
        return;
    }
    let identity = body_metadata[body_index];
    if (identity.y & (BODY_FLAG_LIVE | BODY_FLAG_FIXED)) != BODY_FLAG_LIVE {
        return;
    }
    let tile_count = particle_tile_count();
    var position_delta = vec2f(0.0);
    var angle_delta = 0.0;
    var contribution_count = 0.0;
    for (var tile = 0u; tile < tile_count; tile += 1u) {
        let partial = partials[body_index * tile_count + tile];
        position_delta += partial.xy;
        angle_delta += partial.z;
        contribution_count += partial.w;
    }
    if contribution_count == 0.0
        || (length(position_delta) <= 0.000000001 && abs(angle_delta) <= 0.000000001) {
        return;
    }
    let pose = body_poses[body_index];
    let motion = body_motions[body_index];
    body_poses[body_index] = vec4f(
        pose.xy + position_delta,
        pose.z + angle_delta,
        pose.w,
    );
    body_motions[body_index] = vec4f(
        motion.xy + position_delta / params.dt_gravity.x,
        motion.z + angle_delta / params.dt_gravity.x,
        motion.w,
    );
    atomicOr(&wake_requests[body_index], CAUSE_FLUID);
    atomicAdd(&counters[COUNTER_FLUID_WAKE_ROOTS], 1u);
}

@compute @workgroup_size(64)
fn gather_body_velocity_response(@builtin(global_invocation_id) invocation: vec3u) {
    let body_index = invocation.x;
    if body_index >= params.counts.y {
        return;
    }
    let identity = body_metadata[body_index];
    if (identity.y & (BODY_FLAG_LIVE | BODY_FLAG_FIXED)) != BODY_FLAG_LIVE {
        return;
    }
    let tile_count = particle_tile_count();
    var linear_delta = vec2f(0.0);
    var angular_delta = 0.0;
    var contribution_count = 0.0;
    for (var tile = 0u; tile < tile_count; tile += 1u) {
        let partial = partials[body_index * tile_count + tile];
        linear_delta += partial.xy;
        angular_delta += partial.z;
        contribution_count += partial.w;
    }
    if contribution_count == 0.0 {
        return;
    }
    let motion = body_motions[body_index];
    body_motions[body_index] = vec4f(
        motion.xy + linear_delta,
        motion.z + angular_delta,
        motion.w,
    );
    atomicOr(&wake_requests[body_index], CAUSE_FLUID);
}
