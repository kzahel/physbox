struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> contributions: array<vec4f>;
@group(0) @binding(1) var<storage, read> body_metadata: array<vec4u>;
@group(0) @binding(2) var<storage, read_write> partials: array<vec4f>;
@group(0) @binding(3) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_FIXED: u32 = 1u << 2u;
const TILE_SIZE: u32 = 64u;
const NO_BODY: u32 = 0xffffffffu;

@compute @workgroup_size(64)
fn reduce_body_contributions(
    @builtin(workgroup_id) workgroup: vec3u,
    @builtin(local_invocation_id) local: vec3u,
) {
    let body_index = workgroup.y * TILE_SIZE + local.x;
    let tile = workgroup.x;
    let tile_count = (params.counts.x + TILE_SIZE - 1u) / TILE_SIZE;
    if body_index >= params.counts.y || tile >= tile_count {
        return;
    }
    let partial_index = body_index * tile_count + tile;
    partials[partial_index] = vec4f(0.0);
    let identity = body_metadata[body_index];
    if (identity.y & (BODY_FLAG_LIVE | BODY_FLAG_FIXED)) != BODY_FLAG_LIVE {
        return;
    }

    let first_particle = tile * TILE_SIZE;
    let end_particle = min(first_particle + TILE_SIZE, params.counts.x);
    var total = vec3f(0.0);
    var count = 0.0;
    for (var particle_index = first_particle; particle_index < end_particle; particle_index += 1u) {
        let contribution = contributions[particle_index];
        let target_body = bitcast<u32>(contribution.w);
        if target_body == NO_BODY || target_body != body_index {
            continue;
        }
        total += contribution.xyz;
        count += 1.0;
    }
    partials[partial_index] = vec4f(total, count);
}
