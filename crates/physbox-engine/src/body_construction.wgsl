struct ConstructionIntent {
    kind_sequence_padding: vec4u,
    target_padding: vec4f,
};

struct ConstructionState {
    header: vec4u,
    last_identity_kind: vec4u,
    success_counts: vec4u,
    outcome_counts: vec4u,
    extra_counts: vec4u,
};

struct GrabState {
    identity_status: vec4u,
    local_anchor_target: vec4f,
    counters: vec4u,
    activity: vec4u,
};

struct ConstructionParams {
    counts: vec4u,
    circle_material: vec4f,
    rectangle_mass: vec4f,
    material_padding: vec4f,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> properties: array<vec4f>;
@group(0) @binding(3) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(4) var<storage, read> intents: array<ConstructionIntent>;
@group(0) @binding(5) var<storage, read_write> state: ConstructionState;
@group(0) @binding(6) var<storage, read_write> grab: GrabState;
@group(0) @binding(7) var<uniform> params: ConstructionParams;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const GRAB_ACTIVE: u32 = 1u;
const NO_SLOT: u32 = 0xffffffffu;
const PLACE_CIRCLE: u32 = 1u;
const PLACE_RECTANGLE: u32 = 2u;
const ERASE_AT: u32 = 3u;
const RESULT_NONE: u32 = 0u;
const RESULT_PLACED: u32 = 1u;
const RESULT_ERASED: u32 = 2u;
const RESULT_CAPACITY_FULL: u32 = 3u;
const RESULT_DEEP_OVERLAP: u32 = 4u;
const RESULT_ERASE_MISS: u32 = 5u;
const RESULT_GENERATION_EXHAUSTED: u32 = 6u;

fn inverse_rotate(vector: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * vector.x + s * vector.y, -s * vector.x + c * vector.y);
}

fn target_hits_body(point: vec2f, slot: u32) -> bool {
    let pose = poses[slot];
    let property = properties[slot];
    let flags = metadata[slot].y;
    let local = inverse_rotate(point - pose.xy, pose.z);
    if ((flags & FLAG_CIRCLE) != 0u) {
        return dot(local, local) <= property.x * property.x;
    }
    if ((flags & FLAG_RECTANGLE) != 0u) {
        return all(abs(local) <= property.xy);
    }
    return false;
}

fn candidate_before(distance: f32, slot: u32, generation: u32,
                    current_distance: f32, current_slot: u32,
                    current_generation: u32) -> bool {
    return distance < current_distance ||
        (distance == current_distance &&
         (slot < current_slot ||
          (slot == current_slot && generation < current_generation)));
}

fn increment(value: u32) -> u32 {
    return select(value + 1u, value, value == 0xffffffffu);
}

fn begin_result(current: ConstructionState, kind: u32,
                sequence: u32) -> ConstructionState {
    var output = current;
    output.success_counts.x = increment(output.success_counts.x);
    output.header.z = sequence;
    output.header.w = RESULT_NONE;
    output.last_identity_kind = vec4u(NO_SLOT, 0u, kind, 0u);
    return output;
}

fn fail_result(current: ConstructionState, result: u32) -> ConstructionState {
    var output = current;
    output.header.w = result;
    if (result == RESULT_CAPACITY_FULL) {
        output.outcome_counts.y = increment(output.outcome_counts.y);
    } else if (result == RESULT_DEEP_OVERLAP) {
        output.outcome_counts.z = increment(output.outcome_counts.z);
    } else if (result == RESULT_ERASE_MISS) {
        output.outcome_counts.w = increment(output.outcome_counts.w);
    } else if (result == RESULT_GENERATION_EXHAUSTED) {
        output.extra_counts.x = increment(output.extra_counts.x);
    }
    return output;
}

fn place_body(current: ConstructionState, kind: u32,
              point: vec2f) -> ConstructionState {
    var output = current;
    var free_slot = NO_SLOT;
    var deep_overlap = false;
    for (var slot = 0u; slot < params.counts.z; slot += 1u) {
        let identity = metadata[slot];
        if ((identity.y & FLAG_LIVE) == 0u) {
            if (free_slot == NO_SLOT) {
                free_slot = slot;
            }
        } else if (target_hits_body(point, slot)) {
            deep_overlap = true;
        }
    }
    if (deep_overlap) {
        return fail_result(output, RESULT_DEEP_OVERLAP);
    }
    if (free_slot == NO_SLOT) {
        return fail_result(output, RESULT_CAPACITY_FULL);
    }
    var generation = metadata[free_slot].x;
    if (generation == 0u) {
        generation = 1u;
    }
    poses[free_slot] = vec4f(point, 0.0, 0.0);
    motions[free_slot] = vec4f(0.0);
    var flags = FLAG_LIVE;
    if (kind == PLACE_CIRCLE) {
        let radius = params.circle_material.x;
        properties[free_slot] = vec4f(
            radius,
            radius,
            params.circle_material.y,
            params.circle_material.z,
        );
        flags |= FLAG_CIRCLE;
        output.success_counts.z = increment(output.success_counts.z);
    } else {
        properties[free_slot] = params.rectangle_mass;
        flags |= FLAG_RECTANGLE;
        output.success_counts.w = increment(output.success_counts.w);
    }
    metadata[free_slot] = vec4u(
        generation,
        flags,
        bitcast<u32>(params.circle_material.w),
        bitcast<u32>(params.material_padding.x),
    );
    output.header.y = increment(output.header.y);
    output.header.w = RESULT_PLACED;
    output.last_identity_kind = vec4u(free_slot, generation, kind, 0u);
    output.success_counts.y = increment(output.success_counts.y);
    return output;
}

fn erase_body(current: ConstructionState, kind: u32,
              point: vec2f) -> ConstructionState {
    var output = current;
    var best_distance = 3.402823e+38;
    var best_slot = NO_SLOT;
    var best_generation = NO_SLOT;
    for (var slot = 0u; slot < params.counts.z; slot += 1u) {
        let identity = metadata[slot];
        if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE ||
            !target_hits_body(point, slot)) {
            continue;
        }
        let delta = point - poses[slot].xy;
        let distance = dot(delta, delta);
        if (candidate_before(distance, slot, identity.x,
                             best_distance, best_slot, best_generation)) {
            best_distance = distance;
            best_slot = slot;
            best_generation = identity.x;
        }
    }
    if (best_slot == NO_SLOT) {
        return fail_result(output, RESULT_ERASE_MISS);
    }
    if (best_generation == 0xffffffffu) {
        return fail_result(output, RESULT_GENERATION_EXHAUSTED);
    }
    if (grab.identity_status.x == best_slot &&
        grab.identity_status.y == best_generation &&
        grab.identity_status.z == GRAB_ACTIVE) {
        grab.identity_status = vec4u(NO_SLOT, 0u, 0u, output.header.z);
        output.extra_counts.y = increment(output.extra_counts.y);
    }
    poses[best_slot] = vec4f(0.0);
    motions[best_slot] = vec4f(0.0);
    properties[best_slot] = vec4f(0.0);
    metadata[best_slot] = vec4u(best_generation + 1u, 0u, 0u, 0u);
    output.header.y -= 1u;
    output.header.w = RESULT_ERASED;
    output.last_identity_kind = vec4u(best_slot, best_generation, kind, 0u);
    output.success_counts.y = increment(output.success_counts.y);
    output.outcome_counts.x = increment(output.outcome_counts.x);
    return output;
}

@compute @workgroup_size(1)
fn apply_construction_intents(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    let end = params.counts.x + params.counts.y;
    var current = state;
    for (var index = params.counts.x; index < end; index += 1u) {
        let intent = intents[index];
        let kind = intent.kind_sequence_padding.x;
        current = begin_result(current, kind, intent.kind_sequence_padding.y);
        if (kind == PLACE_CIRCLE || kind == PLACE_RECTANGLE) {
            current = place_body(current, kind, intent.target_padding.xy);
        } else if (kind == ERASE_AT) {
            current = erase_body(current, kind, intent.target_padding.xy);
        }
    }
    state = current;
}
