struct ManifoldRecord {
    identities: vec4u,
    normal_material: vec4f,
    point_range: vec4u,
}

struct ContactPoint {
    anchors: vec4f,
    world_separation: vec4f,
    features: vec4u,
}

struct Contribution {
    impulse: vec2f,
    anchor: vec2f,
    body_index: u32,
    point_index: u32,
    side: u32,
    padding: u32,
}

struct StabilizationParams {
    tuning: vec4f,
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read_write> body_poses: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read_write> body_motions: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read> body_properties: array<vec4f>;
@group(0) @binding(4) var<storage, read> manifolds: array<ManifoldRecord>;
@group(0) @binding(5) var<storage, read> points: array<ContactPoint>;
@group(0) @binding(6) var<storage, read> manifold_capacity: array<u32>;
@group(0) @binding(7) var<storage, read_write> contributions: array<Contribution>;
@group(0) @binding(8) var<storage, read_write> wake_requests: array<atomic<u32>>;
@group(0) @binding(9) var<uniform> params: StabilizationParams;

const CAUSE_FLUID: u32 = 1u << 10u;

fn rotate(vector: vec2f, angle: f32) -> vec2f {
    let cosine = cos(angle);
    let sine = sin(angle);
    return vec2f(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y);
}

fn cross(a: vec2f, b: vec2f) -> f32 {
    return a.x * b.y - a.y * b.x;
}

fn load_pose(body: u32) -> vec4f {
    let base = body * 4u;
    return vec4f(
        bitcast<f32>(atomicLoad(&body_poses[base])),
        bitcast<f32>(atomicLoad(&body_poses[base + 1u])),
        bitcast<f32>(atomicLoad(&body_poses[base + 2u])),
        bitcast<f32>(atomicLoad(&body_poses[base + 3u])));
}

fn load_motion(body: u32) -> vec4f {
    let base = body * 4u;
    return vec4f(
        bitcast<f32>(atomicLoad(&body_motions[base])),
        bitcast<f32>(atomicLoad(&body_motions[base + 1u])),
        bitcast<f32>(atomicLoad(&body_motions[base + 2u])),
        bitcast<f32>(atomicLoad(&body_motions[base + 3u])));
}

fn point_velocity(motion: vec4f, anchor: vec2f) -> vec2f {
    return motion.xy + vec2f(-motion.z * anchor.y, motion.z * anchor.x);
}

fn clear_contribution(index: u32) {
    contributions[index].impulse = vec2f(0.0);
    contributions[index].anchor = vec2f(0.0);
    contributions[index].body_index = 0xffffffffu;
    contributions[index].point_index = 0u;
    contributions[index].side = 0u;
    contributions[index].padding = 0u;
}

fn emit_endpoint(index: u32, body: u32, impulse: vec2f, anchor: vec2f,
                 velocity_impulse: vec2f) {
    contributions[index].impulse = impulse;
    contributions[index].anchor = anchor;
    contributions[index].body_index = body;
    contributions[index].point_index = bitcast<u32>(velocity_impulse.x);
    contributions[index].side = bitcast<u32>(velocity_impulse.y);
    contributions[index].padding = 1u;
}

@compute @workgroup_size(64)
fn prepare_stabilization(@builtin(global_invocation_id) invocation: vec3u) {
    let manifold_index = invocation.x;
    if (manifold_index >= params.counts.x) {
        return;
    }
    let contribution_base = manifold_index * 4u;
    for (var local = 0u; local < 4u; local += 1u) {
        clear_contribution(contribution_base + local);
    }
    if (manifold_index >= manifold_capacity[2]) {
        return;
    }
    let manifold = manifolds[manifold_index];
    let body_a = manifold.identities.x;
    let body_b = manifold.identities.z;
    if (body_a >= params.counts.y || body_b >= params.counts.y) {
        return;
    }
    let property_a = body_properties[body_a];
    let property_b = body_properties[body_b];
    if (property_a.z <= 0.0 && property_b.z <= 0.0) {
        return;
    }
    let pose_a = load_pose(body_a);
    let pose_b = load_pose(body_b);
    let motion_a = load_motion(body_a);
    let motion_b = load_motion(body_b);
    let normal = manifold.normal_material.xy;
    let point_count = min(manifold.point_range.y, 2u);
    let distribution = 1.0 / f32(max(point_count, 1u));
    for (var local = 0u; local < point_count; local += 1u) {
        let point = points[manifold.point_range.x + local];
        let anchor_a = rotate(point.anchors.xy, pose_a.z);
        let anchor_b = rotate(point.anchors.zw, pose_b.z);
        let cross_a = cross(anchor_a, normal);
        let cross_b = cross(anchor_b, normal);
        let inverse_effective_mass =
            property_a.z + property_b.z +
            property_a.w * cross_a * cross_a +
            property_b.w * cross_b * cross_b;
        if (inverse_effective_mass <= 0.0) {
            continue;
        }
        let penetration = max(-point.world_separation.z - params.tuning.x, 0.0);
        let correction = min(
            penetration * params.tuning.y * distribution,
            params.tuning.z);
        if (correction <= 0.0) {
            continue;
        }
        let position_lambda = correction / inverse_effective_mass;
        let relative_normal_velocity = dot(
            point_velocity(motion_b, anchor_b) -
            point_velocity(motion_a, anchor_a),
            normal);
        let velocity_lambda =
            max(-relative_normal_velocity, 0.0) * distribution /
            inverse_effective_mass;
        let impulse = normal * position_lambda;
        let velocity_impulse = normal * velocity_lambda;
        let point_base = contribution_base + local * 2u;
        emit_endpoint(
            point_base,
            body_a,
            -impulse,
            anchor_a,
            -velocity_impulse);
        emit_endpoint(
            point_base + 1u,
            body_b,
            impulse,
            anchor_b,
            velocity_impulse);
    }
}

fn atomic_add_pose(word: u32, delta: f32) {
    if (delta == 0.0) {
        return;
    }
    var prior = atomicLoad(&body_poses[word]);
    loop {
        let next = bitcast<u32>(bitcast<f32>(prior) + delta);
        let result = atomicCompareExchangeWeak(&body_poses[word], prior, next);
        if (result.exchanged) {
            return;
        }
        prior = result.old_value;
    }
}

fn atomic_add_motion(word: u32, delta: f32) {
    if (delta == 0.0) {
        return;
    }
    var prior = atomicLoad(&body_motions[word]);
    loop {
        let next = bitcast<u32>(bitcast<f32>(prior) + delta);
        let result = atomicCompareExchangeWeak(&body_motions[word], prior, next);
        if (result.exchanged) {
            return;
        }
        prior = result.old_value;
    }
}

@compute @workgroup_size(64)
fn apply_stabilization(@builtin(global_invocation_id) invocation: vec3u) {
    let manifold_index = invocation.x;
    if (manifold_index >= params.counts.x ||
        manifold_index >= manifold_capacity[2]) {
        return;
    }
    let point_count = min(manifolds[manifold_index].point_range.y, 2u);
    let contribution_base = manifold_index * 4u;
    for (var local = 0u; local < point_count * 2u; local += 1u) {
        let contribution = contributions[contribution_base + local];
        let body = contribution.body_index;
        if (body >= params.counts.y) {
            continue;
        }
        let property = body_properties[body];
        if (property.z <= 0.0) {
            continue;
        }
        let position_delta = contribution.impulse * property.z;
        let angle_delta =
            property.w * cross(contribution.anchor, contribution.impulse);
        let velocity_impulse = vec2f(
            bitcast<f32>(contribution.point_index),
            bitcast<f32>(contribution.side));
        let velocity_delta = velocity_impulse * property.z;
        let angular_velocity_delta =
            property.w * cross(contribution.anchor, velocity_impulse);
        let base = body * 4u;
        atomic_add_pose(base, position_delta.x);
        atomic_add_pose(base + 1u, position_delta.y);
        atomic_add_pose(base + 2u, angle_delta);
        atomic_add_motion(base, velocity_delta.x);
        atomic_add_motion(base + 1u, velocity_delta.y);
        atomic_add_motion(base + 2u, angular_velocity_delta);
        atomicOr(&wake_requests[body], CAUSE_FLUID);
    }
}
