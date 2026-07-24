struct GrabState {
    identity_status: vec4u,
    local_anchor_target: vec4f,
    counters: vec4u,
    activity: vec4u,
    last_selection: vec4u,
};

struct RenderParams {
    viewport: vec4f,
    camera: vec4f,
};

@group(0) @binding(0) var<storage, read> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(2) var<storage, read> grab: GrabState;
@group(0) @binding(3) var<uniform> params: RenderParams;
@group(0) @binding(4) var<storage, read_write> vertices: array<vec4f>;

const FLAG_LIVE: u32 = 1u << 0u;
const FLAG_FIXED: u32 = 1u << 2u;

@compute @workgroup_size(12)
fn prepare_grab_vertices(@builtin(global_invocation_id) id: vec3u) {
    let vertex_index = id.x;
    let slot = grab.identity_status.x;
    if (vertex_index >= 12u || slot >= arrayLength(&poses) ||
        metadata[slot].x != grab.identity_status.y ||
        (metadata[slot].y & (FLAG_LIVE | FLAG_FIXED)) != FLAG_LIVE) {
        if (vertex_index < 12u) {
            vertices[vertex_index] = vec4f(0.0);
        }
        return;
    }

    let corners = array<vec2f, 6>(
        vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0),
    );
    let pose = poses[slot];
    let local_anchor = grab.local_anchor_target.xy;
    let c = cos(pose.z);
    let s = sin(pose.z);
    let anchor = pose.xy + vec2f(c * local_anchor.x - s * local_anchor.y,
                                 s * local_anchor.x + c * local_anchor.y);
    let pointer_target = grab.local_anchor_target.zw;
    let primitive = u32(vertex_index >= 6u);
    let local = corners[vertex_index % 6u];
    let pixel_world = params.camera.zw * 2.0 / max(params.viewport.xy, vec2f(1.0));
    var world: vec2f;
    if (primitive == 0u) {
        let delta = pointer_target - anchor;
        let distance = max(length(delta), 0.00001);
        let direction = delta / distance;
        let perpendicular = vec2f(-direction.y, direction.x);
        world = 0.5 * (anchor + pointer_target) + direction * local.x * distance * 0.5
            + perpendicular * local.y * pixel_world.y * 2.5;
    } else {
        world = pointer_target + local * pixel_world * 6.0;
    }
    vertices[vertex_index] = vec4f(world, 1.0, 0.0);
}
