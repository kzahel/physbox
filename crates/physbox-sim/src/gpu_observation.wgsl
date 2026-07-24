struct BodyObservation {
    pose: vec4f,
    motion: vec4f,
    identity: vec4u,
};

const FLAG_LIVE: u32 = 1u;
const FLAG_SLEEPING: u32 = 2u;
const FLAG_FIXED: u32 = 4u;
const PUBLIC_FLAGS: u32 = FLAG_LIVE | FLAG_SLEEPING | FLAG_FIXED;

@group(0) @binding(0) var<storage, read> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> observations: array<BodyObservation>;

@compute @workgroup_size(64)
fn observe_bodies(@builtin(global_invocation_id) invocation: vec3u) {
    let slot = invocation.x;
    if slot >= arrayLength(&observations) {
        return;
    }
    let source_identity = metadata[slot];
    observations[slot] = BodyObservation(
        poses[slot],
        motions[slot],
        vec4u(source_identity.x, source_identity.y & PUBLIC_FLAGS, slot, 0u),
    );
}
