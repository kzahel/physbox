// Consumer-owned showcase rendering over the public PhysBox observation.

struct BodyObservation {
    pose: vec4f,
    motion: vec4f,
    identity: vec4u,
};

struct ShapeVisual {
    geometry: vec4f,
    color: vec4f,
    identity: vec4u,
};

struct RenderParams {
    camera: vec4f,
    viewport: vec4f,
    material: vec4f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) color: vec4f,
    @location(2) @interpolate(flat) kind: u32,
    @location(3) speed: f32,
};

@group(0) @binding(0) var<storage, read> bodies: array<BodyObservation>;
@group(0) @binding(1) var<storage, read> shapes: array<ShapeVisual>;
@group(0) @binding(2) var<uniform> params: RenderParams;

const BODY_LIVE: u32 = 1u;
const BODY_SLEEPING: u32 = 2u;

@vertex
fn body_vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    let body = bodies[instance_index];
    let shape = shapes[instance_index];
    let live = (body.identity.y & BODY_LIVE) != 0u
        && body.identity.x == shape.identity.x;
    let local = corners[vertex_index];
    if !live {
        return VertexOutput(
            vec4f(2.0, 2.0, 0.0, 1.0),
            local,
            vec4f(0.0),
            0u,
            0.0,
        );
    }

    let kind = shape.identity.y;
    let half_size = select(shape.geometry.xy, shape.geometry.zz, kind == 1u);
    let scaled = local * half_size;
    let sine = sin(body.pose.z);
    let cosine = cos(body.pose.z);
    let rotated = vec2f(
        cosine * scaled.x - sine * scaled.y,
        sine * scaled.x + cosine * scaled.y,
    );
    let world = body.pose.xy + rotated;
    let clip = (world - params.camera.xy) / max(params.camera.zw, vec2f(0.0001));
    let sleeping = (body.identity.y & BODY_SLEEPING) != 0u;
    let color = select(shape.color, mix(shape.color, vec4f(0.45, 0.53, 0.62, 1.0), 0.45), sleeping);
    return VertexOutput(
        vec4f(clip, 0.0, 1.0),
        local,
        color,
        kind,
        clamp(length(body.motion.xy) * 0.12, 0.0, 1.0),
    );
}

@fragment
fn body_fragment(input: VertexOutput) -> @location(0) vec4f {
    if input.kind == 1u && length(input.local) > 1.0 {
        discard;
    }
    let edge = select(
        max(abs(input.local.x), abs(input.local.y)),
        length(input.local),
        input.kind == 1u,
    );
    let rim = smoothstep(0.82, 0.98, edge);
    let motion_tint = vec3f(0.12, 0.21, 0.28) * input.speed;
    var color = mix(input.color.rgb + motion_tint, input.color.rgb * 0.48, rim);
    if input.kind == 1u {
        // Local +X rotates with the body, making angular motion visible without
        // adding another render pass or changing the public shape observation.
        let spoke_width = 0.055;
        let spoke_cross = 1.0 - smoothstep(spoke_width, spoke_width * 1.8, abs(input.local.y));
        let spoke_start = smoothstep(0.04, 0.13, input.local.x);
        let spoke_end = 1.0 - smoothstep(0.68, 0.82, input.local.x);
        let spoke = spoke_cross * spoke_start * spoke_end;
        color = mix(color, color * 0.24, spoke * 0.88);
    }
    return vec4f(color, input.color.a);
}
