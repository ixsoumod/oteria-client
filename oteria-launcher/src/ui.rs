//! The look the launcher and the setup share: a dark navy with Oteria's blue, flat surfaces with
//! small corners, Geist for text and Pixelify Sans for the name tag over the player
//! (`crates/horizon-theme/fonts`, OFL), the logo and Bootstrap Icons rendered from their SVGs
//! (`assets/icons`, MIT) at the size they are drawn.
use eframe::egui::{
    self, epaint, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle, Vec2,
};
use std::collections::HashMap;
use std::sync::Arc;

pub mod c {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x0c, 0x11, 0x18);
    pub const SIDEBAR: Color32 = Color32::from_rgb(0x0c, 0x15, 0x24);
    pub const PANEL: Color32 = Color32::from_rgb(0x13, 0x1a, 0x24);
    pub const RAISED: Color32 = Color32::from_rgb(0x1a, 0x23, 0x2f);
    pub const HOVER: Color32 = Color32::from_rgb(0x22, 0x2d, 0x3b);
    pub const BORDER: Color32 = Color32::from_rgb(0x23, 0x2e, 0x3d);
    pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xee, 0xf6);
    pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x9a, 0xb2);
    pub const FAINT: Color32 = Color32::from_rgb(0x55, 0x65, 0x7e);
    /// The logo's big wave.
    pub const ACCENT: Color32 = Color32::from_rgb(0x18, 0x8f, 0xd1);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x26, 0xa3, 0xe6);
    /// The darker edge under the main button.
    pub const ACCENT_EDGE: Color32 = Color32::from_rgb(0x0e, 0x62, 0x97);
    /// The logo's cyan wave, for small accents.
    pub const CYAN: Color32 = Color32::from_rgb(0x2b, 0xbf, 0xe9);
    pub const SELECTED: Color32 = Color32::from_rgb(0x1f, 0x2b, 0x3a);
    pub const GOOD: Color32 = Color32::from_rgb(0x46, 0xc2, 0x7a);
    pub const WARN: Color32 = Color32::from_rgb(0xe8, 0xb0, 0x45);
    pub const BAD: Color32 = Color32::from_rgb(0xd9, 0x4b, 0x4b);
    pub const BAD_HOVER: Color32 = Color32::from_rgb(0xe6, 0x5c, 0x5c);
    pub const BAD_EDGE: Color32 = Color32::from_rgb(0x9a, 0x2f, 0x2f);
}

const GEIST: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Geist-Regular.ttf");
const GEIST_MEDIUM: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Geist-Medium.ttf");
const GEIST_SEMI: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Geist-SemiBold.ttf");
const PIXEL: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/PixelifySans-Bold.ttf");
/// The game's own font, for the pseudo tag over the player (not Pixelify Sans, a lookalike).
const MINECRAFT: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Minecraft.ttf");
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/oteria-logo.png");
/// Oteria's own character, for a player who has no skin yet (its own design, not Mojang's Steve:
/// see `AGENTS.md`, never commit or ship Mojang's files).
const DEFAULT_SKIN_PNG: &[u8] = include_bytes!("../assets/default-skin.png");
pub const LOGO_SVG: &[u8] = include_bytes!("../../branding/oteria-logo.svg");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("geist", GEIST),
        ("geist-medium", GEIST_MEDIUM),
        ("geist-semi", GEIST_SEMI),
        ("pixel", PIXEL),
        ("minecraft", MINECRAFT),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "geist".into());
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    for (family, first) in [
        ("medium", "geist-medium"),
        ("semi", "geist-semi"),
        ("pixel", "pixel"),
        ("minecraft", "minecraft"),
    ] {
        let mut list = vec![first.to_owned()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
    ctx.set_fonts(fonts);
    ctx.style_mut(|s| {
        s.visuals = egui::Visuals::dark();
        s.visuals.panel_fill = c::BG;
        s.visuals.window_fill = c::PANEL;
        s.visuals.extreme_bg_color = c::BG;
        s.visuals.selection.bg_fill = c::ACCENT;
        s.visuals.override_text_color = Some(c::TEXT);
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;
    });
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
pub fn semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semi".into()))
}
/// Pixelify Sans, for the wordmark and the big title.
pub fn pixel(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("pixel".into()))
}
/// Minecraft's own font, for the pseudo tag over the player.
pub fn minecraft(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("minecraft".into()))
}
/// The setup's titles.
pub fn display(size: f32) -> FontId {
    semi(size)
}

pub fn texture_from_png(ctx: &egui::Context, name: &str, png: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
        egui::TextureOptions::LINEAR,
    ))
}

/// An SVG rendered to a square texture of `px` pixels. `white` turns `currentColor` white, so
/// the icon takes its colour from the tint it is drawn with.
pub fn svg_texture(
    ctx: &egui::Context,
    name: &str,
    svg: &[u8],
    px: u32,
    white: bool,
) -> Option<TextureHandle> {
    let text = String::from_utf8_lossy(svg);
    let text = if white {
        text.replace("currentColor", "#ffffff")
    } else {
        text.into_owned()
    };
    let tree =
        resvg::usvg::Tree::from_data(text.as_bytes(), &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / size.width().max(size.height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let image =
        egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data());
    // Mipmaps: drawn far smaller than rendered, thin strokes (a plus, the arrows) stay visible.
    let options = egui::TextureOptions {
        mipmap_mode: Some(egui::TextureFilter::Linear),
        ..egui::TextureOptions::LINEAR
    };
    Some(ctx.load_texture(name, image, options))
}

/// The Bootstrap Icons the launcher draws, by file name.
pub struct Icons(HashMap<&'static str, TextureHandle>);

macro_rules! icon_files {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/icons/", $name, ".svg")) as &[u8])),*]
    };
}

const ICONS: &[(&str, &[u8])] = icon_files![
    "controller",
    "puzzle-fill",
    "shop",
    "gear-fill",
    "play-fill",
    "stop-fill",
    "download",
    "folder2-open",
    "box-arrow-up-right",
    "person-fill",
    "person-plus",
    "people-fill",
    "chevron-down",
    "chevron-right",
    "shuffle",
    "windows",
    "x-lg",
    "dash-lg",
    "dash",
    "x",
    "arrows-angle-expand",
    "arrows-angle-contract",
    "cart-fill",
    "folder-fill",
    "file-earmark-image-fill",
    "square",
    "check-lg",
    "cpu",
    "hdd",
    "server",
    "arrow-repeat",
    "trash3",
    "box-seam",
    "plus-lg",
    "clipboard",
    "newspaper",
    "hdd-network",
];

impl Icons {
    pub fn load(ctx: &egui::Context) -> Self {
        let px = ((64.0 * ctx.pixels_per_point().max(1.0)) as u32).min(128);
        Self(
            ICONS
                .iter()
                .filter_map(|(name, svg)| Some((*name, svg_texture(ctx, name, svg, px, true)?)))
                .collect(),
        )
    }

    /// Draws icon `name` so that its drawing itself (not its 16×16 box) is `size` points: the
    /// dash, the x and the play triangle fill only part of their box, the arrows all of it, and
    /// side by side they should look the same size.
    pub fn draw_fit(&self, p: &egui::Painter, name: &str, center: Pos2, size: f32, color: Color32) {
        // The share of the 16-unit box each icon's drawing spans (its larger side).
        let span = match name {
            "dash" => 8.0,
            "x" => 6.8,
            "play-fill" => 9.0,
            _ => 16.0,
        };
        self.draw(p, name, center, size * 16.0 / span, color);
    }

    /// Draws icon `name` centred on `center`, `size` points wide, in `color`.
    pub fn draw(&self, p: &egui::Painter, name: &str, center: Pos2, size: f32, color: Color32) {
        if let Some(t) = self.0.get(name) {
            p.image(
                t.id(),
                Rect::from_center_size(center, Vec2::splat(size)),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                color,
            );
        }
    }
}

pub fn icon_data() -> egui::IconData {
    let img = image::load_from_memory(LOGO_PNG).unwrap().to_rgba8();
    egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}
pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// A flat surface with a one-pixel border.
pub fn plate(painter: &egui::Painter, rect: Rect, radius: u8, fill: Color32, border: Color32) {
    painter.rect(
        rect,
        CornerRadius::same(radius),
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
}

/// Text clipped to `rows` lines within `width`.
pub fn wrapped(
    painter: &egui::Painter,
    pos: Pos2,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    rows: usize,
) -> Rect {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, width);
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = false;
    let galley = painter.layout_job(job);
    let rect = Rect::from_min_size(pos, galley.size());
    painter.galley(pos, galley, color);
    rect
}

pub fn text(
    painter: &egui::Painter,
    pos: Pos2,
    align: Align2,
    s: &str,
    font: FontId,
    color: Color32,
) -> Rect {
    painter.text(pos, align, s, font, color)
}

/// Text with its letters spaced out, for small capitals (`LAUNCHER`, section titles).
pub fn spaced(
    painter: &egui::Painter,
    pos: Pos2,
    s: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
) -> Rect {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        s,
        0.0,
        egui::TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let rect = Rect::from_min_size(pos, galley.size());
    painter.galley(pos, galley, color);
    rect
}

/// A skin's pixels (64x64 or legacy 64x32), for drawing the player.
pub struct Skin {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
    slim: bool,
}

impl Skin {
    pub fn load(path: &std::path::Path) -> Option<Self> {
        Self::from_rgba(image::open(path).ok()?.to_rgba8())
    }

    /// A skin's PNG bytes, as downloaded from a Microsoft account's profile.
    pub fn from_bytes(png: &[u8]) -> Option<Self> {
        Self::from_rgba(image::load_from_memory(png).ok()?.to_rgba8())
    }

    /// Oteria's own character, for a player who has no skin yet.
    pub fn default_skin() -> Self {
        Self::from_bytes(DEFAULT_SKIN_PNG).expect("default-skin.png")
    }

    fn from_rgba(img: image::RgbaImage) -> Option<Self> {
        let (w, h) = (img.width() as usize, img.height() as usize);
        if w != 64 || (h != 64 && h != 32) {
            return None;
        }
        let mut rgba = img.into_raw();
        // Vanilla's "Notch hack": an old 64×32 skin whose hat area is fully opaque means no hat.
        if h == 32 {
            let hat = |y: usize| (32..64).map(move |x| (y * 64 + x) * 4 + 3);
            if (0..16).flat_map(hat).all(|i| rgba[i] == 255) {
                for i in (0..16).flat_map(hat) {
                    rgba[i] = 0;
                }
            }
        }
        let a = |x: usize, y: usize| rgba[(y * 64 + x) * 4 + 3];
        let slim = h == 64 && (20..32).all(|y| a(54, y) == 0 && a(55, y) == 0);
        Some(Self { w, h, rgba, slim })
    }

    /// Narrow (3 wide) arms, like Alex's.
    pub fn slim(&self) -> bool {
        self.slim
    }

    /// 64×32: no overlay layer, no left arm/leg of its own.
    pub fn legacy(&self) -> bool {
        self.h == 32
    }

    pub fn size(&self) -> [f32; 2] {
        [self.w as f32, self.h as f32]
    }

    /// Uploads this skin as a GPU texture, nearest-filtered so its texels stay crisp.
    pub fn texture(&self, ctx: &egui::Context, name: &str) -> TextureHandle {
        ctx.load_texture(
            name,
            egui::ColorImage::from_rgba_unmultiplied([self.w, self.h], &self.rgba),
            egui::TextureOptions::NEAREST,
        )
    }

    fn px(&self, x: usize, y: usize) -> Color32 {
        if x >= self.w || y >= self.h {
            return Color32::TRANSPARENT;
        }
        let i = (y * self.w + x) * 4;
        Color32::from_rgba_unmultiplied(
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        )
    }
}

/// Draws `w`×`h` texels of a source (`get(x, y)`) into `rect`, one rectangle per texel,
/// darkened by `shade` (0 to 1) for the sides that face away from the light.
fn blit(
    painter: &egui::Painter,
    rect: Rect,
    w: usize,
    h: usize,
    get: impl Fn(usize, usize) -> Color32,
    shade: f32,
) {
    let (sx, sy) = (rect.width() / w as f32, rect.height() / h as f32);
    for y in 0..h {
        for x in 0..w {
            let col = get(x, y);
            if col.a() == 0 {
                continue;
            }
            let col = lerp(
                col,
                Color32::from_rgba_unmultiplied(0, 0, 0, col.a()),
                shade,
            );
            let min = Pos2::new(rect.min.x + x as f32 * sx, rect.min.y + y as f32 * sy);
            painter.rect_filled(
                Rect::from_min_size(min, Vec2::new(sx + 0.6, sy + 0.6)),
                0.0,
                col,
            );
        }
    }
}

/// Oteria's own default player, for a player who wears no skin yet: 16×32 texels front on, an
/// Oteria-blue hoodie with the wave on the chest.
fn default_texel(x: usize, y: usize) -> Color32 {
    let hair = Color32::from_rgb(0x2a, 0x1d, 0x16);
    let skin = Color32::from_rgb(0xd9, 0xa5, 0x7e);
    let skin_dark = Color32::from_rgb(0xbf, 0x8a, 0x66);
    let eye = Color32::from_rgb(0x1f, 0x6f, 0xb0);
    let hood = Color32::from_rgb(0x15, 0x90, 0xcc);
    let hood_dark = Color32::from_rgb(0x0f, 0x6c, 0x9c);
    let wave = Color32::from_rgb(0xe9, 0xf7, 0xff);
    let pants = Color32::from_rgb(0x1c, 0x24, 0x33);
    let shoe = Color32::from_rgb(0x0e, 0x12, 0x1a);
    match y {
        0..=7 if (4..12).contains(&x) => match (x - 4, y) {
            (_, 0..=1) => hair,
            (0 | 7, 2) => hair,
            (1 | 6, 4) => Color32::WHITE,
            (2 | 5, 4) => eye,
            (3..=4, 6) => skin_dark,
            _ => skin,
        },
        8..=19 => {
            if !(4..12).contains(&x) {
                if y >= 18 {
                    skin
                } else if y == 17 {
                    hood_dark
                } else {
                    hood
                }
            } else {
                let bx = x - 4;
                let chest = (y == 11 && (2..=5).contains(&bx))
                    || (y == 12 && (bx == 1 || bx == 6))
                    || (y == 13 && (2..=5).contains(&bx));
                if chest {
                    wave
                } else if (y == 8 && (bx == 3 || bx == 4)) || y >= 18 {
                    hood_dark
                } else {
                    hood
                }
            }
        }
        20..=31 if (4..12).contains(&x) => {
            if y >= 30 {
                shoe
            } else if x == 7 || x == 8 {
                Color32::from_rgb(0x16, 0x1d, 0x29)
            } else {
                pants
            }
        }
        _ => Color32::TRANSPARENT,
    }
}

/// The player's face (with the hat layer), for the account.
pub fn draw_head(painter: &egui::Painter, rect: Rect, skin: Option<&Skin>) {
    match skin {
        Some(s) => {
            blit(painter, rect, 8, 8, |x, y| s.px(8 + x, 8 + y), 0.0);
            blit(painter, rect, 8, 8, |x, y| s.px(40 + x, 8 + y), 0.0);
        }
        None => blit(painter, rect, 8, 8, |x, y| default_texel(4 + x, y), 0.0),
    }
}

// ----- The player, in 3D, for the Jouer page --------------------------------------------------
//
// The vanilla player model (head, body, arms, legs, each with its jacket/hat/sleeve/pants layer),
// posed leaning over the Jouer button and holding it, seen from the front and a little above in
// perspective. Model space is the game's own, in texels, Y up, the feet at 0, the face toward +Z.

type V3 = [f32; 3];

fn v_add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn v_sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v_scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn v_dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn v_cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn v_norm(a: V3) -> V3 {
    let l = v_dot(a, a).sqrt().max(1e-6);
    v_scale(a, 1.0 / l)
}
/// Turns around X: a positive angle brings +Y toward +Z (toward the viewer).
fn rot_x(v: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c]
}
/// Turns around Z: a positive angle brings +X toward +Y.
fn rot_z(v: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]]
}

/// The pose and the camera. Angles in radians.
const CAM_PITCH: f32 = 0.34;
const CAM_DIST: f32 = 62.0;
const CAM_TARGET: V3 = [0.0, 19.0, 0.0];
const HIP: V3 = [0.0, 12.0, 0.0];
/// The whole upper body bends forward around the hips, over the button.
const LEAN: f32 = 0.22;
const NECK: V3 = [0.0, 24.0, 0.0];
/// The head looks down at the button.
const HEAD_DOWN: f32 = 0.26;
/// The arms reach forward and down, and a little outward, to the button's ends.
const ARM_FORWARD: f32 = -0.85;
const ARM_OUT: f32 = 0.26;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Limb {
    Head,
    Body,
    Arm,
    Leg,
}

/// One box of the model: its corners (before `inflate`), its spot in the skin (the vanilla
/// box layout from `uv`), and how it is turned around its `pivot`.
struct Bone {
    min: V3,
    max: V3,
    uv: [f32; 2],
    inflate: f32,
    /// A legacy (64×32) skin's left limb: the right one's texture, mirrored.
    mirror: bool,
    pivot: V3,
    pitch: f32,
    roll: f32,
    limb: Limb,
}

fn bones(slim: bool, legacy: bool) -> Vec<Bone> {
    let aw = if slim { 3.0 } else { 4.0 };
    // A slim arm hangs half a texel lower (vanilla's own offset).
    let sy = if slim { 21.5 } else { 22.0 };
    let bone = |min: V3, max: V3, uv: [f32; 2], inflate: f32, mirror: bool, pivot: V3, pitch: f32, roll: f32, limb: Limb| Bone {
        min,
        max,
        uv,
        inflate,
        mirror,
        pivot,
        pitch,
        roll,
        limb,
    };
    let mut out = Vec::new();
    for (uv, inflate) in [([0.0, 0.0], 0.0), ([32.0, 0.0], 0.5)] {
        out.push(bone([-4.0, 24.0, -4.0], [4.0, 32.0, 4.0], uv, inflate, false, NECK, HEAD_DOWN, 0.0, Limb::Head));
    }
    let body = |uv, inflate| bone([-4.0, 12.0, -2.0], [4.0, 24.0, 2.0], uv, inflate, false, HIP, 0.0, 0.0, Limb::Body);
    out.push(body([16.0, 16.0], 0.0));
    let right_arm = |uv, inflate, mirror| {
        bone([-4.0 - aw, sy - 10.0, -2.0], [-4.0, sy + 2.0, 2.0], uv, inflate, mirror, [-5.0, sy, 0.0], ARM_FORWARD, -ARM_OUT, Limb::Arm)
    };
    let left_arm = |uv, inflate, mirror| {
        bone([4.0, sy - 10.0, -2.0], [4.0 + aw, sy + 2.0, 2.0], uv, inflate, mirror, [5.0, sy, 0.0], ARM_FORWARD, ARM_OUT, Limb::Arm)
    };
    let right_leg = |uv, inflate, mirror| bone([-4.0, 0.0, -2.0], [0.0, 12.0, 2.0], uv, inflate, mirror, HIP, 0.0, 0.0, Limb::Leg);
    let left_leg = |uv, inflate, mirror| bone([0.0, 0.0, -2.0], [4.0, 12.0, 2.0], uv, inflate, mirror, HIP, 0.0, 0.0, Limb::Leg);
    out.push(right_arm([40.0, 16.0], 0.0, false));
    out.push(right_leg([0.0, 16.0], 0.0, false));
    if legacy {
        out.push(left_arm([40.0, 16.0], 0.0, true));
        out.push(left_leg([0.0, 16.0], 0.0, true));
    } else {
        out.push(body([16.0, 32.0], 0.25));
        out.push(right_arm([40.0, 32.0], 0.25, false));
        out.push(right_leg([0.0, 32.0], 0.25, false));
        out.push(left_arm([32.0, 48.0], 0.0, false));
        out.push(left_arm([48.0, 48.0], 0.25, false));
        out.push(left_leg([16.0, 48.0], 0.0, false));
        out.push(left_leg([0.0, 48.0], 0.25, false));
    }
    out
}

/// A model-space point of `b`, posed and turned to the camera (view space: the camera sits at
/// `(0, 0, CAM_DIST)` looking toward -Z).
fn pose(b: &Bone, p: V3) -> V3 {
    let mut p = v_add(rot_z(rot_x(v_sub(p, b.pivot), b.pitch), b.roll), b.pivot);
    if b.limb != Limb::Leg {
        p = v_add(rot_x(v_sub(p, HIP), LEAN), HIP);
    }
    rot_x(v_sub(p, CAM_TARGET), CAM_PITCH)
}

/// A view-space point on the screen, one texel at the target's distance being one unit, Y down.
fn project(v: V3) -> Pos2 {
    let d = (CAM_DIST - v[2]).max(1.0);
    Pos2::new(CAM_DIST * v[0] / d, -CAM_DIST * v[1] / d)
}

/// One textured quad (one texel of one face), in unit screen space.
#[derive(Clone, Copy)]
struct Cell {
    pts: [Pos2; 4],
    uv: Rect,
    color: Color32,
}

/// The posed player, ready to draw at any size: built once per skin (`PlayerFigure::new`), then
/// only scaled and moved each frame.
pub struct PlayerFigure {
    /// Back to front.
    cells: Vec<Cell>,
    /// The arms' own cells, drawn a second time over the button where the hands hold it.
    arm_cells: Vec<Cell>,
    /// The highest point of the head (hat included), in unit screen space.
    pub head_top: f32,
    /// The hands (the arms' ends), left then right on the screen, in unit screen space.
    pub hands: [Pos2; 2],
}

impl PlayerFigure {
    pub fn new(skin: &Skin) -> Self {
        let atlas = skin.size();
        let light = v_norm([0.3, 0.6, 0.75]);
        let camera: V3 = [0.0, 0.0, CAM_DIST];
        let mut faces: Vec<(f32, bool, Vec<Cell>)> = Vec::new();
        let mut head_top = f32::MAX;
        let mut hands = Vec::new();
        for b in bones(skin.slim(), skin.legacy()) {
            let [w, h, d] = v_sub(b.max, b.min);
            let i = b.inflate;
            let [x0, y0, z0] = v_sub(b.min, [i, i, i]);
            let [x1, y1, z1] = v_add(b.max, [i, i, i]);
            let [u, v] = b.uv;
            let side_minus = [u, v + d, d, h];
            let side_plus = [u + d + w, v + d, d, h];
            let (sm, sp) = if b.mirror {
                (side_plus, side_minus)
            } else {
                (side_minus, side_plus)
            };
            // Each face: its rect in the skin, then its corners top-left, top-right,
            // bottom-right, bottom-left, as seen from outside.
            let list: [([f32; 4], [V3; 4]); 6] = [
                ([u + d, v + d, w, h], [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]]),
                ([u + 2.0 * d + w, v + d, w, h], [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]]),
                (sm, [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]]),
                (sp, [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]]),
                ([u + d, v, w, d], [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]]),
                ([u + d + w, v, w, d], [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]]),
            ];
            if b.limb == Limb::Head {
                for (_, corners) in &list {
                    for c in corners {
                        head_top = head_top.min(project(pose(&b, *c)).y);
                    }
                }
            }
            if b.limb == Limb::Arm && b.inflate == 0.0 {
                let end = [(b.min[0] + b.max[0]) / 2.0, b.min[1], 0.0];
                hands.push(project(pose(&b, end)));
            }
            for (rect, corners) in list {
                let [tl, tr, br, bl] = corners.map(|c| pose(&b, c));
                let n = v_norm(v_cross(v_sub(bl, tl), v_sub(tr, tl)));
                let center = v_scale(v_add(v_add(tl, tr), v_add(br, bl)), 0.25);
                if v_dot(n, v_sub(camera, center)) <= 0.0 {
                    continue;
                }
                let k = 0.6 + 0.4 * v_dot(n, light).max(0.0);
                let color = Color32::from_gray((k.min(1.0) * 255.0) as u8);
                let (cols, rows) = (rect[2] as usize, rect[3] as usize);
                let at = |s: f32, t: f32| {
                    project(v_add(
                        tl,
                        v_add(v_scale(v_sub(tr, tl), s), v_scale(v_sub(bl, tl), t)),
                    ))
                };
                let mut cells = Vec::new();
                for j in 0..rows {
                    for i in 0..cols {
                        let col = if b.mirror { cols - 1 - i } else { i };
                        let (tx, ty) = (rect[0] as usize + col, rect[1] as usize + j);
                        if skin.px(tx, ty).a() == 0 {
                            continue;
                        }
                        let (s0, s1) = (i as f32 / cols as f32, (i + 1) as f32 / cols as f32);
                        let (t0, t1) = (j as f32 / rows as f32, (j + 1) as f32 / rows as f32);
                        // Inset a hair inside the texel so nearest sampling never bleeds.
                        let e = 0.02;
                        cells.push(Cell {
                            pts: [at(s0, t0), at(s1, t0), at(s1, t1), at(s0, t1)],
                            uv: Rect::from_min_max(
                                Pos2::new((tx as f32 + e) / atlas[0], (ty as f32 + e) / atlas[1]),
                                Pos2::new(
                                    (tx as f32 + 1.0 - e) / atlas[0],
                                    (ty as f32 + 1.0 - e) / atlas[1],
                                ),
                            ),
                            color,
                        });
                    }
                }
                faces.push((CAM_DIST - center[2], b.limb == Limb::Arm, cells));
            }
        }
        faces.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut cells = Vec::new();
        let mut arm_cells = Vec::new();
        for (_, arm, c) in faces {
            if arm {
                arm_cells.extend_from_slice(&c);
            }
            cells.extend(c);
        }
        hands.sort_by(|a, b| a.x.total_cmp(&b.x));
        let hands = [hands[0], hands[hands.len() - 1]];
        Self {
            cells,
            arm_cells,
            head_top,
            hands,
        }
    }

    /// Draws the figure (or only its arms), its unit point (0, 0) at `origin`, one unit being
    /// `scale` points.
    pub fn draw(
        &self,
        painter: &egui::Painter,
        tex: &TextureHandle,
        origin: Pos2,
        scale: f32,
        arms_only: bool,
    ) {
        let cells = if arms_only {
            &self.arm_cells
        } else {
            &self.cells
        };
        let mut mesh = epaint::Mesh::with_texture(tex.id());
        mesh.reserve_triangles(cells.len() * 2);
        mesh.reserve_vertices(cells.len() * 4);
        for c in cells {
            let base = mesh.vertices.len() as u32;
            let uvs = [
                c.uv.left_top(),
                c.uv.right_top(),
                c.uv.right_bottom(),
                c.uv.left_bottom(),
            ];
            for (p, uv) in c.pts.iter().zip(uvs) {
                mesh.vertices.push(epaint::Vertex {
                    pos: origin + p.to_vec2() * scale,
                    uv,
                    color: c.color,
                });
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        painter.add(Shape::mesh(mesh));
    }
}

/// Turns off Windows' minimize, maximize and restore animations for this thread's windows: on a
/// window without a title bar, Windows animates a stale picture of it, blending the old size with
/// the new (doubled, stretched textures). Call once the window exists: only the first call acts.
pub fn no_window_animations() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    #[cfg(windows)]
    ONCE.call_once(|| unsafe {
        use windows_sys::Win32::Foundation::{HWND, LPARAM};
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
        };
        use windows_sys::Win32::System::Threading::GetCurrentThreadId;
        use windows_sys::Win32::UI::WindowsAndMessaging::EnumThreadWindows;
        unsafe extern "system" fn each(hwnd: HWND, _: LPARAM) -> windows_sys::core::BOOL {
            let off: i32 = 1;
            unsafe {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                    (&off as *const i32).cast(),
                    std::mem::size_of::<i32>() as u32,
                );
            }
            1
        }
        EnumThreadWindows(GetCurrentThreadId(), Some(each), 0);
    });
    #[cfg(not(windows))]
    let _ = &ONCE;
}

/// How far a maximized window without a title bar hangs past the screen on each side but the
/// bottom (Windows keeps room for a frame it does not draw), in points.
pub fn maximized_overhang(ctx: &egui::Context) -> f32 {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if cfg!(windows) && maximized {
        8.0
    } else {
        0.0
    }
}

/// The pointing hand over anything clickable while the mouse is on it.
pub fn hand(resp: &egui::Response) {
    if resp.hovered() {
        resp.ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// The hover bubble behind an icon: faint white, rounded, a little larger than the icon.
pub fn bubble(p: &egui::Painter, center: Pos2, size: f32, strength: f32) {
    p.rect_filled(
        Rect::from_center_size(center, Vec2::splat(size)),
        CornerRadius::same((size * 0.28).round() as u8),
        Color32::from_white_alpha((22.0 * strength).round() as u8),
    );
}

/// The window buttons' drawn size and spacing.
pub const WINDOW_ICON: f32 = 13.0;
const WINDOW_STEP: f32 = 36.0;

/// Minimise, maximise (or restore) and close, at the top right of `bar`: the same size, the same
/// distance apart, a bubble behind the one under the mouse.
pub fn window_controls(ui: &mut egui::Ui, bar: Rect, icons: &Icons) {
    let ctx = ui.ctx().clone();
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let resize = if maximized {
        "arrows-angle-contract"
    } else {
        "arrows-angle-expand"
    };
    let cy = bar.top() + bar.height().min(60.0) / 2.0;
    for (i, kind) in ["x", resize, "dash"].into_iter().enumerate() {
        let center = Pos2::new(bar.right() - 26.0 - WINDOW_STEP * i as f32, cy);
        let rect = Rect::from_center_size(center, Vec2::splat(WINDOW_STEP - 4.0));
        let resp = ui.interact(rect, ui.id().with(("winctl", i)), egui::Sense::click());
        hand(&resp);
        let p = ui.painter();
        if resp.hovered() {
            bubble(p, center, WINDOW_ICON + 16.0, 1.0);
        }
        let col = if resp.hovered() {
            Color32::WHITE
        } else {
            c::MUTED
        };
        icons.draw_fit(p, kind, center, WINDOW_ICON, col);
        if resp.clicked() {
            match i {
                0 => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                1 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)),
                _ => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            }
        }
    }
}

/// A 1×64 texture that ramps from `top` to `bottom` opacity of one dark colour, laid over a
/// picture (a rounded rectangle textured with it) so text on the picture stays readable.
pub fn vramp_texture(ctx: &egui::Context, name: &str, top: f32, bottom: f32) -> TextureHandle {
    let h = 64;
    let px = (0..h)
        .map(|y| {
            let t = y as f32 / (h - 1) as f32;
            Color32::from_rgba_unmultiplied(5, 9, 15, ((top + (bottom - top) * t) * 255.0) as u8)
        })
        .collect();
    ctx.load_texture(
        name,
        egui::ColorImage::new([1, h], px),
        egui::TextureOptions::LINEAR,
    )
}

/// Saves a screenshot event's picture as a PNG.
pub fn save_screenshot(image: &egui::ColorImage, path: &std::path::Path) -> Result<(), String> {
    let [w, h] = image.size;
    let mut buf = Vec::with_capacity(w * h * 4);
    for p in &image.pixels {
        buf.extend_from_slice(&p.to_srgba_unmultiplied());
    }
    image::save_buffer(path, &buf, w as u32, h as u32, image::ColorType::Rgba8)
        .map_err(|e| e.to_string())
}
