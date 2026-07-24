struct Params {
    counts: vec4u,
    radix: vec4u,
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

@group(0) @binding(0) var<storage, read> manifolds: array<ManifoldRecord>;
@group(0) @binding(1) var<storage, read> points: array<ContactPoint>;
@group(0) @binding(2) var<storage, read_write> manifold_capacity: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> point_capacity: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> current_source: array<CurrentRecord>;
@group(0) @binding(5) var<storage, read_write> current_destination: array<CurrentRecord>;
@group(0) @binding(6) var<storage, read_write> radix_blocks: array<u32>;
@group(0) @binding(7) var<storage, read> previous_cache: array<PersistentRecord>;
@group(0) @binding(8) var<storage, read_write> previous_capacity: array<atomic<u32>>;
@group(0) @binding(9) var<storage, read_write> output_cache: array<PersistentRecord>;
@group(0) @binding(10) var<storage, read_write> output_capacity: array<atomic<u32>>;
@group(0) @binding(11) var<storage, read_write> work: array<atomic<u32>>;
@group(0) @binding(12) var<uniform> params: Params;
@group(0) @binding(13) var<storage, read_write> step_counter: array<atomic<u32>>;

var<workgroup> radix_digits: array<u32, 256>;
var<workgroup> radix_histogram: array<atomic<u32>, 16>;

fn invalid_current() -> CurrentRecord {
    var record: CurrentRecord;
    record.key0 = vec4u(0xffffffffu);
    record.key1 = vec4u(0xffffffffu);
    record.cache = vec4u(0u);
    record.location = vec4u(0u);
    return record;
}

fn invalid_persistent() -> PersistentRecord {
    var record: PersistentRecord;
    record.key0 = vec4u(0xffffffffu);
    record.key1 = vec4u(0xffffffffu);
    record.cache = vec4u(0u);
    return record;
}

fn key_word(record: CurrentRecord, field: u32) -> u32 {
    if (field < 4u) { return record.key0[field]; }
    return record.key1[field - 4u];
}

fn key_less_values(left0: vec4u, left1: vec4u,
                   right0: vec4u, right1: vec4u) -> bool {
    for (var index = 0u; index < 4u; index++) {
        if (left0[index] != right0[index]) {
            return left0[index] < right0[index];
        }
    }
    for (var index = 0u; index < 4u; index++) {
        if (left1[index] != right1[index]) {
            return left1[index] < right1[index];
        }
    }
    return false;
}

fn key_equal_values(left0: vec4u, left1: vec4u,
                    right0: vec4u, right1: vec4u) -> bool {
    return all(left0 == right0) && all(left1 == right1);
}

@compute @workgroup_size(64)
fn clear_persistence(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < params.counts.x) {
        current_source[index] = invalid_current();
        current_destination[index] = invalid_current();
        output_cache[index] = invalid_persistent();
    }
    if (index < params.counts.y * 16u) {
        radix_blocks[index] = 0u;
    }
    if (index < 4u) {
        atomicStore(&output_capacity[index], 0u);
    }
    if (index >= 8u && index <= 14u) {
        atomicStore(&work[index], 0u);
    }
}

@compute @workgroup_size(1)
fn prepare_persistence(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    atomicAdd(&step_counter[0], 1u);
    let attempted = atomicLoad(&point_capacity[1]);
    let intrinsic_overflow = attempted > params.counts.x;
    let upstream_rejected = atomicLoad(&manifold_capacity[3]) != 0u ||
        atomicLoad(&point_capacity[3]) != 0u || atomicLoad(&previous_capacity[3]) != 0u;
    let rejected = intrinsic_overflow || upstream_rejected;
    atomicStore(&output_capacity[0], params.counts.x);
    atomicStore(&output_capacity[1], attempted);
    atomicStore(&output_capacity[2], select(attempted, 0u, rejected));
    atomicStore(&output_capacity[3], select(0u, 1u, intrinsic_overflow));
}

@compute @workgroup_size(64)
fn build_current_keys(@builtin(global_invocation_id) invocation: vec3u) {
    let manifold_index = invocation.x;
    if (manifold_index >= atomicLoad(&manifold_capacity[2]) ||
        atomicLoad(&output_capacity[2]) == 0u) {
        return;
    }
    let manifold = manifolds[manifold_index];
    for (var point_index = 0u; point_index < manifold.point_range.y; point_index++) {
        let source_index = manifold.point_range.x + point_index;
        if (source_index >= params.counts.x) { continue; }
        let target_index = select(
            source_index,
            params.counts.x - 1u - source_index,
            params.counts.w != 0u);
        let point = points[source_index];
        var record: CurrentRecord;
        record.key0 = manifold.identities;
        record.key1 = point.features;
        record.cache = vec4u(0u);
        record.location = vec4u(source_index, manifold_index, point_index, 0u);
        current_source[target_index] = record;
    }
}

@compute @workgroup_size(256)
fn radix_histograms(@builtin(global_invocation_id) global: vec3u,
                    @builtin(local_invocation_id) local: vec3u,
                    @builtin(workgroup_id) group: vec3u) {
    if (local.x < 16u) {
        atomicStore(&radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    if (global.x < params.counts.x) {
        let digit = (key_word(current_source[global.x], params.radix.x) >> params.radix.y) & 15u;
        atomicAdd(&radix_histogram[digit], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        radix_blocks[group.x * 16u + local.x] = atomicLoad(&radix_histogram[local.x]);
    }
}

@compute @workgroup_size(1)
fn radix_prefix(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    for (var digit = 0u; digit < 16u; digit++) {
        for (var block = 0u; block < params.counts.y; block++) {
            let index = block * 16u + digit;
            let count = radix_blocks[index];
            radix_blocks[index] = running;
            running += count;
        }
    }
}

@compute @workgroup_size(256)
fn radix_scatter(@builtin(global_invocation_id) global: vec3u,
                 @builtin(local_invocation_id) local: vec3u,
                 @builtin(workgroup_id) group: vec3u) {
    var digit = 16u;
    if (global.x < params.counts.x) {
        digit = (key_word(current_source[global.x], params.radix.x) >> params.radix.y) & 15u;
    }
    radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= params.counts.x) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, radix_digits[lane] == digit);
    }
    let output_index = radix_blocks[group.x * 16u + digit] + local_rank;
    current_destination[output_index] = current_source[global.x];
}

@compute @workgroup_size(64)
fn merge_cache(@builtin(global_invocation_id) invocation: vec3u) {
    let current_index = invocation.x;
    let current_count = atomicLoad(&output_capacity[2]);
    if (current_index >= current_count) { return; }
    let current = current_source[current_index];
    let previous_count = atomicLoad(&previous_capacity[2]);
    var lower = 0u;
    var upper = previous_count;
    loop {
        if (lower >= upper) { break; }
        let middle = lower + (upper - lower) / 2u;
        let previous = previous_cache[middle];
        if (key_less_values(previous.key0, previous.key1, current.key0, current.key1)) {
            lower = middle + 1u;
        } else {
            upper = middle;
        }
    }
    var output: PersistentRecord;
    output.key0 = current.key0;
    output.key1 = current.key1;
    let completed_step = atomicLoad(&step_counter[0]);
    output.cache = vec4u(0u, 0u, completed_step, 1u);
    if (lower < previous_count) {
        let previous = previous_cache[lower];
        if (key_equal_values(previous.key0, previous.key1, current.key0, current.key1)) {
            output.cache = vec4u(previous.cache.x, previous.cache.y, completed_step, 0u);
            atomicAdd(&work[12], 1u);
        } else {
            atomicAdd(&work[13], 1u);
        }
    } else {
        atomicAdd(&work[13], 1u);
    }
    output_cache[current_index] = output;
}

@compute @workgroup_size(1)
fn finalize_persistence(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let previous = atomicLoad(&previous_capacity[2]);
    let current = atomicLoad(&output_capacity[2]);
    let matched = atomicLoad(&work[12]);
    let rejected = atomicLoad(&output_capacity[2]) != atomicLoad(&output_capacity[1]);
    atomicStore(&work[8], current);
    atomicStore(&work[9], params.radix.z);
    atomicStore(&work[10], previous);
    atomicStore(&work[11], current);
    atomicStore(&work[14], select(previous - min(previous, matched), 0u, rejected));
    atomicStore(&work[15], params.radix.w);
}
