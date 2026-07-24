struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
}

struct Params {
    // body capacity, reserved
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(1) var<storage, read_write> parents: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> component_values: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> component_stats: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> sleep_state: array<SleepStateV1>;
@group(0) @binding(5) var<storage, read_write> local_frontier: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> exact_frontier: array<atomic<u32>>;
@group(0) @binding(7) var<uniform> params: Params;

const INVALID: u32 = 0xffffffffu;
const FLAG_LIVE: u32 = 1u;
const FLAG_FIXED: u32 = 4u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;

fn is_dynamic(body: u32) -> bool {
    return body < params.counts.x &&
        (metadata[body].y & (FLAG_LIVE | FLAG_FIXED)) == FLAG_LIVE;
}

fn matching_state(body: u32) -> bool {
    return is_dynamic(body) &&
        sleep_state[body].identity_flags.x == metadata[body].x &&
        sleep_state[body].provenance.z == SLEEP_STATE_LAYOUT_VERSION;
}

fn component_failed() -> bool {
    return atomicLoad(&component_stats[8]) != 0u ||
        atomicLoad(&component_stats[9]) != 0u ||
        atomicLoad(&component_stats[10]) != 0u;
}

fn valid_root(body: u32) -> bool {
    let root = atomicLoad(&parents[body]);
    return root != INVALID && root < params.counts.x &&
        atomicLoad(&parents[root]) == root;
}

@compute @workgroup_size(64)
fn aggregate_component_values(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (!matching_state(body) || component_failed() || !valid_root(body)) { return; }
    let value = atomicLoad(&local_frontier[body]);
    if (value != 0u) {
        atomicOr(&component_values[atomicLoad(&parents[body])], value);
    }
}

@compute @workgroup_size(64)
fn broadcast_component_values(@builtin(global_invocation_id) invocation: vec3u) {
    let body = invocation.x;
    if (body >= params.counts.x) { return; }
    if (!matching_state(body)) {
        atomicStore(&exact_frontier[body], 0u);
        return;
    }
    // Any structural failure keeps every dynamic body awake for this step.
    if (component_failed() || !valid_root(body)) {
        atomicStore(&exact_frontier[body], INVALID);
        return;
    }
    atomicStore(
        &exact_frontier[body],
        atomicLoad(&component_values[atomicLoad(&parents[body])]),
    );
}
