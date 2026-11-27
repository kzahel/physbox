struct V { @builtin(position) position: vec4f, @location(0) uv: vec2f };
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var filter_sampler: sampler;
@vertex fn vertex(@builtin(vertex_index) i: u32) -> V {
    let xy = array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[i];
    return V(vec4f(xy,0,1),xy * vec2f(0.5,-0.5) + vec2f(0.5));
}
@fragment fn fragment(v: V) -> @location(0) vec4f {
    let uv = v.uv;
    let d = 1.0 / vec2f(textureDimensions(field));
    let f = textureSampleLevel(field,filter_sampler,uv,0.0).rgb;
    let above = textureSampleLevel(field,filter_sampler,uv - d * vec2f(1.0,2.0),0.0).r;
    let below = textureSampleLevel(field,filter_sampler,uv + d * vec2f(1.0,2.0),0.0).r;
    let opacity = smoothstep(0.052,0.15,f.r);
    let speed = clamp(f.g / max(f.r,0.025),0.0,1.0);
    let lit = smoothstep(0.14, 0.65, below-above) * (1.0 - smoothstep(0.4,0.85,above));
    let height = clamp(f.b / max(f.r,0.025),0.0,1.0);
    let body = mix(vec3f(0.008,0.105,0.26),vec3f(0.05,0.49,0.61),0.30*speed + 0.70*height);
    let rim = mix(vec3f(0.22,0.69,0.72),vec3f(0.64,0.89,0.9),speed);
    return vec4f(mix(body,rim,lit),opacity*0.96);
}
