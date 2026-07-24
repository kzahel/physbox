struct Words {
    values: array<u32, 64>,
}

@group(0) @binding(0) var<storage, read> input_words: Words;
@group(0) @binding(1) var<storage, read_write> output_words: Words;

fn input_value(index: u32) -> f32 {
    return bitcast<f32>(input_words.values[index]);
}

fn retain(index: u32, value: f32) -> f32 {
    output_words.values[index] = bitcast<u32>(value);
    return value;
}

fn rotate(vector: vec2f, angle: f32, output_base: u32) -> vec2f {
    let c = retain(output_base, cos(angle));
    let s = retain(output_base + 1u, sin(angle));
    return vec2f(
        retain(output_base + 2u, c * vector.x - s * vector.y),
        retain(output_base + 3u, s * vector.x + c * vector.y));
}

fn native_rotate(vector: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * vector.x - s * vector.y, s * vector.x + c * vector.y);
}

@compute @workgroup_size(1)
fn evaluate() {
    let normal = vec2f(input_value(0u), input_value(1u));
    let anchor_a = vec2f(input_value(2u), input_value(3u));
    let anchor_b = vec2f(input_value(4u), input_value(5u));
    let pose_a = vec2f(input_value(6u), input_value(7u));
    let pose_b = vec2f(input_value(8u), input_value(9u));
    let initial_position_a = vec2f(input_value(10u), input_value(11u));
    let initial_position_b = vec2f(input_value(12u), input_value(13u));
    let angle_a = input_value(14u);
    let angle_b = input_value(15u);
    let initial_angle_a = input_value(16u);
    let initial_angle_b = input_value(17u);
    let input_separation = input_value(18u);

    let delta_position_a = vec2f(
        retain(0u, pose_a.x - initial_position_a.x),
        retain(1u, pose_a.y - initial_position_a.y));
    let delta_position_b = vec2f(
        retain(2u, pose_b.x - initial_position_b.x),
        retain(3u, pose_b.y - initial_position_b.y));
    let delta_position = vec2f(
        retain(4u, delta_position_b.x - delta_position_a.x),
        retain(5u, delta_position_b.y - delta_position_a.y));
    let delta_angle_a = retain(6u, angle_a - initial_angle_a);
    let delta_angle_b = retain(7u, angle_b - initial_angle_b);
    let rotated_anchor_a = rotate(anchor_a, delta_angle_a, 8u);
    let rotated_anchor_b = rotate(anchor_b, delta_angle_b, 12u);
    let prepared_anchor_delta = vec2f(
        retain(16u, anchor_b.x - anchor_a.x),
        retain(17u, anchor_b.y - anchor_a.y));
    let prepared_anchor_projection = retain(
        18u, prepared_anchor_delta.x * normal.x + prepared_anchor_delta.y * normal.y);
    let base_separation = retain(19u, input_separation - prepared_anchor_projection);
    let rotated_anchor_delta = vec2f(
        retain(20u, rotated_anchor_b.x - rotated_anchor_a.x),
        retain(21u, rotated_anchor_b.y - rotated_anchor_a.y));
    let original_motion = vec2f(
        retain(22u, delta_position.x + rotated_anchor_delta.x),
        retain(23u, delta_position.y + rotated_anchor_delta.y));
    let original_projection = retain(
        24u, original_motion.x * normal.x + original_motion.y * normal.y);
    retain(25u, base_separation + original_projection);

    let rotation_displacement_a = vec2f(
        retain(26u, rotated_anchor_a.x - anchor_a.x),
        retain(27u, rotated_anchor_a.y - anchor_a.y));
    let rotation_displacement_b = vec2f(
        retain(28u, rotated_anchor_b.x - anchor_b.x),
        retain(29u, rotated_anchor_b.y - anchor_b.y));
    let relative_rotation_displacement = vec2f(
        retain(30u, rotation_displacement_b.x - rotation_displacement_a.x),
        retain(31u, rotation_displacement_b.y - rotation_displacement_a.y));
    let robust_motion = vec2f(
        retain(32u, delta_position.x + relative_rotation_displacement.x),
        retain(33u, delta_position.y + relative_rotation_displacement.y));
    let robust_projection = retain(
        34u, robust_motion.x * normal.x + robust_motion.y * normal.y);
    retain(35u, input_separation + robust_projection);

    // These two outputs deliberately retain only the complete expression.
    // They reproduce the optimizer-visible S13 and S14 operation graphs;
    // outputs 0-35 are a separately materialized arithmetic microscope.
    let native_delta_position_a = pose_a - initial_position_a;
    let native_delta_position_b = pose_b - initial_position_b;
    let native_delta_position = native_delta_position_b - native_delta_position_a;
    let native_anchor_a = native_rotate(anchor_a, angle_a - initial_angle_a);
    let native_anchor_b = native_rotate(anchor_b, angle_b - initial_angle_b);
    let native_base_separation = input_separation - dot(anchor_b - anchor_a, normal);
    output_words.values[36u] = bitcast<u32>(native_base_separation +
        dot(native_delta_position + (native_anchor_b - native_anchor_a), normal));
    output_words.values[37u] = bitcast<u32>(input_separation +
        dot(native_delta_position +
            ((native_anchor_b - anchor_b) - (native_anchor_a - anchor_a)), normal));
}
