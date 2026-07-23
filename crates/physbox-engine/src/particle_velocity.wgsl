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
@group(0) @binding(4) var<storage, read> cell_bins: array<u32>;
@group(0) @binding(5) var<storage, read_write> corrections: array<vec2f>;
@group(0) @binding(6) var<uniform> params: ParticleParams;

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
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        if index < params.counts.x {
            corrections[index] = vec2f(0.0);
        }
        return;
    }
    let position = positions[index].xy;
    let velocity = velocities[index].xy;
    let own_cell = cell_for(position);
    var weighted_delta = vec2f(0.0);
    var weight_sum = 0.0;
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
