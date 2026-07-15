struct CandidateParams {
    counts: vec4u,
    execution: vec4u,
}

struct BodyAabb {
    identity_flags: vec4u,
    lower_upper: vec4f,
}

struct CandidatePair {
    identities: vec4u,
}

@group(0) @binding(0) var<storage, read> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read> properties: array<vec4f>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> aabbs: array<BodyAabb>;
@group(0) @binding(4) var<storage, read_write> pair_flags: array<u32>;
@group(0) @binding(5) var<storage, read_write> pair_offsets: array<u32>;
@group(0) @binding(6) var<storage, read_write> pair_scratch: array<CandidatePair>;
@group(0) @binding(7) var<storage, read_write> candidates: array<CandidatePair>;
@group(0) @binding(8) var<storage, read_write> scan_blocks: array<u32>;
@group(0) @binding(9) var<storage, read_write> work: array<atomic<u32>>;
@group(0) @binding(10) var<storage, read_write> capacity: array<atomic<u32>>;
@group(0) @binding(11) var<uniform> params: CandidateParams;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;

var<workgroup> scan_values: array<u32, 256>;

@compute @workgroup_size(64)
fn clear_candidate_state(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < params.counts.y) {
        pair_flags[index] = 0u;
        pair_offsets[index] = 0u;
    }
    if (index < 32u) {
        atomicStore(&work[index], 0u);
    }
    if (index < 4u) {
        atomicStore(&capacity[index], 0u);
    }
}

@compute @workgroup_size(64)
fn compute_body_aabbs(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= params.counts.x) {
        return;
    }
    let identity = metadata[index];
    let flags = identity.y;
    if ((flags & FLAG_LIVE) == 0u) {
        aabbs[index].identity_flags = vec4u(index, identity.x, flags, 0u);
        aabbs[index].lower_upper = vec4f(0.0);
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
    let lower = pose.xy - extent;
    let upper = pose.xy + extent;
    aabbs[index].identity_flags = vec4u(index, identity.x, flags, 0u);
    aabbs[index].lower_upper = vec4f(lower, upper);
}

fn triangular_pair_index(body_a: u32, body_b: u32, body_count: u32) -> u32 {
    return body_a * (2u * body_count - body_a - 1u) / 2u + (body_b - body_a - 1u);
}

@compute @workgroup_size(8, 8)
fn classify_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let body_a = invocation.x;
    let body_b = invocation.y;
    if (body_a >= params.counts.x || body_b >= params.counts.x || body_a >= body_b) {
        return;
    }
    let identity_a = metadata[body_a];
    let identity_b = metadata[body_b];
    if ((identity_a.y & FLAG_LIVE) == 0u || (identity_b.y & FLAG_LIVE) == 0u) {
        return;
    }
    let pair_index = triangular_pair_index(body_a, body_b, params.counts.x);
    atomicAdd(&work[1], 1u);
    pair_scratch[pair_index].identities = vec4u(body_a, identity_a.x, body_b, identity_b.x);
    if ((identity_a.y & FLAG_FIXED) != 0u && (identity_b.y & FLAG_FIXED) != 0u) {
        atomicAdd(&work[2], 1u);
        return;
    }
    let bounds_a = aabbs[body_a].lower_upper;
    let bounds_b = aabbs[body_b].lower_upper;
    let overlaps = bounds_a.x <= bounds_b.z && bounds_b.x <= bounds_a.z
        && bounds_a.y <= bounds_b.w && bounds_b.y <= bounds_a.w;
    if (!overlaps) {
        atomicAdd(&work[3], 1u);
        return;
    }
    pair_flags[pair_index] = 1u;
}

@compute @workgroup_size(256)
fn scan_pair_blocks(@builtin(global_invocation_id) global: vec3u,
                    @builtin(local_invocation_id) local: vec3u,
                    @builtin(workgroup_id) group: vec3u) {
    let index = global.x;
    var value = 0u;
    if (index < params.counts.y) {
        value = pair_flags[index];
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
    if (index < params.counts.y) {
        pair_offsets[index] = scan_values[local.x] - value;
    }
    if (local.x == 255u) {
        scan_blocks[group.x] = scan_values[local.x];
    }
}

@compute @workgroup_size(1)
fn scan_pair_block_totals(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    for (var block = 0u; block < params.counts.w; block++) {
        let value = scan_blocks[block];
        scan_blocks[block] = running;
        running += value;
    }
}

@compute @workgroup_size(256)
fn add_pair_block_offsets(@builtin(global_invocation_id) global: vec3u,
                          @builtin(workgroup_id) group: vec3u) {
    if (global.x >= params.counts.y) { return; }
    pair_offsets[global.x] += scan_blocks[group.x];
}

@compute @workgroup_size(1)
fn finalize_candidate_counts(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let last = params.counts.y - 1u;
    let attempted = pair_offsets[last] + pair_flags[last];
    let overflowed = attempted > params.counts.z;
    let committed = select(attempted, 0u, overflowed);
    atomicStore(&capacity[0], params.counts.z);
    atomicStore(&capacity[1], attempted);
    atomicStore(&capacity[2], committed);
    atomicStore(&capacity[3], select(0u, 1u, overflowed));
    atomicStore(&work[0], params.execution.x);
    atomicStore(&work[4], attempted);
    atomicStore(&work[15], params.execution.y);
}

@compute @workgroup_size(64)
fn scatter_candidates(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    if (pair_index >= params.counts.y || atomicLoad(&capacity[3]) != 0u) {
        return;
    }
    if (pair_flags[pair_index] != 0u) {
        candidates[pair_offsets[pair_index]] = pair_scratch[pair_index];
    }
}
