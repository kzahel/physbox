struct UiParams {
    viewport: vec2f,
    padding: vec2f,
};

@group(0) @binding(0) var<uniform> params: UiParams;
@group(0) @binding(1) var glyph_atlas: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
    @location(2) mode: f32,
};

@vertex
fn vertex_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) rect: vec4f,
    @location(1) uv_rect: vec4f,
    @location(2) color: vec4f,
    @location(3) mode: vec4f,
) -> VertexOutput {
    let corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
        vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0),
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
    output.uv = mix(uv_rect.xy, uv_rect.zw, corner);
    output.color = color;
    output.mode = mode.x;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4f {
    if (input.mode < 0.5) {
        return input.color;
    }
    let coverage = textureSampleLevel(glyph_atlas, glyph_sampler, input.uv, 0.0).r;
    if (coverage < 0.5) {
        discard;
    }
    return vec4f(input.color.rgb, input.color.a * coverage);
}
