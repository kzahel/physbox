struct Params {
    count: u32,
    block_count: u32,
    shift: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read_write> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> destination: array<u32>;
@group(0) @binding(2) var<storage, read_write> aux_a: array<u32>;
@group(0) @binding(3) var<storage, read_write> aux_b: array<u32>;
@group(0) @binding(4) var<storage, read_write> blocks: array<u32>;
@group(0) @binding(5) var<storage, read_write> metadata: array<u32>;
@group(0) @binding(6) var<uniform> params: Params;

var<workgroup> scan_values: array<u32, 256>;
var<workgroup> radix_digits: array<u32, 256>;
var<workgroup> radix_histogram: array<atomic<u32>, 16>;

@compute @workgroup_size(256)
fn scan_blocks(@builtin(global_invocation_id) global: vec3u,
               @builtin(local_invocation_id) local: vec3u,
               @builtin(workgroup_id) group: vec3u) {
    let index = global.x;
    var value = 0u;
    if (index < params.count) {
        value = source[index];
    }
    scan_values[local.x] = value;
    workgroupBarrier();
    var offset = 1u;
    loop {
        if (offset >= 256u) { break; }
        var addition = 0u;
        if (local.x >= offset) {
            addition = scan_values[local.x - offset];
        }
        workgroupBarrier();
        scan_values[local.x] += addition;
        workgroupBarrier();
        offset *= 2u;
    }
    if (index < params.count) {
        destination[index] = scan_values[local.x] - value;
    }
    if (local.x == 255u) {
        blocks[group.x] = scan_values[local.x];
    }
}

@compute @workgroup_size(1)
fn scan_block_totals(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    var running = 0u;
    for (var block = 0u; block < params.block_count; block++) {
        let value = blocks[block];
        blocks[block] = running;
        running += value;
    }
}

@compute @workgroup_size(256)
fn add_block_offsets(@builtin(global_invocation_id) global: vec3u,
                     @builtin(workgroup_id) group: vec3u) {
    if (global.x >= params.count) { return; }
    destination[global.x] += blocks[group.x];
}

@compute @workgroup_size(256)
fn radix_histograms(@builtin(global_invocation_id) global: vec3u,
                    @builtin(local_invocation_id) local: vec3u,
                    @builtin(workgroup_id) group: vec3u) {
    if (local.x < 16u) {
        atomicStore(&radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    if (global.x < params.count) {
        let digit = (source[global.x] >> params.shift) & 15u;
        atomicAdd(&radix_histogram[digit], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        blocks[group.x * 16u + local.x] = atomicLoad(&radix_histogram[local.x]);
    }
}

@compute @workgroup_size(1)
fn radix_prefix(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    var running = 0u;
    for (var digit = 0u; digit < 16u; digit++) {
        for (var block = 0u; block < params.block_count; block++) {
            let index = block * 16u + digit;
            let count = blocks[index];
            blocks[index] = running;
            running += count;
        }
    }
}

@compute @workgroup_size(256)
fn radix_scatter(@builtin(global_invocation_id) global: vec3u,
                 @builtin(local_invocation_id) local: vec3u,
                 @builtin(workgroup_id) group: vec3u) {
    var digit = 16u;
    if (global.x < params.count) {
        digit = (source[global.x] >> params.shift) & 15u;
    }
    radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= params.count) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, radix_digits[lane] == digit);
    }
    let output = blocks[group.x * 16u + digit] + local_rank;
    destination[output] = source[global.x];
}

@compute @workgroup_size(256)
fn unique_flags(@builtin(global_invocation_id) global: vec3u) {
    if (global.x >= params.count) { return; }
    var flag = 0u;
    if (global.x == 0u || source[global.x] != source[global.x - 1u]) {
        flag = 1u;
    }
    aux_a[global.x] = flag;
}

@compute @workgroup_size(256)
fn compaction_flags(@builtin(global_invocation_id) global: vec3u) {
    if (global.x >= params.count) { return; }
    aux_a[global.x] = select(0u, 1u, source[global.x] != 0u);
}

@compute @workgroup_size(256)
fn flagged_scatter(@builtin(global_invocation_id) global: vec3u) {
    if (global.x >= params.count) { return; }
    if (aux_a[global.x] != 0u) {
        destination[aux_b[global.x]] = source[global.x];
    }
    if (global.x + 1u == params.count) {
        metadata[0] = aux_b[global.x] + aux_a[global.x];
    }
}
