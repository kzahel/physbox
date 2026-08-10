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
@group(0) @binding(3) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(4) var<uniform> params: ParticleParams;

const BODY_FLAG_LIVE: u32 = 1u << 0u;
const BODY_FLAG_FIXED: u32 = 1u << 2u;
const TILE_SIZE: u32 = 64u;
const PRESSURE_BODY_BIT: u32 = 1u << 31u;
const HAS_ADDITIONAL_BODY_BIT: u32 = 1u << 30u;
const BODY_INDEX_MASK: u32 = 0x3fffffffu;
const OVERFLOW_BODY: u32 = 0x7f7ffffeu;
const NO_BODY: u32 = 0x7f7fffffu;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_BODY_CONTRIBUTION_OVERFLOW: u32 = 18u;
const COUNTER_BOUNDARY_PRESSURE_EDGES: u32 = 20u;
const OVERFLOW_BODY_CONTRIBUTIONS: u32 = 1u << 4u;
const BODY_CONTRIBUTIONS_PER_PARTICLE: u32 = 4u;

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
    let dynamic_body = (identity.y & (BODY_FLAG_LIVE | BODY_FLAG_FIXED)) == BODY_FLAG_LIVE;
    if body_index != 0u && !dynamic_body {
        return;
    }

    let first_particle = tile * TILE_SIZE;
    let end_particle = min(first_particle + TILE_SIZE, params.counts.x);
    var total = vec3f(0.0);
    var count = 0.0;
    var pressure_count = 0u;
    var overflow_count = 0u;
    for (var particle_index = first_particle; particle_index < end_particle; particle_index += 1u) {
        let first = contributions[particle_index];
        let first_encoded_body = bitcast<u32>(first.w);
        if first_encoded_body == OVERFLOW_BODY {
            if body_index == 0u {
                overflow_count += 1u;
            }
            continue;
        }
        if !dynamic_body || first_encoded_body == NO_BODY {
            continue;
        }
        let contribution_count =
            select(1u, BODY_CONTRIBUTIONS_PER_PARTICLE,
                (first_encoded_body & HAS_ADDITIONAL_BODY_BIT) != 0u);
        for (var slot = 0u; slot < contribution_count; slot += 1u) {
            var contribution = first;
            if slot > 0u {
                contribution = contributions[particle_index + slot * params.counts.x];
            }
            let encoded_body = bitcast<u32>(contribution.w);
            let pressure_edge = (encoded_body & PRESSURE_BODY_BIT) != 0u;
            let target_body = encoded_body & BODY_INDEX_MASK;
            if target_body != body_index {
                continue;
            }
            total += contribution.xyz;
            count += 1.0;
            pressure_count += select(0u, 1u, pressure_edge);
        }
    }
    if overflow_count > 0u {
        atomicAdd(&counters[COUNTER_BODY_CONTRIBUTION_OVERFLOW], overflow_count);
        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], OVERFLOW_BODY_CONTRIBUTIONS);
    }
    if pressure_count > 0u {
        atomicAdd(&counters[COUNTER_BOUNDARY_PRESSURE_EDGES], pressure_count);
    }
    if dynamic_body {
        partials[partial_index] = vec4f(total, count);
    }
}
