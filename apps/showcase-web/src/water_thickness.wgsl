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
    @location(2) height: f32,
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
        return VertexOutput(vec4f(2.0, 2.0, 0.0, 1.0), local, 0.0, 0.0);
    }
    let velocity = particle.motion.xy;
    let speed = length(velocity);
    let direction = velocity / max(speed, 0.001);
    let stretch = smoothstep(3.0, 12.0, speed);
    let along = direction * dot(local, direction);
    let world = particle.position.xy + params.material.x * (2.6 * local + 0.35 * stretch * along);
    let clip = (world - params.camera.xy) / max(params.camera.zw, vec2f(0.0001));
    return VertexOutput(
        vec4f(clip, 0.0, 1.0),
        local,
        clamp(length(particle.motion.xy) * 0.09, 0.0, 1.0),
        clamp((world.y + 8.0) / 13.0, 0.0, 1.0),
    );
}

@fragment
fn particle_fragment(input: VertexOutput) -> @location(0) vec4f {
    let radius = length(input.local);
    if radius > 1.0 {
        discard;
    }
    let weight = (1.0 - radius * radius) * (1.0 - radius * radius);
    return vec4f(0.20 * weight, 0.20 * weight * input.speed, 0.20 * weight * input.height, 1.0);
}
