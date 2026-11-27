struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read> cell_offsets: array<u32>;
@group(0) @binding(4) var<storage, read> cell_records: array<u32>;
@group(0) @binding(5) var<storage, read_write> density_states: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(7) var<uniform> params: ParticleParams;

const COUNTER_NEIGHBOR_CHECKS: u32 = 4u;
const COUNTER_ACCEPTED_NEIGHBORS: u32 = 5u;

@compute @workgroup_size(64)
fn estimate_density(@builtin(global_invocation_id) invocation: vec3u) {
    let lane = invocation.x;
    if lane >= params.counts.x {
        return;
    }
    // Preserve cleared outputs for inactive slots. Live outputs have a single
    // writer, selected below in spatial order from the compact cell records.
    if atomicLoad(&active_flags[lane]) == 0u {
        density_states[lane] = vec4f(0.0);
    }
    if lane >= grid_record_count() {
        return;
    }
    let index = cell_records[lane];
    let position = positions[index].xy;
    let own_cell = cell_for(position);
    // Retain a margin at floating-point cell boundaries.
    let reach = vec2f(params.material.y + 0.00001);
    let first_cell = max(own_cell - vec2i(1), cell_for(position - reach));
    let last_cell = min(own_cell + vec2i(1), cell_for(position + reach));
    var density = 0.0;
    var near_density = 0.0;
    var checks = 0u;
    var accepted = 0u;
    for (var cell_y = first_cell.y; cell_y <= last_cell.y; cell_y += 1) {
        for (var cell_x = first_cell.x; cell_x <= last_cell.x; cell_x += 1) {
            let cell = vec2i(cell_x, cell_y);
            if !cell_is_valid(cell) {
                continue;
            }
            let target_cell = cell_index(cell);
            let start = cell_offsets[target_cell];
            let stored = atomicLoad(&cell_counts[target_cell]);
            for (var slot = 0u; slot < stored; slot += 1u) {
                let other = cell_records[start + slot];
                if other == index {
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
