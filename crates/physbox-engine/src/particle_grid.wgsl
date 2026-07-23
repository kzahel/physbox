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
@group(0) @binding(3) var<storage, read> active_indices: array<u32>;
@group(0) @binding(4) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> cell_block_keys: array<u32>;
@group(0) @binding(6) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(7) var<uniform> params: ParticleParams;

const COUNTER_ACTIVE: u32 = 0u;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_GRID_INSERTS: u32 = 2u;
const COUNTER_ESCAPED: u32 = 7u;
const COUNTER_KILLED: u32 = 11u;
const GRID_BLOCK_SPAN: u32 = 512u;
const GRID_BLOCK_SHIFT: u32 = 9u;
const INVALID_KEY: u32 = 0xffffffffu;

var<workgroup> sort_keys: array<u32, 512>;

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

fn increment(value: u32) -> u32 {
    return select(value + 1u, value, value == 0xffffffffu);
}

@compute @workgroup_size(64)
fn clear_grid(@builtin(global_invocation_id) invocation: vec3u) {
    let cell_count = params.counts.z * params.counts.w;
    if invocation.x < cell_count {
        atomicStore(&cell_counts[invocation.x], 0u);
    }
}

@compute @workgroup_size(64)
fn count_grid(
    @builtin(local_invocation_id) local: vec3u,
    @builtin(workgroup_id) group: vec3u,
) {
    let active_count = atomicLoad(&counters[COUNTER_ACTIVE]);
    for (var item = 0u; item < GRID_BLOCK_SPAN / 64u; item += 1u) {
        let local_index = item * 64u + local.x;
        let active_index = group.x * GRID_BLOCK_SPAN + local_index;
        var key = INVALID_KEY;
        if active_index < active_count {
            let index = active_indices[active_index];
            if atomicLoad(&active_flags[index]) != 0u {
                if positions[index].y < params.grid.w {
                    atomicStore(&active_flags[index], 0u);
                    let previous_metadata = metadata[index];
                    metadata[index] = vec4u(
                        increment(previous_metadata.x),
                        previous_metadata.y,
                        previous_metadata.z,
                        0u,
                    );
                    atomicAdd(&counters[COUNTER_KILLED], 1u);
                } else {
                    let cell = cell_for(positions[index].xy);
                    if !cell_is_valid(cell) {
                        atomicStore(&active_flags[index], 0u);
                        let previous_metadata = metadata[index];
                        metadata[index] = vec4u(
                            increment(previous_metadata.x),
                            previous_metadata.y,
                            previous_metadata.z,
                            0u,
                        );
                        atomicAdd(&counters[COUNTER_ESCAPED], 1u);
                        atomicAdd(&counters[COUNTER_KILLED], 1u);
                        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 2u);
                    } else {
                        let target_cell = cell_index(cell);
                        let previous_metadata = metadata[index];
                        metadata[index] = vec4u(
                            previous_metadata.x,
                            previous_metadata.y,
                            target_cell,
                            previous_metadata.w,
                        );
                        atomicAdd(&cell_counts[target_cell], 1u);
                        atomicAdd(&counters[COUNTER_GRID_INSERTS], 1u);
                        key = (target_cell << GRID_BLOCK_SHIFT) | local_index;
                    }
                }
            }
        }
        sort_keys[local_index] = key;
    }
    workgroupBarrier();

    var width = 2u;
    loop {
        if width > GRID_BLOCK_SPAN {
            break;
        }
        var stride = width / 2u;
        loop {
            for (var item = 0u; item < GRID_BLOCK_SPAN / 64u; item += 1u) {
                let index = item * 64u + local.x;
                let other = index ^ stride;
                if other > index {
                    let ascending = (index & width) == 0u;
                    let left = sort_keys[index];
                    let right = sort_keys[other];
                    if (left > right) == ascending {
                        sort_keys[index] = right;
                        sort_keys[other] = left;
                    }
                }
            }
            workgroupBarrier();
            if stride == 1u {
                break;
            }
            stride /= 2u;
        }
        width *= 2u;
    }

    let block_base = group.x * GRID_BLOCK_SPAN;
    for (var item = 0u; item < GRID_BLOCK_SPAN / 64u; item += 1u) {
        let local_index = item * 64u + local.x;
        cell_block_keys[block_base + local_index] = sort_keys[local_index];
    }
}
