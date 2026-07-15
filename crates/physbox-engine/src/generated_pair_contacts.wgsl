struct BridgeParams {
    counts: vec4u,
}

struct ManifoldRecord {
    identities: vec4u,
    normal_material: vec4f,
    point_range: vec4u,
}

struct ContactPoint {
    anchors: vec4f,
    world_separation: vec4f,
    features: vec4u,
}

struct PairRecord {
    key: vec4u,
    state: vec4u,
    features0: vec4u,
    cache0: vec4u,
    features1: vec4u,
    cache1: vec4u,
}

struct PointInput {
    ids: vec4u,
    normal: vec2f,
    local_anchor_a: vec2f,
    local_anchor_b: vec2f,
    padding0: vec2f,
    material: vec4f,
    cache: vec2f,
    padding1: vec2f,
}

struct PreparedPoint {
    ids: vec4u,
    normal: vec2f,
    tangent: vec2f,
    anchor_a: vec2f,
    anchor_b: vec2f,
    data0: vec4f,
    data1: vec4f,
    data2: vec4f,
}

@group(0) @binding(0) var<storage, read> manifolds: array<ManifoldRecord>;
@group(0) @binding(1) var<storage, read> points: array<ContactPoint>;
@group(0) @binding(2) var<storage, read_write> pair_records: array<PairRecord>;
@group(0) @binding(3) var<storage, read_write> pair_capacity: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> point_capacity: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> solver_points: array<PointInput>;
@group(0) @binding(6) var<storage, read> prepared_points: array<PreparedPoint>;
@group(0) @binding(7) var<uniform> params: BridgeParams;

fn invalid_point() -> PointInput {
    var output: PointInput;
    output.ids = vec4u(0xffffffffu);
    output.normal = vec2f(0.0);
    output.local_anchor_a = vec2f(0.0);
    output.local_anchor_b = vec2f(0.0);
    output.padding0 = vec2f(0.0);
    output.material = vec4f(0.0);
    output.cache = vec2f(0.0);
    output.padding1 = vec2f(0.0);
    return output;
}

@compute @workgroup_size(64)
fn build_pair_solver_points(@builtin(global_invocation_id) invocation: vec3u) {
    let point_index = invocation.x;
    if (point_index >= params.counts.x) { return; }
    solver_points[point_index] = invalid_point();
    let manifold_index = point_index / 2u;
    let local_index = point_index % 2u;
    if (manifold_index >= atomicLoad(&pair_capacity[2])) { return; }
    let manifold = manifolds[manifold_index];
    let pair = pair_records[manifold_index];
    if (local_index >= pair.state.z) { return; }
    let features = select(pair.features0, pair.features1, local_index == 1u);
    var source_index = manifold.point_range.x;
    if (!all(points[source_index].features == features) && manifold.point_range.y >= 2u) {
        source_index += 1u;
    }
    if (!all(points[source_index].features == features)) { return; }
    let point = points[source_index];
    let cache = select(pair.cache0, pair.cache1, local_index == 1u);
    var output: PointInput;
    output.ids = vec4u(
        manifold.identities.x,
        manifold.identities.z,
        manifold_index,
        local_index);
    output.normal = manifold.normal_material.xy;
    output.local_anchor_a = point.anchors.xy;
    output.local_anchor_b = point.anchors.zw;
    output.padding0 = vec2f(0.0);
    output.material = vec4f(
        point.world_separation.z,
        manifold.normal_material.zw,
        0.0);
    output.cache = vec2f(bitcast<f32>(cache.x), bitcast<f32>(cache.y));
    output.padding1 = vec2f(0.0);
    solver_points[point_index] = output;
}

@compute @workgroup_size(64)
fn store_pair_solver_cache(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    if (pair_index >= atomicLoad(&pair_capacity[2])) { return; }
    let manifold = manifolds[pair_index];
    var pair = pair_records[pair_index];
    if (manifold.point_range.y >= 1u) {
        let prepared = prepared_points[pair_index * 2u];
        pair.cache0.x = bitcast<u32>(prepared.data1.z);
        pair.cache0.y = bitcast<u32>(prepared.data1.w);
    }
    if (manifold.point_range.y >= 2u) {
        let prepared = prepared_points[pair_index * 2u + 1u];
        pair.cache1.x = bitcast<u32>(prepared.data1.z);
        pair.cache1.y = bitcast<u32>(prepared.data1.w);
    }
    pair_records[pair_index] = pair;
}
