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

struct CurrentRecord {
    key0: vec4u,
    key1: vec4u,
    cache: vec4u,
    location: vec4u,
}

struct PersistentRecord {
    key0: vec4u,
    key1: vec4u,
    cache: vec4u,
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
@group(0) @binding(2) var<storage, read> sorted_current: array<CurrentRecord>;
@group(0) @binding(3) var<storage, read_write> cache: array<PersistentRecord>;
@group(0) @binding(4) var<storage, read_write> cache_capacity: array<atomic<u32>>;
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
fn build_solver_points(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= params.counts.x) { return; }
    solver_points[index] = invalid_point();
    if (index >= atomicLoad(&cache_capacity[2])) { return; }
    let current = sorted_current[index];
    let manifold = manifolds[current.location.y];
    let point = points[current.location.x];
    let persistent = cache[index];
    var output: PointInput;
    output.ids = vec4u(manifold.identities.x, manifold.identities.z,
                       current.location.y, current.location.z);
    output.normal = manifold.normal_material.xy;
    output.local_anchor_a = point.anchors.xy;
    output.local_anchor_b = point.anchors.zw;
    output.padding0 = vec2f(bitcast<f32>(persistent.cache.w), 0.0);
    output.material = vec4f(point.world_separation.z,
                            manifold.normal_material.zw, 0.0);
    output.cache = vec2f(bitcast<f32>(persistent.cache.x),
                         bitcast<f32>(persistent.cache.y));
    output.padding1 = bitcast<vec2f>(manifold.identities.yw);
    solver_points[index] = output;
}

@compute @workgroup_size(64)
fn store_solver_cache(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= atomicLoad(&cache_capacity[2])) { return; }
    var persistent = cache[index];
    let prepared = prepared_points[index];
    persistent.cache.x = bitcast<u32>(prepared.data1.z);
    persistent.cache.y = bitcast<u32>(prepared.data1.w);
    cache[index] = persistent;
}
