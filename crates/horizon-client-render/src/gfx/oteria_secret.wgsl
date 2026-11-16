// Super Secret Settings (oteria_secret.rs): 1.8's fun screen effects, redrawn from the frame's copy.
// Each effect works on a colour brought into 0..1 (its own brightest channel above 1 divides it) and
// gives that light back after, so a pixel brighter than paper white stays as bright (hdr-passes.md).
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var<uniform> u: Params;

struct Params {
    effect: u32,
    time: f32,
    size: vec2<f32>,
};

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

// The frame at `uv` (0..1), the nearest pixel, kept on screen.
fn at(uv: vec2<f32>) -> vec3<f32> {
    let p = clamp(vec2<i32>(uv * u.size), vec2<i32>(0), vec2<i32>(u.size) - vec2<i32>(1));
    let c = textureLoad(frame, p, 0).rgb;
    // No NaN, infinity or negative value.
    return min(select(vec3(0.0), max(c, vec3(0.0)), c == c), vec3(65504.0));
}
fn px(uv: vec2<f32>, dx: f32, dy: f32) -> vec3<f32> {
    return at(uv + vec2(dx, dy) / u.size);
}
fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.299, 0.587, 0.114));
}
fn peak(c: vec3<f32>) -> f32 {
    return max(1.0, max(c.r, max(c.g, c.b)));
}
fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}
fn sobel(uv: vec2<f32>) -> f32 {
    let tl = luma(px(uv, -1.0, -1.0));
    let t = luma(px(uv, 0.0, -1.0));
    let tr = luma(px(uv, 1.0, -1.0));
    let l = luma(px(uv, -1.0, 0.0));
    let r = luma(px(uv, 1.0, 0.0));
    let bl = luma(px(uv, -1.0, 1.0));
    let b = luma(px(uv, 0.0, 1.0));
    let br = luma(px(uv, 1.0, 1.0));
    let gx = (tr + 2.0 * r + br) - (tl + 2.0 * l + bl);
    let gy = (bl + 2.0 * b + br) - (tl + 2.0 * t + tr);
    return sqrt(gx * gx + gy * gy);
}
fn blur(uv: vec2<f32>, r: f32) -> vec3<f32> {
    var sum = vec3(0.0);
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            sum += px(uv, f32(x) * r, f32(y) * r);
        }
    }
    return sum / 25.0;
}

@fragment fn fs_secret(in: PostOut) -> @location(0) vec4<f32> {
    var uv = in.uv;
    let t = u.time;
    var c: vec3<f32>;
    switch u.effect {
        // Notch: big pixels in a few colours.
        case 1u: {
            let cell = 4.0 / u.size;
            let q = (floor(uv / cell) + 0.5) * cell;
            let s = at(q);
            let k = peak(s);
            c = floor(s / k * 6.0 + 0.5) / 6.0 * k;
        }
        // FXAA: each pixel eased towards its neighbours where the light changes.
        case 2u: {
            let m = at(uv);
            let n = (px(uv, 1.0, 0.0) + px(uv, -1.0, 0.0) + px(uv, 0.0, 1.0) + px(uv, 0.0, -1.0)) * 0.25;
            let e = clamp(sobel(uv) * 2.0, 0.0, 1.0);
            c = mix(m, n, e * 0.75);
        }
        // Art: flat colours with dark strokes, as a painting.
        case 3u: {
            let s = blur(uv, 1.0);
            let k = peak(s);
            let flat = floor(s / k * 5.0 + 0.5) / 5.0;
            let edge = smoothstep(0.15, 0.45, sobel(uv));
            c = mix(flat, flat * 0.15, edge) * k;
        }
        // Bumpy: embossed.
        case 4u: {
            let s = at(uv);
            let d = luma(px(uv, -1.0, -1.0)) - luma(px(uv, 1.0, 1.0));
            c = s * (0.6 + clamp(d * 3.0, -0.5, 0.5) + 0.4);
        }
        // Blobs: round lenses drift over the screen.
        case 5u: {
            var o = vec2(0.0);
            for (var i = 0; i < 6; i++) {
                let fi = f32(i);
                let centre = vec2(0.5 + 0.35 * sin(t * 0.3 + fi * 1.7), 0.5 + 0.35 * cos(t * 0.23 + fi * 2.3));
                let d = (uv - centre) * vec2(u.size.x / u.size.y, 1.0);
                let r = length(d);
                o += d * 0.12 * exp(-r * r * 40.0);
            }
            c = at(uv - o);
        }
        // Pencil: grey strokes on paper.
        case 6u: {
            let e = sobel(uv);
            let grain = hash(floor(uv * u.size / 2.0)) * 0.08;
            let shade = 1.0 - clamp(e * 2.5, 0.0, 1.0);
            let tone = floor(luma(at(uv)) / peak(at(uv)) * 4.0) / 4.0;
            let hatch = step(0.5, fract((uv.x + uv.y) * u.size.y / 6.0)) * (1.0 - tone) * 0.25;
            c = vec3(clamp(shade - hatch - grain, 0.0, 1.0)) * vec3(0.98, 0.96, 0.9);
        }
        // Color Convolve: more saturation and contrast.
        case 7u: {
            let s = at(uv);
            let g = luma(s);
            let sat = mix(vec3(g), s, 1.8);
            c = max((sat - 0.5 * peak(s)) * 1.25 + 0.5 * peak(s), vec3(0.0));
        }
        // Deconverge: red, green and blue drift apart from the middle.
        case 8u: {
            let d = (uv - 0.5) * (0.012 + 0.006 * sin(t * 1.3));
            c = vec3(at(uv + d).r, at(uv).g, at(uv - d).b);
        }
        // Flip: upside down.
        case 9u: {
            c = at(vec2(uv.x, 1.0 - uv.y));
        }
        // Invert.
        case 10u: {
            let s = at(uv);
            let k = peak(s);
            c = (vec3(1.0) - s / k) * k;
        }
        // NTSC: an old television: soft colours bleeding sideways, scanlines and a roll.
        case 11u: {
            let roll = fract(t * 0.07);
            let y = fract(uv.y + 0.002 * sin(uv.y * 300.0 + t * 20.0));
            let base = vec2(uv.x, y);
            let s = (at(base) * 0.5 + px(base, -2.0, 0.0) * 0.3 + px(base, -4.0, 0.0) * 0.2);
            let chroma = vec3(px(base, 2.0, 0.0).r, s.g, px(base, -2.0, 0.0).b);
            let line = 0.85 + 0.15 * sin(uv.y * u.size.y * 3.14159);
            let band = 1.0 - 0.15 * smoothstep(0.0, 0.05, 0.05 - abs(uv.y - roll));
            c = mix(s, chroma, 0.6) * line * band + vec3(hash(uv * u.size + t) * 0.04);
        }
        // Outline: black lines round everything.
        case 12u: {
            let e = smoothstep(0.1, 0.3, sobel(uv));
            c = at(uv) * (1.0 - e);
        }
        // Scan Pincushion: a curved screen with scanlines.
        case 13u: {
            let p = uv * 2.0 - 1.0;
            let q = p * (1.0 + 0.12 * dot(p, p));
            let w = q * 0.5 + 0.5;
            if any(w < vec2(0.0)) || any(w > vec2(1.0)) {
                c = vec3(0.0);
            } else {
                let line = 0.75 + 0.25 * sin(w.y * u.size.y * 1.5);
                let vignette = 1.0 - 0.35 * dot(p, p);
                c = at(w) * line * vignette;
            }
        }
        // Sobel: glowing edges on black.
        case 14u: {
            let e = sobel(uv);
            c = at(uv) * clamp(e * 3.0, 0.0, 1.0) * 1.5;
        }
        // Bits: few colours, dithered.
        case 15u: {
            let cell = 2.0 / u.size;
            let q = (floor(uv / cell) + 0.5) * cell;
            let s = at(q);
            let k = peak(s);
            let p = vec2<u32>(floor(uv * u.size / 2.0));
            let bayer = f32(((p.x ^ p.y) & 1u) * 2u + (p.y & 1u)) / 4.0 - 0.375;
            c = floor(s / k * 3.0 + 0.5 + bayer) / 3.0 * k;
        }
        // Desaturate.
        case 16u: {
            c = vec3(luma(at(uv)));
        }
        // Green: night vision.
        case 17u: {
            let g = luma(at(uv)) * 1.6 + hash(uv * u.size + floor(t * 30.0)) * 0.08;
            let vignette = 1.0 - 0.6 * length(uv - 0.5);
            c = vec3(0.1, 1.0, 0.2) * g * vignette;
        }
        // Blur.
        case 18u: {
            c = blur(uv, 2.0);
        }
        // Wobble: the screen sways.
        case 19u: {
            let o = vec2(sin(uv.y * 12.0 + t * 2.0), cos(uv.x * 10.0 + t * 1.7)) * 0.012;
            c = at(uv + o);
        }
        // Creeper: green, grainy, low.
        case 20u: {
            let cell = 3.0 / u.size;
            let q = (floor(uv / cell) + 0.5) * cell;
            let s = at(q);
            let g = luma(s);
            c = vec3(g * 0.5, g * 1.3, g * 0.45) * (0.9 + 0.1 * hash(floor(uv / cell)));
        }
        // Spider: eight eyes, each seeing a little apart.
        case 21u: {
            var sum = vec3(0.0);
            for (var i = 0; i < 8; i++) {
                let a = f32(i) * 0.785398;
                let o = vec2(cos(a), sin(a)) * 0.025;
                sum += at((uv - 0.5) * 0.95 + 0.5 + o);
            }
            let vignette = 1.0 - 0.5 * length(uv - 0.5);
            c = sum / 8.0 * vignette;
        }
        default: {
            c = at(uv);
        }
    }
    return vec4(max(select(vec3(0.0), c, c == c), vec3(0.0)), 1.0);
}
