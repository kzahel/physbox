struct RenderParams {
    viewport: vec4f,
    camera: vec4f,
    sleep_visualization: vec4u,
};

struct SleepStateV1 {
    identity_flags: vec4u,
    provenance: vec4u,
};

struct GrabState {
    identity_status: vec4u,
    local_anchor_target: vec4f,
    counters: vec4u,
    activity: vec4u,
};

struct BodyVertexOutput {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) flags: u32,
    @location(2) @interpolate(flat) sleep_visual_state: u32,
    @location(3) @interpolate(flat) quiet_progress: f32,
};

struct GrabVertexOutput {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) primitive: u32,
    @location(2) @interpolate(flat) is_active: u32,
};

@group(0) @binding(0) var<storage, read> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read> properties: array<vec4f>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<uniform> params: RenderParams;
@group(0) @binding(4) var<storage, read> active_bodies: array<u32>;
@group(0) @binding(5) var<storage, read> sleep_states: array<SleepStateV1>;
@group(0) @binding(6) var<storage, read> grab: GrabState;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const SLEEP_FLAG_AWAKE: u32 = 1u << 0u;
const SLEEP_STATE_LAYOUT_VERSION: u32 = 1u;
const CAUSE_INITIALIZATION: u32 = 1u << 0u;
const CAUSE_CONFIGURATION: u32 = 1u << 1u;
const CAUSE_AUTOMATIC: u32 = 1u << 9u;
const GRAB_ACTIVE: u32 = 1u;
const VISUAL_NORMAL: u32 = 0u;
const VISUAL_QUIET: u32 = 1u;
const VISUAL_ASLEEP: u32 = 2u;
const VISUAL_RECENTLY_WOKEN: u32 = 3u;

@vertex
fn background_vertex(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    return vec4f(x, y, 0.0, 1.0);
}

@fragment
fn background_fragment(@builtin(position) pixel: vec4f) -> @location(0) vec4f {
    let size = max(params.viewport.xy, vec2f(1.0));
    let clip = vec2f(pixel.x / size.x * 2.0 - 1.0,
                     1.0 - pixel.y / size.y * 2.0);
    let world = params.camera.xy + clip * params.camera.zw;
    let horizon = clamp(0.48 + world.y * 0.18, 0.0, 1.0);
    var color = mix(vec3f(0.018, 0.035, 0.085), vec3f(0.075, 0.16, 0.25), horizon);
    let grid_x = abs(fract(world.x * 5.0 + 0.5) - 0.5);
    let grid_y = abs(fract(world.y * 5.0 + 0.5) - 0.5);
    let grid = 1.0 - smoothstep(0.012, 0.025, min(grid_x, grid_y));
    color += vec3f(0.03, 0.055, 0.08) * grid;
    return vec4f(color, 1.0);
}

@vertex
fn body_vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> BodyVertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    let body_index = active_bodies[20u + instance_index];
    let identity = metadata[body_index];
    let flags = identity.y;
    let local = corners[vertex_index];
    if ((flags & FLAG_LIVE) == 0u) {
        return BodyVertexOutput(
            vec4f(2.0, 2.0, 0.0, 1.0),
            local,
            flags,
            VISUAL_NORMAL,
            0.0,
        );
    }
    var sleep_visual_state = VISUAL_NORMAL;
    var quiet_progress = 0.0;
    if (params.sleep_visualization.x != 0u && (flags & FLAG_FIXED) == 0u) {
        let actively_grabbed = grab.identity_status.x == body_index &&
            grab.identity_status.y == identity.x &&
            grab.identity_status.z == GRAB_ACTIVE;
        let state = sleep_states[body_index];
        let state_flags = state.identity_flags.y;
        let valid = state.identity_flags.x == identity.x &&
            state.provenance.z == SLEEP_STATE_LAYOUT_VERSION;
        if (actively_grabbed) {
            sleep_visual_state = VISUAL_RECENTLY_WOKEN;
        } else if (valid && (state_flags & SLEEP_FLAG_AWAKE) == 0u) {
            sleep_visual_state = VISUAL_ASLEEP;
        } else if (valid) {
            let transition_causes = state.provenance.x &
                ~(CAUSE_INITIALIZATION | CAUSE_CONFIGURATION | CAUSE_AUTOMATIC);
            let transition_age = params.sleep_visualization.y - state.identity_flags.w;
            let recently_woken = state.identity_flags.w != 0u &&
                transition_causes != 0u &&
                transition_age <= params.sleep_visualization.w;
            if (recently_woken) {
                sleep_visual_state = VISUAL_RECENTLY_WOKEN;
            } else if (state.identity_flags.z != 0u) {
                sleep_visual_state = VISUAL_QUIET;
                quiet_progress = clamp(
                    f32(state.identity_flags.z) /
                        f32(max(params.sleep_visualization.z, 1u)),
                    0.0,
                    1.0,
                );
            }
        }
    }
    let pose = poses[body_index];
    let half_extents = properties[body_index].xy;
    let scaled = local * half_extents;
    let c = cos(pose.z);
    let s = sin(pose.z);
    let world = pose.xy + vec2f(c * scaled.x - s * scaled.y,
                                s * scaled.x + c * scaled.y);
    let camera_half_extents = max(params.camera.zw, vec2f(0.0001));
    let clip = (world - params.camera.xy) / camera_half_extents;
    return BodyVertexOutput(
        vec4f(clip, 0.0, 1.0),
        local,
        flags,
        sleep_visual_state,
        quiet_progress,
    );
}

fn segment_distance(point: vec2f, start: vec2f, end: vec2f) -> f32 {
    let line = end - start;
    let projection = clamp(dot(point - start, line) / max(dot(line, line), 0.0001), 0.0, 1.0);
    return length(point - (start + projection * line));
}

fn z_mark(local: vec2f) -> f32 {
    let top = segment_distance(local, vec2f(-0.34, 0.28), vec2f(0.34, 0.28));
    let slash = segment_distance(local, vec2f(0.34, 0.28), vec2f(-0.34, -0.28));
    let bottom = segment_distance(local, vec2f(-0.34, -0.28), vec2f(0.34, -0.28));
    return 1.0 - smoothstep(0.055, 0.095, min(top, min(slash, bottom)));
}

fn wake_mark(local: vec2f) -> f32 {
    let stem = segment_distance(local, vec2f(0.0, 0.34), vec2f(0.0, -0.12));
    let dot = length(local - vec2f(0.0, -0.34));
    return 1.0 - smoothstep(0.055, 0.095, min(stem, dot));
}

@fragment
fn body_fragment(input: BodyVertexOutput) -> @location(0) vec4f {
    let circle = (input.flags & FLAG_CIRCLE) != 0u;
    if (circle && dot(input.local, input.local) > 1.0) {
        discard;
    }
    let rectangle_edge = 1.0 - max(abs(input.local.x), abs(input.local.y));
    let circle_edge = 1.0 - length(input.local);
    let edge_distance = select(rectangle_edge, circle_edge, circle);
    let rim = 1.0 - smoothstep(0.0, 0.10, edge_distance);
    let fixed = (input.flags & FLAG_FIXED) != 0u;
    var low = select(vec3f(0.10, 0.76, 0.88), vec3f(0.12, 0.20, 0.28), fixed);
    var high = select(vec3f(0.44, 0.96, 0.78), vec3f(0.24, 0.34, 0.42), fixed);
    var rim_color = select(vec3f(0.55, 1.0, 0.92), vec3f(0.34, 0.46, 0.56), fixed);
    if (input.sleep_visual_state == VISUAL_QUIET) {
        low = vec3f(0.82, 0.34, 0.035);
        high = vec3f(1.0, 0.73, 0.13);
        rim_color = vec3f(1.0, 0.92, 0.46);
    } else if (input.sleep_visual_state == VISUAL_ASLEEP) {
        low = vec3f(0.20, 0.23, 0.55);
        high = vec3f(0.40, 0.52, 0.86);
        rim_color = vec3f(0.72, 0.82, 1.0);
    } else if (input.sleep_visual_state == VISUAL_RECENTLY_WOKEN) {
        low = vec3f(0.78, 0.04, 0.38);
        high = vec3f(1.0, 0.32, 0.62);
        rim_color = vec3f(1.0, 0.88, 0.32);
    }
    var color = mix(low, high, clamp(input.local.y * 0.5 + 0.5, 0.0, 1.0));
    color += rim_color * rim * 0.48;
    if (input.sleep_visual_state == VISUAL_QUIET) {
        let bar = 1.0 - smoothstep(
            0.045,
            0.085,
            segment_distance(input.local, vec2f(-0.42, -0.48), vec2f(0.42, -0.48)),
        );
        let progress_edge = input.quiet_progress * 0.84 - 0.42;
        let filled = select(0.15, 1.0, input.local.x <= progress_edge);
        color = mix(color, vec3f(1.0, 0.98, 0.72), bar * filled * 0.9);
    } else if (input.sleep_visual_state == VISUAL_ASLEEP) {
        color = mix(color, vec3f(0.94, 0.97, 1.0), z_mark(input.local) * 0.88);
    } else if (input.sleep_visual_state == VISUAL_RECENTLY_WOKEN) {
        color = mix(color, vec3f(1.0, 0.98, 0.64), wake_mark(input.local) * 0.92);
    }
    return vec4f(color, 1.0);
}

fn world_to_clip(world: vec2f) -> vec4f {
    let camera_half_extents = max(params.camera.zw, vec2f(0.0001));
    return vec4f((world - params.camera.xy) / camera_half_extents, 0.0, 1.0);
}

@vertex
fn grab_vertex(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) world_active: vec4f,
) -> GrabVertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    if (world_active.z < 0.5) {
        return GrabVertexOutput(vec4f(2.0, 2.0, 0.0, 1.0), vec2f(0.0), 0u, 0u);
    }
    let primitive = u32(vertex_index >= 6u);
    let local = corners[vertex_index % 6u];
    return GrabVertexOutput(world_to_clip(world_active.xy), local, primitive, 1u);
}

@fragment
fn grab_fragment(input: GrabVertexOutput) -> @location(0) vec4f {
    if (input.is_active == 0u || (input.primitive == 1u && dot(input.local, input.local) > 1.0)) {
        discard;
    }
    let color = select(vec3f(0.22, 0.92, 1.0), vec3f(0.72, 1.0, 0.88),
                       input.primitive == 1u);
    return vec4f(color, select(0.72, 0.94, input.primitive == 1u));
}
