struct RenderParams {
    viewport: vec4f,
    camera: vec4f,
};

struct BodyVertexOutput {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) flags: u32,
};

@group(0) @binding(0) var<storage, read> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read> properties: array<vec4f>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<uniform> params: RenderParams;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;
const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;

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
    let identity = metadata[instance_index];
    let flags = identity.y;
    let local = corners[vertex_index];
    if ((flags & FLAG_LIVE) == 0u) {
        return BodyVertexOutput(vec4f(2.0, 2.0, 0.0, 1.0), local, flags);
    }
    let pose = poses[instance_index];
    let half_extents = properties[instance_index].xy;
    let scaled = local * half_extents;
    let c = cos(pose.z);
    let s = sin(pose.z);
    let world = pose.xy + vec2f(c * scaled.x - s * scaled.y,
                                s * scaled.x + c * scaled.y);
    let camera_half_extents = max(params.camera.zw, vec2f(0.0001));
    let clip = (world - params.camera.xy) / camera_half_extents;
    return BodyVertexOutput(vec4f(clip, 0.0, 1.0), local, flags);
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
    let low = select(vec3f(0.10, 0.76, 0.88), vec3f(0.12, 0.20, 0.28), fixed);
    let high = select(vec3f(0.44, 0.96, 0.78), vec3f(0.24, 0.34, 0.42), fixed);
    var color = mix(low, high, clamp(input.local.y * 0.5 + 0.5, 0.0, 1.0));
    color += select(vec3f(0.55, 1.0, 0.92), vec3f(0.34, 0.46, 0.56), fixed) * rim * 0.48;
    return vec4f(color, 1.0);
}
