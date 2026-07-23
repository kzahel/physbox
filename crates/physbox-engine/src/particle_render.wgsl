struct RenderParams {
    viewport: vec4f,
    camera: vec4f,
    material: vec4f,
    debug: vec4u,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) color: vec3f,
};

@group(0) @binding(0) var<storage, read> positions: array<vec4f>;
@group(0) @binding(1) var<storage, read> velocities: array<vec4f>;
@group(0) @binding(2) var<storage, read> density_states: array<vec4f>;
@group(0) @binding(3) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(4) var<uniform> params: RenderParams;

@vertex
fn particle_vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    let index = instance_index;
    let local = corners[vertex_index];
    if metadata[index].w == 0u {
        return VertexOutput(vec4f(2.0, 2.0, 0.0, 1.0), local, vec3f(0.0));
    }
    let world = positions[index].xy + local * params.material.x;
    let clip = (world - params.camera.xy) / max(params.camera.zw, vec2f(0.0001));
    let speed = clamp(length(velocities[index].xy) * 0.16, 0.0, 1.0);
    let density = density_states[index].x;
    var color = mix(vec3f(0.035, 0.24, 0.58), vec3f(0.12, 0.58, 0.86), speed);
    if params.debug.x == 1u {
        color = select(
            mix(vec3f(0.03, 0.12, 0.42), vec3f(0.10, 0.58, 0.86),
                clamp(density / max(params.material.z, 0.0001), 0.0, 1.0)),
            mix(vec3f(0.10, 0.58, 0.86), vec3f(1.0, 0.25, 0.08),
                clamp((density - params.material.z) * 0.5, 0.0, 1.0)),
            density > params.material.z,
        );
    }
    return VertexOutput(vec4f(clip, 0.0, 1.0), local, color);
}

@fragment
fn particle_fragment(input: VertexOutput) -> @location(0) vec4f {
    let radius = length(input.local);
    if radius > 1.0 {
        discard;
    }
    let coverage = 1.0 - smoothstep(0.76, 1.0, radius);
    let body = mix(input.color * 0.76, input.color + vec3f(0.08, 0.15, 0.18),
                   1.0 - smoothstep(0.0, 0.86, radius));
    let highlight = 1.0 - smoothstep(
        0.12,
        0.34,
        length(input.local - vec2f(-0.28, 0.32)),
    );
    let color = mix(body, vec3f(0.66, 0.88, 1.0), highlight * 0.36);
    return vec4f(color, coverage * 0.90);
}
