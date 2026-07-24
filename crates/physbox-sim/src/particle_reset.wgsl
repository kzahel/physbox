struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> initial_positions: array<vec4f>;
@group(0) @binding(5) var<storage, read> initial_velocities: array<vec4f>;
@group(0) @binding(6) var<storage, read> initial_metadata: array<vec4u>;
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(8) var<uniform> params: ParticleParams;

@compute @workgroup_size(64)
fn reset_particles(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index < params.counts.x {
        let previous_metadata = metadata[index];
        let authored_metadata = initial_metadata[index];
        positions[index] = initial_positions[index];
        velocities[index] = initial_velocities[index];
        metadata[index] = vec4u(
            previous_metadata.x + 1u,
            authored_metadata.y,
            authored_metadata.z,
            authored_metadata.w,
        );
        atomicStore(&active_flags[index], authored_metadata.w);
    }
    if index < 21u {
        atomicStore(&counters[index], 0u);
    }
    workgroupBarrier();
    if index == 0u {
        atomicStore(&counters[8], params.limits.z);
    }
}
