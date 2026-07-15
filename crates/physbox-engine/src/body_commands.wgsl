struct BodyCommand {
    kind_slot_generation_sequence: vec4u,
    data0: vec4f,
    data1: vec4f,
};

struct CommandParams {
    counts: vec4u,
};

@group(0) @binding(0) var<storage, read_write> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> motions: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read> commands: array<BodyCommand>;
@group(0) @binding(4) var<uniform> params: CommandParams;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_GRABBED: u32 = 1u << 3u;
const COMMAND_RESET: u32 = 1u;
const COMMAND_GRAB_BEGIN: u32 = 2u;
const COMMAND_GRAB_TARGET: u32 = 3u;
const COMMAND_GRAB_END: u32 = 4u;

@compute @workgroup_size(1)
fn apply_commands(@builtin(global_invocation_id) id: vec3u) {
    if (id.x != 0u) { return; }
    let start = params.counts.x;
    let end = start + params.counts.y;
    for (var command_index = start; command_index < end; command_index += 1u) {
        let command = commands[command_index];
        let kind = command.kind_slot_generation_sequence.x;
        let slot = command.kind_slot_generation_sequence.y;
        let generation = command.kind_slot_generation_sequence.z;
        if (slot >= params.counts.z) { continue; }
        var identity = metadata[slot];
        if (identity.x != generation || (identity.y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
            continue;
        }
        var pose = poses[slot];
        var motion = motions[slot];
        if (kind == COMMAND_RESET) {
            pose = command.data0;
            motion = command.data1;
            identity.y &= ~FLAG_GRABBED;
        } else if (kind == COMMAND_GRAB_BEGIN || kind == COMMAND_GRAB_TARGET) {
            pose.x = command.data0.x;
            pose.y = command.data0.y;
            pose.w = command.data0.x;
            motion = vec4f(0.0, 0.0, 0.0, command.data0.y);
            identity.y |= FLAG_GRABBED;
        } else if (kind == COMMAND_GRAB_END) {
            motion = vec4f(0.0);
            identity.y &= ~FLAG_GRABBED;
        }
        poses[slot] = pose;
        motions[slot] = motion;
        metadata[slot] = identity;
    }
}

@compute @workgroup_size(64)
fn hold_grabbed(@builtin(global_invocation_id) id: vec3u) {
    let slot = id.x;
    if (slot >= params.counts.z) { return; }
    let identity = metadata[slot];
    if ((identity.y & FLAG_GRABBED) == 0u) { return; }
    var pose = poses[slot];
    var motion = motions[slot];
    pose.x = pose.w;
    pose.y = motion.w;
    motion.x = 0.0;
    motion.y = 0.0;
    motion.z = 0.0;
    poses[slot] = pose;
    motions[slot] = motion;
}
