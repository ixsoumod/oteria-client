//! The look the launcher and the setup share: Oteria's palette (the logo's blues over a night
//! navy), the design system's fonts (Geist, Bricolage Grotesque: `crates/horizon-theme/fonts`,
//! OFL), the logo, and painting helpers.
use eframe::egui::{
    self, epaint::Mesh, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily,
    FontId, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle, Vec2,
};
use std::sync::Arc;

pub mod c {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x09, 0x0d, 0x14);
    pub const SIDEBAR: Color32 = Color32::from_rgb(0x0c, 0x11, 0x1a);
    pub const PANEL: Color32 = Color32::from_rgb(0x11, 0x18, 0x23);
    pub const CARD: Color32 = Color32::from_rgb(0x14, 0x1c, 0x29);
    pub const CARD_HOVER: Color32 = Color32::from_rgb(0x1a, 0x25, 0x36);
    pub const BORDER: Color32 = Color32::from_rgb(0x1f, 0x2a, 0x3b);
    pub const TEXT: Color32 = Color32::from_rgb(0xe9, 0xef, 0xf7);
    pub const MUTED: Color32 = Color32::from_rgb(0x86, 0x94, 0xa9);
    pub const FAINT: Color32 = Color32::from_rgb(0x4d, 0x5a, 0x6e);
    /// The logo's cyan wave.
    pub const ACCENT: Color32 = Color32::from_rgb(0x2b, 0xbf, 0xe9);
    /// The logo's big wave.
    pub const ACCENT_DEEP: Color32 = Color32::from_rgb(0x15, 0x90, 0xcc);
    pub const ACCENT_HI: Color32 = Color32::from_rgb(0x5c, 0xc6, 0xef);
    pub const ON_ACCENT: Color32 = Color32::from_rgb(0x04, 0x16, 0x24);
    pub const GOOD: Color32 = Color32::from_rgb(0x3d, 0xd6, 0x8c);
    pub const WARN: Color32 = Color32::from_rgb(0xf5, 0xb9, 0x4a);
    pub const BAD: Color32 = Color32::from_rgb(0xf2, 0x5f, 0x6b);
}

const GEIST: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Geist-Regular.ttf");
const GEIST_MEDIUM: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Geist-Medium.ttf");
const GEIST_SEMI: &[u8] = include_bytes!("../../crates/horizon-theme/fonts/Geist-SemiBold.ttf");
const DISPLAY: &[u8] =
    include_bytes!("../../crates/horizon-theme/fonts/BricolageGrotesqueDisplay-Bold.ttf");
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/oteria-logo.png");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("geist", GEIST),
        ("geist-medium", GEIST_MEDIUM),
        ("geist-semi", GEIST_SEMI),
        ("display", DISPLAY),
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
        ("display", "display"),
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
        s.visuals.selection.bg_fill = c::ACCENT_DEEP;
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
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("display".into()))
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

/// A rectangle filled with a gradient from `a` (top-left) to `b` (bottom-right), at an angle:
/// `horizontal` 0 is vertical, 1 is horizontal.
pub fn gradient(painter: &egui::Painter, rect: Rect, a: Color32, b: Color32, horizontal: f32) {
    let mut mesh = Mesh::default();
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let ts = [0.0, horizontal, 1.0, 1.0 - horizontal];
    for (p, t) in corners.iter().zip(ts) {
        mesh.colored_vertex(*p, lerp(a, b, t));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// A soft round glow: rings fading out from `color` at the centre.
pub fn glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    let steps = 24;
    for i in (1..=steps).rev() {
        let t = i as f32 / steps as f32;
        painter.circle_filled(
            center,
            radius * t,
            alpha(color, strength * (1.0 - t) * 0.16),
        );
    }
}

/// A button-like plate: fill, hover fill, and a one-pixel border.
pub fn plate(painter: &egui::Painter, rect: Rect, radius: u8, fill: Color32, border: Color32) {
    painter.rect(
        rect,
        CornerRadius::same(radius),
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
}

/// A small rounded label (`MINECRAFT 26.3`, a news tag).
pub fn pill(painter: &egui::Painter, pos: Pos2, text: &str, fg: Color32, bg: Color32) -> Rect {
    let galley = painter.layout_no_wrap(text.to_owned(), semi(11.0), fg);
    let rect = Rect::from_min_size(pos, galley.size() + Vec2::new(16.0, 8.0));
    painter.rect_filled(rect, CornerRadius::same(255), bg);
    painter.galley(rect.min + Vec2::new(8.0, 4.0), galley, fg);
    rect
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

/// Draws `w`×`h` texels of a source (`get(x, y)`) into `rect`, one rectangle per texel.
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
        // Head: x 4..12.
        0..=7 if (4..12).contains(&x) => match (x - 4, y) {
            (_, 0..=1) => hair,
            (0 | 7, 2) => hair,
            (1 | 6, 4) => Color32::WHITE,
            (2 | 5, 4) => eye,
            (3..=4, 6) => skin_dark,
            _ => skin,
        },
        // Arms (x 0..4 and 12..16) and the body (4..12), rows 8..20.
        8..=19 => {
            if x < 4 || x >= 12 {
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
                } else if y == 8 && (bx == 3 || bx == 4) {
                    hood_dark
                } else if y >= 18 {
                    hood_dark
                } else {
                    hood
                }
            }
        }
        // Legs: x 4..12, rows 20..32.
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
    let w = unit * 16.0;
    let left = rect.center().x - w / 2.0;
    let top = rect.top();
    let r = |x: f32, y: f32, ww: f32, hh: f32| {
        Rect::from_min_size(
            Pos2::new(left + x * unit, top + y * unit),
            Vec2::new(ww * unit, hh * unit),
        )
    };
    // A shadow on the ground.
    painter.add(Shape::ellipse_filled(
        Pos2::new(rect.center().x, rect.bottom()),
        Vec2::new(w * 0.55, unit * 1.4),
        Color32::from_black_alpha(110),
    ));
    let Some(s) = skin else {
        blit(painter, r(0.0, 0.0, 16.0, 32.0), 16, 32, default_texel, 0.0);
        return;
    };
    let arm_w = if s.slim { 3.0 } else { 4.0 };
    let arm = arm_w as usize;
    let modern = s.h == 64;
    // (source x, y, width, height) -> destination (x, y in units)
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
    // Legs: right leg (on the left of the picture) at 4,20; left leg at 20,52 (legacy: mirrored).
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
    part(20, 20, 8, 12, 4.0, 8.0, 0.0); // body
                                        // Arms: right arm at 44,20; left arm at 36,52 (legacy: mirrored).
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
    part(8, 8, 8, 8, 4.0, 0.0, 0.0); // head
                                     // The outer layer, a little larger.
    let outer = |sx: usize, sy: usize, sw: usize, sh: usize, dx: f32, dy: f32| {
        let grow = 0.5;
        blit(
            painter,
            r(
                dx - grow * 0.5,
                dy - grow * 0.5,
                sw as f32 + grow,
                sh as f32 + grow,
            ),
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

/// The player's face (with the hat layer), for the account chip.
pub fn draw_head(painter: &egui::Painter, rect: Rect, skin: Option<&Skin>) {
    match skin {
        Some(s) => {
            blit(painter, rect, 8, 8, |x, y| s.px(8 + x, 8 + y), 0.0);
            blit(painter, rect, 8, 8, |x, y| s.px(40 + x, 8 + y), 0.0);
        }
        None => blit(painter, rect, 8, 8, |x, y| default_texel(4 + x, y), 0.0),
    }
}

/// Window buttons drawn as in the references: minimise, maximise, close.
pub fn window_controls(ui: &mut egui::Ui, bar: Rect) {
    let ctx = ui.ctx().clone();
    let w = 46.0;
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    for (i, kind) in ["close", "max", "min"].into_iter().enumerate() {
        let rect = Rect::from_min_size(
            Pos2::new(bar.right() - w * (i as f32 + 1.0), bar.top()),
            Vec2::new(w, bar.height()),
        );
        let resp = ui.interact(rect, ui.id().with(("winctl", kind)), egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(
                rect,
                0.0,
                if kind == "close" {
                    Color32::from_rgb(0xc4, 0x2b, 0x1c)
                } else {
                    c::CARD_HOVER
                },
            );
        }
        let col = if resp.hovered() {
            Color32::WHITE
        } else {
            c::MUTED
        };
        let cn = rect.center();
        let st = Stroke::new(1.2_f32, col);
        match kind {
            "close" => {
                p.line_segment([cn + Vec2::new(-5.0, -5.0), cn + Vec2::new(5.0, 5.0)], st);
                p.line_segment([cn + Vec2::new(5.0, -5.0), cn + Vec2::new(-5.0, 5.0)], st);
            }
            "max" => {
                p.rect_stroke(
                    Rect::from_center_size(cn, Vec2::splat(10.0)),
                    1.0,
                    st,
                    StrokeKind::Inside,
                );
            }
            _ => {
                p.line_segment([cn + Vec2::new(-5.0, 0.0), cn + Vec2::new(5.0, 0.0)], st);
            }
        }
        if resp.clicked() {
            match kind {
                "close" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                "max" => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)),
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
