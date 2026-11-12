//! Oteria's menu on Right Shift, over the world (as Lunar's): its logo and name, then three
//! buttons — Boost (a rocket), Mods, and Cosmetics (a t-shirt). The game goes on behind it,
//! blurred (`Screen::blurs_background`).
//!
//! Every motion runs on the clock, not on the frame count, so the menu looks the same at 60 or
//! 600 FPS: it eases in and out, the buttons light up and give under the pointer, Boost glows
//! violet in a loop, and pressing Boost sends a shock wave across the screen. For now Boost and
//! Cosmetics only animate; Mods fades the buttons out for the mods page (`oteria_mods`), and
//! Escape fades back.
//!
//! The GUI is drawn pixel for pixel (crisp text and icons), so a slow drift would move one pixel
//! at a time and stutter: what loops slowly changes its light (a glow's opacity), and only fast
//! motions (the ripple, the sparks, the shock wave) change places.

use super::oteria_mods::{Action, Mods, TILES};
use crate::game::{Game, Screen};
use crate::ui_theme::{Face, Ui};
use horizon_client::hud::HudBatch;
use horizon_client::oteria_art;
use std::cell::RefCell;
use std::f32::consts::TAU;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// How long the menu takes to come in and to go, and the shock wave to cross the screen (s).
const OPEN_S: f32 = 0.22;
const CLOSE_S: f32 = 0.14;
const SHOCK_S: f32 = 0.9;
/// The way between the buttons and the mods page, either direction (s).
const PAGE_S: f32 = 0.2;
/// Boost's glow breathes, and its ripple leaves it, once a period (s).
const PULSE_S: f32 = 1.8;
/// How fast the hover light follows the pointer (per second, exponential).
const FOLLOW: f32 = 16.0;
/// Boost's violet, and the white of the glass, as `0xRRGGBB`.
const VIOLET: u32 = 0x9B5C_FF;
const WHITE: u32 = 0xFF_FFFF;
/// The buttons, in dp: the square ones either side, Mods between them.
const SQUARE: f32 = 46.0;
const MODS_W: f32 = 190.0;
const GAP: f32 = 12.0;
const RADIUS: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Button {
    Boost,
    Mods,
    Cosmetics,
}
const BUTTONS: [Button; 3] = [Button::Boost, Button::Mods, Button::Cosmetics];

struct Menu {
    opened: Instant,
    closing: Option<Instant>,
    /// The last frame, for the easings' time step.
    last: Instant,
    /// Each button's hover light, 0 to 1, eased towards the pointer.
    hover: [f32; 3],
    /// When each button was last pressed: it gives, then springs back.
    pressed: [Option<Instant>; 3],
    /// The shock wave's start and centre (GUI pixels).
    shock: Option<(Instant, [f32; 2])>,
    /// Where each button was drawn (GUI pixels), for clicks.
    rects: [[f32; 4]; 3],
    /// The mods page, from when it was asked for; `back` when it is fading back to the buttons.
    mods: Option<Mods>,
    mods_at: Option<Instant>,
    back: Option<Instant>,
}

impl Menu {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            opened: now,
            closing: None,
            last: now,
            hover: [0.0; 3],
            pressed: [None; 3],
            shock: None,
            rects: [[0.0; 4]; 3],
            mods: None,
            mods_at: None,
            back: None,
        }
    }
}

thread_local! {
    /// The open menu: the render thread's, as the screen is.
    static MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

/// `t` from 0 to 1, slowing to a stop.
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}
/// `t` from 0 to 1 with a small overshoot at the end: the menu's pop.
fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let c = 1.6;
    1.0 + (c + 1.0) * t.powi(3) + c * t.powi(2)
}
/// `rgb` (`0xRRGGBB`) at `alpha` (0 to 1), as the batch takes colours: `0xAABBGGRR`, red in
/// the low byte (`hud_layout::abgr`).
pub(in crate::game) fn argb(rgb: u32, alpha: f32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    let (r, g, b) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
    a << 24 | b << 16 | g << 8 | r
}
/// `[x, y, w, h]` grown by `by` on every side.
fn grown([x, y, w, h]: [f32; 4], by: f32) -> [f32; 4] {
    [x - by, y - by, w + 2.0 * by, h + 2.0 * by]
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

impl Game<'_> {
    /// Opens the menu from play: the pointer is freed, the game goes on.
    pub(in crate::game) fn open_oteria_menu(&mut self) {
        MENU.with(|m| *m.borrow_mut() = Some(Menu::new()));
        self.screen = Screen::OteriaMenu;
        self.keys.clear();
        self.left = false;
        self.right = false;
        self.mining = None;
        self.capture(false);
    }

    /// Starts the menu's way out; it is gone, and the pointer back to play, once it has faded.
    pub(in crate::game) fn close_oteria_menu(&mut self) {
        MENU.with(|m| {
            if let Some(menu) = m.borrow_mut().as_mut()
                && menu.closing.is_none()
            {
                menu.closing = Some(Instant::now());
            }
        });
    }

    /// The menu gone at once (its fade done, or another screen opening from it).
    fn drop_oteria_menu(&mut self) {
        MENU.with(|m| *m.borrow_mut() = None);
        if self.screen == Screen::OteriaMenu {
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }

    /// Escape: out of the search, then back to the buttons, then the menu closes.
    pub(in crate::game) fn oteria_menu_back(&mut self) {
        let closes = MENU.with(|m| {
            let mut m = m.borrow_mut();
            let Some(menu) = m.as_mut() else {
                return true;
            };
            match menu.mods.as_mut() {
                Some(mods) if menu.back.is_none() => {
                    if mods.typing {
                        mods.typing = false;
                    } else {
                        menu.back = Some(Instant::now());
                    }
                    false
                }
                _ => true,
            }
        });
        if closes {
            self.close_oteria_menu();
        }
    }

    /// A window event while the menu is open: a click, the wheel over the mods, or typing in
    /// their search. True when taken.
    pub(in crate::game) fn oteria_menu_event(&mut self, event: &WindowEvent) -> bool {
        let cursor = self.cursor;
        let mut taken = matches!(event, WindowEvent::MouseInput { .. });
        let action = MENU.with(|m| {
            let mut m = m.borrow_mut();
            let menu = m.as_mut()?;
            if menu.closing.is_some() {
                return None;
            }
            let on_mods = menu.back.is_none() && menu.mods.is_some();
            match event {
                WindowEvent::MouseWheel { delta, .. } if on_mods => {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    };
                    menu.mods.as_mut()?.wheel(lines);
                    taken = true;
                    None
                }
                WindowEvent::KeyboardInput { event: key, .. } if on_mods => {
                    let mods = menu.mods.as_mut()?;
                    let code = match key.physical_key {
                        PhysicalKey::Code(code) => Some(code),
                        _ => None,
                    };
                    // Right Shift still closes the menu; the rest goes to the search.
                    if !mods.typing || code == Some(KeyCode::ShiftRight) {
                        return None;
                    }
                    taken = true;
                    if key.state == ElementState::Pressed {
                        if code == Some(KeyCode::Escape) || code == Some(KeyCode::Enter) {
                            mods.typing = false;
                        } else {
                            mods.type_key(key.text.as_deref(), code == Some(KeyCode::Backspace));
                        }
                    }
                    None
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    if on_mods {
                        return menu.mods.as_mut()?.click(cursor);
                    }
                    let i = menu.rects.iter().position(|r| inside(*r, cursor))?;
                    menu.pressed[i] = Some(Instant::now());
                    match BUTTONS[i] {
                        Button::Boost => {
                            let [x, y, w, h] = menu.rects[i];
                            menu.shock = Some((Instant::now(), [x + w / 2.0, y + h / 2.0]));
                        }
                        Button::Mods => {
                            menu.mods.get_or_insert_with(Mods::default);
                            menu.mods_at = Some(Instant::now());
                            menu.back = None;
                        }
                        Button::Cosmetics => {}
                    }
                    None
                }
                _ => None,
            }
        });
        let Some(action) = action else {
            return taken;
        };
        // Another screen from the menu: the menu goes, and the pointer stays free on the way
        // (dropping the menu would grab it back for play).
        let leave = || MENU.with(|m| *m.borrow_mut() = None);
        match action {
            Action::Toggle(i) => {
                let tile = &TILES[i];
                let on = !(tile.get)(&self.settings.extras);
                (tile.set)(&mut self.settings.extras, on);
                self.settings.save(&Self::options_path());
                self.apply_settings();
            }
            Action::Options(i) => {
                leave();
                self.open_options_at(false, TILES[i].page);
                self.capture(false);
            }
            Action::Settings => {
                leave();
                self.open_options_at(false, horizon_client::options_ui::Page::Horizon);
                self.capture(false);
            }
            Action::EditHud => {
                leave();
                self.open_hud_editor(false);
            }
            Action::Close => self.close_oteria_menu(),
        }
        true
    }

    pub(in crate::game) fn draw_oteria_menu(&mut self, batch: &mut HudBatch) {
        horizon_client::widgets::begin_screen(|| {
            horizon_client::widgets::ScreenInfo::new("oteria_menu", "Oteria")
        });
        let now = Instant::now();
        // The fade out ends the menu.
        let gone = MENU.with(|m| match m.borrow().as_ref() {
            None => true,
            Some(menu) => menu
                .closing
                .is_some_and(|c| c.elapsed().as_secs_f32() >= CLOSE_S),
        });
        if gone {
            self.drop_oteria_menu();
            return;
        }
        let scale = batch.scale;
        let cursor = self.cursor;
        let atlas = &self.gui.atlas;
        let extras = &self.settings.extras;
        MENU.with(|m| {
            let mut m = m.borrow_mut();
            let Some(menu) = m.as_mut() else {
                return;
            };
            let dt = now.duration_since(menu.last).as_secs_f32().min(0.1);
            menu.last = now;
            // How far the mods page has come in (0: the buttons, 1: the page).
            let page = match (menu.mods_at, menu.back) {
                (_, Some(back)) => {
                    let s = now.duration_since(back).as_secs_f32() / PAGE_S;
                    if s >= 1.0 {
                        (menu.mods, menu.mods_at, menu.back) = (None, None, None);
                        0.0
                    } else {
                        1.0 - ease_out(s)
                    }
                }
                (Some(at), None) => ease_out(now.duration_since(at).as_secs_f32() / PAGE_S),
                _ => 0.0,
            };
            let t = now.duration_since(menu.opened).as_secs_f32();
            let open = ease_out(t / OPEN_S);
            let close = menu
                .closing
                .map_or(0.0, |c| ease_out(c.elapsed().as_secs_f32() / CLOSE_S));
            // How much of the menu shows (fades together), and its pop.
            let shown = open * (1.0 - close);
            // The buttons fade as the page comes.
            let a = shown * (1.0 - page);
            let pop = 0.94 + 0.06 * ease_out_back(t / OPEN_S) - 0.03 * close;
            let mut ui = Ui::new(atlas, batch, cursor);
            let d = ui.density;
            // dp to GUI pixels, for the batch's own drawing and the clicks.
            let gui = |v: f32| v * d / scale;
            let (w, h) = (ui.w, ui.h);
            let cx = w / 2.0;

            // A light veil over the blurred world.
            ui.rounded(0.0, 0.0, w, h, 0.0, argb(0x000000, (0.32 + 0.16 * page) * shown));

            // The logo drawn one texel to a pixel (sharp), floating a little; the name under it.
            let logo = 160.0 / d * pop;
            let title_size = 24.0 * pop;
            // Mods sits in the middle of the window, where the freed pointer appears: open, and the
            // pointer is already on it. The logo and the name go above it.
            let by = (h - SQUARE) / 2.0 + (1.0 - open) * 10.0;
            let top = (by - 22.0 - title_size * 1.45 - 8.0 - logo).max(8.0);
            // A soft light behind the logo that breathes: its opacity, which changes smoothly.
            let glow_breath = 0.5 + 0.5 * (t * TAU / 3.2).sin();
            let halo = logo * 0.62;
            for (k, spread) in [(0.0, 1.0), (1.0, 1.35), (2.0, 1.75)] {
                let size = halo * spread;
                let alpha = (0.07 - 0.02 * k) * (0.6 + 0.4 * glow_breath) * a;
                ui.rounded(cx - size / 2.0, top + logo / 2.0 - size / 2.0, size, size, size / 2.0, argb(0x8FD8FF, alpha));
            }
            if let Some(r) = ui.atlas.sprites.get(oteria_art::LOGO).copied() {
                let (lx, ly) = (cx - logo / 2.0, top);
                ui.batch.rect_uv(gui(lx), gui(ly), gui(logo), gui(logo), r, argb(WHITE, a));
            }
            let ty = top + logo + 8.0;
            let (bold, light) = ("OTERIA", " CLIENT");
            let bw = ui.measure(Face::SemiBold, title_size, bold);
            let lw = ui.measure(Face::Regular, title_size, light);
            let tx = cx - (bw + lw) / 2.0;
            ui.text(Face::SemiBold, title_size, tx, ty, argb(WHITE, a), bold);
            ui.text(Face::Regular, title_size, tx + bw, ty, argb(WHITE, 0.82 * a), light);

            // The buttons' places, in dp.
            let row = SQUARE * 2.0 + MODS_W + GAP * 2.0;
            let x0 = cx - row / 2.0;
            let places = [
                [x0, by, SQUARE, SQUARE],
                [x0 + SQUARE + GAP, by, MODS_W, SQUARE],
                [x0 + SQUARE + GAP + MODS_W + GAP, by, SQUARE, SQUARE],
            ];
            let pointer = ui.cursor;
            let follow = 1.0 - (-dt * FOLLOW).exp();
            for (i, place) in places.iter().enumerate() {
                let over = menu.closing.is_none() && page == 0.0 && inside(*place, pointer);
                menu.hover[i] += ((if over { 1.0 } else { 0.0 }) - menu.hover[i]) * follow;
                menu.rects[i] = place.map(gui);
                let name = match BUTTONS[i] {
                    Button::Boost => "Boost",
                    Button::Mods => "Mods",
                    Button::Cosmetics => "Cosmetics",
                };
                horizon_client::widgets::record(name, menu.rects[i], true);
                horizon_client::widgets::widget_cursor(over, true);
            }

            // Boost's loop under its button: a breathing violet glow, and a ripple leaving it.
            let phase = (t / PULSE_S).fract();
            let breath = 0.5 + 0.5 * (t * TAU / PULSE_S).sin();
            let boost = places[0];
            let glow = grown(boost, 5.0);
            ui.rounded(glow[0], glow[1], glow[2], glow[3], RADIUS + 4.0, argb(VIOLET, (0.12 + 0.22 * breath) * a));
            // The ripple leaves fast enough to move several pixels a frame; then waits.
            let leave = (phase / 0.55).min(1.0);
            if leave < 1.0 {
                let ripple = grown(boost, 2.0 + 34.0 * ease_out(leave));
                let fade = (1.0 - leave).powi(2) * 0.6 * a;
                ui.border(ripple[0], ripple[1], ripple[2], ripple[3], RADIUS + 10.0, argb(VIOLET, fade));
            }

            // The buttons: glass that lights up under the pointer, and gives when pressed.
            for (i, place) in places.iter().enumerate() {
                let hover = menu.hover[i];
                let give = menu.pressed[i].map_or(0.0, |p| {
                    let s = p.elapsed().as_secs_f32();
                    // Down fast, back with a spring.
                    if s < 0.08 { s / 0.08 } else { (1.0 - (s - 0.08) / 0.25).max(0.0) }
                });
                let r = grown(*place, -1.5 * give + 1.0 * hover);
                let fill = argb(WHITE, (0.10 + 0.10 * hover - 0.04 * give) * a);
                ui.rounded(r[0], r[1], r[2], r[3], RADIUS, fill);
                let edge = match BUTTONS[i] {
                    Button::Boost => argb(VIOLET, (0.65 + 0.35 * hover) * a),
                    _ => argb(WHITE, (0.40 + 0.50 * hover) * a),
                };
                ui.border(r[0], r[1], r[2], r[3], RADIUS, edge);
                let (mx, my) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                match BUTTONS[i] {
                    Button::Mods => {
                        let size = 15.0;
                        let label = "MODS";
                        let lw = ui.measure(Face::Medium, size, label);
                        ui.text(Face::Medium, size, mx - lw / 2.0, my - size * 1.45 / 2.0, argb(WHITE, a), label);
                    }
                    button => {
                        let sprite = if button == Button::Boost { oteria_art::ROCKET } else { oteria_art::SHIRT };
                        let icon = 32.0 / d;
                        // Boost's rocket climbs a little under the pointer (quickly, eased).
                        let lift = if button == Button::Boost { 2.0 * hover } else { 0.0 };
                        if button == Button::Boost {
                            // Sparks falling from the rocket's nozzle, in a loop.
                            for k in 0..3 {
                                let p = ((t * 2.4) + k as f32 / 3.0).fract();
                                let sy = my + icon * 0.42 - lift + p * 16.0;
                                let sx = mx + (k as f32 - 1.0) * 2.5;
                                let size = 2.2 * (1.0 - p);
                                ui.rounded(sx - size / 2.0, sy, size, size, size / 2.0, argb(VIOLET, (1.0 - p) * 0.9 * a));
                            }
                        }
                        if let Some(rect) = ui.atlas.sprites.get(sprite).copied() {
                            let (ix, iy) = (mx - icon / 2.0, my - icon / 2.0 - lift);
                            ui.batch.rect_uv(gui(ix), gui(iy), gui(icon), gui(icon), rect, argb(WHITE, a));
                        }
                        if button == Button::Cosmetics {
                            // A small dot: something new to look at, as Lunar marks it.
                            ui.rounded(r[0] + r[2] - 9.0, r[1] + 3.0, 6.0, 6.0, 3.0, argb(0x3FD27A, a));
                        }
                    }
                }
            }

            // The shock wave from Boost: a violet ring out to the screen's corners, and a flash.
            if let Some((start, [sx, sy])) = menu.shock {
                let s = start.elapsed().as_secs_f32() / SHOCK_S;
                if s >= 1.0 {
                    menu.shock = None;
                } else {
                    let flash = (1.0 - s).powi(3) * 0.14;
                    ui.rounded(0.0, 0.0, w, h, 0.0, argb(VIOLET, flash * a));
                    let (sx, sy) = (sx * scale / d, sy * scale / d);
                    let reach = (sx.max(w - sx)).hypot(sy.max(h - sy));
                    let radius = ease_out(s) * reach;
                    let thick = 4.0 + 26.0 * (1.0 - s);
                    let colour = argb(VIOLET, (1.0 - s).powf(1.5) * 0.6 * a);
                    let centre = [gui(sx), gui(sy)];
                    let atlas = ui.atlas;
                    ring(&mut *ui.batch, atlas, centre, gui((radius - thick).max(0.0)), gui(radius), colour);
                    let inner = argb(WHITE, (1.0 - s).powi(2) * 0.35 * a);
                    ring(&mut *ui.batch, atlas, centre, gui((radius - 2.0).max(0.0)), gui(radius), inner);
                }
            }
            // The mods page, rising into place as it comes.
            if page > 0.0
                && let Some(mods) = menu.mods.as_mut()
            {
                mods.draw(&mut ui, extras, shown * page, (1.0 - page) * 18.0, dt, &gui);
            }
            ui.finish();
        });
    }
}

/// An annulus around `centre` from `inner` to `outer` (GUI pixels), as a fan of quads.
fn ring(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    [cx, cy]: [f32; 2],
    inner: f32,
    outer: f32,
    colour: u32,
) {
    if outer <= 0.0 || colour >> 24 == 0 {
        return;
    }
    // Enough sides that the edge stays round however big it gets.
    let sides = ((outer * 0.6) as usize).clamp(48, 160);
    let at = |r: f32, k: usize| {
        let a = k as f32 / sides as f32 * TAU;
        [cx + r * a.cos(), cy + r * a.sin()]
    };
    for k in 0..sides {
        batch.fill_quad(atlas, [at(inner, k), at(outer, k), at(outer, k + 1), at(inner, k + 1)], colour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_easings_start_at_rest_and_settle() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(2.0), 1.0, "past the end it holds");
        assert!(ease_out(0.5) > 0.5, "it slows down towards the end");
        assert!((ease_out_back(0.0)).abs() < 1e-6);
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-6);
        assert!(
            (0..10).any(|i| ease_out_back(0.6 + i as f32 * 0.04) > 1.0),
            "it overshoots a little"
        );
    }

    #[test]
    fn colours_take_their_alpha_on_top_and_red_in_the_low_byte() {
        // The batch reads 0xAABBGGRR: the violet's blue (0xFF) high, its red (0x9B) low.
        assert_eq!(argb(VIOLET, 1.0), 0xFFFF_5C9B);
        assert_eq!(argb(VIOLET, 0.0), 0x00FF_5C9B);
        assert_eq!(argb(WHITE, 2.0), 0xFFFF_FFFF, "clamped");
    }
}
