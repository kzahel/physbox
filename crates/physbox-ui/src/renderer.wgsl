struct UiParams {
    viewport: vec2f,
    padding: vec2f,
};

// The procedural toolbar icon distance fields selectively adapt PhysBox 4
// revision 92f745eb85908243c69080a4fa4dba24f05c22cf
// (`crates/physbox-render/src/toolbar.wgsl`). PhysBox 5 assigns neutral icon
// identities and keeps application meaning in the caller.

@group(0) @binding(0) var<uniform> params: UiParams;
@group(0) @binding(1) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
    @location(2) local: vec2f,
    @location(3) size: vec2f,
    @location(4) data: vec4f,
    @location(5) border: vec4f,
};

@vertex
fn vertex_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) rect: vec4f,
    @location(1) uv_or_border: vec4f,
    @location(2) color: vec4f,
    @location(3) data: vec4f,
) -> VertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let pixel = rect.xy + corner * rect.zw;
    var output: VertexOutput;
    output.position = vec4f(
        pixel.x / max(params.viewport.x, 1.0) * 2.0 - 1.0,
        1.0 - pixel.y / max(params.viewport.y, 1.0) * 2.0,
        0.0,
        1.0,
    );
    output.uv = mix(uv_or_border.xy, uv_or_border.zw, corner);
    output.color = color;
    output.local = corner * 2.0 - 1.0;
    output.size = rect.zw;
    output.data = data;
    output.border = uv_or_border;
    return output;
}

fn rounded_box_distance(point: vec2f, half_size: vec2f, radius: f32) -> f32 {
    let resolved_radius = min(radius, min(half_size.x, half_size.y));
    let q = abs(point) - half_size + vec2f(resolved_radius);
    return length(max(q, vec2f(0.0))) +
        min(max(q.x, q.y), 0.0) - resolved_radius;
}

fn segment_distance(point: vec2f, a: vec2f, b: vec2f) -> f32 {
    let segment = b - a;
    let projection = clamp(
        dot(point - a, segment) / max(dot(segment, segment), 0.0001),
        0.0,
        1.0,
    );
    return length(point - (a + segment * projection));
}

fn icon_distance(icon: u32, point: vec2f) -> f32 {
    if icon == 1u {
        let left = length(point - vec2f(-0.42, 0.0)) - 0.10;
        let middle = length(point) - 0.10;
        let right = length(point - vec2f(0.42, 0.0)) - 0.10;
        return min(left, min(middle, right));
    }
    if icon == 2u {
        let palm = abs(length(point - vec2f(0.0, 0.08)) - 0.30) - 0.065;
        let left_finger = segment_distance(
            point, vec2f(-0.24, -0.06), vec2f(-0.24, -0.48),
        ) - 0.065;
        let middle_finger = segment_distance(
            point, vec2f(0.0, -0.18), vec2f(0.0, -0.58),
        ) - 0.065;
        let right_finger = segment_distance(
            point, vec2f(0.24, -0.06), vec2f(0.24, -0.45),
        ) - 0.065;
        let wrist = segment_distance(
            point, vec2f(0.0, 0.30), vec2f(0.0, 0.62),
        ) - 0.075;
        return min(palm, min(wrist, min(left_finger, min(middle_finger, right_finger))));
    }
    if icon == 3u {
        return abs(length(point) - 0.46) - 0.065;
    }
    if icon == 4u {
        return abs(max(abs(point.x), abs(point.y)) - 0.46) - 0.065;
    }
    if icon == 5u {
        return min(
            segment_distance(point, vec2f(-0.46), vec2f(0.46)),
            segment_distance(point, vec2f(-0.46, 0.46), vec2f(0.46, -0.46)),
        ) - 0.075;
    }
    if icon == 6u {
        let left = length(point - vec2f(-0.30, 0.18)) - 0.22;
        let middle = length(point - vec2f(0.0, -0.24)) - 0.24;
        let right = length(point - vec2f(0.32, 0.16)) - 0.21;
        return min(left, min(middle, right));
    }
    if icon == 7u {
        return segment_distance(
            point, vec2f(-0.55, 0.30), vec2f(0.55, -0.30),
        ) - 0.075;
    }
    return 1.0;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4f {
    let mode = input.data.x;
    let point = input.local * input.size * 0.5;
    let rounded_distance = rounded_box_distance(point, input.size * 0.5, input.data.y);
    let rounded_antialias = max(fwidth(rounded_distance), 0.75);
    let icon_shape_distance = icon_distance(u32(input.data.w + 0.5), input.local);
    let icon_antialias = max(fwidth(icon_shape_distance), 0.018);
    if mode < 0.5 {
        let radius = input.data.y;
        if radius <= 0.0 {
            return input.color;
        }
        let coverage = 1.0 - smoothstep(
            -rounded_antialias,
            rounded_antialias,
            rounded_distance,
        );
        if coverage <= 0.0 {
            discard;
        }
        let border_width = max(input.data.z, 0.0);
        let border_mix = smoothstep(
            -border_width - rounded_antialias,
            -border_width + rounded_antialias,
            rounded_distance,
        );
        let color = mix(input.color, input.border, select(0.0, border_mix, border_width > 0.0));
        return vec4f(color.rgb, color.a * coverage);
    }
    if mode < 1.5 {
        let coverage = textureSampleLevel(glyph_atlas, glyph_sampler, input.uv, 0.0).r;
        if coverage < 0.5 {
            discard;
        }
        return vec4f(input.color.rgb, input.color.a * coverage);
    }
    let coverage = 1.0 - smoothstep(
        -icon_antialias,
        icon_antialias,
        icon_shape_distance,
    );
    if coverage <= 0.0 {
        discard;
    }
    return vec4f(input.color.rgb, input.color.a * coverage);
}
