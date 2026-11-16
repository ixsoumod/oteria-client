// Oteria's motion blur (motion_blur.rs): the frames before this one, kept in a history, laid
// over it at the blend constant's share. Colour only; alpha stays the frame's (hdr-passes.md).
@group(0) @binding(0) var history: texture_2d<f32>;
@group(0) @binding(1) var history_sampler: sampler;

struct PostOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs_quad(@builtin(vertex_index) i: u32) -> PostOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: PostOut;
    out.clip = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2(p.x, 1.0 - p.y);
    return out;
}

@fragment fn fs_blur(in: PostOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(history, history_sampler, in.uv, 0.0);
    // No NaN, infinity or negative value into the frame (and so the history).
    let safe = select(vec3(0.0), max(c.rgb, vec3(0.0)), c.rgb == c.rgb);
    return vec4(min(safe, vec3(65504.0)), 1.0);
}
