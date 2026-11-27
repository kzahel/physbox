struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> cell_offsets: array<u32>;
@group(0) @binding(5) var<storage, read> cell_records: array<u32>;
@group(0) @binding(6) var<storage, read_write> corrections: array<vec2f>;
@group(0) @binding(7) var<uniform> params: ParticleParams;

@compute @workgroup_size(64)
fn reconstruct_velocity(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let previous = velocities[index];
    velocities[index] = vec4f(
        (positions[index].xy - positions[index].zw) / params.dt_gravity.x,
        previous.zw,
    );
}

@compute @workgroup_size(64)
fn gather_viscosity(@builtin(global_invocation_id) invocation: vec3u) {
    let lane = invocation.x;
    if lane >= params.counts.x {
        return;
    }
    // Preserve cleared outputs for inactive slots. Live outputs have a single
    // writer, selected below in spatial order from the compact cell records.
    if atomicLoad(&active_flags[lane]) == 0u {
        corrections[lane] = vec2f(0.0);
    }
    if lane >= grid_record_count() {
        return;
    }
    let index = cell_records[lane];
    let position = positions[index].xy;
    let velocity = velocities[index].xy;
    let own_cell = cell_for(position);
    let reach = vec2f(params.material.y + 0.00001);
    let first_cell = max(own_cell - vec2i(1), cell_for(position - reach));
    let last_cell = min(own_cell + vec2i(1), cell_for(position + reach));
    var weighted_delta = vec2f(0.0);
    var weight_sum = 0.0;
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
                let distance = length(positions[other].xy - position);
                if distance >= params.material.y {
                    continue;
                }
                let weight = clamp(1.0 - distance / params.material.y, 0.0, 1.0);
                weighted_delta += weight * (velocities[other].xy - velocity);
                weight_sum += weight;
            }
        }
    }
    corrections[index] = select(vec2f(0.0),
        weighted_delta / weight_sum * params.material.w,
        weight_sum > 0.0);
}

@compute @workgroup_size(64)
fn apply_viscosity(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let previous = velocities[index];
    velocities[index] = vec4f(previous.xy + corrections[index], previous.zw);
}
