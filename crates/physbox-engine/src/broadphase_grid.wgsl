struct Params {
    counts0: vec4u,
    counts1: vec4u,
    counts2: vec4u,
    radix: vec4u,
    grid: vec4f,
}

struct Proxy {
    identity_mobility: vec4u,
    lower_upper: vec4f,
}

struct SortRecord {
    values: vec4u,
}

struct CandidatePair {
    identities: vec4u,
}

@group(0) @binding(0) var<storage, read> input_proxies: array<Proxy>;
@group(0) @binding(1) var<storage, read_write> scan_source: array<u32>;
@group(0) @binding(2) var<storage, read_write> scan_offsets: array<u32>;
@group(0) @binding(3) var<storage, read_write> scan_blocks: array<u32>;
@group(0) @binding(4) var<storage, read_write> cell_counts: array<u32>;
@group(0) @binding(5) var<storage, read_write> cell_offsets: array<u32>;
@group(0) @binding(6) var<storage, read_write> global_flags: array<u32>;
@group(0) @binding(7) var<storage, read_write> global_offsets: array<u32>;
@group(0) @binding(8) var<storage, read_write> global_statics: array<u32>;
@group(0) @binding(9) var<storage, read_write> cells_source: array<SortRecord>;
@group(0) @binding(10) var<storage, read_write> cells_destination: array<SortRecord>;
@group(0) @binding(11) var<storage, read_write> cell_radix_blocks: array<u32>;
@group(0) @binding(12) var<storage, read_write> pairs_source: array<SortRecord>;
@group(0) @binding(13) var<storage, read_write> pairs_destination: array<SortRecord>;
@group(0) @binding(14) var<storage, read_write> pair_radix_blocks: array<u32>;
@group(0) @binding(15) var<storage, read_write> unique_flags: array<u32>;
@group(0) @binding(16) var<storage, read_write> unique_offsets: array<u32>;
@group(0) @binding(17) var<storage, read_write> candidates: array<CandidatePair>;
@group(0) @binding(18) var<storage, read_write> work: array<atomic<u32>>;
@group(0) @binding(19) var<storage, read_write> capacities: array<atomic<u32>>;
@group(0) @binding(20) var<uniform> params: Params;
@group(0) @binding(21) var<storage, read_write> indirect: array<atomic<u32>>;

const MOBILITY_FIXED: u32 = 1u;
const SENTINEL: u32 = 0xffffffffu;
const CELL_CAPACITY_BASE: u32 = 0u;
const GLOBAL_CAPACITY_BASE: u32 = 4u;
const EMISSION_CAPACITY_BASE: u32 = 8u;
const CANDIDATE_CAPACITY_BASE: u32 = 12u;

var<workgroup> scan_values: array<u32, 256>;
var<workgroup> radix_digits: array<u32, 256>;
var<workgroup> radix_histogram: array<atomic<u32>, 16>;

fn any_input_overflow() -> bool {
    return atomicLoad(&capacities[CELL_CAPACITY_BASE + 3u]) != 0u ||
        atomicLoad(&capacities[GLOBAL_CAPACITY_BASE + 3u]) != 0u;
}

fn any_overflow() -> bool {
    return any_input_overflow() ||
        atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 3u]) != 0u ||
        atomicLoad(&capacities[CANDIDATE_CAPACITY_BASE + 3u]) != 0u;
}

fn cell_bounds(proxy: Proxy) -> vec4i {
    return vec4i(
        i32(floor(proxy.lower_upper.x / params.grid.x)),
        i32(floor(proxy.lower_upper.y / params.grid.x)),
        i32(floor(proxy.lower_upper.z / params.grid.x)),
        i32(floor(proxy.lower_upper.w / params.grid.x)));
}

fn cell_count(bounds: vec4i) -> u32 {
    return u32(bounds.z - bounds.x + 1) * u32(bounds.w - bounds.y + 1);
}

fn cell_key(value: i32) -> u32 {
    return u32(value + 32768);
}

fn sort_key(record: SortRecord) -> u32 {
    return select(record.values.y, record.values.x, params.radix.x != 0u);
}

fn overlaps(left: Proxy, right: Proxy) -> bool {
    return left.lower_upper.x <= right.lower_upper.z &&
        right.lower_upper.x <= left.lower_upper.z &&
        left.lower_upper.y <= right.lower_upper.w &&
        right.lower_upper.y <= left.lower_upper.w;
}

fn emit_pair(ordinal_a: u32, ordinal_b: u32) {
    let output = atomicAdd(&work[5], 1u);
    if (output < params.counts1.y) {
        pairs_source[output].values = vec4u(min(ordinal_a, ordinal_b),
                                             max(ordinal_a, ordinal_b), 0u, 0u);
    }
}

@compute @workgroup_size(64)
fn clear_grid_proxy_state(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < params.counts0.x) {
        cell_counts[index] = 0u;
        cell_offsets[index] = 0u;
        global_flags[index] = 0u;
        global_offsets[index] = 0u;
    }
    if (index < 16u) {
        atomicStore(&work[index], 0u);
        atomicStore(&capacities[index], 0u);
    }
    if (index < 15u) {
        atomicStore(&indirect[index], 0u);
    }
}

@compute @workgroup_size(64)
fn clear_grid_cell_state(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < params.counts0.z) {
        let sentinel = SortRecord(vec4u(SENTINEL));
        cells_source[index] = sentinel;
        cells_destination[index] = sentinel;
    }
    if (index < params.counts1.x) {
        global_statics[index] = SENTINEL;
    }
}

@compute @workgroup_size(64)
fn clear_grid_pair_state(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index < params.counts1.y) {
        let sentinel = SortRecord(vec4u(SENTINEL));
        pairs_source[index] = sentinel;
        pairs_destination[index] = sentinel;
        unique_flags[index] = 0u;
        unique_offsets[index] = 0u;
    }
}

@compute @workgroup_size(64)
fn classify_grid_proxies(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= params.counts0.x) { return; }
    let proxy = input_proxies[index];
    let count = cell_count(cell_bounds(proxy));
    let global_static = proxy.identity_mobility.z == MOBILITY_FIXED && count > params.counts2.z;
    cell_counts[index] = select(count, 0u, global_static);
    global_flags[index] = select(0u, 1u, global_static);
}

@compute @workgroup_size(256)
fn scan_grid_blocks(@builtin(global_invocation_id) global: vec3u,
                    @builtin(local_invocation_id) local: vec3u,
                    @builtin(workgroup_id) group: vec3u) {
    let index = global.x;
    var value = 0u;
    if (index < params.radix.z) {
        value = scan_source[index];
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
    if (index < params.radix.z) {
        scan_offsets[index] = scan_values[local.x] - value;
    }
    if (local.x == 255u) {
        scan_blocks[group.x] = scan_values[local.x];
    }
}

@compute @workgroup_size(1)
fn scan_grid_totals(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    for (var block = 0u; block < params.radix.w; block++) {
        let value = scan_blocks[block];
        scan_blocks[block] = running;
        running += value;
    }
}

@compute @workgroup_size(256)
fn add_grid_offsets(@builtin(global_invocation_id) global: vec3u,
                    @builtin(workgroup_id) group: vec3u) {
    if (global.x >= params.radix.z) { return; }
    scan_offsets[global.x] += scan_blocks[group.x];
}

@compute @workgroup_size(1)
fn finalize_grid_inputs(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let last = params.counts0.x - 1u;
    let cell_attempted = cell_offsets[last] + cell_counts[last];
    let global_attempted = global_offsets[last] + global_flags[last];
    let cell_overflow = cell_attempted > params.counts0.z;
    let global_overflow = global_attempted > params.counts1.x;
    atomicStore(&capacities[CELL_CAPACITY_BASE], params.counts0.z);
    atomicStore(&capacities[CELL_CAPACITY_BASE + 1u], cell_attempted);
    atomicStore(&capacities[CELL_CAPACITY_BASE + 2u], select(cell_attempted, 0u, cell_overflow));
    atomicStore(&capacities[CELL_CAPACITY_BASE + 3u], select(0u, 1u, cell_overflow));
    atomicStore(&capacities[GLOBAL_CAPACITY_BASE], params.counts1.x);
    atomicStore(&capacities[GLOBAL_CAPACITY_BASE + 1u], global_attempted);
    atomicStore(&capacities[GLOBAL_CAPACITY_BASE + 2u], select(global_attempted, 0u, global_overflow));
    atomicStore(&capacities[GLOBAL_CAPACITY_BASE + 3u], select(0u, 1u, global_overflow));
    let cell_committed = select(cell_attempted, 0u, cell_overflow || global_overflow);
    let global_committed = select(global_attempted, 0u, cell_overflow || global_overflow);
    atomicStore(&indirect[0], (cell_committed + 255u) / 256u);
    atomicStore(&indirect[1], 1u);
    atomicStore(&indirect[2], 1u);
    atomicStore(&indirect[9], (global_committed + 63u) / 64u);
    atomicStore(&indirect[10], 1u);
    atomicStore(&indirect[11], 1u);
    atomicStore(&indirect[12], (cell_committed + 63u) / 64u);
    atomicStore(&indirect[13], 1u);
    atomicStore(&indirect[14], 1u);
    atomicStore(&work[0], params.counts0.x);
    atomicStore(&work[1], params.counts2.x);
    atomicStore(&work[2], params.counts2.y);
    atomicStore(&work[8], 0u);
    atomicStore(&work[9], 18u);
    atomicStore(&work[10], cell_attempted);
    atomicStore(&work[11], global_attempted);
    atomicStore(&work[14], params.counts2.w);
}

@compute @workgroup_size(64)
fn scatter_grid_records(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= params.counts0.x || any_input_overflow()) { return; }
    if (global_flags[index] != 0u) {
        global_statics[global_offsets[index]] = index;
        return;
    }
    let bounds = cell_bounds(input_proxies[index]);
    var local = 0u;
    for (var y = bounds.y; y <= bounds.w; y++) {
        for (var x = bounds.x; x <= bounds.z; x++) {
            cells_source[cell_offsets[index] + local].values =
                vec4u(cell_key(x), cell_key(y), index, 0u);
            local += 1u;
        }
    }
}

@compute @workgroup_size(256)
fn cell_radix_histograms(@builtin(global_invocation_id) global: vec3u,
                         @builtin(local_invocation_id) local: vec3u,
                         @builtin(workgroup_id) group: vec3u) {
    if (local.x < 16u) {
        atomicStore(&radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    let committed = atomicLoad(&capacities[CELL_CAPACITY_BASE + 2u]);
    if (global.x < committed) {
        atomicAdd(&radix_histogram[(sort_key(cells_source[global.x]) >> params.radix.y) & 15u], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        cell_radix_blocks[group.x * 16u + local.x] = atomicLoad(&radix_histogram[local.x]);
    }
}

@compute @workgroup_size(1)
fn cell_radix_prefix(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    let block_count = (atomicLoad(&capacities[CELL_CAPACITY_BASE + 2u]) + 255u) / 256u;
    for (var digit = 0u; digit < 16u; digit++) {
        for (var block = 0u; block < block_count; block++) {
            let index = block * 16u + digit;
            let count = cell_radix_blocks[index];
            cell_radix_blocks[index] = running;
            running += count;
        }
    }
}

@compute @workgroup_size(256)
fn cell_radix_scatter(@builtin(global_invocation_id) global: vec3u,
                      @builtin(local_invocation_id) local: vec3u,
                      @builtin(workgroup_id) group: vec3u) {
    var digit = 16u;
    let committed = atomicLoad(&capacities[CELL_CAPACITY_BASE + 2u]);
    if (global.x < committed) {
        digit = (sort_key(cells_source[global.x]) >> params.radix.y) & 15u;
    }
    radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= committed) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, radix_digits[lane] == digit);
    }
    cells_destination[cell_radix_blocks[group.x * 16u + digit] + local_rank] =
        cells_source[global.x];
}

@compute @workgroup_size(64)
fn enumerate_grid_cells(@builtin(global_invocation_id) invocation: vec3u) {
    let index_a = invocation.x;
    let committed = atomicLoad(&capacities[CELL_CAPACITY_BASE + 2u]);
    if (index_a >= committed || any_input_overflow()) { return; }
    let record_a = cells_source[index_a].values;
    let proxy_a = input_proxies[record_a.z];
    var occupancy = 1u;
    for (var index_b = index_a + 1u; index_b < committed; index_b++) {
        let record_b = cells_source[index_b].values;
        if (record_a.x != record_b.x || record_a.y != record_b.y) { break; }
        occupancy += 1u;
        let proxy_b = input_proxies[record_b.z];
        atomicAdd(&work[3], 1u);
        if (proxy_a.identity_mobility.z == MOBILITY_FIXED &&
            proxy_b.identity_mobility.z == MOBILITY_FIXED) {
            atomicAdd(&work[4], 1u);
            continue;
        }
        if (overlaps(proxy_a, proxy_b)) {
            emit_pair(record_a.z, record_b.z);
        }
    }
    atomicMax(&work[13], occupancy);
}

@compute @workgroup_size(64)
fn enumerate_global_statics(@builtin(global_invocation_id) invocation: vec3u) {
    let global_index = invocation.x;
    let committed = atomicLoad(&capacities[GLOBAL_CAPACITY_BASE + 2u]);
    if (global_index >= committed || any_input_overflow()) { return; }
    let ordinal_a = global_statics[global_index];
    let proxy_a = input_proxies[ordinal_a];
    for (var ordinal_b = 0u; ordinal_b < params.counts0.x; ordinal_b++) {
        if (ordinal_a == ordinal_b) { continue; }
        let proxy_b = input_proxies[ordinal_b];
        if (proxy_b.identity_mobility.z == MOBILITY_FIXED) { continue; }
        atomicAdd(&work[3], 1u);
        if (overlaps(proxy_a, proxy_b)) {
            emit_pair(ordinal_a, ordinal_b);
        }
    }
}

@compute @workgroup_size(1)
fn finalize_grid_emissions(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let attempted = atomicLoad(&work[5]);
    let overflowed = attempted > params.counts1.y;
    atomicStore(&capacities[EMISSION_CAPACITY_BASE], params.counts1.y);
    atomicStore(&capacities[EMISSION_CAPACITY_BASE + 1u], attempted);
    atomicStore(&capacities[EMISSION_CAPACITY_BASE + 2u], select(attempted, 0u, overflowed));
    atomicStore(&capacities[EMISSION_CAPACITY_BASE + 3u], select(0u, 1u, overflowed));
    let committed = select(attempted, 0u, overflowed || any_input_overflow());
    atomicStore(&indirect[3], (committed + 255u) / 256u);
    atomicStore(&indirect[4], 1u);
    atomicStore(&indirect[5], 1u);
    atomicStore(&indirect[6], (committed + 63u) / 64u);
    atomicStore(&indirect[7], 1u);
    atomicStore(&indirect[8], 1u);
}

@compute @workgroup_size(256)
fn pair_radix_histograms(@builtin(global_invocation_id) global: vec3u,
                         @builtin(local_invocation_id) local: vec3u,
                         @builtin(workgroup_id) group: vec3u) {
    if (local.x < 16u) {
        atomicStore(&radix_histogram[local.x], 0u);
    }
    workgroupBarrier();
    let committed = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]);
    if (global.x < committed) {
        atomicAdd(&radix_histogram[(sort_key(pairs_source[global.x]) >> params.radix.y) & 15u], 1u);
    }
    workgroupBarrier();
    if (local.x < 16u) {
        pair_radix_blocks[group.x * 16u + local.x] = atomicLoad(&radix_histogram[local.x]);
    }
}

@compute @workgroup_size(1)
fn pair_radix_prefix(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    let block_count = (atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]) + 255u) / 256u;
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
    let committed = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]);
    if (global.x < committed) {
        digit = (sort_key(pairs_source[global.x]) >> params.radix.y) & 15u;
    }
    radix_digits[local.x] = digit;
    workgroupBarrier();
    if (global.x >= committed) { return; }
    var local_rank = 0u;
    for (var lane = 0u; lane < local.x; lane++) {
        local_rank += select(0u, 1u, radix_digits[lane] == digit);
    }
    pairs_destination[pair_radix_blocks[group.x * 16u + digit] + local_rank] =
        pairs_source[global.x];
}

@compute @workgroup_size(64)
fn mark_unique_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    let committed = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]);
    if (index >= committed || any_input_overflow()) { return; }
    let current = pairs_source[index].values.xy;
    unique_flags[index] = select(
        0u,
        1u,
        index == 0u || any(current != pairs_source[index - 1u].values.xy));
}

@compute @workgroup_size(256)
fn scan_unique_blocks(@builtin(global_invocation_id) global: vec3u,
                      @builtin(local_invocation_id) local: vec3u,
                      @builtin(workgroup_id) group: vec3u) {
    let count = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]);
    let index = global.x;
    var value = 0u;
    if (index < count) { value = unique_flags[index]; }
    scan_values[local.x] = value;
    workgroupBarrier();
    var offset = 1u;
    loop {
        if (offset >= 256u) { break; }
        var addition = 0u;
        if (local.x >= offset) { addition = scan_values[local.x - offset]; }
        workgroupBarrier();
        scan_values[local.x] += addition;
        workgroupBarrier();
        offset *= 2u;
    }
    if (index < count) { unique_offsets[index] = scan_values[local.x] - value; }
    if (local.x == 255u) { scan_blocks[group.x] = scan_values[local.x]; }
}

@compute @workgroup_size(1)
fn scan_unique_totals(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    let block_count = (atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]) + 255u) / 256u;
    for (var block = 0u; block < block_count; block++) {
        let value = scan_blocks[block];
        scan_blocks[block] = running;
        running += value;
    }
}

@compute @workgroup_size(256)
fn add_unique_offsets(@builtin(global_invocation_id) global: vec3u,
                      @builtin(workgroup_id) group: vec3u) {
    let count = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]);
    if (global.x >= count) { return; }
    unique_offsets[global.x] += scan_blocks[group.x];
}

@compute @workgroup_size(1)
fn finalize_unique_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let emission_overflow = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 3u]) != 0u;
    var unique = 0u;
    if (!any_input_overflow() && !emission_overflow) {
        let attempted = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 1u]);
        if (attempted > 0u) {
            unique = unique_offsets[attempted - 1u] + unique_flags[attempted - 1u];
        }
    } else if (emission_overflow) {
        unique = atomicLoad(&work[5]);
    }
    let overflowed = !emission_overflow && unique > params.counts1.w;
    atomicStore(&capacities[CANDIDATE_CAPACITY_BASE], params.counts1.w);
    atomicStore(&capacities[CANDIDATE_CAPACITY_BASE + 1u], select(unique, 0u, emission_overflow));
    atomicStore(&capacities[CANDIDATE_CAPACITY_BASE + 2u],
                select(select(unique, 0u, overflowed), 0u, emission_overflow));
    atomicStore(&capacities[CANDIDATE_CAPACITY_BASE + 3u], select(0u, 1u, overflowed));
    let emissions = atomicLoad(&work[5]);
    atomicStore(&work[7], unique);
    atomicStore(&work[6], select(emissions - unique, 0u, emission_overflow));
    atomicStore(&work[8],
                atomicLoad(&capacities[CELL_CAPACITY_BASE + 2u]) +
                atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]));
}

@compute @workgroup_size(64)
fn materialize_unique_pairs(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    let emissions = atomicLoad(&capacities[EMISSION_CAPACITY_BASE + 2u]);
    if (index >= emissions || unique_flags[index] == 0u || any_overflow()) { return; }
    let output = unique_offsets[index];
    let pair = pairs_source[index].values;
    let body_a = input_proxies[pair.x].identity_mobility;
    let body_b = input_proxies[pair.y].identity_mobility;
    candidates[output].identities = vec4u(body_a.x, body_a.y, body_b.x, body_b.y);
}
