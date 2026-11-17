//! The look the launcher and the setup share: a dark navy with Oteria's blue, flat surfaces with
//! small corners and one-pixel borders, Geist for text and Pixelify Sans for the wordmark
//! (`crates/horizon-theme/fonts`, OFL), the logo and Tabler icons rendered from their SVGs
//! (`assets/icons`, MIT) at the size they are drawn.
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect,
    Stroke, StrokeKind, TextureHandle, Vec2,
};
use std::collections::HashMap;
use std::sync::Arc;

pub mod c {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x0a, 0x11, 0x1d);
    pub const SIDEBAR: Color32 = Color32::from_rgb(0x0c, 0x15, 0x24);
    pub const PANEL: Color32 = Color32::from_rgb(0x10, 0x1b, 0x2d);
    pub const RAISED: Color32 = Color32::from_rgb(0x16, 0x24, 0x3a);
    pub const HOVER: Color32 = Color32::from_rgb(0x1c, 0x2d, 0x48);
    pub const BORDER: Color32 = Color32::from_rgb(0x1d, 0x2c, 0x44);
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
    pub const SELECTED: Color32 = Color32::from_rgb(0x12, 0x2e, 0x4c);
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
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/oteria-logo.png");
pub const LOGO_SVG: &[u8] = include_bytes!("../../branding/oteria-logo.svg");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("geist", GEIST),
        ("geist-medium", GEIST_MEDIUM),
        ("geist-semi", GEIST_SEMI),
        ("pixel", PIXEL),
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
/// The setup's titles.
pub fn display(size: f32) -> FontId {
    pixel(size)
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
    Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
}

/// The Tabler icons the launcher draws, by file name.
pub struct Icons(HashMap<&'static str, TextureHandle>);

macro_rules! icon_files {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/icons/", $name, ".svg")) as &[u8])),*]
    };
}

const ICONS: &[(&str, &[u8])] = icon_files![
    "device-gamepad-2",
    "puzzle",
    "building-store",
    "settings",
    "download",
    "folder",
    "external-link",
    "cpu",
    "server",
    "users",
    "refresh",
    "chevron-right",
    "user",
    "minus",
    "square",
    "x",
    "alert-triangle",
    "news",
    "player-play-filled",
    "player-stop-filled",
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
        let img = image::open(path).ok()?.to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        if w != 64 || (h != 64 && h != 32) {
            return None;
        }
        let rgba = img.into_raw();
        let a = |x: usize, y: usize| rgba[(y * 64 + x) * 4 + 3];
        let slim = h == 64 && (20..32).all(|y| a(54, y) == 0 && a(55, y) == 0);
        Some(Self { w, h, rgba, slim })
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

/// The player standing, front on, filling `rect`'s height (16:32).
pub fn draw_player(painter: &egui::Painter, rect: Rect, skin: Option<&Skin>) {
    let unit = rect.height() / 32.0;
    let left = rect.center().x - unit * 8.0;
    let top = rect.top();
    let r = |x: f32, y: f32, ww: f32, hh: f32| {
        Rect::from_min_size(
            Pos2::new(left + x * unit, top + y * unit),
            Vec2::new(ww * unit, hh * unit),
        )
    };
    let Some(s) = skin else {
        blit(painter, r(0.0, 0.0, 16.0, 32.0), 16, 32, default_texel, 0.0);
        return;
    };
    let arm_w = if s.slim { 3.0 } else { 4.0 };
    let arm = arm_w as usize;
    let modern = s.h == 64;
    let part = |sx: usize, sy: usize, sw: usize, sh: usize, dx: f32, dy: f32, shade: f32| {
        blit(
            painter,
            r(dx, dy, sw as f32, sh as f32),
            sw,
            sh,
            |x, y| s.px(sx + x, sy + y),
            shade,
        );
    };
    part(4, 20, 4, 12, 4.0, 20.0, 0.08);
    if modern {
        part(20, 52, 4, 12, 8.0, 20.0, 0.0);
    } else {
        blit(
            painter,
            r(8.0, 20.0, 4.0, 12.0),
            4,
            12,
            |x, y| s.px(4 + 3 - x, 20 + y),
            0.0,
        );
    }
    part(20, 20, 8, 12, 4.0, 8.0, 0.0);
    part(44, 20, arm, 12, 4.0 - arm_w, 8.0, 0.12);
    if modern {
        part(36, 52, arm, 12, 12.0, 8.0, 0.0);
    } else {
        blit(
            painter,
            r(12.0, 8.0, 4.0, 12.0),
            4,
            12,
            |x, y| s.px(44 + 3 - x, 20 + y),
            0.0,
        );
    }
    part(8, 8, 8, 8, 4.0, 0.0, 0.0);
    // The outer layer, a little larger.
    let outer = |sx: usize, sy: usize, sw: usize, sh: usize, dx: f32, dy: f32| {
        blit(
            painter,
            r(dx - 0.25, dy - 0.25, sw as f32 + 0.5, sh as f32 + 0.5),
            sw,
            sh,
            |x, y| s.px(sx + x, sy + y),
            0.0,
        );
    };
    if modern {
        outer(4, 36, 4, 12, 4.0, 20.0);
        outer(4, 52, 4, 12, 8.0, 20.0);
        outer(20, 36, 8, 12, 4.0, 8.0);
        outer(44, 36, arm, 12, 4.0 - arm_w, 8.0);
        outer(52, 52, arm, 12, 12.0, 8.0);
    }
    outer(40, 8, 8, 8, 4.0, 0.0);
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

/// Minimise, maximise and close, at the right of `bar`.
pub fn window_controls(ui: &mut egui::Ui, bar: Rect, icons: &Icons) {
    let ctx = ui.ctx().clone();
    let w = 46.0;
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    for (i, kind) in ["x", "square", "minus"].into_iter().enumerate() {
        let rect = Rect::from_min_size(
            Pos2::new(bar.right() - w * (i as f32 + 1.0), bar.top()),
            Vec2::new(w, bar.height().min(40.0)),
        );
        let resp = ui.interact(rect, ui.id().with(("winctl", kind)), egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            let fill = if kind == "x" {
                Color32::from_rgb(0xc4, 0x2b, 0x1c)
            } else {
                c::HOVER
            };
            p.rect_filled(rect, 0.0, fill);
        }
        let col = if resp.hovered() {
            Color32::WHITE
        } else {
            c::MUTED
        };
        let size = if kind == "square" { 14.0 } else { 18.0 };
        icons.draw(p, kind, rect.center(), size, col);
        if resp.clicked() {
            match kind {
                "x" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                "square" => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)),
                _ => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            }
        }
    }
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
