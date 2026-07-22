struct PointInput {
    ids: vec4u,
    normal: vec2f,
    local_anchor_a: vec2f,
    local_anchor_b: vec2f,
    padding0: vec2f,
    material: vec4f,
    cache: vec2f,
    generations: vec2f,
}

struct Params {
    // body capacity, point capacity, one-based phase, phase bound
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read> points: array<PointInput>;
@group(0) @binding(2) var<storage, read_write> parents: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> sizes: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> stats: array<atomic<u32>>;
@group(0) @binding(5) var<uniform> params: Params;

const INVALID: u32 = 0xffffffffu;
const FLAG_LIVE: u32 = 1u;
const FLAG_FIXED: u32 = 4u;

fn is_live(body: u32) -> bool {
    return body < params.counts.x && (metadata[body].y & FLAG_LIVE) != 0u;
}

fn is_dynamic(body: u32) -> bool {
    return is_live(body) && (metadata[body].y & FLAG_FIXED) == 0u;
}

fn valid_point_edge(point: PointInput) -> bool {
    if (point.ids.w != 0u || point.ids.x == INVALID || point.ids.y == INVALID) {
        return false;
    }
    if (!is_live(point.ids.x) || !is_live(point.ids.y) || point.ids.x == point.ids.y) {
        return false;
    }
    let generations = bitcast<vec2u>(point.generations);
    return metadata[point.ids.x].x == generations.x &&
        metadata[point.ids.y].x == generations.y;
}

@compute @workgroup_size(64)
fn initialize(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (body >= params.counts.x) { return; }
    if (body == 0u) {
        atomicStore(&stats[11], params.counts.w);
        atomicStore(&stats[12], params.counts.x);
        atomicStore(&stats[13], params.counts.y);
    }
    if (is_dynamic(body)) {
        atomicStore(&parents[body], body);
        atomicAdd(&stats[0], 1u);
    } else {
        atomicStore(&parents[body], INVALID);
    }
}

@compute @workgroup_size(64)
fn hook(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= params.counts.y) { return; }
    let point = points[point_index];
    if (!valid_point_edge(point)) { return; }
    let body_a = point.ids.x;
    let body_b = point.ids.y;
    if (!is_dynamic(body_a) || !is_dynamic(body_b)) { return; }
    if (params.counts.z == 1u) {
        atomicAdd(&stats[1], 1u);
    }
    let parent_a = atomicLoad(&parents[body_a]);
    let parent_b = atomicLoad(&parents[body_b]);
    if (parent_a == INVALID || parent_b == INVALID || parent_a == parent_b) { return; }
    let high = max(parent_a, parent_b);
    let low = min(parent_a, parent_b);
    atomicAdd(&stats[2], 1u);
    let previous = atomicMin(&parents[high], low);
    if (previous > low) {
        atomicAdd(&stats[3], 1u);
        atomicMax(&stats[5], params.counts.z);
    }
}

@compute @workgroup_size(64)
fn shortcut(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (body >= params.counts.x) { return; }
    let parent = atomicLoad(&parents[body]);
    if (parent == INVALID) { return; }
    if (parent >= params.counts.x) {
        atomicAdd(&stats[8], 1u);
        return;
    }
    let grandparent = atomicLoad(&parents[parent]);
    if (grandparent == INVALID || grandparent >= params.counts.x) {
        atomicAdd(&stats[8], 1u);
        return;
    }
    if (grandparent < parent) {
        atomicStore(&parents[body], grandparent);
        atomicAdd(&stats[4], 1u);
        atomicMax(&stats[5], params.counts.z);
    }
}

@compute @workgroup_size(64)
fn finalize(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (body >= params.counts.x || !is_dynamic(body)) { return; }
    let root = atomicLoad(&parents[body]);
    if (root >= params.counts.x || !is_dynamic(root) || atomicLoad(&parents[root]) != root) {
        atomicAdd(&stats[8], 1u);
        return;
    }
    if (root == body) {
        atomicAdd(&stats[6], 1u);
    }
    let component_size = atomicAdd(&sizes[root], 1u) + 1u;
    atomicMax(&stats[7], component_size);
}

@compute @workgroup_size(64)
fn validate_edges(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= params.counts.y) { return; }
    let point = points[point_index];
    if (point.ids.w != 0u || point.ids.x == INVALID) { return; }
    if (!valid_point_edge(point)) {
        atomicAdd(&stats[10], 1u);
        return;
    }
    if (!is_dynamic(point.ids.x) || !is_dynamic(point.ids.y)) { return; }
    if (atomicLoad(&parents[point.ids.x]) != atomicLoad(&parents[point.ids.y])) {
        atomicAdd(&stats[9], 1u);
    }
}
