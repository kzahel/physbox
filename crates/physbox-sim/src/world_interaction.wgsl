struct WorldCommand {
    header: vec4u,
    handles: vec4u,
    data0: vec4f,
    data1: vec4f,
};

struct BodySelection {
    identity_status: vec4u,
    point_padding: vec4f,
};

struct PointConstraint {
    identity_status: vec4u,
};

struct WorldResult {
    command_status_kind_body: vec4u,
    identity_handles: vec4u,
};

struct WorldInteractionStorage {
    header: vec4u,
    commands: array<WorldCommand, 64>,
    selections: array<BodySelection, 64>,
    constraint: PointConstraint,
    results: array<WorldResult, 64>,
};

struct GrabState {
    identity_status: vec4u,
    local_anchor_target: vec4f,
    counters: vec4u,
    activity: vec4u,
    last_selection: vec4u,
};

struct GrabParams {
    dt_tuning: vec4f,
    limits_padding: vec4f,
    counts: vec4u,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> properties: array<vec4f>;
@group(0) @binding(4) var<storage, read_write> grab: GrabState;
@group(0) @binding(5) var<storage, read_write> wake_requests: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> interaction: WorldInteractionStorage;
@group(0) @binding(7) var<storage, read_write> grab_params: GrabParams;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const GRAB_ACTIVE: u32 = 1u;
const NO_SLOT: u32 = 0xffffffffu;
const CAUSE_GRAB: u32 = 1u << 2u;

const KIND_POINT_QUERY: u32 = 1u;
const KIND_CREATE_POINT_CONSTRAINT: u32 = 2u;
const KIND_SET_POINT_CONSTRAINT_TARGET: u32 = 3u;
const KIND_DESTROY_POINT_CONSTRAINT: u32 = 4u;
const KIND_DESTROY_SELECTED_BODY: u32 = 5u;

const STATUS_SUCCEEDED: u32 = 1u;
const STATUS_INVALID: u32 = 2u;
const STATUS_MISSED: u32 = 3u;
const STATUS_CANCELLED: u32 = 4u;
const STATUS_FAILED: u32 = 6u;

const SELECTION_HIT: u32 = 1u;
const SELECTION_MISS: u32 = 2u;
const CONSTRAINT_ACTIVE: u32 = 1u;
const CONSTRAINT_FAILED: u32 = 2u;

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

fn write_result(index: u32, command: WorldCommand, status: u32,
                body_slot: u32, body_generation: u32) {
    interaction.results[index].command_status_kind_body =
        vec4u(command.header.y, status, command.header.x, body_slot);
    interaction.results[index].identity_handles =
        vec4u(body_generation, command.handles.x, command.handles.z, command.handles.w);
}

fn process_point_query(command: WorldCommand, result_index: u32) {
    let selection_slot = command.handles.x;
    let selection_generation = command.handles.y;
    if (selection_slot >= 64u || selection_generation == 0u) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    let point = command.data0.xy;
    var best_distance = 3.402823e+38;
    var best_slot = NO_SLOT;
    var best_generation = NO_SLOT;
    for (var slot = 0u; slot < interaction.header.y; slot += 1u) {
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
        interaction.selections[selection_slot].identity_status =
            vec4u(selection_generation, SELECTION_MISS, NO_SLOT, 0u);
        interaction.selections[selection_slot].point_padding = vec4f(point, 0.0, 0.0);
        write_result(result_index, command, STATUS_MISSED, NO_SLOT, 0u);
        return;
    }
    interaction.selections[selection_slot].identity_status =
        vec4u(selection_generation, SELECTION_HIT, best_slot, best_generation);
    interaction.selections[selection_slot].point_padding = vec4f(point, 0.0, 0.0);
    write_result(result_index, command, STATUS_SUCCEEDED, best_slot, best_generation);
}

fn process_create_constraint(command: WorldCommand, result_index: u32) {
    let selection_slot = command.handles.x;
    let selection_generation = command.handles.y;
    let constraint_slot = command.handles.z;
    let constraint_generation = command.handles.w;
    if (constraint_slot != 0u || constraint_generation == 0u ||
        selection_slot >= 64u) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    interaction.constraint.identity_status =
        vec4u(constraint_generation, CONSTRAINT_FAILED, NO_SLOT, 0u);
    let selection = interaction.selections[selection_slot];
    if (selection.identity_status.x != selection_generation) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    if (selection.identity_status.y == SELECTION_MISS) {
        write_result(result_index, command, STATUS_MISSED, NO_SLOT, 0u);
        return;
    }
    if (selection.identity_status.y != SELECTION_HIT) {
        write_result(result_index, command, STATUS_FAILED, NO_SLOT, 0u);
        return;
    }
    let body_slot = selection.identity_status.z;
    let body_generation = selection.identity_status.w;
    if (body_slot >= interaction.header.y ||
        metadata[body_slot].x != body_generation ||
        (metadata[body_slot].y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
        write_result(result_index, command, STATUS_INVALID, body_slot, body_generation);
        return;
    }
    let pose = poses[body_slot];
    grab.identity_status =
        vec4u(body_slot, body_generation, GRAB_ACTIVE, command.header.y);
    grab.local_anchor_target = vec4f(
        inverse_rotate(selection.point_padding.xy - pose.xy, pose.z),
        command.data0.xy,
    );
    grab.last_selection =
        vec4u(command.header.y, body_slot, body_generation, 1u);
    grab_params.dt_tuning =
        vec4f(grab_params.dt_tuning.x, command.data0.z, command.data0.w, command.data1.x);
    grab_params.limits_padding =
        vec4f(command.data1.y, command.data1.z, 0.0, 0.0);
    atomicOr(&wake_requests[body_slot], CAUSE_GRAB);
    interaction.constraint.identity_status =
        vec4u(constraint_generation, CONSTRAINT_ACTIVE, body_slot, body_generation);
    write_result(result_index, command, STATUS_SUCCEEDED, body_slot, body_generation);
}

fn process_target(command: WorldCommand, result_index: u32) {
    let constraint_generation = command.handles.w;
    let constraint = interaction.constraint.identity_status;
    if (constraint.x != constraint_generation) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    if (constraint.y == CONSTRAINT_FAILED) {
        write_result(result_index, command, STATUS_CANCELLED, NO_SLOT, 0u);
        return;
    }
    if (constraint.y != CONSTRAINT_ACTIVE ||
        grab.identity_status.x != constraint.z ||
        grab.identity_status.y != constraint.w ||
        grab.identity_status.z != GRAB_ACTIVE) {
        write_result(result_index, command, STATUS_INVALID, constraint.z, constraint.w);
        return;
    }
    grab.local_anchor_target =
        vec4f(grab.local_anchor_target.xy, command.data0.xy);
    grab.identity_status.w = command.header.y;
    grab.activity.x += 1u;
    write_result(result_index, command, STATUS_SUCCEEDED, constraint.z, constraint.w);
}

fn process_destroy_constraint(command: WorldCommand, result_index: u32) {
    let constraint_generation = command.handles.w;
    let constraint = interaction.constraint.identity_status;
    if (constraint.x != constraint_generation) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    if (constraint.y == CONSTRAINT_FAILED) {
        interaction.constraint.identity_status =
            vec4u(constraint_generation, 0u, NO_SLOT, 0u);
        write_result(result_index, command, STATUS_CANCELLED, NO_SLOT, 0u);
        return;
    }
    if (constraint.y != CONSTRAINT_ACTIVE) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    if (grab.identity_status.x == constraint.z &&
        grab.identity_status.y == constraint.w) {
        grab.identity_status =
            vec4u(NO_SLOT, 0u, 0u, command.header.y);
        grab.activity.y += 1u;
    }
    interaction.constraint.identity_status =
        vec4u(constraint_generation, 0u, NO_SLOT, 0u);
    write_result(result_index, command, STATUS_SUCCEEDED, constraint.z, constraint.w);
}

fn process_destroy_selected_body(command: WorldCommand, result_index: u32) {
    let selection_slot = command.handles.x;
    let selection_generation = command.handles.y;
    if (selection_slot >= 64u) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    let selection = interaction.selections[selection_slot];
    if (selection.identity_status.x != selection_generation) {
        write_result(result_index, command, STATUS_INVALID, NO_SLOT, 0u);
        return;
    }
    if (selection.identity_status.y == SELECTION_MISS) {
        write_result(result_index, command, STATUS_MISSED, NO_SLOT, 0u);
        return;
    }
    if (selection.identity_status.y != SELECTION_HIT) {
        write_result(result_index, command, STATUS_FAILED, NO_SLOT, 0u);
        return;
    }
    let body_slot = selection.identity_status.z;
    let body_generation = selection.identity_status.w;
    if (body_slot >= interaction.header.y ||
        metadata[body_slot].x != body_generation ||
        (metadata[body_slot].y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
        write_result(result_index, command, STATUS_INVALID, body_slot, body_generation);
        return;
    }
    if (body_generation == 0xffffffffu) {
        write_result(result_index, command, STATUS_FAILED, body_slot, body_generation);
        return;
    }
    if (grab.identity_status.x == body_slot &&
        grab.identity_status.y == body_generation) {
        grab.identity_status =
            vec4u(NO_SLOT, 0u, 0u, command.header.y);
    }
    if (interaction.constraint.identity_status.y == CONSTRAINT_ACTIVE &&
        interaction.constraint.identity_status.z == body_slot &&
        interaction.constraint.identity_status.w == body_generation) {
        interaction.constraint.identity_status.y = CONSTRAINT_FAILED;
    }
    poses[body_slot] = vec4f(0.0);
    motions[body_slot] = vec4f(0.0);
    properties[body_slot] = vec4f(0.0);
    metadata[body_slot] = vec4u(body_generation + 1u, 0u, 0u, 0u);
    write_result(result_index, command, STATUS_SUCCEEDED, body_slot, body_generation);
}

@compute @workgroup_size(1)
fn apply_world_commands(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) {
        return;
    }
    for (var index = 0u; index < interaction.header.x; index += 1u) {
        let command = interaction.commands[index];
        let result_index = command.header.z;
        if (result_index >= 64u) {
            continue;
        }
        if (command.header.x == KIND_POINT_QUERY) {
            process_point_query(command, result_index);
        } else if (command.header.x == KIND_CREATE_POINT_CONSTRAINT) {
            process_create_constraint(command, result_index);
        } else if (command.header.x == KIND_SET_POINT_CONSTRAINT_TARGET) {
            process_target(command, result_index);
        } else if (command.header.x == KIND_DESTROY_POINT_CONSTRAINT) {
            process_destroy_constraint(command, result_index);
        } else if (command.header.x == KIND_DESTROY_SELECTED_BODY) {
            process_destroy_selected_body(command, result_index);
        } else {
            write_result(result_index, command, STATUS_FAILED, NO_SLOT, 0u);
        }
    }
}
