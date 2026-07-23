struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(2) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> cell_bins: array<u32>;
@group(0) @binding(5) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(6) var<uniform> params: ParticleParams;

const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_GRID_INSERTS: u32 = 2u;
const COUNTER_GRID_OVERFLOW: u32 = 3u;
const COUNTER_ESCAPED: u32 = 7u;
const COUNTER_KILLED: u32 = 11u;

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
fn clear_grid(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    let cell_count = params.counts.z * params.counts.w;
    if index < cell_count {
        atomicStore(&cell_counts[index], 0u);
    }
}

@compute @workgroup_size(64)
fn fill_grid(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let cell = cell_for(positions[index].xy);
    if !cell_is_valid(cell) {
        let removed = atomicCompareExchangeWeak(&active_flags[index], 1u, 0u);
        if removed.exchanged {
            let previous_metadata = metadata[index];
            metadata[index] = vec4u(
                previous_metadata.x + 1u,
                previous_metadata.y,
                previous_metadata.z,
                0u,
            );
            atomicAdd(&counters[COUNTER_ESCAPED], 1u);
            atomicAdd(&counters[COUNTER_KILLED], 1u);
            atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 2u);
        }
        return;
    }
    let target_cell = cell_index(cell);
    let previous_metadata = metadata[index];
    metadata[index] = vec4u(
        previous_metadata.x,
        previous_metadata.y,
        target_cell,
        previous_metadata.w,
    );
    let slot = atomicAdd(&cell_counts[target_cell], 1u);
    atomicAdd(&counters[COUNTER_GRID_INSERTS], 1u);
    if slot < params.limits.x {
        cell_bins[target_cell * params.limits.x + slot] = index;
    } else {
        atomicAdd(&counters[COUNTER_GRID_OVERFLOW], 1u);
        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 4u);
    }
}
