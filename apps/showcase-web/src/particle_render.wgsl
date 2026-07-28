// Consumer-owned water rendering over the public PhysBox observation.

struct ParticleObservation {
    position: vec4f,
    motion: vec4f,
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
    @location(1) speed: f32,
};

@group(0) @binding(0) var<storage, read> particles: array<ParticleObservation>;
@group(0) @binding(1) var<uniform> params: RenderParams;

const PARTICLE_LIVE: u32 = 1u;

@vertex
fn particle_vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    let particle = particles[instance_index];
    let local = corners[vertex_index];
    if (particle.identity.y & PARTICLE_LIVE) == 0u {
        return VertexOutput(vec4f(2.0, 2.0, 0.0, 1.0), local, 0.0);
    }
    let world = particle.position.xy + local * params.material.x;
    let clip = (world - params.camera.xy) / max(params.camera.zw, vec2f(0.0001));
    return VertexOutput(
        vec4f(clip, 0.0, 1.0),
        local,
        clamp(length(particle.motion.xy) * 0.14, 0.0, 1.0),
    );
}

@fragment
fn particle_fragment(input: VertexOutput) -> @location(0) vec4f {
    let radius = length(input.local);
    if radius > 1.0 {
        discard;
    }
    let coverage = 1.0 - smoothstep(0.74, 1.0, radius);
    let deep = vec3f(0.035, 0.29, 0.66);
    let bright = vec3f(0.10, 0.64, 0.92);
    var color = mix(deep, bright, input.speed);
    let highlight = 1.0 - smoothstep(
        0.10,
        0.34,
        length(input.local - vec2f(-0.28, 0.30)),
    );
    color = mix(color, vec3f(0.72, 0.93, 1.0), highlight * 0.40);
    return vec4f(color, coverage * 0.92);
}
