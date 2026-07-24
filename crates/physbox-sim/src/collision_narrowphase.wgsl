struct NarrowParams {
    counts: vec4u,
    execution: vec4u,
    policy: vec4f,
}

struct CandidatePair {
    identities: vec4u,
}

struct ContactPoint {
    anchors: vec4f,
    world_separation: vec4f,
    features: vec4u,
}

struct NarrowScratch {
    identities: vec4u,
    normal_material: vec4f,
    point_count: vec4u,
    point0: ContactPoint,
    point1: ContactPoint,
}

struct ManifoldRecord {
    identities: vec4u,
    normal_material: vec4f,
    point_range: vec4u,
}

struct PairRecord {
    key: vec4u,
    state: vec4u,
    features0: vec4u,
    cache0: vec4u,
    features1: vec4u,
    cache1: vec4u,
}

struct ClipSegment {
    point0: vec2f,
    point1: vec2f,
    vertices: vec2u,
    count: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> poses: array<vec4f>;
@group(0) @binding(1) var<storage, read> properties: array<vec4f>;
@group(0) @binding(2) var<storage, read> metadata: array<vec4u>;
@group(0) @binding(3) var<storage, read> candidates: array<CandidatePair>;
@group(0) @binding(4) var<storage, read_write> candidate_capacity: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> narrow_scratch: array<NarrowScratch>;
@group(0) @binding(6) var<storage, read_write> manifold_flags: array<u32>;
@group(0) @binding(7) var<storage, read_write> manifold_offsets: array<u32>;
@group(0) @binding(8) var<storage, read_write> scan_blocks: array<u32>;
@group(0) @binding(9) var<storage, read_write> manifolds: array<ManifoldRecord>;
@group(0) @binding(10) var<storage, read_write> points: array<ContactPoint>;
@group(0) @binding(11) var<storage, read_write> manifold_capacity: array<atomic<u32>>;
@group(0) @binding(12) var<storage, read_write> point_capacity: array<atomic<u32>>;
@group(0) @binding(13) var<storage, read_write> work: array<atomic<u32>>;
@group(0) @binding(14) var<uniform> params: NarrowParams;
@group(0) @binding(15) var<storage, read> previous_pairs: array<PairRecord>;
@group(0) @binding(16) var<storage, read_write> previous_pair_capacity: array<atomic<u32>>;
@group(0) @binding(17) var<storage, read_write> narrow_indirect: array<atomic<u32>>;

const FLAG_RECTANGLE: u32 = 1u << 8u;
const FLAG_CIRCLE: u32 = 1u << 9u;
const FEATURE_CIRCLE: u32 = 0u;
const FEATURE_FACE: u32 = 1u;
const FEATURE_VERTEX: u32 = 2u;
const REFERENCE_AXIS_RELATIVE_BIAS: f32 = 0.98;
const REFERENCE_AXIS_ABSOLUTE_BIAS: f32 = 1.0e-3;
const NORMAL_EPSILON_SQUARED: f32 = 1.0e-12;

var<workgroup> scan_values: array<u32, 256>;

fn rotate(vector: vec2f, angle: f32) -> vec2f {
    let cosine = cos(angle);
    let sine = sin(angle);
    return vec2f(cosine * vector.x - sine * vector.y,
                 sine * vector.x + cosine * vector.y);
}

fn inverse_rotate(vector: vec2f, angle: f32) -> vec2f {
    return rotate(vector, -angle);
}

fn make_point(body_a: u32, body_b: u32, anchor_a: vec2f, anchor_b: vec2f,
              separation: f32, features: vec4u) -> ContactPoint {
    var point: ContactPoint;
    point.anchors = vec4f(
        inverse_rotate(anchor_a - poses[body_a].xy, poses[body_a].z),
        inverse_rotate(anchor_b - poses[body_b].xy, poses[body_b].z));
    point.world_separation = vec4f(0.5 * (anchor_a + anchor_b), separation, 0.0);
    point.features = features;
    return point;
}

fn empty_scratch(pair: CandidatePair) -> NarrowScratch {
    var output: NarrowScratch;
    output.identities = pair.identities;
    output.normal_material = vec4f(0.0);
    output.point_count = vec4u(0u);
    output.point0.anchors = vec4f(0.0);
    output.point0.world_separation = vec4f(0.0);
    output.point0.features = vec4u(0u);
    output.point1 = output.point0;
    return output;
}

fn finish_scratch(pair: CandidatePair, normal: vec2f,
                  point0: ContactPoint, point1: ContactPoint,
                  point_count: u32) -> NarrowScratch {
    var output = empty_scratch(pair);
    let material_a = metadata[pair.identities.x];
    let material_b = metadata[pair.identities.z];
    output.normal_material = vec4f(
        normal,
        sqrt(bitcast<f32>(material_a.z) * bitcast<f32>(material_b.z)),
        max(bitcast<f32>(material_a.w), bitcast<f32>(material_b.w)));
    output.point_count.x = point_count;
    output.point0 = point0;
    output.point1 = point1;
    if (u32(params.policy.y) == 4u) {
        let tangent = vec2f(normal.y, -normal.x);
        if (point_count == 2u &&
                dot(output.point0.world_separation.xy, tangent) <
                dot(output.point1.world_separation.xy, tangent)) {
            let swap = output.point0;
            output.point0 = output.point1;
            output.point1 = swap;
        }
        let reference_a = point_count == 2u &&
            output.point0.features.x == FEATURE_FACE &&
            output.point1.features.x == FEATURE_FACE;
        let reference_b = point_count == 2u &&
            output.point0.features.z == FEATURE_FACE &&
            output.point1.features.z == FEATURE_FACE;
        let reference_face_a = output.point0.features.y;
        let reference_face_b = output.point0.features.w;
        let midpoint0 = output.point0.world_separation.xy;
        output.point0.anchors = vec4f(
            inverse_rotate(midpoint0 - poses[pair.identities.x].xy,
                           poses[pair.identities.x].z),
            inverse_rotate(midpoint0 - poses[pair.identities.z].xy,
                           poses[pair.identities.z].z));
        if (reference_a) {
            output.point0.features = vec4u(
                FEATURE_VERTEX, reference_face_a,
                FEATURE_VERTEX, output.point0.features.w);
        } else if (reference_b) {
            output.point0.features = vec4u(
                FEATURE_VERTEX, output.point0.features.y,
                FEATURE_VERTEX, (reference_face_b + 1u) % 4u);
        } else {
            output.point0.features = vec4u(
                FEATURE_VERTEX, closest_rectangle_vertex(pair.identities.x,
                    output.point0.anchors.xy),
                FEATURE_VERTEX, closest_rectangle_vertex(pair.identities.z,
                    output.point0.anchors.zw));
        }
        if (point_count == 2u) {
            let midpoint1 = output.point1.world_separation.xy;
            output.point1.anchors = vec4f(
                inverse_rotate(midpoint1 - poses[pair.identities.x].xy,
                               poses[pair.identities.x].z),
                inverse_rotate(midpoint1 - poses[pair.identities.z].xy,
                               poses[pair.identities.z].z));
            if (reference_a) {
                output.point1.features = vec4u(
                    FEATURE_VERTEX, (reference_face_a + 1u) % 4u,
                    FEATURE_VERTEX, output.point1.features.w);
            } else if (reference_b) {
                output.point1.features = vec4u(
                    FEATURE_VERTEX, output.point1.features.y,
                    FEATURE_VERTEX, reference_face_b);
            } else {
                output.point1.features = vec4u(
                    FEATURE_VERTEX, closest_rectangle_vertex(pair.identities.x,
                        output.point1.anchors.xy),
                    FEATURE_VERTEX, closest_rectangle_vertex(pair.identities.z,
                        output.point1.anchors.zw));
            }
            // Authority identity rewrites the feature tuple after the
            // geometric tangent-order control above. A face-index wrap (for
            // example vertex 3 followed by vertex 0) can therefore reverse
            // the persistent-key order even though the two contact points
            // remain geometrically ordered. Pair-slot persistence already
            // canonicalizes these rewritten keys before solving; keep the
            // validation manifold on that same canonical order as well.
            if (point_key_greater(output.point0, output.point1)) {
                let swap = output.point0;
                output.point0 = output.point1;
                output.point1 = swap;
            }
        }
    } else if (point_count == 2u && point_key_greater(output.point0, output.point1)) {
        let swap = output.point0;
        output.point0 = output.point1;
        output.point1 = swap;
    }
    return output;
}

fn closest_rectangle_vertex(body: u32, local_anchor: vec2f) -> u32 {
    var selected = 0u;
    var best = 1.0e30;
    for (var vertex = 0u; vertex < 4u; vertex += 1u) {
        let delta = shifted_rectangle_vertex(properties[body].xy, vertex) - local_anchor;
        let distance = dot(delta, delta);
        if (distance < best) {
            best = distance;
            selected = vertex;
        }
    }
    return selected;
}

fn point_key_greater(a: ContactPoint, b: ContactPoint) -> bool {
    if (a.features.x != b.features.x) { return a.features.x > b.features.x; }
    if (a.features.y != b.features.y) { return a.features.y > b.features.y; }
    if (a.features.z != b.features.z) { return a.features.z > b.features.z; }
    return a.features.w > b.features.w;
}

fn pair_key_less(left: vec4u, right: vec4u) -> bool {
    for (var index = 0u; index < 4u; index++) {
        if (left[index] != right[index]) { return left[index] < right[index]; }
    }
    return false;
}

// 0 = no compatible prior, 1 = body A owned the reference, 2 = body B.
fn previous_reference_owner(key: vec4u) -> u32 {
    let previous_count = atomicLoad(&previous_pair_capacity[2]);
    var lower = 0u;
    var upper = previous_count;
    loop {
        if (lower >= upper) { break; }
        let middle = lower + (upper - lower) / 2u;
        if (pair_key_less(previous_pairs[middle].key, key)) {
            lower = middle + 1u;
        } else {
            upper = middle;
        }
    }
    if (lower >= previous_count || !all(previous_pairs[lower].key == key)) { return 0u; }
    let previous = previous_pairs[lower];
    if (previous.state.z == 0u) { return 0u; }
    if (previous.features0.x == FEATURE_FACE && previous.features0.z != FEATURE_FACE) { return 1u; }
    if (previous.features0.z == FEATURE_FACE && previous.features0.x != FEATURE_FACE) { return 2u; }
    return 0u;
}

fn collide_circles(pair: CandidatePair) -> NarrowScratch {
    let body_a = pair.identities.x;
    let body_b = pair.identities.z;
    let delta = poses[body_b].xy - poses[body_a].xy;
    let distance_squared = dot(delta, delta);
    let combined_radius = properties[body_a].x + properties[body_b].x;
    let speculative_radius = combined_radius + params.policy.x;
    if (distance_squared > speculative_radius * speculative_radius) {
        return empty_scratch(pair);
    }
    let distance = sqrt(distance_squared);
    var normal = vec2f(1.0, 0.0);
    if (distance_squared > NORMAL_EPSILON_SQUARED) {
        normal = delta / distance;
    }
    let anchor_a = poses[body_a].xy + normal * properties[body_a].x;
    let anchor_b = poses[body_b].xy - normal * properties[body_b].x;
    let point = make_point(body_a, body_b, anchor_a, anchor_b,
                           distance - combined_radius,
                           vec4u(FEATURE_CIRCLE, 0u, FEATURE_CIRCLE, 0u));
    return finish_scratch(pair, normal, point, point, 1u);
}

fn rectangle_feature(local_circle: vec2f, closest: vec2f, half: vec2f) -> vec2u {
    let clamped_x = local_circle.x < -half.x || local_circle.x > half.x;
    let clamped_y = local_circle.y < -half.y || local_circle.y > half.y;
    if (clamped_x && clamped_y) {
        var index = 0u;
        if (closest.x > 0.0 && closest.y <= 0.0) { index = 1u; }
        if (closest.x > 0.0 && closest.y > 0.0) { index = 2u; }
        if (closest.x <= 0.0 && closest.y > 0.0) { index = 3u; }
        return vec2u(FEATURE_VERTEX, index);
    }
    if (clamped_x) {
        return vec2u(FEATURE_FACE, select(3u, 1u, closest.x > 0.0));
    }
    return vec2u(FEATURE_FACE, select(0u, 2u, closest.y > 0.0));
}

fn collide_rectangle_circle(pair: CandidatePair, rectangle_is_a: bool) -> NarrowScratch {
    let rectangle = select(pair.identities.z, pair.identities.x, rectangle_is_a);
    let circle = select(pair.identities.x, pair.identities.z, rectangle_is_a);
    let half = properties[rectangle].xy;
    let circle_local = inverse_rotate(
        poses[circle].xy - poses[rectangle].xy, poses[rectangle].z);
    let closest = clamp(circle_local, -half, half);
    let local_delta = circle_local - closest;
    let distance_squared = dot(local_delta, local_delta);
    let radius = properties[circle].x;
    var local_normal: vec2f;
    var rectangle_local: vec2f;
    var rectangle_contact_feature: vec2u;
    var separation: f32;
    if (distance_squared > NORMAL_EPSILON_SQUARED) {
        let speculative_radius = radius + params.policy.x;
        if (distance_squared > speculative_radius * speculative_radius) {
            return empty_scratch(pair);
        }
        let distance = sqrt(distance_squared);
        local_normal = local_delta / distance;
        rectangle_local = closest;
        rectangle_contact_feature = rectangle_feature(circle_local, closest, half);
        separation = distance - radius;
    } else {
        let margin_x = half.x - abs(circle_local.x);
        let margin_y = half.y - abs(circle_local.y);
        if (margin_x <= margin_y) {
            let sign = select(-1.0, 1.0, circle_local.x >= 0.0);
            local_normal = vec2f(sign, 0.0);
            rectangle_local = vec2f(sign * half.x, circle_local.y);
            rectangle_contact_feature = vec2u(
                FEATURE_FACE, select(3u, 1u, sign > 0.0));
            separation = -margin_x - radius;
        } else {
            let sign = select(-1.0, 1.0, circle_local.y >= 0.0);
            local_normal = vec2f(0.0, sign);
            rectangle_local = vec2f(circle_local.x, sign * half.y);
            rectangle_contact_feature = vec2u(
                FEATURE_FACE, select(0u, 2u, sign > 0.0));
            separation = -margin_y - radius;
        }
    }
    let rectangle_to_circle = rotate(local_normal, poses[rectangle].z);
    let anchor_rectangle = poses[rectangle].xy +
        rotate(rectangle_local, poses[rectangle].z);
    let anchor_circle = poses[circle].xy - rectangle_to_circle * radius;
    var normal: vec2f;
    var anchor_a: vec2f;
    var anchor_b: vec2f;
    var features: vec4u;
    if (rectangle_is_a) {
        normal = rectangle_to_circle;
        anchor_a = anchor_rectangle;
        anchor_b = anchor_circle;
        features = vec4u(rectangle_contact_feature, FEATURE_CIRCLE, 0u);
    } else {
        normal = -rectangle_to_circle;
        anchor_a = anchor_circle;
        anchor_b = anchor_rectangle;
        features = vec4u(FEATURE_CIRCLE, 0u, rectangle_contact_feature);
    }
    let point = make_point(pair.identities.x, pair.identities.z,
                           anchor_a, anchor_b, separation, features);
    return finish_scratch(pair, normal, point, point, 1u);
}

fn rectangle_vertex(body: u32, index: u32) -> vec2f {
    let half = properties[body].xy;
    var local = vec2f(-half.x, -half.y);
    if (index == 1u) { local = vec2f(half.x, -half.y); }
    if (index == 2u) { local = vec2f(half.x, half.y); }
    if (index == 3u) { local = vec2f(-half.x, half.y); }
    return poses[body].xy + rotate(local, poses[body].z);
}

fn shifted_rectangle_vertex(half: vec2f, index: u32) -> vec2f {
    if (index == 0u) { return vec2f(0.0); }
    if (index == 1u) { return vec2f(2.0 * half.x, 0.0); }
    if (index == 2u) { return 2.0 * half; }
    return vec2f(0.0, 2.0 * half.y);
}

fn local_face_normal(face: u32) -> vec2f {
    if (face == 0u) { return vec2f(0.0, -1.0); }
    if (face == 1u) { return vec2f(1.0, 0.0); }
    if (face == 2u) { return vec2f(0.0, 1.0); }
    return vec2f(-1.0, 0.0);
}

fn face_for_normal(body: u32, normal: vec2f) -> u32 {
    let axis_x = rotate(vec2f(1.0, 0.0), poses[body].z);
    let axis_y = rotate(vec2f(0.0, 1.0), poses[body].z);
    let x = dot(normal, axis_x);
    let y = dot(normal, axis_y);
    if (abs(x) >= abs(y)) { return select(3u, 1u, x >= 0.0); }
    return select(0u, 2u, y >= 0.0);
}

fn face_normal(body: u32, face: u32) -> vec2f {
    let axis_x = rotate(vec2f(1.0, 0.0), poses[body].z);
    let axis_y = rotate(vec2f(0.0, 1.0), poses[body].z);
    if (face == 0u) { return -axis_y; }
    if (face == 1u) { return axis_x; }
    if (face == 2u) { return axis_y; }
    return -axis_x;
}

fn face_vertices(body: u32, face: u32) -> vec4f {
    if (face == 0u) { return vec4f(rectangle_vertex(body, 0u), rectangle_vertex(body, 1u)); }
    if (face == 1u) { return vec4f(rectangle_vertex(body, 1u), rectangle_vertex(body, 2u)); }
    if (face == 2u) { return vec4f(rectangle_vertex(body, 2u), rectangle_vertex(body, 3u)); }
    return vec4f(rectangle_vertex(body, 3u), rectangle_vertex(body, 0u));
}

fn incident_vertices(body: u32, reference_normal: vec2f) -> vec2u {
    let face = face_for_normal(body, -reference_normal);
    if (face == 0u) { return vec2u(0u, 1u); }
    if (face == 1u) { return vec2u(1u, 2u); }
    if (face == 2u) { return vec2u(2u, 3u); }
    return vec2u(3u, 0u);
}

fn clip_segment(segment: ClipSegment, tangent: vec2f,
                limit: f32, keep_greater: bool) -> ClipSegment {
    if (segment.count != 2u) { return segment; }
    var distance_a = dot(segment.point0, tangent) - limit;
    var distance_b = dot(segment.point1, tangent) - limit;
    if (!keep_greater) {
        distance_a = -distance_a;
        distance_b = -distance_b;
    }
    let inside_a = distance_a >= 0.0;
    let inside_b = distance_b >= 0.0;
    if (inside_a && inside_b) { return segment; }
    var output = segment;
    if (!inside_a && !inside_b) {
        output.count = 0u;
        return output;
    }
    let fraction = distance_a / (distance_a - distance_b);
    let intersection = segment.point0 + fraction * (segment.point1 - segment.point0);
    if (inside_a) {
        output.point1 = intersection;
    } else {
        output.point0 = intersection;
    }
    return output;
}

fn collide_rectangles_with_prior(pair: CandidatePair, prior_owner: u32) -> NarrowScratch {
    let body_a = pair.identities.x;
    let body_b = pair.identities.z;
    var separation_a = -1.0e30;
    var separation_b = -1.0e30;
    var face_a = 0u;
    var face_b = 0u;
    let origin = rectangle_vertex(body_a, 0u);
    for (var face = 0u; face < 4u; face++) {
        let normal_a = local_face_normal(face);
        let vertex_a = shifted_rectangle_vertex(properties[body_a].xy, face);
        var candidate_a = 1.0e30;
        for (var vertex = 0u; vertex < 4u; vertex++) {
            candidate_a = min(candidate_a,
                dot(normal_a,
                    inverse_rotate(rectangle_vertex(body_b, vertex) - origin,
                                   poses[body_a].z) - vertex_a));
        }
        if (candidate_a > separation_a) {
            separation_a = candidate_a;
            face_a = face;
        }
        let normal_b = inverse_rotate(face_normal(body_b, face), poses[body_a].z);
        let vertex_b = inverse_rotate(rectangle_vertex(body_b, face) - origin,
                                      poses[body_a].z);
        var candidate_b = 1.0e30;
        for (var vertex = 0u; vertex < 4u; vertex++) {
            candidate_b = min(candidate_b,
                dot(normal_b,
                    shifted_rectangle_vertex(properties[body_a].xy, vertex) - vertex_b));
        }
        if (candidate_b > separation_b) {
            separation_b = candidate_b;
            face_b = face;
        }
    }
    if (separation_a > params.policy.x || separation_b > params.policy.x) {
        return empty_scratch(pair);
    }
    let same_half = all(properties[body_a].xy == properties[body_b].xy);
    let center_delta = poses[body_b].xy - poses[body_a].xy;
    let deeply_coincident_identical = same_half && dot(center_delta, center_delta) <= 0.01 &&
        separation_a <= -0.05 && separation_b <= -0.05;
    // Policy 0 is the selected direct answer. Policy 1 applies the established
    // relative/absolute coherence bias to every rectangle pair. Policy 2 uses
    // a symmetric absolute near-tie band and canonical body-A ownership.
    let direct = separation_a >= separation_b;
    let relative_bias = separation_b <= params.policy.z * separation_a + params.policy.w;
    let absolute_band = select(direct, true, abs(separation_a - separation_b) <= params.policy.w);
    let policy = u32(params.policy.y);
    var reference_is_a = direct;
    if (policy == 1u || deeply_coincident_identical) { reference_is_a = relative_bias; }
    if (policy == 2u && !deeply_coincident_identical) { reference_is_a = absolute_band; }
    if (policy == 3u && !deeply_coincident_identical) {
        if (prior_owner == 1u && separation_b <= separation_a + params.policy.w) {
            reference_is_a = true;
        }
        if (prior_owner == 2u && separation_a <= separation_b + params.policy.w) {
            reference_is_a = false;
        }
    }
    let reference = select(body_b, body_a, reference_is_a);
    let incident = select(body_a, body_b, reference_is_a);
    let reference_face = select(face_b, face_a, reference_is_a);
    let reference_normal = face_normal(reference, reference_face);
    let selected_axis = select(-reference_normal, reference_normal, reference_is_a);
    let reference_edge = face_vertices(reference, reference_face);
    let incident_indices = incident_vertices(incident, reference_normal);
    var segment: ClipSegment;
    segment.point0 = rectangle_vertex(incident, incident_indices.x);
    segment.point1 = rectangle_vertex(incident, incident_indices.y);
    segment.vertices = incident_indices;
    segment.count = 2u;
    segment.padding = 0u;
    let tangent = normalize(reference_edge.zw - reference_edge.xy);
    let side0 = dot(reference_edge.xy, tangent);
    let side1 = dot(reference_edge.zw, tangent);
    segment = clip_segment(segment, tangent, min(side0, side1), true);
    segment = clip_segment(segment, tangent, max(side0, side1), false);
    let reference_center = 0.5 * (reference_edge.xy + reference_edge.zw);
    var point_count = 0u;
    var point0: ContactPoint;
    var point1: ContactPoint;
    for (var index = 0u; index < segment.count; index++) {
        let clipped = select(segment.point1, segment.point0, index == 0u);
        let incident_vertex = select(segment.vertices.y, segment.vertices.x, index == 0u);
        let separation = dot(clipped - reference_center, reference_normal);
        let reference_anchor = clipped - separation * reference_normal;
        var anchor_a: vec2f;
        var anchor_b: vec2f;
        var features: vec4u;
        if (reference_is_a) {
            anchor_a = reference_anchor;
            anchor_b = clipped;
            features = vec4u(FEATURE_FACE, reference_face,
                             FEATURE_VERTEX, incident_vertex);
        } else {
            anchor_a = clipped;
            anchor_b = reference_anchor;
            features = vec4u(FEATURE_VERTEX, incident_vertex,
                             FEATURE_FACE, reference_face);
        }
        let contact = make_point(body_a, body_b, anchor_a, anchor_b,
                                 separation, features);
        if (point_count == 0u) { point0 = contact; } else { point1 = contact; }
        point_count += 1u;
    }
    if (point_count == 0u) { return empty_scratch(pair); }
    if (point_count == 1u) { point1 = point0; }
    return finish_scratch(pair, selected_axis, point0, point1, point_count);
}

fn collide_rectangles(pair: CandidatePair) -> NarrowScratch {
    return collide_rectangles_with_prior(pair, 0u);
}

@compute @workgroup_size(64)
fn stabilize_narrowphase_identity(@builtin(global_invocation_id) invocation: vec3u) {
    let manifold_index = invocation.x;
    if (u32(params.policy.y) != 3u ||
        manifold_index >= atomicLoad(&manifold_capacity[2])) { return; }
    let current = manifolds[manifold_index];
    let body_a = current.identities.x;
    let body_b = current.identities.z;
    if ((metadata[body_a].y & FLAG_RECTANGLE) == 0u ||
        (metadata[body_b].y & FLAG_RECTANGLE) == 0u) { return; }
    let prior_owner = previous_reference_owner(current.identities);
    if (prior_owner == 0u) { return; }
    var pair: CandidatePair;
    pair.identities = current.identities;
    let stabilized = collide_rectangles_with_prior(pair, prior_owner);
    let point_count = stabilized.point_count.x;
    if (point_count != current.point_range.y || point_count == 0u) { return; }
    // Never preserve a prior feature frame across a materially different normal.
    if (dot(stabilized.normal_material.xy, current.normal_material.xy) < 0.9999) { return; }
    var output = current;
    output.normal_material = stabilized.normal_material;
    manifolds[manifold_index] = output;
    points[current.point_range.x] = stabilized.point0;
    if (point_count == 2u) { points[current.point_range.x + 1u] = stabilized.point1; }
}

@compute @workgroup_size(1)
fn prepare_narrowphase(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    for (var index = 0u; index < 4u; index += 1u) {
        atomicStore(&manifold_capacity[index], 0u);
        atomicStore(&point_capacity[index], 0u);
    }
    let candidate_count = atomicLoad(&candidate_capacity[2]);
    atomicStore(&narrow_indirect[0], (candidate_count + 63u) / 64u);
    atomicStore(&narrow_indirect[1], 1u);
    atomicStore(&narrow_indirect[2], 1u);
    atomicStore(&narrow_indirect[3], (candidate_count + 255u) / 256u);
    atomicStore(&narrow_indirect[4], 1u);
    atomicStore(&narrow_indirect[5], 1u);
}

@compute @workgroup_size(64)
fn generate_narrowphase(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    let candidate_count = atomicLoad(&candidate_capacity[2]);
    if (index >= candidate_count) { return; }
    let pair = candidates[index];
    let flags_a = metadata[pair.identities.x].y;
    let flags_b = metadata[pair.identities.z].y;
    var output = empty_scratch(pair);
    if ((flags_a & FLAG_CIRCLE) != 0u && (flags_b & FLAG_CIRCLE) != 0u) {
        output = collide_circles(pair);
    } else if ((flags_a & FLAG_RECTANGLE) != 0u && (flags_b & FLAG_CIRCLE) != 0u) {
        output = collide_rectangle_circle(pair, true);
    } else if ((flags_a & FLAG_CIRCLE) != 0u && (flags_b & FLAG_RECTANGLE) != 0u) {
        output = collide_rectangle_circle(pair, false);
    } else if ((flags_a & FLAG_RECTANGLE) != 0u && (flags_b & FLAG_RECTANGLE) != 0u) {
        output = collide_rectangles(pair);
    }
    narrow_scratch[index] = output;
    manifold_flags[index] = select(0u, 1u, output.point_count.x > 0u);
    if (output.point_count.x > 0u) {
        atomicAdd(&point_capacity[1], output.point_count.x);
    }
}

@compute @workgroup_size(256)
fn scan_manifold_blocks(@builtin(global_invocation_id) global: vec3u,
                        @builtin(local_invocation_id) local: vec3u,
                        @builtin(workgroup_id) group: vec3u) {
    let index = global.x;
    var value = 0u;
    let candidate_count = atomicLoad(&candidate_capacity[2]);
    if (index < candidate_count) { value = manifold_flags[index]; }
    scan_values[local.x] = value;
    workgroupBarrier();
    var offset = 1u;
    loop {
        if (offset >= 256u) { break; }
        var addition = 0u;
        if (local.x >= offset) { addition = scan_values[local.x - offset]; }
        workgroupBarrier();
        scan_values[local.x] += addition;
        workgroupBarrier();
        offset *= 2u;
    }
    if (index < candidate_count) {
        manifold_offsets[index] = scan_values[local.x] - value;
    }
    if (local.x == 255u) { scan_blocks[group.x] = scan_values[local.x]; }
}

@compute @workgroup_size(1)
fn scan_manifold_block_totals(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    var running = 0u;
    let block_count = (atomicLoad(&candidate_capacity[2]) + 255u) / 256u;
    for (var block = 0u; block < block_count; block++) {
        let value = scan_blocks[block];
        scan_blocks[block] = running;
        running += value;
    }
}

@compute @workgroup_size(256)
fn add_manifold_block_offsets(@builtin(global_invocation_id) global: vec3u,
                              @builtin(workgroup_id) group: vec3u) {
    if (global.x >= atomicLoad(&candidate_capacity[2])) { return; }
    manifold_offsets[global.x] += scan_blocks[group.x];
}

@compute @workgroup_size(1)
fn finalize_narrowphase(@builtin(global_invocation_id) invocation: vec3u) {
    if (invocation.x != 0u) { return; }
    let candidate_count = atomicLoad(&candidate_capacity[2]);
    var manifold_attempted = 0u;
    if (candidate_count > 0u) {
        let last = candidate_count - 1u;
        manifold_attempted = manifold_offsets[last] + manifold_flags[last];
    }
    let point_attempted = atomicLoad(&point_capacity[1]);
    let manifold_overflowed = manifold_attempted > params.counts.y;
    let point_overflowed = point_attempted > params.counts.z;
    let stage_rejected = manifold_overflowed || point_overflowed;
    let manifold_committed = select(manifold_attempted, 0u, stage_rejected);
    let point_committed = select(point_attempted, 0u, stage_rejected);
    atomicStore(&manifold_capacity[0], params.counts.y);
    atomicStore(&manifold_capacity[1], manifold_attempted);
    atomicStore(&manifold_capacity[2], manifold_committed);
    atomicStore(&manifold_capacity[3], select(0u, 1u, manifold_overflowed));
    atomicStore(&point_capacity[0], params.counts.z);
    atomicStore(&point_capacity[2], point_committed);
    atomicStore(&point_capacity[3], select(0u, 1u, point_overflowed));
    atomicStore(&point_capacity[4], (point_committed + 63u) / 64u);
    atomicStore(&point_capacity[5], 1u);
    atomicStore(&point_capacity[6], 1u);
    atomicStore(&point_capacity[7], (point_committed + 255u) / 256u);
    atomicStore(&point_capacity[8], 1u);
    atomicStore(&point_capacity[9], 1u);
    atomicStore(&point_capacity[10], (point_committed * 2u + 63u) / 64u);
    atomicStore(&point_capacity[11], 1u);
    atomicStore(&point_capacity[12], 1u);
    atomicStore(&point_capacity[13], (point_committed * 2u + 255u) / 256u);
    atomicStore(&point_capacity[14], 1u);
    atomicStore(&point_capacity[15], 1u);
    atomicStore(&work[5], atomicLoad(&candidate_capacity[2]));
    atomicStore(&work[6], manifold_attempted);
    atomicStore(&work[7], point_attempted);
    atomicStore(&work[15], params.execution.x);
}

@compute @workgroup_size(64)
fn scatter_narrowphase(@builtin(global_invocation_id) invocation: vec3u) {
    let index = invocation.x;
    if (index >= atomicLoad(&candidate_capacity[2]) || manifold_flags[index] == 0u ||
        atomicLoad(&manifold_capacity[3]) != 0u || atomicLoad(&point_capacity[3]) != 0u) {
        return;
    }
    let output_index = manifold_offsets[index];
    let scratch = narrow_scratch[index];
    manifolds[output_index].identities = scratch.identities;
    manifolds[output_index].normal_material = scratch.normal_material;
    manifolds[output_index].point_range = vec4u(output_index * 2u, scratch.point_count.x, 0u, 0u);
    points[output_index * 2u] = scratch.point0;
    if (scratch.point_count.x == 2u) { points[output_index * 2u + 1u] = scratch.point1; }
}
