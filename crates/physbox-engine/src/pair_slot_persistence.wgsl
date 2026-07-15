struct Params {
    counts: vec4u,
    control: vec4u,
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

struct CacheMatch {
    cache: vec4u,
    matched: u32,
}

@group(0) @binding(0) var<storage, read> manifolds: array<ManifoldRecord>;
@group(0) @binding(1) var<storage, read> points: array<ContactPoint>;
@group(0) @binding(2) var<storage, read_write> manifold_capacity: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> point_capacity: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> previous_pairs: array<PairRecord>;
@group(0) @binding(5) var<storage, read_write> previous_capacity: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> output_pairs: array<PairRecord>;
@group(0) @binding(8) var<storage, read_write> new_flags: array<u32>;
@group(0) @binding(9) var<storage, read_write> new_offsets: array<u32>;
@group(0) @binding(10) var<storage, read_write> slot_flags: array<u32>;
@group(0) @binding(11) var<storage, read_write> slot_offsets: array<u32>;
@group(0) @binding(12) var<storage, read_write> new_blocks: array<u32>;
@group(0) @binding(13) var<storage, read_write> free_blocks: array<u32>;
@group(0) @binding(14) var<storage, read_write> free_slots: array<u32>;
@group(0) @binding(15) var<storage, read_write> slot_generations: array<u32>;
@group(0) @binding(16) var<storage, read_write> output_capacity: array<atomic<u32>>;
@group(0) @binding(17) var<storage, read_write> work: array<atomic<u32>>;
@group(0) @binding(18) var<uniform> params: Params;

var<workgroup> scan_new: array<u32, 256>;
var<workgroup> scan_free: array<u32, 256>;

fn invalid_pair() -> PairRecord {
    var record: PairRecord;
    record.key = vec4u(0xffffffffu);
    record.state = vec4u(0xffffffffu, 0u, 0u, 0u);
    record.features0 = vec4u(0xffffffffu);
    record.cache0 = vec4u(0u);
    record.features1 = vec4u(0xffffffffu);
    record.cache1 = vec4u(0u);
    return record;
}

fn pair_less(left: vec4u, right: vec4u) -> bool {
    for (var index = 0u; index < 4u; index++) {
        if (left[index] != right[index]) {
            return left[index] < right[index];
        }
    }
    return false;
}

fn feature_fingerprint(features: vec4u) -> u32 {
    var hash = 0x811c9dc5u;
    for (var index = 0u; index < 4u; index++) {
        hash = (hash ^ features[index]) * 0x01000193u;
    }
    return hash & params.control.z;
}

fn match_cache(features: vec4u, previous: PairRecord) -> CacheMatch {
    let fingerprint = feature_fingerprint(features);
    var result: CacheMatch;
    result.cache = vec4u(0u, 0u, fingerprint, 0u);
    result.matched = 0u;
    if (previous.state.z >= 1u && previous.cache0.z == fingerprint &&
        all(previous.features0 == features)) {
        result.cache = vec4u(previous.cache0.x, previous.cache0.y, fingerprint, 0u);
        result.matched = 1u;
        return result;
    }
    if (previous.state.z >= 2u && previous.cache1.z == fingerprint &&
        all(previous.features1 == features)) {
        result.cache = vec4u(previous.cache1.x, previous.cache1.y, fingerprint, 0u);
        result.matched = 1u;
    }
    return result;
}

@compute @workgroup_size(64)
fn clear_pair_slots(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < params.counts.x) {
        new_flags[index] = 0u;
        new_offsets[index] = 0u;
        slot_flags[index] = 0u;
        slot_offsets[index] = 0u;
    }
    if (index < params.counts.y) {
        new_blocks[index] = 0u;
        free_blocks[index] = 0u;
    }
    if (index < 4u) {
        atomicStore(&output_capacity[index], 0u);
    }
    if (index >= 8u && index <= 15u) {
        atomicStore(&work[index], 0u);
    }
}

@compute @workgroup_size(1)
fn prepare_pair_slots(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let attempted_pairs = atomicLoad(&manifold_capacity[1]);
    let attempted_points = atomicLoad(&point_capacity[1]);
    let intrinsic_overflow = attempted_pairs > params.counts.x ||
        attempted_points > params.counts.x * 2u;
    let upstream_rejected = atomicLoad(&manifold_capacity[3]) != 0u ||
        atomicLoad(&point_capacity[3]) != 0u || atomicLoad(&previous_capacity[3]) != 0u;
    let rejected = intrinsic_overflow || upstream_rejected;
    atomicStore(&output_capacity[0], params.counts.x);
    atomicStore(&output_capacity[1], attempted_pairs);
    atomicStore(&output_capacity[2], select(attempted_pairs, 0u, rejected));
    atomicStore(&output_capacity[3], select(0u, 1u, rejected));
}

@compute @workgroup_size(64)
fn match_current_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let canonical_index = invocation.x;
    let current_count = atomicLoad(&output_capacity[2]);
    if (canonical_index >= current_count) { return; }
    let source_index = select(
        canonical_index,
        current_count - 1u - canonical_index,
        params.control.y != 0u);
    let manifold = manifolds[source_index];

    let previous_count = atomicLoad(&previous_capacity[2]);
    var lower = 0u;
    var upper = previous_count;
    loop {
        if (lower >= upper) { break; }
        let middle = lower + (upper - lower) / 2u;
        if (pair_less(previous_pairs[middle].key, manifold.identities)) {
            lower = middle + 1u;
        } else {
            upper = middle;
        }
    }

    var record = invalid_pair();
    record.key = manifold.identities;
    record.state = vec4u(0xffffffffu, 0u, manifold.point_range.y, params.counts.z);
    var retained = false;
    var previous = invalid_pair();
    if (lower < previous_count && all(previous_pairs[lower].key == manifold.identities)) {
        previous = previous_pairs[lower];
        let slot = previous.state.x;
        let generation = previous.state.y;
        if (slot < params.counts.x && generation != 0u &&
            slot_generations[slot] == generation) {
            retained = true;
            record.state.x = slot;
            record.state.y = generation;
        }
    }
    if (manifold.point_range.y >= 1u) {
        let point = points[manifold.point_range.x];
        record.features0 = point.features;
        var cache: CacheMatch;
        if (retained) {
            cache = match_cache(point.features, previous);
        } else {
            cache.cache = vec4u(0u, 0u, feature_fingerprint(point.features), 0u);
            cache.matched = 0u;
        }
        record.cache0 = cache.cache;
        atomicAdd(&work[12], cache.matched);
        atomicAdd(&work[13], 1u - cache.matched);
    }
    if (manifold.point_range.y >= 2u) {
        let point = points[manifold.point_range.x + 1u];
        record.features1 = point.features;
        var cache: CacheMatch;
        if (retained) {
            cache = match_cache(point.features, previous);
        } else {
            cache.cache = vec4u(0u, 0u, feature_fingerprint(point.features), 0u);
            cache.matched = 0u;
        }
        record.cache1 = cache.cache;
        atomicAdd(&work[12], cache.matched);
        atomicAdd(&work[13], 1u - cache.matched);
    }
    output_pairs[source_index] = record;
}

@compute @workgroup_size(64)
fn classify_pair_slots(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    let current_count = atomicLoad(&output_capacity[2]);
    if (pair_index >= current_count) { return; }
    let record = output_pairs[pair_index];
    if (record.state.y == 0u) {
        new_flags[pair_index] = 1u;
    } else if (record.state.x < params.counts.x) {
        slot_flags[record.state.x] = 1u;
    }
}

@compute @workgroup_size(256)
fn scan_pair_slot_blocks(@builtin(global_invocation_id) global: vec3u,
                         @builtin(local_invocation_id) local: vec3u,
                         @builtin(workgroup_id) group_id: vec3u) {
    let index = global.x;
    let lane = local.x;
    var new_value = 0u;
    var free_value = 0u;
    if (index < params.counts.x) {
        new_value = new_flags[index];
        free_value = select(1u, 0u, slot_flags[index] != 0u);
    }
    scan_new[lane] = new_value;
    scan_free[lane] = free_value;
    workgroupBarrier();
    for (var offset = 1u; offset < 256u; offset *= 2u) {
        var add_new = 0u;
        var add_free = 0u;
        if (lane >= offset) {
            add_new = scan_new[lane - offset];
            add_free = scan_free[lane - offset];
        }
        workgroupBarrier();
        scan_new[lane] += add_new;
        scan_free[lane] += add_free;
        workgroupBarrier();
    }
    if (index < params.counts.x) {
        new_offsets[index] = scan_new[lane] - new_value;
        slot_offsets[index] = scan_free[lane] - free_value;
    }
    if (lane == 255u) {
        new_blocks[group_id.x] = scan_new[lane];
        free_blocks[group_id.x] = scan_free[lane];
    }
}

@compute @workgroup_size(1)
fn scan_pair_slot_totals(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var new_running = 0u;
    var free_running = 0u;
    for (var block = 0u; block < params.counts.y; block++) {
        let new_count = new_blocks[block];
        let free_count = free_blocks[block];
        new_blocks[block] = new_running;
        free_blocks[block] = free_running;
        new_running += new_count;
        free_running += free_count;
    }
}

@compute @workgroup_size(256)
fn add_pair_slot_offsets(@builtin(global_invocation_id) global: vec3u,
                         @builtin(workgroup_id) group_id: vec3u) {
    let index = global.x;
    if (index >= params.counts.x) { return; }
    new_offsets[index] += new_blocks[group_id.x];
    slot_offsets[index] += free_blocks[group_id.x];
}

@compute @workgroup_size(64)
fn scatter_free_slots(@builtin(global_invocation_id) invocation: vec3u) {
    let slot = invocation.x;
    if (slot < params.counts.x && slot_flags[slot] == 0u) {
        free_slots[slot_offsets[slot]] = slot;
    }
}

@compute @workgroup_size(64)
fn assign_new_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let pair_index = invocation.x;
    let current_count = atomicLoad(&output_capacity[2]);
    if (pair_index >= current_count || new_flags[pair_index] == 0u) { return; }
    let slot = free_slots[new_offsets[pair_index]];
    let generation = slot_generations[slot] + 1u;
    if (generation == 0u) {
        atomicStore(&output_capacity[3], 1u);
        return;
    }
    slot_generations[slot] = generation;
    output_pairs[pair_index].state.x = slot;
    output_pairs[pair_index].state.y = generation;
}

@compute @workgroup_size(1)
fn finalize_pair_slots(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let pair_count = atomicLoad(&output_capacity[2]);
    let allocation_failed = atomicLoad(&output_capacity[3]) != 0u;
    if (allocation_failed) {
        atomicStore(&output_capacity[2], 0u);
        atomicStore(&output_capacity[3], 1u);
    }

    let previous_pair_count = atomicLoad(&previous_capacity[2]);
    var previous_points = 0u;
    for (var pair_index = 0u; pair_index < previous_pair_count; pair_index++) {
        previous_points += previous_pairs[pair_index].state.z;
    }
    let accepted = !allocation_failed && pair_count == atomicLoad(&output_capacity[1]);
    let current_points = select(0u, atomicLoad(&point_capacity[2]), accepted);
    let matched = select(0u, atomicLoad(&work[12]), accepted);
    atomicStore(&work[8], current_points);
    atomicStore(&work[9], 0u);
    atomicStore(&work[10], previous_points);
    atomicStore(&work[11], current_points);
    atomicStore(&work[12], matched);
    atomicStore(&work[13], current_points - matched);
    atomicStore(&work[14], select(0u, previous_points - min(previous_points, matched), accepted));
    atomicStore(&work[15], params.control.x);
}
