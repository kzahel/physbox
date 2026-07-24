struct ConstructionState {
    header: vec4u,
    last_identity_kind: vec4u,
    success_counts: vec4u,
    outcome_counts: vec4u,
    extra_counts: vec4u,
    recent_results: array<vec4u, 4>,
};

struct GrabState {
    identity_status: vec4u,
    local_anchor_target: vec4f,
    counters: vec4u,
    activity: vec4u,
    last_selection: vec4u,
};

struct BodyLifecycleParams {
    counts: vec4u,
    kill_zone: vec4f,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> properties: array<vec4f>;
@group(0) @binding(3) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(4) var<storage, read_write> active_bodies: array<u32>;
@group(0) @binding(5) var<storage, read_write> construction: ConstructionState;
@group(0) @binding(6) var<storage, read_write> grab: GrabState;
@group(0) @binding(7) var<storage, read_write> scratch: array<u32>;
@group(0) @binding(8) var<uniform> params: BodyLifecycleParams;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const GRAB_ACTIVE: u32 = 1u;
const NO_SLOT: u32 = 0xffffffffu;
const ACTIVE_SLOT_BASE: u32 = 20u;

var<workgroup> scan_values: array<u32, 256>;

fn increment(value: u32) -> u32 {
    return select(value + 1u, value, value == 0xffffffffu);
}

@compute @workgroup_size(256)
fn classify_and_scan(
    @builtin(global_invocation_id) invocation: vec3u,
    @builtin(local_invocation_id) local: vec3u,
    @builtin(workgroup_id) workgroup: vec3u,
) {
    let slot = invocation.x;
    var live = 0u;
    if (slot < params.counts.x) {
        let identity = metadata[slot];
        let flags = identity.y;
        let dynamic_live = (flags & (FLAG_LIVE | FLAG_FIXED)) == FLAG_LIVE;
        if (dynamic_live && poses[slot].y < params.kill_zone.x) {
            poses[slot] = vec4f(0.0);
            motions[slot] = vec4f(0.0);
            properties[slot] = vec4f(0.0);
            metadata[slot] = vec4u(increment(identity.x), 0u, 0u, 0u);
        } else if ((flags & FLAG_LIVE) != 0u) {
            live = 1u;
        }
    }

    scan_values[local.x] = live;
    workgroupBarrier();
    var offset = 1u;
    loop {
        if (offset >= 256u) {
            break;
        }
        var addition = 0u;
        if (local.x >= offset) {
            addition = scan_values[local.x - offset];
        }
        workgroupBarrier();
        scan_values[local.x] += addition;
        workgroupBarrier();
        offset *= 2u;
    }
    if (slot < params.counts.x) {
        scratch[slot] = scan_values[local.x] - live;
    }
    if (local.x == 255u) {
        scratch[params.counts.x + workgroup.x] = scan_values[local.x];
    }
}

@compute @workgroup_size(1)
fn clear_retired_grab(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u || grab.identity_status.z != GRAB_ACTIVE) {
        return;
    }
    let slot = grab.identity_status.x;
    if (slot >= params.counts.x ||
        metadata[slot].x != grab.identity_status.y ||
        (metadata[slot].y & FLAG_LIVE) == 0u) {
        grab.identity_status = vec4u(NO_SLOT, 0u, 0u, grab.identity_status.w);
        grab.local_anchor_target = vec4f(0.0);
        grab.activity = vec4u(0u);
        construction.extra_counts.y = increment(construction.extra_counts.y);
    }
}

@compute @workgroup_size(1)
fn scan_blocks(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) {
        return;
    }
    var total = 0u;
    for (var block = 0u; block < params.counts.y; block += 1u) {
        let index = params.counts.x + block;
        let count = scratch[index];
        scratch[index] = total;
        total += count;
    }
    scratch[params.counts.x + params.counts.y] = total;
}

@compute @workgroup_size(256)
fn scatter_live_bodies(
    @builtin(global_invocation_id) invocation: vec3u,
    @builtin(workgroup_id) workgroup: vec3u,
) {
    let slot = invocation.x;
    if (slot >= params.counts.x || (metadata[slot].y & FLAG_LIVE) == 0u) {
        return;
    }
    let destination = scratch[params.counts.x + workgroup.x] + scratch[slot];
    active_bodies[ACTIVE_SLOT_BASE + destination] = slot;
}

@compute @workgroup_size(1)
fn finalize_active_bodies(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) {
        return;
    }
    let old_live_count = active_bodies[0];
    let live_count = scratch[params.counts.x + params.counts.y];
    let fixed_count = active_bodies[2];
    active_bodies[0] = live_count;
    active_bodies[1] = live_count - fixed_count;
    if (live_count != old_live_count) {
        active_bodies[3] = increment(active_bodies[3]);
    }
    active_bodies[4] = (live_count + 63u) / 64u;
    active_bodies[5] = 1u;
    active_bodies[6] = 1u;
    active_bodies[7] = (live_count + 255u) / 256u;
    active_bodies[8] = 1u;
    active_bodies[9] = 1u;
    active_bodies[10] = (live_count * 32u + 63u) / 64u;
    active_bodies[11] = 1u;
    active_bodies[12] = 1u;
    active_bodies[13] = 6u;
    active_bodies[14] = live_count;
    active_bodies[15] = 0u;
    active_bodies[16] = 0u;
    construction.header.y = live_count;
}
