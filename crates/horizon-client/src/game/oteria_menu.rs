//! Oteria's menu on Right Shift, over the world (as Lunar's): its logo and name, then three
//! buttons — Boost (a rocket), Mods, and Cosmetics (a t-shirt). The game goes on behind it,
//! blurred (`Screen::blurs_background`).
//!
//! Every motion runs on the clock, not on the frame count, so the menu looks the same at 60 or
//! 600 FPS: it eases in and out, the buttons light up and give under the pointer, Boost glows
//! violet in a loop with a rocket that floats, and pressing Boost sends a shock wave across the
//! screen. For now Boost and Cosmetics only animate; Mods opens Oteria's settings.

use crate::game::{Game, Screen};
use crate::ui_theme::{Face, Ui};
use horizon_client::hud::HudBatch;
use horizon_client::oteria_art;
use std::cell::RefCell;
use std::f32::consts::TAU;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, WindowEvent};

/// How long the menu takes to come in and to go, and the shock wave to cross the screen (s).
const OPEN_S: f32 = 0.22;
const CLOSE_S: f32 = 0.14;
const SHOCK_S: f32 = 0.9;
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
fn argb(rgb: u32, alpha: f32) -> u32 {
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

    /// A window event while the menu is open: a left click on a button. True when taken.
    pub(in crate::game) fn oteria_menu_event(&mut self, event: &WindowEvent) -> bool {
        let WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
            ..
        } = event
        else {
            return false;
        };
        let cursor = self.cursor;
        let hit = MENU.with(|m| {
            let mut m = m.borrow_mut();
            let menu = m.as_mut()?;
            if menu.closing.is_some() {
                return None;
            }
            let i = menu.rects.iter().position(|r| inside(*r, cursor))?;
            menu.pressed[i] = Some(Instant::now());
            if BUTTONS[i] == Button::Boost {
                let [x, y, w, h] = menu.rects[i];
                menu.shock = Some((Instant::now(), [x + w / 2.0, y + h / 2.0]));
            }
            Some(BUTTONS[i])
        });
        if hit == Some(Button::Mods) {
            // Oteria's settings: what the menu will hold, for now on the options' own page.
            self.drop_oteria_menu();
            self.open_options_at(false, horizon_client::options_ui::Page::Horizon);
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
        MENU.with(|m| {
            let mut m = m.borrow_mut();
            let Some(menu) = m.as_mut() else {
                return;
            };
            let dt = now.duration_since(menu.last).as_secs_f32().min(0.1);
            menu.last = now;
            let t = now.duration_since(menu.opened).as_secs_f32();
            let open = ease_out(t / OPEN_S);
            let close = menu
                .closing
                .map_or(0.0, |c| ease_out(c.elapsed().as_secs_f32() / CLOSE_S));
            // How much of the menu shows (fades together), and its pop.
            let a = open * (1.0 - close);
            let pop = 0.94 + 0.06 * ease_out_back(t / OPEN_S) - 0.03 * close;
            let mut ui = Ui::new(atlas, batch, cursor);
            let d = ui.density;
            // dp to GUI pixels, for the batch's own drawing and the clicks.
            let gui = |v: f32| v * d / scale;
            let (w, h) = (ui.w, ui.h);
            let cx = w / 2.0;

            // A light veil over the blurred world.
            ui.rounded(0.0, 0.0, w, h, 0.0, argb(0x000000, 0.32 * a));

            // The logo drawn one texel to a pixel (sharp), floating a little; the name under it.
            let logo = 160.0 / d * pop;
            let title_size = 24.0 * pop;
            let block = logo + 8.0 + title_size * 1.45 + 22.0 + SQUARE;
            let top = ((h - block) / 2.0 - 8.0).max(8.0) + (1.0 - open) * 10.0;
            let float = (t * TAU / 3.0).sin() * 2.0;
            if let Some(r) = ui.atlas.sprites.get(oteria_art::LOGO).copied() {
                let (lx, ly) = (cx - logo / 2.0, top + float);
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
            let by = ty + title_size * 1.45 + 22.0;
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
                let over = menu.closing.is_none() && inside(*place, pointer);
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
            let glow = grown(boost, 3.0 + 3.0 * breath);
            ui.rounded(glow[0], glow[1], glow[2], glow[3], RADIUS + 4.0, argb(VIOLET, (0.16 + 0.16 * breath) * a));
            let ripple = grown(boost, 2.0 + 14.0 * ease_out(phase));
            ui.border(ripple[0], ripple[1], ripple[2], ripple[3], RADIUS + 6.0, argb(VIOLET, (1.0 - phase) * 0.55 * a));

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
                        // Boost's rocket floats, and climbs a little under the pointer.
                        let lift = if button == Button::Boost {
                            (t * TAU / 1.2).sin() * 1.2 + 2.0 * hover
                        } else {
                            0.0
                        };
                        if button == Button::Boost {
                            // Sparks falling from the rocket's nozzle, in a loop.
                            for k in 0..3 {
                                let p = ((t * 1.6) + k as f32 / 3.0).fract();
                                let sy = my + icon * 0.42 - lift + p * 9.0;
                                let sx = mx + ((k as f32 * 2.1 + t * 3.0).sin()) * 1.6;
                                let size = 1.6 * (1.0 - p);
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
