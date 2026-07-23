struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

struct ParticleCommand {
    kind_count_material_sequence: vec4u,
    center_radius_spacing: vec4f,
    velocity_padding: vec4f,
};

struct ParticleCommandBatch {
    counts: vec4u,
    commands: array<ParticleCommand>,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read> batch: ParticleCommandBatch;
@group(0) @binding(6) var<uniform> params: ParticleParams;

const COMMAND_EMIT_CIRCLE: u32 = 1u;
const COMMAND_ERASE_CIRCLE: u32 = 2u;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_ALLOCATION_CURSOR: u32 = 8u;
const COUNTER_SPAWNED: u32 = 9u;
const COUNTER_ERASED: u32 = 10u;
const COUNTER_COMMAND_DROPS: u32 = 12u;
const GOLDEN_ANGLE: f32 = 2.399963229728653;

fn hash(value: u32) -> u32 {
    var result = value;
    result ^= result >> 16u;
    result *= 0x7feb352du;
    result ^= result >> 15u;
    result *= 0x846ca68bu;
    result ^= result >> 16u;
    return result;
}

@compute @workgroup_size(64)
fn erase(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let command_count = min(batch.counts.x, params.limits.y);
    for (var command_index = 0u; command_index < command_count; command_index += 1u) {
        let command = batch.commands[command_index];
        if command.kind_count_material_sequence.x != COMMAND_ERASE_CIRCLE {
            continue;
        }
        let delta = positions[index].xy - command.center_radius_spacing.xy;
        if dot(delta, delta) > command.center_radius_spacing.z * command.center_radius_spacing.z {
            continue;
        }
        let removed = atomicCompareExchangeWeak(&active_flags[index], 1u, 0u);
        if removed.exchanged {
            let previous_metadata = metadata[index];
            metadata[index] = vec4u(
                previous_metadata.x + 1u,
                previous_metadata.y,
                previous_metadata.z,
                0u,
            );
            atomicAdd(&counters[COUNTER_ERASED], 1u);
        }
        return;
    }
}

@compute @workgroup_size(64)
fn spawn(@builtin(global_invocation_id) invocation: vec3u) {
    if invocation.x != 0u {
        return;
    }
    let command_count = min(batch.counts.x, params.limits.y);
    var cursor = atomicLoad(&counters[COUNTER_ALLOCATION_CURSOR]);
    for (var command_index = 0u; command_index < command_count; command_index += 1u) {
        let command = batch.commands[command_index];
        let spawn_count = command.kind_count_material_sequence.y;
        if command.kind_count_material_sequence.x != COMMAND_EMIT_CIRCLE {
            continue;
        }
        for (var spawn_index = 0u; spawn_index < spawn_count; spawn_index += 1u) {
            var selected_index = params.counts.x;
            for (var attempt = 0u; attempt < params.counts.x; attempt += 1u) {
                let index = cursor % params.counts.x;
                cursor += 1u;
                let claimed = atomicCompareExchangeWeak(&active_flags[index], 0u, 1u);
                if claimed.exchanged {
                    selected_index = index;
                    break;
                }
            }
            if selected_index == params.counts.x {
                atomicAdd(&counters[COUNTER_COMMAND_DROPS], spawn_count - spawn_index);
                atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 8u);
                break;
            }
            let phase = f32(hash(command.kind_count_material_sequence.w)) / 4294967295.0 *
                6.28318530718;
            let sample = (f32(spawn_index) + 0.5) / f32(max(spawn_count, 1u));
            let angle = phase + f32(spawn_index) * GOLDEN_ANGLE;
            let offset = vec2f(cos(angle), sin(angle)) * sqrt(sample) *
                command.center_radius_spacing.z;
            let position = command.center_radius_spacing.xy + offset;
            positions[selected_index] = vec4f(position, position);
            velocities[selected_index] = vec4f(command.velocity_padding.xy, 0.0, 0.0);
            let previous_metadata = metadata[selected_index];
            metadata[selected_index] = vec4u(
                previous_metadata.x,
                command.kind_count_material_sequence.z,
                previous_metadata.z,
                1u,
            );
            atomicAdd(&counters[COUNTER_SPAWNED], 1u);
        }
    }
    atomicStore(&counters[COUNTER_ALLOCATION_CURSOR], cursor);
}
