struct BodyCommand {
    kind_slot_generation_sequence: vec4u,
    data0: vec4f,
    data1: vec4f,
};

struct GrabIntent {
    kind_sequence_padding: vec4u,
    target_padding: vec4f,
};

struct GrabState {
    identity_status: vec4u,
    local_anchor_target: vec4f,
    counters: vec4u,
    activity: vec4u,
    last_selection: vec4u,
};

struct CommandParams {
    counts: vec4u,
};

struct GrabParams {
    dt_tuning: vec4f,
    limits_padding: vec4f,
    counts: vec4u,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read> commands: array<BodyCommand>;
@group(0) @binding(4) var<uniform> params: CommandParams;
@group(0) @binding(5) var<storage, read> properties: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> grab: GrabState;
@group(0) @binding(7) var<storage, read> intents: array<GrabIntent>;
@group(0) @binding(8) var<uniform> grab_params: GrabParams;
@group(0) @binding(9) var<storage, read_write> wake_requests: array<atomic<u32>>;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const GRAB_ACTIVE: u32 = 1u;
const NO_SLOT: u32 = 0xffffffffu;
const COMMAND_RESET: u32 = 1u;
const COMMAND_GRAB_BEGIN: u32 = 2u;
const COMMAND_GRAB_TARGET: u32 = 3u;
const COMMAND_GRAB_END: u32 = 4u;
const COMMAND_GRAB_CLEAR: u32 = 5u;
const INTENT_BEGIN: u32 = 1u;
const INTENT_TARGET: u32 = 2u;
const INTENT_END: u32 = 3u;
const CAUSE_GRAB: u32 = 1u << 2u;
const CAUSE_RESET: u32 = 1u << 3u;

fn rotate(vector: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * vector.x - s * vector.y, s * vector.x + c * vector.y);
}

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

fn clear_active(sequence: u32) {
    grab.identity_status = vec4u(NO_SLOT, 0u, 0u, sequence);
}

fn activate_slot(slot: u32, point: vec2f, sequence: u32) {
    let pose = poses[slot];
    grab.identity_status = vec4u(slot, metadata[slot].x, GRAB_ACTIVE, sequence);
    grab.local_anchor_target = vec4f(
        inverse_rotate(point - pose.xy, pose.z),
        point,
    );
    atomicOr(&wake_requests[slot], CAUSE_GRAB);
}

fn begin_specific(slot: u32, generation: u32, point: vec2f, sequence: u32) {
    grab.counters.x += 1u;
    clear_active(sequence);
    grab.last_selection = vec4u(sequence, NO_SLOT, 0u, 0u);
    if (slot >= grab_params.counts.x) {
        grab.counters.z += 1u;
        return;
    }
    let identity = metadata[slot];
    if (identity.x != generation || (identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
        grab.counters.z += 1u;
        if ((identity.y & (FLAG_LIVE | FLAG_FIXED)) == (FLAG_LIVE | FLAG_FIXED)) {
            grab.counters.w += 1u;
        }
        return;
    }
    activate_slot(slot, point, sequence);
    grab.last_selection = vec4u(sequence, slot, generation, 1u);
    grab.counters.y += 1u;
}

fn target_specific(slot: u32, generation: u32, point: vec2f, sequence: u32) {
    if (grab.identity_status.x == slot && grab.identity_status.y == generation) {
        grab.local_anchor_target = vec4f(grab.local_anchor_target.xy, point);
        grab.identity_status.w = sequence;
        grab.activity.x += 1u;
    }
}

fn end_specific(slot: u32, generation: u32, sequence: u32) {
    grab.activity.y += 1u;
    if (grab.identity_status.x == slot && grab.identity_status.y == generation) {
        clear_active(sequence);
    }
}

fn process_commands() {
    let start = params.counts.x;
    let end = start + params.counts.y;
    for (var command_index = start; command_index < end; command_index += 1u) {
        let command = commands[command_index];
        let kind = command.kind_slot_generation_sequence.x;
        let slot = command.kind_slot_generation_sequence.y;
        let generation = command.kind_slot_generation_sequence.z;
        let sequence = command.kind_slot_generation_sequence.w;
        if (kind == COMMAND_GRAB_CLEAR) {
            grab.activity.y += 1u;
            clear_active(sequence);
            continue;
        }
        if (slot >= grab_params.counts.x) { continue; }
        let identity = metadata[slot];
        if (identity.x != generation || (identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
            continue;
        }
        if (kind == COMMAND_RESET) {
            poses[slot] = command.data0;
            motions[slot] = command.data1;
            atomicOr(&wake_requests[slot], CAUSE_RESET);
            if (grab.identity_status.x == slot && grab.identity_status.y == generation) {
                clear_active(sequence);
            }
        } else if (kind == COMMAND_GRAB_BEGIN) {
            begin_specific(slot, generation, command.data0.xy, sequence);
        } else if (kind == COMMAND_GRAB_TARGET) {
            target_specific(slot, generation, command.data0.xy, sequence);
        } else if (kind == COMMAND_GRAB_END) {
            end_specific(slot, generation, sequence);
        }
    }
}

fn process_grab_intents() {
    let start = params.counts.z;
    let end = start + params.counts.w;
    for (var intent_index = start; intent_index < end; intent_index += 1u) {
        let intent = intents[intent_index];
        let kind = intent.kind_sequence_padding.x;
        let sequence = intent.kind_sequence_padding.y;
        let point = intent.target_padding.xy;
        if (kind == INTENT_BEGIN) {
            var best_distance = 3.402823e+38;
            var best_slot = NO_SLOT;
            var best_generation = NO_SLOT;
            var saw_fixed = 0u;
            for (var slot = 0u; slot < grab_params.counts.x; slot += 1u) {
                let identity = metadata[slot];
                if ((identity.y & FLAG_LIVE) == 0u || !target_hits_body(point, slot)) {
                    continue;
                }
                if ((identity.y & FLAG_FIXED) != 0u) {
                    saw_fixed = 1u;
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
            grab.counters.x += 1u;
            grab.activity.w = max(grab.activity.w, grab_params.counts.x);
            clear_active(sequence);
            grab.last_selection = vec4u(sequence, NO_SLOT, 0u, 0u);
            if (best_slot == NO_SLOT) {
                grab.identity_status.x = NO_SLOT;
                grab.identity_status.y = 0u;
                grab.counters.z += 1u;
                grab.counters.w += saw_fixed;
            } else {
                activate_slot(best_slot, point, sequence);
                grab.last_selection = vec4u(sequence, best_slot, best_generation, 1u);
                grab.counters.y += 1u;
            }
        } else {
            if (kind == INTENT_TARGET && grab.identity_status.x != NO_SLOT) {
                grab.local_anchor_target = vec4f(grab.local_anchor_target.xy, point);
                grab.identity_status.w = sequence;
                grab.activity.x += 1u;
            } else if (kind == INTENT_END) {
                grab.activity.y += 1u;
                clear_active(sequence);
            }
        }
    }
}

fn apply_grab_constraint() {
    if (grab.identity_status.x == NO_SLOT) { return; }
    let slot = grab.identity_status.x;
    if (slot >= grab_params.counts.x) {
        clear_active(grab.identity_status.w);
        grab.activity.z += 1u;
        return;
    }
    let identity = metadata[slot];
    if (identity.x != grab.identity_status.y ||
        (identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
        clear_active(grab.identity_status.w);
        grab.activity.z += 1u;
        return;
    }
    atomicOr(&wake_requests[slot], CAUSE_GRAB);
    let pose = poses[slot];
    var motion = motions[slot];
    let property = properties[slot];
    let world_offset = rotate(grab.local_anchor_target.xy, pose.z);
    let world_anchor = pose.xy + world_offset;
    let anchor_velocity = motion.xy + vec2f(-motion.z * world_offset.y,
                                             motion.z * world_offset.x);
    var acceleration = (grab.local_anchor_target.zw - world_anchor) * grab_params.dt_tuning.y
        - anchor_velocity * grab_params.dt_tuning.z;
    let acceleration_length = length(acceleration);
    if (acceleration_length > grab_params.dt_tuning.w) {
        acceleration *= grab_params.dt_tuning.w / acceleration_length;
    }
    let inverse_mass = property.z;
    if (inverse_mass <= 0.0) {
        clear_active(grab.identity_status.w);
        grab.activity.z += 1u;
        return;
    }
    let impulse = acceleration * grab_params.dt_tuning.x / inverse_mass;
    motion = vec4f(motion.xy + impulse * inverse_mass, motion.zw);
    motion.z += property.w * (world_offset.x * impulse.y - world_offset.y * impulse.x);
    let linear_speed = length(motion.xy);
    if (linear_speed > grab_params.limits_padding.x) {
        motion = vec4f(motion.xy * (grab_params.limits_padding.x / linear_speed), motion.zw);
    }
    motion.z = clamp(motion.z, -grab_params.limits_padding.y, grab_params.limits_padding.y);
    motions[slot] = motion;
}

@compute @workgroup_size(1)
fn apply_step_commands_and_grab(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    process_commands();
    process_grab_intents();
    apply_grab_constraint();
}

@compute @workgroup_size(1)
fn steer_grab(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    apply_grab_constraint();
}
