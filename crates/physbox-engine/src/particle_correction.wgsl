struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(2) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> cell_bins: array<u32>;
@group(0) @binding(5) var<storage, read> density_states: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> corrections: array<vec2f>;
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(8) var<uniform> params: ParticleParams;

const COUNTER_CORRECTIONS: u32 = 13u;
const COUNTER_CLAMPED_CORRECTIONS: u32 = 14u;

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
fn gather_corrections(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        if index < params.counts.x {
            corrections[index] = vec2f(0.0);
        }
        return;
    }
    let position = positions[index].xy;
    let own_cell = cell_for(position);
    let self_state = density_states[index];
    var correction = vec2f(0.0);
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
                let delta = positions[other].xy - position;
                let distance = length(delta);
                if distance <= 0.000001 || distance >= params.material.y {
                    continue;
                }
                let weight = clamp(1.0 - distance / params.material.y, 0.0, 1.0);
                let other_state = density_states[other];
                let magnitude = params.dt_gravity.x * params.dt_gravity.x *
                    ((self_state.z + other_state.z) * weight +
                     (self_state.w + other_state.w) * weight * weight);
                correction -= delta / distance * magnitude * 0.5;
            }
        }
    }
    let max_correction = params.pressure.z;
    let magnitude = length(correction);
    if magnitude > max_correction {
        correction *= max_correction / magnitude;
        atomicAdd(&counters[COUNTER_CLAMPED_CORRECTIONS], 1u);
    }
    if dot(correction, correction) > 0.0 {
        atomicAdd(&counters[COUNTER_CORRECTIONS], 1u);
    }
    corrections[index] = correction;
}

@compute @workgroup_size(64)
fn apply_corrections(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let previous = positions[index];
    positions[index] = vec4f(previous.xy + corrections[index], previous.zw);
}
