// Public render-neutral particle observation projection.

struct ParticleObservation {
    position: vec4f,
    motion: vec4f,
    identity: vec4u,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> observations: array<ParticleObservation>;

const PARTICLE_LIVE: u32 = 1u;

@compute @workgroup_size(64)
fn observe_particles(@builtin(global_invocation_id) global_id: vec3u) {
    let slot = global_id.x;
    if slot >= arrayLength(&observations) {
        return;
    }
    let source = metadata[slot];
    let live = select(0u, PARTICLE_LIVE, source.w != 0u);
    observations[slot].position = positions[slot];
    observations[slot].motion = velocities[slot];
    observations[slot].identity = vec4u(source.x, live, slot, source.y);
}
