struct BootstrapState {
    position: vec2f,
    angle: f32,
    inverse_mass: f32,
    linear_velocity: vec2f,
    angular_velocity: f32,
    inverse_inertia: f32,
};

struct FrameParams {
    dt_frame: vec4f,
    camera: vec4f,
};

@group(0) @binding(0) var<storage, read_write> state: BootstrapState;
@group(0) @binding(1) var<uniform> params: FrameParams;

@compute @workgroup_size(1)
fn step_main() {
    state.position += state.linear_velocity * params.dt_frame.x;
    state.angle += state.angular_velocity * params.dt_frame.x;

    if (state.position.x > 0.72) {
        state.position.x = 0.72;
        state.linear_velocity.x = -abs(state.linear_velocity.x);
    }
    if (state.position.x < -0.72) {
        state.position.x = -0.72;
        state.linear_velocity.x = abs(state.linear_velocity.x);
    }
    if (state.position.y > 0.56) {
        state.position.y = 0.56;
        state.linear_velocity.y = -abs(state.linear_velocity.y);
    }
    if (state.position.y < -0.56) {
        state.position.y = -0.56;
        state.linear_velocity.y = abs(state.linear_velocity.y);
    }
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    return vec4f(x, y, 0.0, 1.0);
}

fn rotate(point: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * point.x + s * point.y, -s * point.x + c * point.y);
}

fn box_distance(point: vec2f, half_size: vec2f) -> f32 {
    let delta = abs(point) - half_size;
    return length(max(delta, vec2f(0.0))) + min(max(delta.x, delta.y), 0.0);
}

@fragment
fn fragment_main(@builtin(position) pixel: vec4f) -> @location(0) vec4f {
    let size = max(params.dt_frame.zw, vec2f(1.0));
    let clip = vec2f(pixel.x / size.x * 2.0 - 1.0,
                     1.0 - pixel.y / size.y * 2.0);
    let world = params.camera.xy + clip * params.camera.zw;

    let horizon = clamp(0.52 + world.y * 0.08, 0.0, 1.0);
    var color = mix(vec3f(0.018, 0.024, 0.040), vec3f(0.038, 0.052, 0.078), horizon);

    let local = rotate(world - state.position, state.angle);
    let distance = box_distance(local, vec2f(0.16, 0.105));
    let edge = max(fwidth(distance), 0.0015);
    let fill = 1.0 - smoothstep(-edge, edge, distance);
    let rim = 1.0 - smoothstep(edge, edge * 2.6, abs(distance));
    let body_color = vec3f(0.48, 0.36, 0.82);
    color = mix(color, body_color, fill);
    color = mix(color, vec3f(0.64, 0.54, 0.94), rim * 0.78);

    return vec4f(color, 1.0);
}
