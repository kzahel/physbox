struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(2) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> cell_bins: array<u32>;
@group(0) @binding(5) var<storage, read_write> density_states: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(7) var<uniform> params: ParticleParams;

const COUNTER_NEIGHBOR_CHECKS: u32 = 4u;
const COUNTER_ACCEPTED_NEIGHBORS: u32 = 5u;

fn cell_for(position: vec2f) -> vec2i {
    return vec2i(floor((position - params.grid.xy) / params.grid.z));
}

fn cell_is_valid(cell: vec2i) -> bool {
    return all(cell >= vec2i(0)) &&
        cell.x < i32(params.counts.z) &&
        cell.y < i32(params.counts.w);
}

fn cell_index(cell: vec2i) -> u32 {
    return u32(cell.y) * params.counts.z + u32(cell.x);
}

@compute @workgroup_size(64)
fn estimate_density(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        if index < params.counts.x {
            density_states[index] = vec4f(0.0);
        }
        return;
    }
    let position = positions[index].xy;
    let own_cell = cell_for(position);
    var density = 0.0;
    var near_density = 0.0;
    var checks = 0u;
    var accepted = 0u;
    for (var offset_y = -1; offset_y <= 1; offset_y += 1) {
        for (var offset_x = -1; offset_x <= 1; offset_x += 1) {
            let cell = own_cell + vec2i(offset_x, offset_y);
            if !cell_is_valid(cell) {
                continue;
            }
            let target_cell = cell_index(cell);
            let stored = min(atomicLoad(&cell_counts[target_cell]), params.limits.x);
            for (var slot = 0u; slot < stored; slot += 1u) {
                let other = cell_bins[target_cell * params.limits.x + slot];
                if other == index || atomicLoad(&active_flags[other]) == 0u {
                    continue;
                }
                checks += 1u;
                let distance = length(positions[other].xy - position);
                if distance >= params.material.y {
                    continue;
                }
                accepted += 1u;
                let weight = clamp(1.0 - distance / params.material.y, 0.0, 1.0);
                density += weight * weight;
                near_density += weight * weight * weight;
            }
        }
    }
    let pressure = params.pressure.x * max(density - params.material.z, 0.0);
    let near_pressure = params.pressure.y * near_density;
    density_states[index] = vec4f(density, near_density, pressure, near_pressure);
    atomicAdd(&counters[COUNTER_NEIGHBOR_CHECKS], checks);
    atomicAdd(&counters[COUNTER_ACCEPTED_NEIGHBORS], accepted);
}
