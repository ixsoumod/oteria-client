//! Oteria's menu on Right Shift, over the world (as Lunar's): its logo and name, then three
//! buttons — Boost (a rocket), Mods, and Cosmetics (a t-shirt). The game goes on behind it,
//! blurred (`Screen::blurs_background`).
//!
//! Every motion runs on the clock, not on the frame count, so the menu looks the same at 60 or
//! 600 FPS: it eases in and out, the buttons light up and give under the pointer, Boost glows
//! violet in a loop. Boost is the performance mode (`oteria_boost`): pressed, it sends a shock
//! wave across the screen, then smoulders while on (the rocket shakes, embers rise around it);
//! pressed again, it calms down slowly as violet trails from every edge of the screen fly into
//! it. Neither can be pressed again before its animation is over. Cosmetics only animates for
//! now; Mods fades the buttons out for the mods page (`oteria_mods`), and Escape fades back.
//!
//! The GUI is drawn pixel for pixel (crisp text and icons), so a slow drift would move one pixel
//! at a time and stutter: what loops slowly changes its light (a glow's opacity), and only fast
//! motions (the ripple, the sparks, the shock wave) change places.

use super::oteria_boost;
use super::oteria_cosmetics::{CosmeticsAction, CosmeticsMenu, CosmeticsView, SkinTile};
use super::oteria_host::{HostAction, HostMenu, HostView};
use super::oteria_keyboard::{Bind, KeyboardAction, KeyboardPage, KeyboardView};
use super::oteria_mods::{Action, Drawing, Mods, ProfileOp, TILES};
use super::oteria_profiles::Profiles;
use super::oteria_voice_ui::VoiceMenu;
use crate::game::{Game, Screen};
use crate::ui_theme::{Face, Ui};
use horizon_client::hud::HudBatch;
use horizon_client::{lang, oteria_art};
use std::cell::RefCell;
use std::f32::consts::{PI, TAU};
use std::time::{Duration, Instant};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// How long the menu takes to come in and to go, and the shock wave to cross the screen (s).
const OPEN_S: f32 = 0.22;
const CLOSE_S: f32 = 0.14;
const SHOCK_S: f32 = 0.9;
/// Boost turned off: the trails' flight into the rocket (s), and how many fly.
const CALM_S: f32 = 1.3;
const TRAILS: usize = 32;
/// How fast Boost's look comes on, and goes (per second, exponential): it calms slowly.
const SMOULDER_IN: f32 = 3.5;
const SMOULDER_OUT: f32 = 1.4;
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
    /// Under Mods: the HUD editor, straight from the menu.
    EditHud,
    /// Under Edit HUD: the keyboard of key bindings (`oteria_keyboard`).
    Keyboard,
}
const BUTTONS: [Button; 5] = [
    Button::Boost,
    Button::Mods,
    Button::Cosmetics,
    Button::EditHud,
    Button::Keyboard,
];
/// Edit HUD's height, under Mods.
const EDIT_H: f32 = 34.0;

struct Menu {
    opened: Instant,
    closing: Option<Instant>,
    /// The last frame, for the easings' time step.
    last: Instant,
    /// Each button's hover light, 0 to 1, eased towards the pointer.
    hover: [f32; 5],
    /// When each button was last pressed: it gives, then springs back.
    pressed: [Option<Instant>; 5],
    /// The shock wave's start and centre (GUI pixels).
    shock: Option<(Instant, [f32; 2])>,
    /// Boost turned off: the trails' start and where they fly to (GUI pixels).
    calm: Option<(Instant, [f32; 2])>,
    /// Boost is on, and its look (0 to 1, eased: it smoulders once the wave has gone).
    boost: bool,
    smoulder: f32,
    /// Boost can't be pressed again before this (its animation plays out).
    busy: Option<Instant>,
    /// Boost's pulse and its nozzle's sparks, run on as phases (they quicken while it's on).
    pulse: f32,
    sparks: f32,
    /// The player's profiles, for the mods page's PROFILES tab.
    profiles: Option<Profiles>,
    /// Where each button was drawn (GUI pixels), for clicks.
    rects: [[f32; 4]; 5],
    /// The mods page, from when it was asked for; `back` when it is fading back to the buttons.
    mods: Option<Mods>,
    /// The voice chat's menu (V), shown as the mods page is.
    voice: Option<VoiceMenu>,
    /// The Host page, from the button at the bottom right.
    host: Option<HostMenu>,
    /// The Cosmetics page, from the t-shirt.
    cosmetics: Option<CosmeticsMenu>,
    /// The Keyboard page, from its button or Controls'.
    keyboard: Option<KeyboardPage>,
    /// Where the Host button was drawn (GUI pixels), and its hover light.
    host_rect: [f32; 4],
    host_hover: f32,
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
            hover: [0.0; 5],
            pressed: [None; 5],
            shock: None,
            calm: None,
            boost: false,
            smoulder: 0.0,
            busy: None,
            pulse: 0.0,
            sparks: 0.0,
            profiles: None,
            rects: [[0.0; 4]; 5],
            mods: None,
            voice: None,
            host: None,
            cosmetics: None,
            keyboard: None,
            host_rect: [0.0; 4],
            host_hover: 0.0,
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
/// `t` from 0 to 1, slow at both ends.
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// A number from 0 to 1 that looks random, the same for the same `seed`.
fn hash(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43_758.547).fract().abs()
}
/// The point `along` (0 to 1) the way round a `w` x `h` screen's edge, from its top left,
/// clockwise, `out` dp past it.
fn edge_point(along: f32, w: f32, h: f32, out: f32) -> [f32; 2] {
    let d = along.rem_euclid(1.0) * 2.0 * (w + h);
    if d < w {
        [d, -out]
    } else if d < w + h {
        [w + out, d - w]
    } else if d < 2.0 * w + h {
        [w - (d - w - h), h + out]
    } else {
        [-out, h - (d - 2.0 * w - h)]
    }
}

impl Game<'_> {
    /// Opens the menu from play: the pointer is freed, the game goes on.
    pub(in crate::game) fn open_oteria_menu(&mut self) {
        let dir = Self::options_dir();
        let first = lang::get_or("oteria.profiles.default", "Default");
        let profiles = Profiles::load(&dir, &first, &self.extras_text());
        let boost = oteria_boost::is_on(&dir);
        // The Sky mode's packs, as the resourcepacks folder has them now.
        self.sky_packs_refresh();
        MENU.with(|m| {
            *m.borrow_mut() = Some(Menu {
                profiles: Some(profiles),
                boost,
                // Already on: it smoulders from the start.
                smoulder: if boost { 1.0 } else { 0.0 },
                ..Menu::new()
            })
        });
        self.screen = Screen::OteriaMenu;
        self.keys.clear();
        self.left = false;
        self.right = false;
        self.mining = None;
        self.capture(false);
    }

    /// Opens the voice chat's menu over the game (V; G: straight on the group).
    pub(in crate::game) fn open_voice_menu(&mut self, group: bool) {
        self.open_oteria_menu();
        MENU.with(|m| {
            if let Some(menu) = m.borrow_mut().as_mut() {
                menu.voice = Some(VoiceMenu::new(group));
                menu.mods_at = Some(Instant::now());
            }
        });
    }

    /// What the Host page shows: the world's options (`with_options`: the first frame only, the
    /// server thread asked once) and sharing's state.
    fn host_view(&mut self, with_options: bool) -> HostView {
        self.tick_friends();
        let options = if with_options {
            self.host_options()
        } else {
            None
        };
        let friends = self.friends_view();
        let short = self.friends_short();
        HostView {
            options,
            sharing: friends.sharing.then(|| super::oteria_host::Sharing {
                code: friends.code.clone(),
                short,
                status: friends.status.clone(),
                reachable: friends.reachable,
                guests: friends.guests.clone(),
                copied: friends.copied,
            }),
        }
    }

    /// What the Host page asked for.
    fn host_action(&mut self, action: HostAction) {
        self.audio.play_button_click();
        match action {
            HostAction::Host(options) => {
                if let Err(e) = self.host_world(options) {
                    self.system_message(horizon_client::text::Text::literal(
                        horizon_client::lang::get_or(
                            "oteria.host.failed",
                            "Couldn't host the world: %s",
                        )
                        .replacen("%s", &e.to_string(), 1),
                    ));
                }
            }
            HostAction::Copy => self.friends_copy(),
            HostAction::CopyShort => self.friends_copy_short(),
            HostAction::Stop => self.friends_stop(),
        }
    }

    /// The folder of `options.txt`, where the profiles and Boost keep their files.
    fn options_dir() -> std::path::PathBuf {
        Self::options_path()
            .parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_default()
    }

    /// The mods' settings on now, as `oteria-extras.txt` has them: what a profile keeps.
    fn extras_text(&self) -> String {
        let mut extras = self.settings.extras.clone();
        extras.emote_key = self.settings.keys.get("key.emote").to_owned();
        extras.pvp.skins.inspect_key = self.settings.keys.get("key.oteria.inspect").to_owned();
        extras.pvp.zoom.key = self.settings.keys.get("key.oteria.zoom").to_owned();
        extras.pvp.music.play_key = self.settings.keys.get("key.oteria.music.play").to_owned();
        extras.pvp.music.next_key = self.settings.keys.get("key.oteria.music.next").to_owned();
        extras.pvp.music.previous_key = self
            .settings
            .keys
            .get("key.oteria.music.previous")
            .to_owned();
        extras.serialize()
    }

    /// Settings changed from the menu: saved and put on, as the options screens do.
    fn settings_changed(&mut self) {
        let _ = self.handle_option_events(vec![horizon_client::options_ui::Event::Changed]);
    }

    /// The profiles tab's asks: each saves what is on into the profile in use first.
    fn profile_op(&mut self, op: ProfileOp) {
        let dir = Self::options_dir();
        let current = self.extras_text();
        let first = lang::get_or("oteria.profiles.default", "Default");
        let base = lang::get_or("oteria.profiles.new", "Profile");
        let put_on = MENU.with(|m| {
            let mut m = m.borrow_mut();
            let menu = m.as_mut()?;
            let profiles = menu
                .profiles
                .get_or_insert_with(|| Profiles::load(&dir, &first, &current));
            match op {
                ProfileOp::Create => {
                    let i = profiles.create(&base, &current);
                    // Its name is typed straight away (or it keeps the one given).
                    if let Some(mods) = menu.mods.as_mut() {
                        mods.renaming = Some((i, String::new()));
                    }
                    None
                }
                ProfileOp::Use(i) => profiles.switch(i, &current),
                ProfileOp::Rename(i, name) => {
                    profiles.rename(i, &name);
                    None
                }
                ProfileOp::Duplicate(i) => {
                    profiles.duplicate(i, &current);
                    None
                }
                ProfileOp::Delete(i) => profiles.delete(i).ok().flatten(),
            }
        });
        if let Some(text) = put_on {
            self.put_on_extras(&text);
        }
    }

    /// A profile's settings on. What belongs to the account or the machine, not to a profile
    /// (cosmetics owned, the AI agent, splitscreen), stays as it is.
    fn put_on_extras(&mut self, text: &str) {
        let old = &self.settings.extras;
        let mut extras = horizon_client::extras::Extras::parse(text);
        extras.cosmetics_owner = old.cosmetics_owner;
        extras.no_ai = old.no_ai;
        extras.agent_control = old.agent_control;
        extras.repro_keys = old.repro_keys;
        extras.splitscreen = old.splitscreen;
        extras.splitscreen_two = old.splitscreen_two;
        extras.splitscreen_three = old.splitscreen_three;
        extras.splitscreen_names = old.splitscreen_names;
        self.settings.extras = extras;
        self.settings_changed();
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
        // The Keyboard page opened from Controls, waiting for nothing: back to Controls.
        let to_controls = MENU.with(|m| {
            m.borrow()
                .as_ref()
                .and_then(|menu| menu.keyboard.as_ref())
                .is_some_and(|k| k.from_options && !k.wants_keys())
        });
        if to_controls {
            self.keyboard_back_to_controls();
            return;
        }
        let mut voice_action = None;
        let closes = MENU.with(|m| {
            let mut m = m.borrow_mut();
            let Some(menu) = m.as_mut() else {
                return true;
            };
            // The Keyboard page: out of what it waits for, else closed (to Controls when it came
            // from there).
            if let Some(page) = menu.keyboard.as_mut() {
                if page.back() {
                    return false;
                }
                if page.from_options {
                    return true;
                }
                if menu.back.is_none() {
                    menu.back = Some(Instant::now());
                }
                return false;
            }
            // The Cosmetics page's preview: closed first.
            if let Some(page) = menu.cosmetics.as_mut()
                && page.close_popup()
            {
                return false;
            }
            // The Host and Cosmetics pages: back to the buttons.
            if menu.host.is_some() || menu.cosmetics.is_some() {
                if menu.back.is_none() {
                    menu.back = Some(Instant::now());
                }
                return false;
            }
            // The voice menu: a page back, then closed (it was opened from the game).
            if let Some(voice) = menu.voice.as_mut() {
                if voice.at_main() {
                    return true;
                }
                voice_action = voice.back();
                return false;
            }
            match menu.mods.as_mut() {
                Some(mods) if menu.back.is_none() => {
                    if !mods.back() {
                        menu.back = Some(Instant::now());
                    }
                    false
                }
                _ => true,
            }
        });
        if let Some(a) = voice_action {
            self.voice_action(a);
        }
        if closes {
            // The microphone test ends with the menu.
            self.voice_action(super::oteria_voice_ui::VAction::MicTest(false));
            self.close_oteria_menu();
        }
    }

    /// A window event while the menu is open: a click, the wheel over the mods, or typing in
    /// their search. True when taken.
    pub(in crate::game) fn oteria_menu_event(&mut self, event: &WindowEvent) -> bool {
        let cursor = self.cursor;
        // The music player's buttons, on the HUD beside the menu's own (none of its pages open).
        let bare = MENU.with(|m| {
            m.borrow().as_ref().is_some_and(|menu| {
                menu.closing.is_none()
                    && menu.mods.is_none()
                    && menu.cosmetics.is_none()
                    && menu.keyboard.is_none()
                    && menu.host.is_none()
                    && menu.voice.is_none()
            })
        });
        if bare
            && matches!(
                event,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                }
            )
            && self.music_click(cursor)
        {
            return true;
        }
        let c = &self.settings.extras.pvp.crosshair;
        let drawn = [
            c.pixels,
            c.indicator_pixels,
            {
                let e = &self.settings.extras.pvp.environment;
                e.particle_pixels().unwrap_or(e.particles_pixels)
            },
        ];
        let mut taken = matches!(event, WindowEvent::MouseInput { .. });
        let action = MENU.with(|m| {
            let mut m = m.borrow_mut();
            let menu = m.as_mut()?;
            if menu.closing.is_some() {
                return None;
            }
            let on_mods = menu.back.is_none() && menu.mods.is_some();
            // The Keyboard page takes the clicks, the wheel, and the keys it waits for.
            if menu.back.is_none()
                && let Some(page) = menu.keyboard.as_mut()
            {
                return match event {
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button,
                        ..
                    } => page
                        .press(&horizon_client::keybinds::mouse_name(*button), cursor)
                        .map(Action::Keyboard),
                    WindowEvent::MouseWheel { delta, .. } => {
                        let lines = match delta {
                            MouseScrollDelta::LineDelta(_, y) => *y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                        };
                        page.wheel(lines);
                        taken = true;
                        None
                    }
                    WindowEvent::KeyboardInput { event: key, .. } if page.wants_keys() => {
                        let code = match key.physical_key {
                            PhysicalKey::Code(code) => Some(code),
                            _ => None,
                        };
                        taken = true;
                        if key.state != ElementState::Pressed {
                            return None;
                        }
                        if code == Some(KeyCode::Escape) {
                            page.back();
                            return None;
                        }
                        let name = code.and_then(horizon_client::keybinds::key_name);
                        page.key(
                            name,
                            key.text.as_deref(),
                            code == Some(KeyCode::Backspace),
                            matches!(code, Some(KeyCode::Enter | KeyCode::NumpadEnter)),
                        )
                        .map(Action::Keyboard)
                    }
                    _ => None,
                };
            }
            // The Cosmetics page takes the clicks while it shows.
            if menu.back.is_none()
                && let Some(page) = menu.cosmetics.as_mut()
            {
                return match event {
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => page.click(cursor).map(Action::Cosmetics),
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Left,
                        ..
                    } => {
                        page.release();
                        None
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        page.wheel(match delta {
                            MouseScrollDelta::LineDelta(_, y) => *y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                        });
                        taken = true;
                        None
                    }
                    // A PNG dropped on the window: a skin to import.
                    WindowEvent::DroppedFile(path) => {
                        Some(Action::Cosmetics(CosmeticsAction::SkinFile(path.clone())))
                    }
                    // The name field has the keyboard (Right Shift still closes the menu).
                    WindowEvent::KeyboardInput { event: key, .. } if page.wants_keys() => {
                        let code = match key.physical_key {
                            PhysicalKey::Code(code) => Some(code),
                            _ => None,
                        };
                        if code == Some(KeyCode::ShiftRight) {
                            return None;
                        }
                        taken = true;
                        if key.state != ElementState::Pressed {
                            return None;
                        }
                        page.type_key(
                            key.text.as_deref(),
                            code == Some(KeyCode::Backspace),
                            matches!(code, Some(KeyCode::Enter | KeyCode::NumpadEnter)),
                            code == Some(KeyCode::Escape),
                        )
                        .map(Action::Cosmetics)
                    }
                    _ => None,
                };
            }
            // The Host page takes the clicks while it shows.
            if menu.back.is_none()
                && let Some(host) = menu.host.as_mut()
            {
                return match event {
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => host.click(cursor).map(Action::Host),
                    _ => None,
                };
            }
            // The voice menu takes the events while it shows.
            if let Some(voice) = menu.voice.as_mut() {
                return match event {
                    WindowEvent::MouseWheel { delta, .. } => {
                        let lines = match delta {
                            MouseScrollDelta::LineDelta(_, y) => *y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                        };
                        voice.wheel(lines);
                        taken = true;
                        None
                    }
                    WindowEvent::KeyboardInput { event: key, .. } if voice.wants_keys() => {
                        let code = match key.physical_key {
                            PhysicalKey::Code(code) => Some(code),
                            _ => None,
                        };
                        if code == Some(KeyCode::ShiftRight) {
                            return None;
                        }
                        taken = true;
                        if key.state != ElementState::Pressed {
                            return None;
                        }
                        if code == Some(KeyCode::Escape) {
                            return voice.back().map(Action::Voice);
                        }
                        let enter = matches!(code, Some(KeyCode::Enter | KeyCode::NumpadEnter));
                        voice
                            .type_key(key.text.as_deref(), code == Some(KeyCode::Backspace), enter)
                            .map(Action::Voice)
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    } => voice.click(cursor).map(Action::Voice),
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        button: MouseButton::Left,
                        ..
                    } => {
                        voice.release();
                        None
                    }
                    _ => None,
                };
            }
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
                    // Right Shift still closes the menu; the rest goes to the search, or to the
                    // profile being renamed.
                    if !mods.wants_keys() || code == Some(KeyCode::ShiftRight) {
                        return None;
                    }
                    taken = true;
                    if key.state == ElementState::Pressed {
                        match code {
                            Some(KeyCode::Escape) => {
                                mods.back();
                            }
                            Some(KeyCode::Enter | KeyCode::NumpadEnter) => {
                                if mods.renaming.is_some() {
                                    return mods.commit_name();
                                }
                                // A typed number or colour is put on.
                                return mods.enter();
                            }
                            _ => {
                                mods.type_key(
                                    key.text.as_deref(),
                                    code == Some(KeyCode::Backspace),
                                );
                            }
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
                        return menu.mods.as_mut()?.click(cursor, drawn);
                    }
                    // The Host button, at the bottom right.
                    if inside(menu.host_rect, cursor) {
                        menu.host = Some(HostMenu::new());
                        menu.mods_at = Some(Instant::now());
                        menu.back = None;
                        return None;
                    }
                    let i = menu.rects.iter().position(|r| inside(*r, cursor))?;
                    let now = Instant::now();
                    menu.pressed[i] = Some(now);
                    match BUTTONS[i] {
                        Button::Boost => {
                            // Not again before the wave or the trails are over.
                            if menu.busy.is_some_and(|b| now < b) {
                                return None;
                            }
                            let [x, y, w, h] = menu.rects[i];
                            let centre = [x + w / 2.0, y + h / 2.0];
                            menu.boost = !menu.boost;
                            let lasts = if menu.boost {
                                menu.shock = Some((now, centre));
                                SHOCK_S
                            } else {
                                menu.calm = Some((now, centre));
                                CALM_S
                            };
                            menu.busy = Some(now + Duration::from_secs_f32(lasts));
                            return Some(Action::Boost(menu.boost));
                        }
                        Button::Mods => {
                            menu.mods.get_or_insert_with(Mods::default);
                            menu.mods_at = Some(Instant::now());
                            menu.back = None;
                        }
                        Button::EditHud => return Some(Action::EditHud),
                        Button::Keyboard => {
                            menu.keyboard = Some(KeyboardPage::new(false));
                            menu.mods_at = Some(Instant::now());
                            menu.back = None;
                        }
                        Button::Cosmetics => {
                            menu.cosmetics = Some(CosmeticsMenu::default());
                            menu.mods_at = Some(Instant::now());
                            menu.back = None;
                        }
                    }
                    None
                }
                // The end of a stroke on the crosshair's grid.
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => menu.mods.as_mut()?.release(cursor),
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
            Action::Cycle(item, back) => {
                item.cycle(&mut self.settings.extras, back);
                self.settings.save(&Self::options_path());
                self.apply_settings();
            }
            Action::Preset(k) => {
                let (_, _, preset) = horizon_client::pvp::crosshair_presets()[k];
                let c = &mut self.settings.extras.pvp.crosshair;
                // The look of the preset; the player's colours and drawing stay.
                *c = horizon_client::pvp::Crosshair {
                    colour: c.colour,
                    hover: c.hover,
                    pixels: c.pixels,
                    indicator: c.indicator,
                    indicator_pixels: c.indicator_pixels,
                    ..preset
                };
                self.settings.save(&Self::options_path());
            }
            Action::Pixels(Drawing::Crosshair, pixels) => {
                let c = &mut self.settings.extras.pvp.crosshair;
                c.pixels = pixels;
                c.shape = horizon_client::pvp::Shape::Drawn;
                c.custom = true;
                self.settings.save(&Self::options_path());
            }
            // The player indicator: drawing it turns it on.
            Action::Pixels(Drawing::Indicator, pixels) => {
                let c = &mut self.settings.extras.pvp.crosshair;
                c.indicator_pixels = pixels;
                c.indicator = true;
                self.settings.save(&Self::options_path());
            }
            // The particles: drawing them makes them the drawing.
            Action::Pixels(Drawing::Particle, pixels) => {
                let e = &mut self.settings.extras.pvp.environment;
                e.particles_pixels = pixels;
                e.particles_shape = horizon_client::pvp::ParticleShape::Drawn;
                e.particles = true;
                self.settings.save(&Self::options_path());
            }
            Action::ParticleShape(shape) => {
                let e = &mut self.settings.extras.pvp.environment;
                e.particles_shape = shape;
                e.particles = true;
                self.settings.save(&Self::options_path());
            }
            Action::Profile(op) => self.profile_op(op),
            Action::Boost(on) => {
                let dir = Self::options_dir();
                if on {
                    if !oteria_boost::turn_on(&mut self.settings, &dir) {
                        // Not saved, so not on: the rocket says so.
                        MENU.with(|m| {
                            if let Some(menu) = m.borrow_mut().as_mut() {
                                menu.boost = false;
                            }
                        });
                    }
                } else {
                    oteria_boost::turn_off(&mut self.settings, &dir);
                }
                self.settings_changed();
            }
            Action::EditHud => {
                leave();
                self.open_hud_editor(false);
            }
            Action::Voice(a) => self.voice_action(a),
            Action::Host(a) => self.host_action(a),
            Action::Cosmetics(CosmeticsAction::ToggleKnife(knife)) => {
                self.audio.play_button_click();
                let skins = &mut self.settings.extras.pvp.skins;
                // The knife on comes off; another one takes its place.
                skins.knife = !(skins.knife && skins.knife_skin == knife);
                skins.knife_skin = knife.to_owned();
                self.settings.save(&Self::options_path());
            }
            Action::Cosmetics(CosmeticsAction::ToggleWear(kind, id)) => {
                use super::oteria_cosmetics::WearKind;
                self.audio.play_button_click();
                let skins = &mut self.settings.extras.pvp.skins;
                let worn = match kind {
                    WearKind::Cape => &mut skins.cape,
                    WearKind::Hat => &mut skins.hat,
                    WearKind::Pet => &mut skins.pet,
                    WearKind::Aura => &mut skins.aura,
                };
                // The one worn comes off; another one takes its place.
                *worn = if worn == id {
                    String::new()
                } else {
                    id.to_owned()
                };
                self.settings.save(&Self::options_path());
            }
            Action::SetColour(..) | Action::SetNumber(..) | Action::SetSlider(..) => {
                self.mods_value(action)
            }
            Action::Keyboard(a) => self.keyboard_action(a),
            Action::Cosmetics(CosmeticsAction::SkinName(name)) => self.skin_from_name(&name),
            Action::Cosmetics(CosmeticsAction::SkinWear(id)) => {
                self.audio.play_button_click();
                self.skin_wear(&id);
            }
            Action::Cosmetics(CosmeticsAction::SkinReset) => {
                self.audio.play_button_click();
                self.skin_wear("");
            }
            Action::Cosmetics(CosmeticsAction::SkinImport) => self.skin_open_import(),
            Action::Cosmetics(CosmeticsAction::SkinFile(path)) => self.skin_import(&path),
            Action::Cosmetics(CosmeticsAction::SkinUpload) => self.skin_upload(),
            Action::Cosmetics(CosmeticsAction::Close) | Action::Close => self.close_oteria_menu(),
        }
        true
    }

    /// The Keyboard page opened over the game (`from_options`: from Controls, which it goes
    /// back to when closed).
    pub(in crate::game) fn open_oteria_keyboard(&mut self, from_options: bool) {
        self.open_oteria_menu();
        MENU.with(|m| {
            if let Some(menu) = m.borrow_mut().as_mut() {
                menu.keyboard = Some(KeyboardPage::new(from_options));
                // Already there: no way in to watch.
                menu.mods_at = Instant::now().checked_sub(std::time::Duration::from_secs(1));
            }
        });
    }

    /// What the Keyboard page asked: a layout, or a binding changed (saved at once).
    fn keyboard_action(&mut self, action: KeyboardAction) {
        let keys = &mut self.settings.keys;
        match action {
            KeyboardAction::Layout(size) => {
                self.audio.play_button_click();
                self.settings.extras.pvp.keyboard_layout = size;
                self.settings.save(&Self::options_path());
                return;
            }
            KeyboardAction::Assign(name, key) => keys.set(name, &key),
            KeyboardAction::Unbind(name) => keys.set(name, "key.keyboard.unknown"),
            KeyboardAction::Reset(name) => {
                keys.set(name, horizon_client::keybinds::KeyBinds::default_of(name))
            }
            KeyboardAction::Close => {
                let from_options = MENU.with(|m| {
                    m.borrow()
                        .as_ref()
                        .and_then(|menu| menu.keyboard.as_ref())
                        .is_some_and(|k| k.from_options)
                });
                if from_options {
                    self.keyboard_back_to_controls();
                } else {
                    MENU.with(|m| {
                        if let Some(menu) = m.borrow_mut().as_mut() {
                            menu.back.get_or_insert_with(Instant::now);
                        }
                    });
                }
                return;
            }
        }
        self.audio.play_button_click();
        self.settings_changed();
    }

    /// Closed from Controls: the menu goes, Controls comes back.
    pub(in crate::game) fn keyboard_back_to_controls(&mut self) {
        MENU.with(|m| *m.borrow_mut() = None);
        self.open_options_at(false, horizon_client::options_ui::Page::Controls);
    }

    /// The Keyboard page's view: every binding, and the layout.
    fn keyboard_view(&self) -> KeyboardView {
        KeyboardView {
            binds: horizon_client::keybinds::all_mappings()
                .map(|&(action, _, category)| Bind {
                    action,
                    key: self.settings.keys.get(action).to_owned(),
                    category,
                })
                .collect(),
            layout: self.settings.extras.pvp.keyboard_layout,
        }
    }

    /// A colour or a number set on a tile's settings. A colour dragged on the picker is put on
    /// each frame and saved once let go.
    fn mods_value(&mut self, action: Action) {
        let pvp = &mut self.settings.extras.pvp;
        let save = match action {
            Action::SetColour(opt, colour, save) => {
                opt.set_colour(pvp, colour);
                save
            }
            Action::SetNumber(opt, v) => {
                opt.set_number(pvp, v);
                true
            }
            Action::SetSlider(opt, v, save) => {
                opt.set_slider(pvp, v);
                save
            }
            _ => return,
        };
        if save {
            self.settings.save(&Self::options_path());
            self.apply_settings();
        }
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
        // The Host page's view: the world's options the first time, sharing's state each frame.
        let host_view = MENU
            .with(|m| {
                m.borrow()
                    .as_ref()
                    .and_then(|menu| menu.host.as_ref().map(HostMenu::needs_options))
            })
            .map(|needs| self.host_view(needs));
        let sharing = self.friends_sharing();
        let scale = batch.scale;
        let cursor = self.cursor;
        // The voice menu's view of the voice chat, read before the menu is borrowed.
        let voice_view = MENU
            .with(|m| m.borrow().as_ref().is_some_and(|menu| menu.voice.is_some()))
            .then(|| self.voice_view());
        let keyboard_view = MENU
            .with(|m| {
                m.borrow()
                    .as_ref()
                    .is_some_and(|menu| menu.keyboard.is_some())
            })
            .then(|| self.keyboard_view());
        let on_cosmetics = MENU.with(|m| {
            m.borrow()
                .as_ref()
                .is_some_and(|menu| menu.cosmetics.is_some())
        });
        let (tiles, worn, busy, status, can_upload) = if on_cosmetics {
            self.skins_frame();
            let (history, worn, busy, status) = self.skins_view();
            let tiles: Vec<SkinTile> = history
                .iter()
                .take(24)
                .map(|e| SkinTile {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    sprite: self.skin_sprite(&e.id).map(|(s, _)| s),
                })
                .collect();
            let status =
                status.map(|st| (lang::get_or(st.key, skin_status_english(st.key)), st.ok));
            (tiles, worn, busy, status, self.skin_can_upload())
        } else {
            Default::default()
        };
        // The turning skin: the one worn in Oteria, else the account's own.
        let turning = if on_cosmetics {
            self.skins_override()
        } else {
            None
        };
        let atlas = &self.gui.atlas;
        let skins = &self.settings.extras.pvp.skins;
        let cosmetics_view = CosmeticsView {
            knife: skins.knife,
            inspect_key: horizon_client::keybinds::display_name(
                self.settings.keys.get("key.oteria.inspect"),
            )
            .plain(),
            knife_skin: skins.knife_skin.clone(),
            rigged: horizon_client::oteria_art::KNIFE_RIGS
                .into_iter()
                .filter(|k| super::oteria_knife_rig::rig(k).is_some())
                .collect(),
            skins: tiles,
            worn,
            busy,
            status,
            can_upload,
            cape: skins.cape.clone(),
            hat: skins.hat.clone(),
            pet: skins.pet.clone(),
            aura: skins.aura.clone(),
        };
        let extras = &self.settings.extras;
        let mut voice_pending = Vec::new();
        let mut mods_pending = Vec::new();
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
                        menu.voice = None;
                        menu.host = None;
                        menu.cosmetics = None;
                        menu.keyboard = None;
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
            ui.rounded(
                0.0,
                0.0,
                w,
                h,
                0.0,
                argb(0x000000, (0.32 + 0.16 * page) * shown),
            );

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
                ui.rounded(
                    cx - size / 2.0,
                    top + logo / 2.0 - size / 2.0,
                    size,
                    size,
                    size / 2.0,
                    argb(0x8FD8FF, alpha),
                );
            }
            if let Some(r) = ui.atlas.sprites.get(oteria_art::LOGO).copied() {
                let (lx, ly) = (cx - logo / 2.0, top);
                ui.batch
                    .rect_uv(gui(lx), gui(ly), gui(logo), gui(logo), r, argb(WHITE, a));
            }
            let ty = top + logo + 8.0;
            let (bold, light) = ("OTERIA", " CLIENT");
            let bw = ui.measure(Face::SemiBold, title_size, bold);
            let lw = ui.measure(Face::Regular, title_size, light);
            let tx = cx - (bw + lw) / 2.0;
            ui.text(Face::SemiBold, title_size, tx, ty, argb(WHITE, a), bold);
            ui.text(
                Face::Regular,
                title_size,
                tx + bw,
                ty,
                argb(WHITE, 0.82 * a),
                light,
            );

            // The buttons' places, in dp.
            let row = SQUARE * 2.0 + MODS_W + GAP * 2.0;
            let x0 = cx - row / 2.0;
            let places = [
                [x0, by, SQUARE, SQUARE],
                [x0 + SQUARE + GAP, by, MODS_W, SQUARE],
                [x0 + SQUARE + GAP + MODS_W + GAP, by, SQUARE, SQUARE],
                [x0 + SQUARE + GAP, by + SQUARE + GAP, MODS_W, EDIT_H],
                [
                    x0 + SQUARE + GAP,
                    by + SQUARE + GAP + EDIT_H + 8.0,
                    MODS_W,
                    EDIT_H,
                ],
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
                    Button::EditHud => "EditHud",
                    Button::Keyboard => "Keyboard",
                };
                horizon_client::widgets::record(name, menu.rects[i], true);
                horizon_client::widgets::widget_cursor(over, true);
            }

            // Boost's look: it comes on once the wave is most of the way out, and goes slowly.
            let waving = menu
                .shock
                .is_some_and(|(start, _)| start.elapsed().as_secs_f32() < SHOCK_S * 0.35);
            let goal = if menu.boost && !waving { 1.0 } else { 0.0 };
            let rate = if goal > menu.smoulder {
                SMOULDER_IN
            } else {
                SMOULDER_OUT
            };
            menu.smoulder += (goal - menu.smoulder) * (1.0 - (-dt * rate).exp());
            let on = smooth(menu.smoulder);
            // Its fire's flicker: a few waves out of step.
            let flicker = 0.5 + 0.25 * (t * 23.0).sin() + 0.25 * (t * 37.0 + 1.3).sin();
            menu.pulse += dt / (PULSE_S * (1.0 - 0.45 * on));
            menu.sparks += dt * (2.4 + 2.2 * on);

            // Boost's loop under its button: a breathing violet glow, and a ripple leaving it.
            let phase = menu.pulse.fract();
            let breath = 0.5 + 0.5 * (menu.pulse * TAU).sin();
            let boost = places[0];
            let glow = grown(boost, 5.0 + 3.0 * on);
            let glow_alpha = (0.12 + 0.22 * breath) * (1.0 - on) + (0.34 + 0.22 * flicker) * on;
            ui.rounded(
                glow[0],
                glow[1],
                glow[2],
                glow[3],
                RADIUS + 4.0,
                argb(VIOLET, glow_alpha * a),
            );
            if on > 0.0 {
                let outer = grown(boost, 14.0);
                ui.rounded(
                    outer[0],
                    outer[1],
                    outer[2],
                    outer[3],
                    RADIUS + 12.0,
                    argb(VIOLET, 0.16 * on * flicker * a),
                );
            }
            // The ripple leaves fast enough to move several pixels a frame; then waits.
            let leave = (phase / 0.55).min(1.0);
            if leave < 1.0 {
                let ripple = grown(boost, 2.0 + 34.0 * ease_out(leave));
                let fade = (1.0 - leave).powi(2) * (0.6 + 0.3 * on) * a;
                ui.border(
                    ripple[0],
                    ripple[1],
                    ripple[2],
                    ripple[3],
                    RADIUS + 10.0,
                    argb(VIOLET, fade),
                );
            }
            // On: embers rise from all round the button, hot white turning violet as they go.
            if on > 0.0 {
                let (bcx, bcy) = (boost[0] + boost[2] / 2.0, boost[1] + boost[3] / 2.0);
                for k in 0..28 {
                    let seed = k as f32 + 1.0;
                    let speed = 1.1 + 1.1 * hash(seed * 7.3);
                    let p = (t * speed + hash(seed * 13.1)).fract();
                    let angle = hash(seed) * TAU + t * 0.4;
                    let (dx, dy) = (angle.cos(), angle.sin());
                    // From the button's edge (a square's), outwards and up.
                    let edge = (boost[2] / 2.0) / dx.abs().max(dy.abs());
                    let out = 3.0 + 22.0 * p;
                    let x = bcx + dx * (edge + out) + (t * 6.0 + seed * 9.0).sin() * 1.5;
                    let y = bcy + dy * (edge + out) - 16.0 * p * p;
                    let size = 1.2 + 3.0 * (1.0 - p);
                    let colour = if p < 0.25 {
                        0xF0E4FF
                    } else if p < 0.55 {
                        0xC9A6FF
                    } else {
                        VIOLET
                    };
                    let twinkle = 0.65 + 0.35 * (t * 19.0 + seed * 4.0).sin();
                    let alpha = on * (1.0 - p).powf(0.9) * twinkle * a;
                    ui.rounded(
                        x - size / 2.0,
                        y - size / 2.0,
                        size,
                        size,
                        size / 2.0,
                        argb(colour, alpha),
                    );
                }
            }

            // The buttons: glass that lights up under the pointer, and gives when pressed.
            for (i, place) in places.iter().enumerate() {
                let hover = menu.hover[i];
                let give = menu.pressed[i].map_or(0.0, |p| {
                    let s = p.elapsed().as_secs_f32();
                    // Down fast, back with a spring.
                    if s < 0.08 {
                        s / 0.08
                    } else {
                        (1.0 - (s - 0.08) / 0.25).max(0.0)
                    }
                });
                let r = grown(*place, -1.5 * give + 1.0 * hover);
                let fill = argb(WHITE, (0.10 + 0.10 * hover - 0.04 * give) * a);
                ui.rounded(r[0], r[1], r[2], r[3], RADIUS, fill);
                if BUTTONS[i] == Button::Boost && on > 0.0 {
                    // On, the glass takes the violet.
                    ui.rounded(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        RADIUS,
                        argb(VIOLET, (0.16 + 0.08 * flicker) * on * a),
                    );
                }
                let edge = match BUTTONS[i] {
                    Button::Boost => argb(
                        VIOLET,
                        (0.65 + 0.35 * hover.max(on * (0.7 + 0.3 * flicker))) * a,
                    ),
                    _ => argb(WHITE, (0.40 + 0.50 * hover) * a),
                };
                ui.border(r[0], r[1], r[2], r[3], RADIUS, edge);
                let (mx, my) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                match BUTTONS[i] {
                    Button::Mods => {
                        let size = 15.0;
                        let label = "MODS";
                        let lw = ui.measure(Face::Medium, size, label);
                        ui.text(
                            Face::Medium,
                            size,
                            mx - lw / 2.0,
                            my - size * 1.45 / 2.0,
                            argb(WHITE, a),
                            label,
                        );
                    }
                    Button::EditHud | Button::Keyboard => {
                        let size = 12.0;
                        let label = if BUTTONS[i] == Button::Keyboard {
                            lang::get_or("oteria.keyboard.button", "Keyboard").to_uppercase()
                        } else {
                            lang::get_or("oteria.mods.edit_hud", "Edit HUD").to_uppercase()
                        };
                        let lw = ui.measure(Face::Medium, size, &label);
                        ui.text(
                            Face::Medium,
                            size,
                            mx - lw / 2.0,
                            my - size * 1.45 / 2.0,
                            argb(WHITE, a),
                            &label,
                        );
                    }
                    button => {
                        let sprite = if button == Button::Boost {
                            oteria_art::ROCKET
                        } else {
                            oteria_art::SHIRT
                        };
                        let icon = 32.0 / d;
                        // Boost's rocket climbs a little under the pointer (quickly, eased), and
                        // shakes while on: a whole pixel this way or that, 30 times a second.
                        let lift = if button == Button::Boost {
                            2.0 * hover
                        } else {
                            0.0
                        };
                        let (jx, jy) = if button == Button::Boost && on > 0.3 {
                            let step = (t * 30.0).floor();
                            let px = 1.0 / d;
                            (
                                ((hash(step) * 3.0).floor() - 1.0) * px,
                                ((hash(step + 0.5) * 3.0).floor() - 1.0) * px,
                            )
                        } else {
                            (0.0, 0.0)
                        };
                        if button == Button::Boost {
                            // Sparks falling from the rocket's nozzle, in a loop: more, longer
                            // and hotter while on.
                            let count = if on > 0.0 { 5 } else { 3 };
                            for k in 0..count {
                                let p = (menu.sparks + k as f32 / count as f32).fract();
                                let sy = my + icon * 0.42 - lift + p * (16.0 + 10.0 * on);
                                let sx = mx + (k as f32 - (count - 1) as f32 / 2.0) * 2.5 + jx;
                                let size = (2.2 + 1.2 * on) * (1.0 - p);
                                let colour = if on > 0.0 && p < 0.3 {
                                    0xF0E4FF
                                } else {
                                    VIOLET
                                };
                                ui.rounded(
                                    sx - size / 2.0,
                                    sy,
                                    size,
                                    size,
                                    size / 2.0,
                                    argb(colour, (1.0 - p) * 0.9 * a),
                                );
                            }
                        }
                        if let Some(rect) = ui.atlas.sprites.get(sprite).copied() {
                            let (ix, iy) = (mx - icon / 2.0 + jx, my - icon / 2.0 - lift + jy);
                            ui.batch.rect_uv(
                                gui(ix),
                                gui(iy),
                                gui(icon),
                                gui(icon),
                                rect,
                                argb(WHITE, a),
                            );
                        }
                        if button == Button::Cosmetics {
                            // A small dot: something new to look at, as Lunar marks it.
                            ui.rounded(
                                r[0] + r[2] - 9.0,
                                r[1] + 3.0,
                                6.0,
                                6.0,
                                3.0,
                                argb(0x3FD27A, a),
                            );
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
                    ring(
                        &mut *ui.batch,
                        atlas,
                        centre,
                        gui((radius - thick).max(0.0)),
                        gui(radius),
                        colour,
                    );
                    let inner = argb(WHITE, (1.0 - s).powi(2) * 0.35 * a);
                    ring(
                        &mut *ui.batch,
                        atlas,
                        centre,
                        gui((radius - 2.0).max(0.0)),
                        gui(radius),
                        inner,
                    );
                }
            }
            // Boost turned off: violet trails from every edge of the screen fly into the rocket,
            // and a ring closes on it as they arrive.
            if let Some((start, [tx, ty])) = menu.calm {
                let s = start.elapsed().as_secs_f32();
                if s >= CALM_S {
                    menu.calm = None;
                } else {
                    let (cx, cy) = (tx * scale / d, ty * scale / d);
                    let atlas = ui.atlas;
                    for k in 0..TRAILS {
                        let seed = k as f32 + 0.37;
                        let delay = hash(seed * 5.7) * 0.28;
                        let flight = 0.62 + hash(seed * 3.1) * 0.2;
                        // Past 1 the head has arrived and the tail follows it in.
                        let v = (s - delay) / flight;
                        if v <= 0.0 || v >= 1.3 {
                            continue;
                        }
                        let along = (k as f32 + hash(seed * 11.0) * 0.7) / TRAILS as f32;
                        let [x0, y0] = edge_point(along, w, h, 6.0);
                        // A bend to the side, so they swirl in rather than all fly straight.
                        let (dx, dy) = (cx - x0, cy - y0);
                        let len = dx.hypot(dy).max(1.0);
                        let bend = (hash(seed * 17.0) - 0.5) * 0.6 * len;
                        let ctrl = [
                            (x0 + cx) / 2.0 - dy / len * bend,
                            (y0 + cy) / 2.0 + dx / len * bend,
                        ];
                        let at = |u: f32| {
                            let (u, q) = (u.clamp(0.0, 1.0), 1.0 - u.clamp(0.0, 1.0));
                            [
                                q * q * x0 + 2.0 * q * u * ctrl[0] + u * u * cx,
                                q * q * y0 + 2.0 * q * u * ctrl[1] + u * u * cy,
                            ]
                        };
                        // Drawn in: slow off the edge, faster as it nears the rocket.
                        let pull = |u: f32| u.clamp(0.0, 1.0).powf(1.8);
                        let (head, tail) = (pull(v), pull(v - 0.3));
                        let fade = smooth(v * 4.0) * (1.0 - smooth((v - 1.0) / 0.3));
                        const SEGMENTS: usize = 14;
                        for n in 0..SEGMENTS {
                            let (u0, u1) =
                                (n as f32 / SEGMENTS as f32, (n + 1) as f32 / SEGMENTS as f32);
                            let (p0, p1) =
                                (at(tail + (head - tail) * u0), at(tail + (head - tail) * u1));
                            let (sx, sy) = (p1[0] - p0[0], p1[1] - p0[1]);
                            let sl = sx.hypot(sy).max(0.001);
                            let (nx, ny) = (-sy / sl, sx / sl);
                            // Thin at the tail, thickest at the head.
                            let (w0, w1) = (2.6 * u0, 2.6 * u1);
                            let quad = [
                                [gui(p0[0] + nx * w0 / 2.0), gui(p0[1] + ny * w0 / 2.0)],
                                [gui(p1[0] + nx * w1 / 2.0), gui(p1[1] + ny * w1 / 2.0)],
                                [gui(p1[0] - nx * w1 / 2.0), gui(p1[1] - ny * w1 / 2.0)],
                                [gui(p0[0] - nx * w0 / 2.0), gui(p0[1] - ny * w0 / 2.0)],
                            ];
                            let colour = argb(VIOLET, u1 * 0.85 * fade * shown);
                            ui.batch.fill_quad(atlas, quad, colour);
                        }
                        if v < 1.0 {
                            let [hx, hy] = at(head);
                            ui.rounded(
                                hx - 2.0,
                                hy - 2.0,
                                4.0,
                                4.0,
                                2.0,
                                argb(0xF0E4FF, 0.9 * fade * shown),
                            );
                        }
                    }
                    // The rocket takes them in: a ring closing on it, and a last soft light.
                    let take = ((s - 0.45) / (CALM_S - 0.45)).clamp(0.0, 1.0);
                    if take > 0.0 {
                        let light = (take * PI).sin();
                        let ring_r = 46.0 * (1.0 - ease_out(take)) + 4.0;
                        let centre = [gui(cx), gui(cy)];
                        let atlas = ui.atlas;
                        ring(
                            &mut *ui.batch,
                            atlas,
                            centre,
                            gui(ring_r - 2.5),
                            gui(ring_r),
                            argb(VIOLET, 0.55 * light * shown),
                        );
                        let halo = grown(boost, 4.0);
                        ui.rounded(
                            halo[0],
                            halo[1],
                            halo[2],
                            halo[3],
                            RADIUS + 4.0,
                            argb(VIOLET, 0.25 * light * a),
                        );
                    }
                }
            }
            // The mods page, rising into place as it comes.
            // The Host button, at the bottom right (only with the buttons).
            {
                let hb = [w - 16.0 - 150.0, h - 16.0 - 42.0, 150.0, 42.0];
                let over = menu.closing.is_none() && page == 0.0 && inside(hb, ui.cursor);
                menu.host_hover += ((if over { 1.0 } else { 0.0 }) - menu.host_hover) * follow;
                menu.host_rect = if page == 0.0 { hb.map(gui) } else { [0.0; 4] };
                let hv = menu.host_hover;
                let r = grown(hb, hv);
                ui.rounded(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    RADIUS,
                    argb(0x23A35C, (0.70 + 0.30 * hv) * a),
                );
                ui.border(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    RADIUS,
                    argb(WHITE, (0.25 + 0.4 * hv) * a),
                );
                let label = horizon_client::lang::get_or("oteria.host.button", "HOST");
                let lw = ui.measure(Face::Medium, 13.0, &label);
                let lx = r[0] + (r[2] - lw - 26.0) / 2.0 + 26.0;
                if let Some(s) = ui
                    .atlas
                    .sprites
                    .get(
                        horizon_client::oteria_icons::small(horizon_client::oteria_icons::SERVER)
                            .as_str(),
                    )
                    .copied()
                {
                    let side = 20.0 / d;
                    ui.batch.rect_uv(
                        gui(lx - 26.0 + 2.0),
                        gui(r[1] + (r[3] - side) / 2.0),
                        gui(side),
                        gui(side),
                        s,
                        argb(WHITE, a),
                    );
                }
                ui.text(
                    Face::Medium,
                    13.0,
                    lx,
                    r[1] + (r[3] - 19.0) / 2.0,
                    argb(WHITE, a),
                    &label,
                );
                if sharing {
                    // Shared now: a light that breathes in its corner.
                    let glow = 0.6 + 0.4 * (t * TAU / 1.6).sin();
                    ui.rounded(
                        r[0] + r[2] - 13.0,
                        r[1] + 5.0,
                        8.0,
                        8.0,
                        4.0,
                        argb(0xB8FFC8, glow * a),
                    );
                }
                horizon_client::widgets::record("host", menu.host_rect, page == 0.0);
                horizon_client::widgets::widget_cursor(over, true);
            }
            if page > 0.0
                && let (Some(keyboard), Some(view)) =
                    (menu.keyboard.as_mut(), keyboard_view.as_ref())
            {
                keyboard.draw(&mut ui, view, shown * page, (1.0 - page) * 18.0, dt, &gui);
            } else if page > 0.0
                && let Some(cosmetics) = menu.cosmetics.as_mut()
            {
                cosmetics.draw(
                    &mut ui,
                    &cosmetics_view,
                    shown * page,
                    (1.0 - page) * 18.0,
                    dt,
                    &gui,
                );
            } else if page > 0.0
                && let (Some(host), Some(view)) = (menu.host.as_mut(), host_view.as_ref())
            {
                host.draw(&mut ui, view, shown * page, (1.0 - page) * 18.0, &gui);
            } else if page > 0.0
                && let (Some(voice), Some(view)) = (menu.voice.as_mut(), voice_view.as_ref())
            {
                voice.draw(
                    &mut ui,
                    view,
                    &extras.pvp,
                    shown * page,
                    (1.0 - page) * 18.0,
                    dt,
                    &gui,
                    cursor,
                );
                voice_pending = voice.take_pending();
            } else if page > 0.0
                && let Some(mods) = menu.mods.as_mut()
            {
                mods.draw(
                    &mut ui,
                    extras,
                    shown * page,
                    (1.0 - page) * 18.0,
                    dt,
                    &gui,
                    cursor,
                    menu.profiles.as_ref(),
                );
                mods_pending = mods.take_pending();
            }
            ui.finish();
        });
        // The Cosmetics page's 3D: each card's model, then the preview over them (its panel, then
        // what is seen in it).
        super::oteria_wear::showing(on_cosmetics);
        let clock = self.started.elapsed().as_secs_f32();
        let cards = MENU.with(|m| {
            m.borrow()
                .as_ref()
                .filter(|menu| menu.back.is_none())
                .and_then(|menu| menu.cosmetics.as_ref().map(|c| c.cards.clone()))
                .unwrap_or_default()
        });
        for card in &cards {
            self.draw_cosmetic_card(batch, card, clock);
        }
        let atlas = &self.gui.atlas;
        MENU.with(|m| {
            let mut m = m.borrow_mut();
            let Some(menu) = m.as_mut() else {
                return;
            };
            if menu.back.is_some() {
                return;
            }
            let Some(page) = menu.cosmetics.as_mut().filter(|c| c.popup_open()) else {
                return;
            };
            let mut ui = Ui::new(atlas, batch, cursor);
            let d = ui.density;
            let gui = |v: f32| v * d / scale;
            page.draw_popup(&mut ui, &cosmetics_view, &gui);
            ui.finish();
        });
        let preview = MENU.with(|m| {
            m.borrow().as_ref().and_then(|menu| {
                menu.cosmetics
                    .as_ref()
                    .and_then(|c| c.preview)
                    .filter(|_| menu.back.is_none())
            })
        });
        if let Some(super::oteria_cosmetics::Preview { well, shown }) = preview {
            match shown {
                super::oteria_cosmetics::Shown::Player { trying, turn } => {
                    use super::oteria_cosmetics::WearKind;
                    let aura = match trying {
                        Some((WearKind::Aura, id)) => {
                            horizon_client::oteria_art::wear_style(&horizon_client::oteria_art::AURAS, id)
                        }
                        _ => horizon_client::oteria_art::wear_style(
                            &horizon_client::oteria_art::AURAS,
                            &cosmetics_view.aura,
                        ),
                    };
                    self.draw_cosmetic_player(batch, well, turn, trying, aura, turning, clock);
                }
                super::oteria_cosmetics::Shown::Knife(knife) => {
                    self.draw_knife_preview(batch, well, knife, clock);
                }
            }
        }
        // A slider dragged on the voice menu.
        for a in voice_pending {
            self.voice_action(a);
        }
        // A colour dragged on the picker, or a value typed.
        for a in mods_pending {
            self.mods_value(a);
        }
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
        batch.fill_quad(
            atlas,
            [
                at(inner, k),
                at(outer, k),
                at(outer, k + 1),
                at(inner, k + 1),
            ],
            colour,
        );
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
    fn the_trails_start_all_round_the_screen() {
        let (w, h) = (400.0, 200.0);
        assert_eq!(edge_point(0.0, w, h, 0.0), [0.0, 0.0]);
        assert_eq!(edge_point(0.25, w, h, 0.0), [300.0, 0.0]);
        assert_eq!(edge_point(0.5, w, h, 0.0), [400.0, 200.0]);
        assert_eq!(edge_point(0.75, w, h, 0.0), [100.0, 200.0]);
        // Past the edge by `out`, on every side.
        for k in 0..20 {
            let [x, y] = edge_point(k as f32 / 20.0, w, h, 5.0);
            assert!(x <= 0.0 || x >= w || y <= 0.0 || y >= h, "{x} {y}");
        }
        assert!((0..100).all(|k| (0.0..1.0).contains(&hash(k as f32))));
    }

    #[test]
    fn colours_take_their_alpha_on_top_and_red_in_the_low_byte() {
        // The batch reads 0xAABBGGRR: the violet's blue (0xFF) high, its red (0x9B) low.
        assert_eq!(argb(VIOLET, 1.0), 0xFFFF_5C9B);
        assert_eq!(argb(VIOLET, 0.0), 0x00FF_5C9B);
        assert_eq!(argb(WHITE, 2.0), 0xFFFF_FFFF, "clamped");
    }
}

/// A skin action's result in English, where the language has no text for it.
fn skin_status_english(key: &str) -> &'static str {
    match key {
        "oteria.skin.worn" => "Skin on. Only Oteria shows it, unless you put it on your account.",
        "oteria.skin.uploaded" => "On your Minecraft account: everyone sees it now.",
        "oteria.skin.error.offline" => {
            "No answer from Minecraft's services. Check your connection."
        }
        "oteria.skin.error.no_player" => "No player has that name.",
        "oteria.skin.error.no_skin" => "That player wears a default skin.",
        "oteria.skin.error.bad_name" => "A player's name: 1 to 16 letters, digits or _.",
        "oteria.skin.error.not_a_skin" => "Not a skin: a 64x64 (or 64x32) PNG.",
        "oteria.skin.error.not_saved" => "The skin couldn't be saved on this computer.",
        "oteria.skin.error.signed_out" => {
            "Sign in with your Microsoft account in the launcher first."
        }
        "oteria.skin.error.too_many" => "Too many changes: wait a minute and try again.",
        "oteria.skin.error.refused" => "Minecraft refused this skin.",
        "oteria.skin.error.nothing_to_send" => "Wear a skin from the history first.",
        "oteria.skin.error.no_folder" => "The import folder couldn't be opened.",
        _ => "",
    }
}
