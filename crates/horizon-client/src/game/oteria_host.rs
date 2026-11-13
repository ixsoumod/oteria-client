//! Oteria's Host page (the menu's Host button, bottom right): shares the singleplayer world
//! being played behind an invite code (Play with Friends, `friends.rs`), after its options:
//! the players' game mode and whether it is forced on them, commands for them and for the
//! host, the host's own game mode and the difficulty. Once shared: the code to copy (friends
//! paste it in Multiplayer, Direct Connection), the relay's short code when there is one, how
//! friends reach this computer, who joined, and Stop.

use super::lan_screen::HostOptions;
use crate::ui_theme::{Face, Ui};
use horizon_client::lang;
use horizon_client::oteria_icons as icon;

const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const ON: u32 = 0x23_A35C;
const OFF: u32 = 0xB3_3346;
const GREY: u32 = 0x4A_5260;

fn argb(rgb: u32, alpha: f32) -> u32 {
    super::oteria_menu::argb(rgb, alpha)
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
fn tr(key: &str, english: &str) -> String {
    lang::get_or(key, english)
}
/// A game mode's name (`MODES` order: survival, creative, adventure, spectator).
fn mode(m: i32) -> String {
    let (key, english) = match m {
        1 => ("oteria.host.mode.creative", "Creative"),
        2 => ("oteria.host.mode.adventure", "Adventure"),
        3 => ("oteria.host.mode.spectator", "Spectator"),
        _ => ("oteria.host.mode.survival", "Survival"),
    };
    tr(key, english)
}
fn difficulty(d: u8) -> String {
    let (key, english) = match d {
        0 => ("oteria.host.difficulty.peaceful", "Peaceful"),
        1 => ("oteria.host.difficulty.easy", "Easy"),
        3 => ("oteria.host.difficulty.hard", "Hard"),
        _ => ("oteria.host.difficulty.normal", "Normal"),
    };
    tr(key, english)
}

/// What the page shows, read from the game each frame.
pub(super) struct HostView {
    /// The world being played, as it is now; None away from a singleplayer world.
    pub options: Option<HostOptions>,
    /// The world is shared: its code (None while it is found), the relay's short code, how
    /// friends reach it, whether they can, who joined, and whether the code was just copied.
    pub sharing: Option<Sharing>,
}
pub(super) struct Sharing {
    pub code: Option<String>,
    pub short: Option<String>,
    pub status: String,
    pub reachable: bool,
    pub guests: Vec<String>,
    pub copied: bool,
}

/// What the page asks of the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HostAction {
    Host(HostOptions),
    Copy,
    CopyShort,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Mode,
    Force,
    GuestCommands,
    Cheats,
    Personal,
    Difficulty,
    Host,
    Copy,
    CopyShort,
    Stop,
}

pub(super) struct HostMenu {
    /// The options being chosen (from the world's, at the first frame).
    options: Option<HostOptions>,
    hits: Vec<([f32; 4], Hit)>,
}

impl HostMenu {
    pub(super) fn new() -> Self {
        Self { options: None, hits: Vec::new() }
    }
    /// Whether the page still waits for the world's options (its first frame).
    pub(super) fn needs_options(&self) -> bool {
        self.options.is_none()
    }

    /// A press at `cursor` (GUI pixels).
    pub(super) fn click(&mut self, cursor: (f32, f32)) -> Option<HostAction> {
        let hit = self.hits.iter().rev().find(|(r, _)| inside(*r, cursor))?.1;
        let o = self.options.as_mut();
        match hit {
            Hit::Host => return o.map(|o| HostAction::Host(*o)),
            Hit::Copy => return Some(HostAction::Copy),
            Hit::CopyShort => return Some(HostAction::CopyShort),
            Hit::Stop => return Some(HostAction::Stop),
            _ => {}
        }
        let o = o?;
        match hit {
            Hit::Mode => o.default_mode = (o.default_mode + 1).rem_euclid(4),
            Hit::Personal => o.personal_mode = (o.personal_mode + 1).rem_euclid(4),
            Hit::Force => o.force = !o.force,
            Hit::Cheats => {
                o.cheats = !o.cheats;
                // No commands for the players without the host's (the world options' rule).
                if !o.cheats {
                    o.guest_commands = false;
                }
            }
            Hit::GuestCommands => {
                o.guest_commands = !o.guest_commands;
                if o.guest_commands {
                    o.cheats = true;
                }
            }
            Hit::Difficulty if !o.locked => o.difficulty = (o.difficulty + 1) % 4,
            _ => {}
        }
        None
    }

    /// Draws the page at `alpha`, `rise` dp under its place.
    pub(super) fn draw(&mut self, ui: &mut Ui, view: &HostView, alpha: f32, rise: f32, gui: &dyn Fn(f32) -> f32) {
        if self.options.is_none() {
            self.options = view.options;
        }
        let mut hits = Vec::new();
        let pointer = ui.cursor;
        let a = alpha;
        let (pw, ph) = (440.0, if view.sharing.is_some() { 400.0 } else { 420.0 });
        let (px, py) = ((ui.w - pw) / 2.0, (ui.h - ph) / 2.0 + rise);
        ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.86 * a));
        ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * a));
        let title = tr("oteria.host.title", "Host this world");
        ui.text_centered(Face::SemiBold, 15.0, px + pw / 2.0, py + 14.0, argb(WHITE, a), &title);
        let (x, w) = (px + 22.0, pw - 44.0);
        let mut y = py + 50.0;
        let mut hit = |ui: &mut Ui, r: [f32; 4], what: Hit| {
            horizon_client::widgets::widget_cursor(inside(r, pointer), true);
            hits.push((r.map(gui), what));
            let _ = ui;
        };
        let button = |ui: &mut Ui, r: [f32; 4], label: &str, colour: u32, sprite: Option<&str>| {
            let over = inside(r, pointer);
            ui.rounded(r[0], r[1], r[2], r[3], 9.0, argb(colour, if over { 1.0 } else { 0.82 } * a));
            let lw = ui.measure(Face::Medium, 12.0, label);
            let iw = if sprite.is_some() { 24.0 } else { 0.0 };
            let lx = r[0] + (r[2] - lw - iw) / 2.0 + iw;
            if let Some(s) = sprite {
                small_icon(ui, s, lx - 14.0, r[1] + r[3] / 2.0, argb(WHITE, a), gui);
            }
            ui.text(Face::Medium, 12.0, lx, r[1] + (r[3] - 17.0) / 2.0, argb(WHITE, a), label);
        };

        let Some(options) = self.options else {
            let none = tr("oteria.host.unavailable", "Open a singleplayer world to host it.");
            ui.text_centered(Face::Regular, 12.0, px + pw / 2.0, y + 40.0, argb(WHITE, 0.6 * a), &none);
            self.hits = hits;
            return;
        };

        match &view.sharing {
            None => {
                // The options, each a row: a value stepped by a click, or a switch.
                let rows: [(String, Result<String, bool>, Hit, bool); 6] = [
                    (tr("oteria.host.players_mode", "Players' game mode"), Ok(mode(options.default_mode)), Hit::Mode, true),
                    (tr("oteria.host.force", "Force it on them"), Err(options.force), Hit::Force, true),
                    (tr("oteria.host.guest_commands", "Commands for the players"), Err(options.guest_commands), Hit::GuestCommands, true),
                    (tr("oteria.host.cheats", "Commands for you (cheats)"), Err(options.cheats), Hit::Cheats, true),
                    (tr("oteria.host.own_mode", "Your game mode"), Ok(mode(options.personal_mode)), Hit::Personal, true),
                    (tr("oteria.host.difficulty", "Difficulty"), Ok(difficulty(options.difficulty)), Hit::Difficulty, !options.locked),
                ];
                for (label, value, what, live) in rows {
                    let r = [x, y, w, 40.0];
                    let over = live && inside(r, pointer);
                    ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(WHITE, if over { 0.08 } else { 0.04 } * a));
                    ui.text(Face::Medium, 12.0, x + 14.0, y + 11.0, argb(WHITE, if live { 0.92 } else { 0.45 } * a), &label);
                    match value {
                        Ok(v) => {
                            let pill = [x + w - 14.0 - 150.0, y + 6.0, 150.0, 28.0];
                            ui.rounded(pill[0], pill[1], pill[2], pill[3], 7.0, argb(WHITE, 0.07 * a));
                            ui.text_centered(Face::Medium, 12.0, pill[0] + 75.0, pill[1] + 6.0, argb(WHITE, if live { 1.0 } else { 0.45 } * a), &v);
                        }
                        Err(on) => {
                            let track = [x + w - 14.0 - 40.0, y + 10.0, 40.0, 20.0];
                            let k = if on { 1.0 } else { 0.0 };
                            ui.rounded(track[0], track[1], track[2], track[3], 10.0, argb(if on { ON } else { GREY }, a));
                            ui.rounded(track[0] + 2.0 + k * 20.0, track[1] + 2.0, 16.0, 16.0, 8.0, argb(WHITE, a));
                        }
                    }
                    if live {
                        hit(ui, r, what);
                    }
                    y += 46.0;
                }
                let how = tr(
                    "oteria.host.how",
                    "Friends join with the invite code: Multiplayer, Direct Connection, then paste it.",
                );
                ui.text(Face::Regular, 11.0, x + 2.0, y + 2.0, argb(WHITE, 0.5 * a), &how);
                let go = [x, py + ph - 22.0 - 40.0, w, 40.0];
                button(ui, go, &tr("oteria.host.start", "Host the world"), ON, Some(icon::SERVER));
                hit(ui, go, Hit::Host);
                horizon_client::widgets::record("oteria-host-start", go.map(gui), true);
            }
            Some(s) => {
                let colour = if s.reachable { 0x6BE0_8A } else { 0xFFC8_6A };
                // How friends reach this computer: all of it, on as many lines as it takes.
                for line in wrap(ui, &s.status, w).into_iter().take(5) {
                    ui.text(Face::Regular, 11.0, x + 2.0, y, argb(colour, a), &line);
                    y += 16.0;
                }
                y += 8.0;
                // The invite code, cut to fit, with Copy beside it.
                let label = tr("oteria.host.code", "Invite code");
                ui.text(Face::Medium, 11.0, x + 2.0, y, argb(WHITE, 0.55 * a), &label);
                y += 18.0;
                let copy_w = 110.0;
                let field = [x, y, w - copy_w - 10.0, 36.0];
                ui.rounded(field[0], field[1], field[2], field[3], 8.0, argb(WHITE, 0.07 * a));
                let code = s.code.clone().unwrap_or_else(|| tr("oteria.host.finding", "Finding how friends can reach you…"));
                let shown = fit(ui, &code, field[2] - 24.0);
                ui.text(Face::Regular, 12.0, field[0] + 12.0, field[1] + 9.0, argb(WHITE, 0.9 * a), &shown);
                let copy = [x + w - copy_w, y, copy_w, 36.0];
                let copied = if s.copied { tr("oteria.host.copied", "Copied!") } else { tr("oteria.host.copy", "Copy") };
                button(ui, copy, &copied, if s.code.is_some() { ACCENT } else { GREY }, None);
                if s.code.is_some() {
                    hit(ui, copy, Hit::Copy);
                    horizon_client::widgets::record("oteria-host-copy", copy.map(gui), true);
                }
                y += 48.0;
                if let Some(short) = &s.short {
                    let label = format!("{} : {short}", tr("oteria.host.short", "Short code (relay)"));
                    ui.text(Face::Medium, 12.0, x + 2.0, y + 8.0, argb(WHITE, a), &label);
                    let r = [x + w - copy_w, y, copy_w, 32.0];
                    button(ui, r, &tr("oteria.host.copy", "Copy"), ACCENT, None);
                    hit(ui, r, Hit::CopyShort);
                    y += 42.0;
                }
                let label = tr("oteria.host.players", "Players");
                ui.text(Face::Medium, 11.0, x + 2.0, y, argb(WHITE, 0.55 * a), &label);
                y += 18.0;
                let names = if s.guests.is_empty() {
                    tr("oteria.host.nobody", "Nobody yet.")
                } else {
                    s.guests.join(", ")
                };
                let names = fit(ui, &names, w);
                ui.text(Face::Regular, 12.0, x + 2.0, y, argb(WHITE, 0.85 * a), &names);
                let stop = [x, py + ph - 22.0 - 40.0, w, 40.0];
                button(ui, stop, &tr("oteria.host.stop", "Stop hosting"), OFF, Some(icon::X));
                hit(ui, stop, Hit::Stop);
                horizon_client::widgets::record("oteria-host-stop", stop.map(gui), true);
            }
        }
        self.hits = hits;
    }
}

/// An icon of the menu's chrome (20 texels) centred on (cx, cy), on whole pixels.
fn small_icon(ui: &mut Ui, name: &str, cx: f32, cy: f32, colour: u32, gui: &dyn Fn(f32) -> f32) {
    let small = icon::small(name);
    if let Some(r) = ui.atlas.sprites.get(small.as_str()).copied() {
        let s = 20.0 / ui.density;
        let k = ui.batch.scale;
        let snap = |v: f32| (gui(v) * k).round() / k;
        ui.batch.rect_uv(snap(cx - s / 2.0), snap(cy - s / 2.0), gui(s), gui(s), r, colour);
    }
}
/// `text` broken into lines of at most `width` dp, at spaces.
fn wrap(ui: &Ui, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if !line.is_empty() && ui.measure(Face::Regular, 11.0, &next) > width {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
fn fit(ui: &Ui, text: &str, width: f32) -> String {
    if ui.measure(Face::Regular, 12.0, text) <= width {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while !cut.is_empty() && ui.measure(Face::Regular, 12.0, &format!("{cut}…")) > width {
        cut.pop();
    }
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> HostOptions {
        HostOptions {
            default_mode: 0,
            personal_mode: 1,
            cheats: false,
            difficulty: 2,
            guest_commands: false,
            force: true,
            locked: false,
        }
    }

    #[test]
    fn the_players_commands_need_the_hosts() {
        let mut m = HostMenu { options: Some(options()), hits: vec![([0.0, 0.0, 10.0, 10.0], Hit::GuestCommands)] };
        m.click((5.0, 5.0));
        let o = m.options.unwrap();
        assert!(o.guest_commands && o.cheats, "the players' commands turn the host's on");
        m.hits = vec![([0.0, 0.0, 10.0, 10.0], Hit::Cheats)];
        m.click((5.0, 5.0));
        let o = m.options.unwrap();
        assert!(!o.cheats && !o.guest_commands, "and go with them");
    }

    #[test]
    fn modes_cycle_and_a_locked_difficulty_stays() {
        let mut m = HostMenu {
            options: Some(HostOptions { locked: true, default_mode: 3, ..options() }),
            hits: vec![([0.0, 0.0, 10.0, 10.0], Hit::Mode), ([20.0, 0.0, 10.0, 10.0], Hit::Difficulty)],
        };
        m.click((5.0, 5.0));
        m.click((25.0, 5.0));
        let o = m.options.unwrap();
        assert_eq!((o.default_mode, o.difficulty), (0, 2));
        m.hits.push(([40.0, 0.0, 10.0, 10.0], Hit::Host));
        assert_eq!(m.click((45.0, 5.0)), Some(HostAction::Host(o)));
    }
}
