struct Params {
    counts: vec4u,
    radix: vec4u,
    execution: vec4u,
    policy: vec4f,
}

struct Proxy {
    identity_mobility: vec4u,
    lower_upper: vec4f,
}

struct PairIndex {
    values: vec4u,
}

struct CandidatePair {
    identities: vec4u,
}

@group(0) @binding(0) var<storage, read_write> input_proxies: array<Proxy>;
@group(0) @binding(1) var<storage, read_write> proxies_source: array<Proxy>;
@group(0) @binding(2) var<storage, read_write> proxies_destination: array<Proxy>;
@group(0) @binding(3) var<storage, read_write> proxy_radix_blocks: array<u32>;
@group(0) @binding(4) var<storage, read_write> pairs_source: array<PairIndex>;
@group(0) @binding(5) var<storage, read_write> pairs_destination: array<PairIndex>;
@group(0) @binding(6) var<storage, read_write> pair_radix_blocks: array<u32>;
@group(0) @binding(7) var<storage, read_write> candidates: array<CandidatePair>;
@group(0) @binding(8) var<storage, read_write> work: array<atomic<u32>>;
@group(0) @binding(9) var<storage, read_write> capacity: array<atomic<u32>>;
@group(0) @binding(10) var<uniform> params: Params;
@group(0) @binding(11) var<storage, read_write> indirect: array<atomic<u32>>;
@group(0) @binding(12) var<storage, read_write> collision_work: array<atomic<u32>>;
@group(0) @binding(13) var<storage, read> poses: array<vec4f>;
@group(0) @binding(14) var<storage, read> properties: array<vec4f>;
@group(0) @binding(15) var<storage, read> metadata: array<vec4u>;

const MOBILITY_FIXED: u32 = 1u;
const MOBILITY_LIVE: u32 = 2u;
const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const SENTINEL: u32 = 0xffffffffu;

var<workgroup> radix_digits: array<u32, 256>;
var<workgroup> radix_histogram: array<atomic<u32>, 16>;

fn ordered_float_key(value: f32) -> u32 {
    let bits = bitcast<u32>(value);
    return select(bits ^ 0x80000000u, ~bits, (bits & 0x80000000u) != 0u);
}

fn pair_key(pair: PairIndex) -> u32 {
    return select(pair.values.y, pair.values.x, params.radix.x != 0u);
}

@compute @workgroup_size(64)
fn clear_sap_state(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < 16u) {
        atomicStore(&work[index], 0u);
    }
    if (index < 4u) {
        atomicStore(&capacity[index], 0u);
    }
    if (index < 6u) {
        atomicStore(&indirect[index], 0u);
    }
    if (index < 32u) {
        atomicStore(&collision_work[index], 0u);
    }
}

@compute @workgroup_size(64)
fn compute_sap_body_aabbs(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= params.counts.x) { return; }
    let identity = metadata[index];
    let flags = identity.y;
    if ((flags & FLAG_LIVE) == 0u) {
        input_proxies[index].identity_mobility = vec4u(index, identity.x, 0u, index);
        input_proxies[index].lower_upper = vec4f(bitcast<f32>(0x7f800000u));
        return;
    }
    let pose = poses[index];
    let shape = properties[index].xy;
    var extent = shape;
    if ((flags & FLAG_RECTANGLE) != 0u) {
        let c = abs(cos(pose.z));
        let s = abs(sin(pose.z));
        extent = vec2f(c * shape.x + s * shape.y,
                       s * shape.x + c * shape.y);
    } else if ((flags & FLAG_CIRCLE) != 0u) {
        extent = vec2f(shape.x);
    }
    let expanded_extent = extent + vec2f(params.policy.x);
    let lower = pose.xy - expanded_extent;
    let upper = pose.xy + expanded_extent;
    let mobility = MOBILITY_LIVE |
        select(0u, MOBILITY_FIXED, (flags & FLAG_FIXED) != 0u);
    input_proxies[index].identity_mobility = vec4u(index, identity.x, mobility, index);
    input_proxies[index].lower_upper = vec4f(lower, upper);
}

@compute @workgroup_size(256)
fn proxy_radix_histograms(@builtin(global_invocation_id) global: vec3u,
                          @builtin(local_invocation_id) local: vec3u,
                          @builtin(workgroup_id) group: vec3u) {
    if (local.x < 16u) {
        atomicStore(&radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    if (global.x < params.counts.x) {
        let key = ordered_float_key(proxies_source[global.x].lower_upper.x);
        atomicAdd(&radix_histogram[(key >> params.radix.y) & 15u], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        proxy_radix_blocks[group.x * 16u + local.x] =
            atomicLoad(&radix_histogram[local.x]);
    }
}

@compute @workgroup_size(1)
fn proxy_radix_prefix(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    for (var digit = 0u; digit < 16u; digit++) {
        for (var block = 0u; block < params.counts.y; block++) {
            let index = block * 16u + digit;
            let count = proxy_radix_blocks[index];
            proxy_radix_blocks[index] = running;
            running += count;
        }
    }
}

@compute @workgroup_size(256)
fn proxy_radix_scatter(@builtin(global_invocation_id) global: vec3u,
                       @builtin(local_invocation_id) local: vec3u,
                       @builtin(workgroup_id) group: vec3u) {
    var digit = 16u;
    if (global.x < params.counts.x) {
        let key = ordered_float_key(proxies_source[global.x].lower_upper.x);
        digit = (key >> params.radix.y) & 15u;
    }
    radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= params.counts.x) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, radix_digits[lane] == digit);
    }
    let output = proxy_radix_blocks[group.x * 16u + digit] + local_rank;
    proxies_destination[output] = proxies_source[global.x];
}

@compute @workgroup_size(64)
fn enumerate_sap_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let index_a = invocation.x;
    if (index_a >= params.counts.x) { return; }
    let proxy_a = proxies_source[index_a];
    if ((proxy_a.identity_mobility.z & MOBILITY_LIVE) == 0u) { return; }
    var window_count = 0u;
    for (var index_b = index_a + 1u; index_b < params.counts.x; index_b++) {
        let proxy_b = proxies_source[index_b];
        if (proxy_b.lower_upper.x > proxy_a.lower_upper.z) { break; }
        window_count += 1u;
        atomicAdd(&work[3], 1u);
        if ((proxy_a.identity_mobility.z & MOBILITY_FIXED) != 0u &&
            (proxy_b.identity_mobility.z & MOBILITY_FIXED) != 0u) {
            atomicAdd(&work[4], 1u);
            continue;
        }
        let overlaps_y = proxy_a.lower_upper.y <= proxy_b.lower_upper.w &&
            proxy_b.lower_upper.y <= proxy_a.lower_upper.w;
        if (!overlaps_y) { continue; }
        let output = atomicAdd(&work[5], 1u);
        if (output < params.counts.z) {
            let ordinal_a = proxy_a.identity_mobility.w;
            let ordinal_b = proxy_b.identity_mobility.w;
            pairs_source[output].values = vec4u(min(ordinal_a, ordinal_b),
                                                 max(ordinal_a, ordinal_b), 0u, 0u);
        }
    }
    atomicMax(&work[12], window_count);
}

@compute @workgroup_size(1)
fn finalize_sap_counts(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let attempted = atomicLoad(&work[5]);
    let overflowed = attempted > params.counts.z;
    atomicStore(&capacity[0], params.counts.z);
    atomicStore(&capacity[1], attempted);
    atomicStore(&capacity[2], select(attempted, 0u, overflowed));
    atomicStore(&capacity[3], select(0u, 1u, overflowed));
    let committed = select(attempted, 0u, overflowed);
    atomicStore(&indirect[0], (committed + 255u) / 256u);
    atomicStore(&indirect[1], 1u);
    atomicStore(&indirect[2], 1u);
    atomicStore(&indirect[3], (committed + 63u) / 64u);
    atomicStore(&indirect[4], 1u);
    atomicStore(&indirect[5], 1u);
    atomicStore(&work[0], params.counts.x);
    atomicStore(&work[1], params.execution.x);
    atomicStore(&work[2], params.execution.y);
    atomicStore(&work[6], 0u);
    atomicStore(&work[7], attempted);
    atomicStore(&work[8], params.counts.x + committed);
    atomicStore(&work[9], params.radix.z + params.radix.w);
    atomicStore(&work[14], params.execution.z);
    let pair_tests = atomicLoad(&work[3]);
    let fixed_fixed = atomicLoad(&work[4]);
    atomicStore(&collision_work[0], params.execution.x + params.execution.y);
    atomicStore(&collision_work[1], pair_tests);
    atomicStore(&collision_work[2], fixed_fixed);
    atomicStore(&collision_work[3], pair_tests - fixed_fixed - attempted);
    atomicStore(&collision_work[4], attempted);
    atomicStore(&collision_work[15], params.execution.z);
}

@compute @workgroup_size(256)
fn pair_radix_histograms(@builtin(global_invocation_id) global: vec3u,
                         @builtin(local_invocation_id) local: vec3u,
                         @builtin(workgroup_id) group: vec3u) {
    if (local.x < 16u) {
        atomicStore(&radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    let committed = atomicLoad(&capacity[2]);
    if (global.x < committed) {
        let key = pair_key(pairs_source[global.x]);
        atomicAdd(&radix_histogram[(key >> params.radix.y) & 15u], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        pair_radix_blocks[group.x * 16u + local.x] =
            atomicLoad(&radix_histogram[local.x]);
    }
}

@compute @workgroup_size(1)
fn pair_radix_prefix(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    let block_count = (atomicLoad(&capacity[2]) + 255u) / 256u;
    for (var digit = 0u; digit < 16u; digit++) {
        for (var block = 0u; block < block_count; block++) {
            let index = block * 16u + digit;
            let count = pair_radix_blocks[index];
            pair_radix_blocks[index] = running;
            running += count;
        }
    }
}

@compute @workgroup_size(256)
fn pair_radix_scatter(@builtin(global_invocation_id) global: vec3u,
                      @builtin(local_invocation_id) local: vec3u,
                      @builtin(workgroup_id) group: vec3u) {
    var digit = 16u;
    let committed = atomicLoad(&capacity[2]);
    if (global.x < committed) {
        digit = (pair_key(pairs_source[global.x]) >> params.radix.y) & 15u;
    }
    radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= committed) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, radix_digits[lane] == digit);
    }
    let output = pair_radix_blocks[group.x * 16u + digit] + local_rank;
    pairs_destination[output] = pairs_source[global.x];
}

@compute @workgroup_size(64)
fn materialize_sap_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    let committed = atomicLoad(&capacity[2]);
    if (index >= committed || atomicLoad(&capacity[3]) != 0u) { return; }
    let pair = pairs_source[index].values;
    let body_a = input_proxies[pair.x].identity_mobility;
    let body_b = input_proxies[pair.y].identity_mobility;
    candidates[index].identities = vec4u(body_a.x, body_a.y, body_b.x, body_b.y);
}
