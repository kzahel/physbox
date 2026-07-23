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
@group(0) @binding(3) var<storage, read> active_indices: array<u32>;
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
    let index = active_indices[instance_index];
    let local = corners[vertex_index];
    let world = positions[index].xy + local * params.material.x;
    let clip = (world - params.camera.xy) / max(params.camera.zw, vec2f(0.0001));
    let speed = clamp(length(velocities[index].xy) * 0.08, 0.0, 1.0);
    let density = density_states[index].x;
    var color = mix(vec3f(0.035, 0.34, 0.86), vec3f(0.18, 0.92, 1.0), speed);
    if params.debug.x == 1u {
        color = select(
            mix(vec3f(0.03, 0.12, 0.48), vec3f(0.08, 0.84, 1.0),
                clamp(density / max(params.material.z, 0.0001), 0.0, 1.0)),
            mix(vec3f(0.08, 0.84, 1.0), vec3f(1.0, 0.18, 0.06),
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
    let rim = smoothstep(0.68, 1.0, radius);
    let color = mix(input.color + vec3f(0.10, 0.16, 0.20), input.color * 0.72, rim);
    return vec4f(color, 0.92);
}
