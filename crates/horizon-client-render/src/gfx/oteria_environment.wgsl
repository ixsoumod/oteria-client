// Oteria's environment (oteria_environment.rs): one pass over the drawn world, reading its
// depth. From back to front: the aurora sky (stars, sparkles, shooting stars) where nothing was
// drawn, the waves of tiles over the ground, the fog, the China hat and its halo, then the
// flakes and sparkles in the air. The colours are premultiplied and laid over the frame; its
// alpha is left alone (hdr-passes.md).

struct Env {
    // Clip space (as the world's depth was drawn) to camera-relative world space.
    inv_view_proj: mat4x4<f32>,
    // Camera-relative feet; w: how far the waves run, in blocks (0: no waves).
    feet: vec4<f32>,
    // Camera-relative top of the head; w: 1 with the hat on.
    head: vec4<f32>,
    ground: vec4<f32>,
    // w: how many particles, 0 to 1 (0: none).
    particles: vec4<f32>,
    hat: vec4<f32>,
    // w: 1 with the sky on.
    sky: vec4<f32>,
    // w: how thick the fog is, 0 to 1 (0: none).
    fog: vec4<f32>,
    // x: seconds, yz: the depth's size, w: the view distance in blocks.
    misc: vec4<f32>,
    // The camera's world position, modulo 4096 (cells and tiles stay precise).
    origin: vec4<f32>,
};

@group(0) @binding(0) var<uniform> env: Env;
@group(0) @binding(1) var depth_tex: texture_depth_2d;

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

const PI: f32 = 3.14159265;
const WHITE: vec3<f32> = vec3(1.0, 1.0, 1.0);

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3(p.x, p.y, p.x) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}
fn hash44(p: vec4<f32>) -> vec4<f32> {
    var p4 = fract(p * vec4(0.1031, 0.1030, 0.0973, 0.1099));
    p4 += dot(p4, p4.wzxy + 33.33);
    return fract((p4.xxyz + p4.yzzw) * p4.zywx);
}
fn hash11(n: f32) -> f32 {
    return fract(sin(n * 127.1) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2(1.0, 0.0));
    let c = hash12(i + vec2(0.0, 1.0));
    let d = hash12(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
fn fbm(p0: vec2<f32>) -> f32 {
    var p = p0;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < 4; i++) {
        v += a * noise(p);
        p = mat2x2(1.6, 1.2, -1.2, 1.6) * p;
        a *= 0.5;
    }
    return v;
}

fn unproject(ndc: vec2<f32>, z: f32) -> vec3<f32> {
    let h = env.inv_view_proj * vec4(ndc, z, 1.0);
    return h.xyz / h.w;
}

// Back-to-front: `c` at coverage `a` over what is there (premultiplied).
fn over(dst: vec4<f32>, c: vec3<f32>, a: f32) -> vec4<f32> {
    let k = clamp(a, 0.0, 1.0);
    return vec4(c * k + dst.rgb * (1.0 - k), k + dst.a * (1.0 - k));
}

// A four-pointed star with its glow, `q` from its centre in units of its size.
fn sparkle(q: vec2<f32>) -> f32 {
    let ax = abs(q.x);
    let ay = abs(q.y);
    let rays = exp(-ax * 26.0) * exp(-ay * 3.2) + exp(-ay * 26.0) * exp(-ax * 3.2);
    let core = exp(-dot(q, q) * 60.0);
    let glow = exp(-length(q) * 5.0) * 0.35;
    return min(rays + core + glow, 1.6);
}

// The sky: purple dark at the horizon, waves of light across it, stars.
fn sky_colour(dir: vec3<f32>) -> vec3<f32> {
    let t = env.misc.x;
    let c = env.sky.rgb;
    let e = dir.y;
    let horizon = c * vec3(0.44, 0.42, 0.40);
    let middle = c * 0.84;
    let top = c * 0.66;
    var col = mix(horizon, middle, smoothstep(-0.02, 0.32, e));
    col = mix(col, top, smoothstep(0.38, 1.0, e));

    // The aurora: soft folds over a plane high above, drifting.
    let h = max(e, 0.03);
    let p = dir.xz / (h + 0.22) * 1.4;
    let drift = vec2(t * 0.035, -t * 0.022);
    let warp = fbm(p * 0.32 + drift);
    let folds = sin(p.x * 0.55 + p.y * 0.95 + warp * 6.5 + t * 0.12) * 0.5 + 0.5;
    let fine = fbm(p * 0.9 - drift * 1.7);
    let up = smoothstep(0.0, 0.22, e) * (1.0 - 0.35 * smoothstep(0.6, 1.0, e));
    let light = pow(folds, 2.6) * (0.55 + 0.6 * fine) * up;
    let shade = pow(1.0 - folds, 5.0) * up;
    let bright = mix(c, WHITE, 0.2) * 1.12;
    col = mix(col, bright, clamp(light * 0.75, 0.0, 1.0));
    col *= 1.0 - 0.45 * shade;

    // Stars: dots on a grid over the sphere, each twinkling.
    let az = atan2(dir.z, dir.x);
    let el = asin(clamp(e, -1.0, 1.0));
    let lat = max(cos(el), 0.15);
    let above = smoothstep(0.02, 0.12, e);
    {
        let g = vec2(az * lat, el) * 70.0;
        let cell = floor(g);
        let r = hash22(cell);
        let q = (g - cell - (0.2 + 0.6 * r)) ;
        let on = step(hash12(cell + 17.0), 0.32);
        let tw = 0.55 + 0.45 * sin(t * (1.3 + r.x * 3.0) + r.y * 40.0);
        let dot_ = exp(-dot(q, q) * 160.0) * on * tw * (0.5 + 0.5 * r.y);
        col = mix(col, mix(bright, WHITE, 0.7), clamp(dot_, 0.0, 1.0) * above);
    }
    // Sparkles: rare four-pointed stars, flaring and fading.
    {
        let g = vec2(az * lat, el) * 9.0;
        let cell = floor(g);
        let r = hash22(cell + 3.7);
        let on = step(hash12(cell + 91.0), 0.22);
        let q = (g - cell - (0.25 + 0.5 * r)) * 7.0;
        let pulse = pow(0.5 + 0.5 * sin(t * (0.9 + r.x * 1.4) + r.y * 30.0), 3.0);
        let s = sparkle(q / max(pulse, 0.05)) * pulse * on;
        col = mix(col, mix(bright, WHITE, 0.5) * 1.2, clamp(s, 0.0, 1.0) * above);
    }
    // A shooting star now and then: a pink streak falling across the sky.
    {
        let period = 6.5;
        let n = floor(t / period);
        let phase = fract(t / period);
        let life = 0.32;
        if (phase < life) {
            let k = phase / life;
            let a0 = hash11(n) * 2.0 * PI - PI;
            let e0 = 0.45 + 0.35 * hash11(n + 5.0);
            let heading = vec2(select(-1.0, 1.0, hash11(n + 9.0) > 0.5) * 0.9, -0.42);
            let start = vec2(a0 * cos(e0), e0);
            let head = start + heading * k * 0.75;
            let tail = head - heading * 0.22 * (0.3 + 0.7 * k);
            let pt = vec2(az * lat, el);
            // The nearest point on the streak; wrap around the azimuth seam.
            var d = pt - tail;
            d.x = d.x - round(d.x / (2.0 * PI * lat)) * 2.0 * PI * lat;
            let seg = head - tail;
            let s = clamp(dot(d, seg) / dot(seg, seg), 0.0, 1.0);
            let off = length(d - seg * s);
            let fade = sin(k * PI);
            let streak = exp(-off * off / 0.000006) * s * fade;
            let glow = exp(-off * 220.0) * s * fade * 0.4;
            col = mix(col, mix(c, WHITE, 0.55) * 1.4, clamp(streak + glow, 0.0, 1.0) * above);
        }
    }
    return max(col, vec3(0.0));
}

fn fog_amount(dist: f32) -> f32 {
    let k = mix(0.006, 0.03, env.fog.w) * 0.85;
    let x = dist * k;
    return (1.0 - exp(-x * x)) * 0.92;
}

// The waves: rings of tiles running out from the feet, each tile lit whole.
fn ground_light(tile_dist: f32) -> f32 {
    let reach = env.feet.w;
    let t = env.misc.x;
    let span = 1.9;
    var lit = 0.0;
    for (var i = 0; i < 2; i++) {
        let age = fract(t / span + f32(i) * 0.5);
        let r = age * (reach + 2.5);
        let behind = smoothstep(r - 3.2, r - 0.6, tile_dist);
        let ahead = 1.0 - step(r + 0.25, tile_dist);
        let fade = 1.0 - smoothstep(0.65, 1.0, age);
        lit = max(lit, behind * ahead * fade);
    }
    let edge = 1.0 - smoothstep(reach * 0.6, reach, tile_dist);
    return lit * edge;
}

// The China hat and the halo above it, where the ray meets them nearer than `limit`.
fn hat_layer(dst0: vec4<f32>, dir: vec3<f32>, limit: f32) -> vec4<f32> {
    var dst = dst0;
    let col = env.hat.rgb;
    let apex = env.head.xyz + vec3(0.0, 0.40, 0.0);
    let height = 0.30;
    let radius = 0.64;
    let k = radius / height;
    let k2 = k * k;
    let ox = -apex.x;
    let oz = -apex.z;
    let oy = apex.y;
    let a = dir.x * dir.x + dir.z * dir.z - k2 * dir.y * dir.y;
    let b = 2.0 * (ox * dir.x + oz * dir.z + k2 * oy * dir.y);
    let c = ox * ox + oz * oz - k2 * oy * oy;
    let disc = b * b - 4.0 * a * c;
    if (disc > 0.0 && abs(a) > 1e-6) {
        let sq = sqrt(disc);
        let t0 = (-b - sq) / (2.0 * a);
        let t1 = (-b + sq) / (2.0 * a);
        let near = min(t0, t1);
        let far = max(t0, t1);
        // The far side first, the near side over it.
        for (var i = 0; i < 2; i++) {
            let t = select(near, far, i == 0);
            let x = dir * t;
            let down = apex.y - x.y;
            if (t > 0.05 && t < limit && down >= 0.0 && down <= height) {
                let radial = x.xz - apex.xz;
                let rl = max(length(radial), 1e-4);
                let n = normalize(vec3(radial.x / rl, k, radial.y / rl));
                let facing = abs(dot(n, dir));
                let rim = pow(1.0 - facing, 2.0);
                let brim = smoothstep(0.86, 1.0, down / height);
                let rings = 0.5 + 0.5 * sin(down / height * 18.0);
                let lightness = 0.82 + 0.25 * n.y + 0.12 * rings;
                let shade = mix(col * lightness, WHITE, 0.25 * rim + 0.45 * brim);
                let alpha = (0.42 + 0.35 * rim + 0.4 * brim) * select(1.0, 0.55, i == 0);
                dst = over(dst, shade * 1.1, alpha);
            }
        }
    }
    // The halo: a thin bright ring floating over the apex, and its glow.
    let ring_y = apex.y + 0.16;
    if (abs(dir.y) > 1e-4) {
        let t = ring_y / dir.y;
        if (t > 0.05 && t < limit) {
            let x = dir * t;
            let rr = length(x.xz - apex.xz);
            let d = abs(rr - 0.34);
            let line = exp(-d * d / 0.00018);
            let glow = exp(-d * 28.0) * 0.35;
            dst = over(dst, mix(col, WHITE, 0.3) * 1.25, clamp(line + glow, 0.0, 1.0) * 0.95);
        }
    }
    return dst;
}

// The flakes and sparkles in the air, traced cell by cell along the ray, nearest first.
fn particle_layer(dir: vec3<f32>, limit: f32) -> vec4<f32> {
    let t_now = env.misc.x;
    let amount = env.particles.w;
    let pcol = env.particles.rgb;
    let cell = 2.4;
    let reach = 42.0;
    // The particles drift with a slow wind and sink; tracing in their frame moves them.
    let drift = vec3(t_now * 0.22, -t_now * 0.16, t_now * 0.11);
    let ro = (env.origin.xyz - drift) / cell;
    var c = floor(ro);
    let st = sign(dir);
    let inv = 1.0 / select(dir, vec3(1e-6), abs(dir) < vec3(1e-6));
    let delta = abs(inv);
    var tmax = (c + max(st, vec3(0.0)) - ro) * inv;
    let right = normalize(cross(dir, vec3(0.0, 1.0, 0.0)) + vec3(1e-5, 0.0, 0.0));
    let up = cross(right, dir);
    let end = min(reach, limit) / cell;
    var acc = vec4(0.0);
    var t_cell = 0.0;
    for (var i = 0; i < 48; i++) {
        if (t_cell > end || acc.a > 0.97) {
            break;
        }
        let h = hash44(vec4(c, 7.0));
        if (h.x < amount * 0.42) {
            let h2 = hash44(vec4(c, 13.0));
            let bob = vec3(sin(t_now * 0.7 + h.y * 30.0), cos(t_now * 0.5 + h.z * 30.0), sin(t_now * 0.6 + h.w * 30.0)) * 0.08;
            let centre = (c + 0.22 + 0.56 * h2.xyz + bob) * cell;
            let rel = centre - ro * cell;
            let t = dot(rel, dir);
            if (t > 0.35 && t < limit && t < reach) {
                let off = rel - dir * t;
                let q = vec2(dot(off, right), dot(off, up));
                let fade = smoothstep(0.8, 2.2, t) * (1.0 - smoothstep(reach * 0.6, reach, t));
                var a = 0.0;
                var col = pcol;
                if (h2.w < 0.22) {
                    // A sparkle, flaring and fading.
                    let pulse = 0.35 + 0.65 * pow(0.5 + 0.5 * sin(t_now * (1.5 + h.y * 2.0) + h.z * 40.0), 2.0);
                    let size = 0.2 * pulse;
                    a = sparkle(q / size) * 0.9;
                    col = mix(pcol, WHITE, 0.35) * 1.3;
                } else {
                    // A flake: a small square, turning.
                    let ang = t_now * (0.6 + h.y * 1.5) + h.z * 6.28;
                    let cs = cos(ang);
                    let sn = sin(ang);
                    let r = vec2(cs * q.x - sn * q.y, sn * q.x + cs * q.y);
                    let size = 0.035 + 0.035 * h.w;
                    let soft = max(size * 0.18, t * 0.0012);
                    a = 1.0 - smoothstep(size - soft, size + soft, max(abs(r.x), abs(r.y)));
                    col = mix(pcol, WHITE, 0.55 + 0.35 * h.y);
                }
                a = clamp(a * fade, 0.0, 1.0);
                // Nearest first: what is behind shows through what is in front.
                acc = vec4(acc.rgb + (1.0 - acc.a) * col * a, acc.a + (1.0 - acc.a) * a);
            }
        }
        // To the next cell along the ray.
        if (tmax.x < tmax.y && tmax.x < tmax.z) {
            t_cell = tmax.x;
            c.x += st.x;
            tmax.x += delta.x;
        } else if (tmax.y < tmax.z) {
            t_cell = tmax.y;
            c.y += st.y;
            tmax.y += delta.y;
        } else {
            t_cell = tmax.z;
            c.z += st.z;
            tmax.z += delta.z;
        }
    }
    return acc;
}

@fragment fn fs_environment(in: PostOut) -> @location(0) vec4<f32> {
    let ndc = vec2(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let size = max(env.misc.yz, vec2(1.0));
    let texel = vec2<i32>(clamp(in.uv * size, vec2(0.0), size - 1.0));
    let depth = textureLoad(depth_tex, texel, 0);
    // Reversed depth: nothing drawn is 0, the sky.
    let is_sky = depth <= 0.0;
    // Two points on the pixel's ray (near plane, twice as far): view bobbing moves the eye as
    // well as turning it, and a point this near the camera taken alone would swing the sky.
    let dir = normalize(unproject(ndc, 0.5) - unproject(ndc, 1.0));
    let pos = unproject(ndc, max(depth, 1e-7));
    let dist = select(length(pos), 1e9, is_sky);

    // Derivatives in uniform control flow, for the ground's lines and faces.
    let world = pos + env.origin.xyz;
    let fw = fwidth(world);
    let normal = normalize(cross(dpdy(pos), dpdx(pos)));

    var acc = vec4(0.0);
    if (is_sky) {
        if (env.sky.w > 0.5) {
            // Below the horizon the sky keeps its darkest colour.
            acc = over(acc, sky_colour(dir), 1.0);
        }
        if (env.fog.w > 0.0) {
            let low = 1.0 - smoothstep(-0.05, 0.22 + 0.2 * env.fog.w, dir.y);
            acc = over(acc, env.fog.rgb, low * 0.85 * select(1.0, 0.55, env.sky.w > 0.5));
        }
    } else {
        // The waves, on the top faces near the feet.
        if (env.feet.w > 0.0) {
            let feet = env.feet.xyz + env.origin.xyz;
            let top = abs(normal.y) > 0.8 && dir.y < 0.0;
            let level = round(world.y);
            let flat_ = abs(world.y - level) < 0.01 + dist * 0.0015;
            let near = abs(level - feet.y) < 4.5;
            if (top && flat_ && near) {
                let tile = floor(world.xz + vec2(1e-3));
                let tile_dist = length(tile + 0.5 - feet.xz);
                let reach = env.feet.w;
                if (tile_dist < reach * 1.25) {
                    let lit = ground_light(tile_dist);
                    let f = fract(world.xz);
                    let edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
                    let lw = max(0.022, max(fw.x, fw.z) * 1.1);
                    let line = 1.0 - smoothstep(lw * 0.5, lw, edge);
                    let around = (1.0 - smoothstep(reach * 0.35, reach * 1.25, tile_dist)) * 0.16;
                    let gc = env.ground.rgb;
                    acc = over(acc, gc, lit * 0.5);
                    acc = over(acc, mix(gc, WHITE, 0.3) * 1.15, line * clamp(around + lit * 0.85, 0.0, 1.0));
                }
            }
        }
        if (env.fog.w > 0.0) {
            // Far off, the fog fades into the sky behind, so the distance and the clouds melt
            // into the horizon.
            let f = fog_amount(dist);
            var fog_col = env.fog.rgb;
            if (env.sky.w > 0.5) {
                fog_col = mix(fog_col, sky_colour(dir), smoothstep(0.45, 0.9, f));
            }
            acc = over(acc, fog_col, f);
        }
    }
    if (env.head.w > 0.5) {
        acc = hat_layer(acc, dir, dist);
    }
    if (env.particles.w > 0.0) {
        let p = particle_layer(dir, dist);
        // Premultiplied, over the rest.
        acc = vec4(p.rgb + acc.rgb * (1.0 - p.a), p.a + acc.a * (1.0 - p.a));
    }
    // No NaN, infinity or negative value into the frame.
    let rgb = select(vec3(0.0), max(acc.rgb, vec3(0.0)), acc.rgb == acc.rgb);
    let a = select(0.0, clamp(acc.a, 0.0, 1.0), acc.a == acc.a);
    return vec4(min(rgb, vec3(65504.0)), a);
}
