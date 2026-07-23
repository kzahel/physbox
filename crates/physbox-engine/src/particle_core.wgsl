struct ParticleParams {
    dt_gravity: vec4f,
    material: vec4f,
    pressure: vec4f,
    grid: vec4f,
    counts: vec4u,
    limits: vec4u,
};

@group(0) @binding(0) var<storage, read_write> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read_write> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read_write> active_flags: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> active_indices: array<u32>;
@group(0) @binding(5) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> indirect: array<u32>;
@group(0) @binding(7) var<uniform> params: ParticleParams;

const COUNTER_ACTIVE: u32 = 0u;
const COUNTER_OVERFLOW_FLAGS: u32 = 1u;
const COUNTER_GRID_INSERTS: u32 = 2u;
const COUNTER_GRID_OVERFLOW: u32 = 3u;
const COUNTER_NEIGHBOR_CHECKS: u32 = 4u;
const COUNTER_ACCEPTED_NEIGHBORS: u32 = 5u;
const COUNTER_BODY_CONTACTS: u32 = 6u;
const COUNTER_ESCAPED: u32 = 7u;
const COUNTER_ALLOCATION_CURSOR: u32 = 8u;
const COUNTER_SPAWNED: u32 = 9u;
const COUNTER_ERASED: u32 = 10u;
const COUNTER_KILLED: u32 = 11u;
const COUNTER_COMMAND_DROPS: u32 = 12u;
const COUNTER_CORRECTIONS: u32 = 13u;
const COUNTER_CLAMPED_CORRECTIONS: u32 = 14u;
const COUNTER_DISPATCHES: u32 = 15u;
const COUNTER_BODY_CONTRIBUTIONS: u32 = 16u;
const COUNTER_FLUID_WAKE_ROOTS: u32 = 17u;
const COUNTER_BODY_CONTRIBUTION_OVERFLOW: u32 = 18u;
const COUNTER_BOUNDARY_DENSITY_CONTACTS: u32 = 19u;
const COUNTER_BOUNDARY_PRESSURE_EDGES: u32 = 20u;

@compute @workgroup_size(64)
fn begin_step(@builtin(global_invocation_id) invocation: vec3u) {
    if invocation.x != 0u {
        return;
    }
    atomicStore(&counters[COUNTER_OVERFLOW_FLAGS], 0u);
    atomicStore(&counters[COUNTER_GRID_INSERTS], 0u);
    atomicStore(&counters[COUNTER_GRID_OVERFLOW], 0u);
    atomicStore(&counters[COUNTER_NEIGHBOR_CHECKS], 0u);
    atomicStore(&counters[COUNTER_ACCEPTED_NEIGHBORS], 0u);
    atomicStore(&counters[COUNTER_BODY_CONTACTS], 0u);
    atomicStore(&counters[COUNTER_ESCAPED], 0u);
    atomicStore(&counters[COUNTER_CORRECTIONS], 0u);
    atomicStore(&counters[COUNTER_CLAMPED_CORRECTIONS], 0u);
    atomicStore(&counters[COUNTER_DISPATCHES], 0u);
    atomicStore(&counters[COUNTER_BODY_CONTRIBUTIONS], 0u);
    atomicStore(&counters[COUNTER_FLUID_WAKE_ROOTS], 0u);
    atomicStore(&counters[COUNTER_BODY_CONTRIBUTION_OVERFLOW], 0u);
    atomicStore(&counters[COUNTER_BOUNDARY_DENSITY_CONTACTS], 0u);
    atomicStore(&counters[COUNTER_BOUNDARY_PRESSURE_EDGES], 0u);
}

@compute @workgroup_size(64)
fn integrate(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let dt = params.dt_gravity.x;
    let position = positions[index];
    let velocity = velocities[index];
    let next_velocity = velocity.xy + params.dt_gravity.yz * params.dt_gravity.w * dt;
    let next_position = position.xy + next_velocity * dt;
    positions[index] = vec4f(next_position, position.xy);
    velocities[index] = vec4f(next_velocity, velocity.zw);
}

@compute @workgroup_size(64)
fn clear_active(@builtin(global_invocation_id) invocation: vec3u) {
    if invocation.x != 0u {
        return;
    }
    atomicStore(&counters[COUNTER_ACTIVE], 0u);
    indirect[0] = 6u;
    indirect[1] = 0u;
    indirect[2] = 0u;
    indirect[3] = 0u;
    indirect[4] = 0u;
    indirect[5] = 1u;
    indirect[6] = 1u;
    indirect[7] = 0u;
}

@compute @workgroup_size(64)
fn compact_active(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if index >= params.counts.x || atomicLoad(&active_flags[index]) == 0u {
        return;
    }
    let destination = atomicAdd(&counters[COUNTER_ACTIVE], 1u);
    if destination < params.counts.x {
        active_indices[destination] = index;
    } else {
        atomicOr(&counters[COUNTER_OVERFLOW_FLAGS], 1u);
    }
}

@compute @workgroup_size(64)
fn finalize_active(@builtin(global_invocation_id) invocation: vec3u) {
    if invocation.x != 0u {
        return;
    }
    let active_count = min(atomicLoad(&counters[COUNTER_ACTIVE]), params.counts.x);
    indirect[1] = active_count;
    indirect[4] = (active_count + 63u) / 64u;
}
