struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read> active_indices: array<u32>;
@group(0) @binding(3) var<storage, read> cell_counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> cell_offsets: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> cell_records: array<u32>;
@group(0) @binding(6) var<storage, read> cell_block_keys: array<u32>;
@group(0) @binding(7) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(8) var<uniform> params: ParticleParams;

const COUNTER_ACTIVE: u32 = 0u;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_GRID_OVERFLOW: u32 = 3u;
const GRID_BLOCK_SPAN: u32 = 512u;
const GRID_BLOCK_SHIFT: u32 = 9u;

var<workgroup> scan_values: array<u32, 256>;

fn lower_bound(block: u32, key: u32) -> u32 {
    let begin = block * GRID_BLOCK_SPAN;
    var low = 0u;
    var high = GRID_BLOCK_SPAN;
    loop {
        if low >= high {
            break;
        }
        let middle = low + (high - low) / 2u;
        if cell_block_keys[begin + middle] < key {
            low = middle + 1u;
        } else {
            high = middle;
        }
    }
    return low;
}

@compute @workgroup_size(256)
fn scan_grid_ranges(
    @builtin(local_invocation_id) local: vec3u,
) {
    let cell_count = params.counts.z * params.counts.w;
    let cells_per_lane = (cell_count + 255u) / 256u;
    let lane_begin = local.x * cells_per_lane;
    let lane_end = min(lane_begin + cells_per_lane, cell_count);
    var lane_sum = 0u;
    for (var cell = lane_begin; cell < lane_end; cell += 1u) {
        lane_sum += atomicLoad(&cell_counts[cell]);
    }
    scan_values[local.x] = lane_sum;
    workgroupBarrier();
    var offset = 1u;
    loop {
        if offset >= 256u {
            break;
        }
        var addition = 0u;
        if local.x >= offset {
            addition = scan_values[local.x - offset];
        }
        workgroupBarrier();
        scan_values[local.x] += addition;
        workgroupBarrier();
        offset *= 2u;
    }

    var running = scan_values[local.x] - lane_sum;
    for (var cell = lane_begin; cell < lane_end; cell += 1u) {
        atomicStore(&cell_offsets[cell], running);
        running += atomicLoad(&cell_counts[cell]);
    }
    if local.x == 255u && scan_values[local.x] > params.limits.x {
        atomicAdd(&counters[COUNTER_GRID_OVERFLOW], 1u);
        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 4u);
    }
}

@compute @workgroup_size(64)
fn scatter_grid(
    @builtin(local_invocation_id) local: vec3u,
    @builtin(workgroup_id) group: vec3u,
) {
    let active_count = atomicLoad(&counters[COUNTER_ACTIVE]);
    let cell_count = params.counts.z * params.counts.w;
    for (var item = 0u; item < GRID_BLOCK_SPAN / 64u; item += 1u) {
        let local_index = item * 64u + local.x;
        let active_index = group.x * GRID_BLOCK_SPAN + local_index;
        if active_index >= active_count {
            continue;
        }
        let index = active_indices[active_index];
        if atomicLoad(&active_flags[index]) == 0u {
            continue;
        }
        let cell = metadata[index].z;
        if cell >= cell_count {
            atomicAdd(&counters[COUNTER_GRID_OVERFLOW], 1u);
            atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 4u);
            continue;
        }

        let cell_key = cell << GRID_BLOCK_SHIFT;
        var rank = 0u;
        for (var block = 0u; block < group.x; block += 1u) {
            rank += lower_bound(block, cell_key + GRID_BLOCK_SPAN) -
                lower_bound(block, cell_key);
        }
        let own_key = cell_key | local_index;
        rank += lower_bound(group.x, own_key) - lower_bound(group.x, cell_key);
        let destination = atomicLoad(&cell_offsets[cell]) + rank;
        if destination < params.limits.x {
            cell_records[destination] = index;
        } else {
            atomicAdd(&counters[COUNTER_GRID_OVERFLOW], 1u);
            atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 4u);
        }
    }
}
