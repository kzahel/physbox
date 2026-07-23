struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read_write> active_indices: array<u32>;
@group(0) @binding(2) var<storage, read_write> active_prefixes: array<u32>;
@group(0) @binding(3) var<storage, read_write> active_block_offsets: array<u32>;
@group(0) @binding(4) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> indirect: array<u32>;
@group(0) @binding(6) var<uniform> params: ParticleParams;

const COUNTER_ACTIVE: u32 = 0u;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;

var<workgroup> scan_values: array<u32, 256>;

@compute @workgroup_size(256)
fn count_active(
    @builtin(global_invocation_id) invocation: vec3u,
    @builtin(local_invocation_id) local: vec3u,
    @builtin(workgroup_id) group: vec3u,
) {
    let index = invocation.x;
    var is_active = 0u;
    if index < params.counts.x && atomicLoad(&active_flags[index]) != 0u {
        is_active = 1u;
    }
    scan_values[local.x] = is_active;
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
    if index < params.counts.x {
        active_prefixes[index] = scan_values[local.x] - is_active;
    }
    if local.x == 255u {
        active_block_offsets[group.x] = scan_values[local.x];
    }
}

@compute @workgroup_size(64)
fn scan_active_blocks(@builtin(global_invocation_id) invocation: vec3u) {
    if invocation.x != 0u {
        return;
    }
    let block_count = (params.counts.x + 255u) / 256u;
    var active_count = 0u;
    for (var block = 0u; block < block_count; block += 1u) {
        let count = active_block_offsets[block];
        active_block_offsets[block] = active_count;
        active_count += count;
    }
    atomicStore(&counters[COUNTER_ACTIVE], active_count);
    indirect[0] = 6u;
    indirect[1] = active_count;
    indirect[2] = 0u;
    indirect[3] = 0u;
    indirect[4] = (active_count + 511u) / 512u;
    indirect[5] = 1u;
    indirect[6] = 1u;
    indirect[7] = 0u;
}

@compute @workgroup_size(256)
fn scatter_active(
    @builtin(global_invocation_id) invocation: vec3u,
    @builtin(workgroup_id) group: vec3u,
) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let destination = active_block_offsets[group.x] + active_prefixes[index];
    if destination < params.counts.x {
        active_indices[destination] = index;
    } else {
        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 1u);
    }
}
