struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

struct SleepParams {
    // body capacity, enabled, quiet steps, K
    counts: vec4u,
    thresholds: vec4f,
}

struct DecisionParams {
    // body capacity, trace index, trace capacity, statistics words
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read_write> parents: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> component_values: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read> sleep_state: array<SleepStateV1>;
@group(0) @binding(4) var<storage, read_write> local_frontier: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> final_frontier: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> decision_stats: array<atomic<u32>>;
@group(0) @binding(7) var<uniform> sleep_params: SleepParams;
@group(0) @binding(8) var<uniform> decision_params: DecisionParams;
@group(0) @binding(9) var<storage, read_write> body_trace: array<u32>;

const INVALID: u32 = 0xffffffffu;
const FLAG_LIVE: u32 = 1u;
const FLAG_FIXED: u32 = 4u;
const SLEEP_FLAG_AWAKE: u32 = 1u;
const SLEEP_FLAG_ELIGIBLE: u32 = 2u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;

fn is_dynamic(body: u32) -> bool {
    return body < decision_params.counts.x &&
        (metadata[body].y & (FLAG_LIVE | FLAG_FIXED)) == FLAG_LIVE;
}

fn matching_state(body: u32) -> bool {
    return is_dynamic(body) &&
        sleep_state[body].identity_flags.x == metadata[body].x &&
        sleep_state[body].provenance.z == SLEEP_STATE_LAYOUT_VERSION;
}

fn stat_index(word: u32) -> u32 {
    return decision_params.counts.y * decision_params.counts.w + word;
}

fn trace_index(body: u32, word: u32) -> u32 {
    return (decision_params.counts.y * decision_params.counts.x + body) * 4u + word;
}

fn valid_root(body: u32) -> bool {
    let root = atomicLoad(&parents[body]);
    return root != INVALID && root < decision_params.counts.x;
}

@compute @workgroup_size(64)
fn aggregate_wake_roots(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (!matching_state(body) || !valid_root(body)) { return; }
    let root = atomicLoad(&parents[body]);
    let local = atomicLoad(&local_frontier[body]);
    body_trace[trace_index(body, 0u)] = root;
    if (local != 0u) {
        atomicOr(&component_values[root], local);
        atomicAdd(&decision_stats[stat_index(2u)], 1u);
        body_trace[trace_index(body, 1u)] = 1u;
    }
}

@compute @workgroup_size(64)
fn compare_wake_decisions(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (!matching_state(body) || !valid_root(body)) { return; }
    let root = atomicLoad(&parents[body]);
    let exact = atomicLoad(&component_values[root]) != 0u;
    let bounded = atomicLoad(&final_frontier[body]) != 0u;
    let awake = (sleep_state[body].identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
    if (body == root) {
        atomicAdd(&decision_stats[stat_index(0u)], 1u);
        if (exact) {
            atomicAdd(&decision_stats[stat_index(1u)], 1u);
        }
    }
    if (exact) { atomicAdd(&decision_stats[stat_index(3u)], 1u); }
    if (bounded) { atomicAdd(&decision_stats[stat_index(4u)], 1u); }
    if (exact && !bounded) { atomicAdd(&decision_stats[stat_index(5u)], 1u); }
    if (bounded && !exact) { atomicAdd(&decision_stats[stat_index(6u)], 1u); }
    if (!awake) {
        atomicAdd(&decision_stats[stat_index(7u)], 1u);
        if (exact) { atomicAdd(&decision_stats[stat_index(8u)], 1u); }
        if (bounded) { atomicAdd(&decision_stats[stat_index(9u)], 1u); }
        if (exact && !bounded) { atomicAdd(&decision_stats[stat_index(10u)], 1u); }
    }
    body_trace[trace_index(body, 1u)] =
        body_trace[trace_index(body, 1u)] |
        select(0u, 1u << 1u, exact) |
        select(0u, 1u << 2u, bounded) |
        select(0u, 1u << 3u, awake);
}

@compute @workgroup_size(64)
fn aggregate_admission_blockers(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (!matching_state(body) || !valid_root(body)) { return; }
    let root = atomicLoad(&parents[body]);
    let local = atomicLoad(&local_frontier[body]);
    body_trace[trace_index(body, 2u)] = select(0u, 1u, local != 0u);
    if (local != 0u) {
        atomicOr(&component_values[root], local);
        atomicAdd(&decision_stats[stat_index(11u)], 1u);
    }
}

@compute @workgroup_size(64)
fn compare_admission_decisions(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (!matching_state(body) || !valid_root(body)) { return; }
    let root = atomicLoad(&parents[body]);
    let local_blocked = (body_trace[trace_index(body, 2u)] & 1u) != 0u;
    let exact_blocked = atomicLoad(&component_values[root]) != 0u;
    let bounded_blocked = atomicLoad(&final_frontier[body]) != 0u;
    let state = sleep_state[body];
    let awake = (state.identity_flags.y & SLEEP_FLAG_AWAKE) != 0u;
    let locally_ready = awake && sleep_params.counts.y != 0u &&
        (state.identity_flags.y & SLEEP_FLAG_ELIGIBLE) != 0u &&
        state.identity_flags.z >= sleep_params.counts.z && !local_blocked;
    let exact_admit = locally_ready && !exact_blocked;
    let bounded_admit = locally_ready && !bounded_blocked;
    if (body == root && exact_blocked) {
        atomicAdd(&decision_stats[stat_index(24u)], 1u);
    }
    if (exact_blocked) { atomicAdd(&decision_stats[stat_index(12u)], 1u); }
    if (bounded_blocked) { atomicAdd(&decision_stats[stat_index(13u)], 1u); }
    if (exact_blocked && !bounded_blocked) {
        atomicAdd(&decision_stats[stat_index(14u)], 1u);
    }
    if (bounded_blocked && !exact_blocked) {
        atomicAdd(&decision_stats[stat_index(15u)], 1u);
    }
    if (locally_ready) { atomicAdd(&decision_stats[stat_index(16u)], 1u); }
    if (exact_admit) { atomicAdd(&decision_stats[stat_index(17u)], 1u); }
    if (bounded_admit) { atomicAdd(&decision_stats[stat_index(18u)], 1u); }
    if (exact_admit && !bounded_admit) {
        atomicAdd(&decision_stats[stat_index(19u)], 1u);
    }
    if (bounded_admit && !exact_admit) {
        atomicAdd(&decision_stats[stat_index(20u)], 1u);
    }
    if (awake) { atomicAdd(&decision_stats[stat_index(21u)], 1u); }
    body_trace[trace_index(body, 2u)] =
        select(0u, 1u << 0u, local_blocked) |
        select(0u, 1u << 1u, exact_blocked) |
        select(0u, 1u << 2u, bounded_blocked) |
        select(0u, 1u << 3u, locally_ready) |
        select(0u, 1u << 4u, exact_admit) |
        select(0u, 1u << 5u, bounded_admit) |
        select(0u, 1u << 6u, awake);
}
