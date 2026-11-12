//! The Mods page of Oteria's menu (`oteria_menu`), as Lunar's and Feather's: a glass panel with
//! the header (logo, MODS and SETTINGS tabs, close), the categories down the left with Edit HUD
//! at their foot, a search field, and a grid of tiles. Each tile is one of the engine's own
//! features (`Extras` and its PvP part): its icon, its name, a gear that opens its settings, and a
//! bar that turns it on or off, eased from red to green.
//!
//! As the menu, it moves on the clock: the hover lights, the switches' colours and the scroll all
//! ease towards where they are going, whatever the frame rate.

use crate::ui_theme::{Face, Ui};
use horizon_client::extras::Extras;
use horizon_client::lang;
use horizon_client::name_badge;
use horizon_client::options_ui::Page;
use horizon_client::oteria_icons as icon;

/// The menu's colours, as `0xRRGGBB` (`argb` makes them the batch's).
const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const ON: u32 = 0x23_A35C;
const OFF: u32 = 0xB3_3346;

/// What a tile belongs to, for the filters down the left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Category {
    All,
    Hud,
    Pvp,
    Visual,
    Utility,
}
const CATEGORIES: [Category; 5] = [
    Category::All,
    Category::Hud,
    Category::Pvp,
    Category::Visual,
    Category::Utility,
];
impl Category {
    fn name(self) -> String {
        match self {
            Category::All => lang::get_or("oteria.mods.all", "All"),
            Category::Hud => lang::get_or("oteria.mods.hud", "HUD"),
            Category::Pvp => lang::get_or("oteria.mods.pvp", "PvP"),
            Category::Visual => lang::get_or("oteria.mods.visual", "Visual"),
            Category::Utility => lang::get_or("oteria.mods.utility", "Utility"),
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Category::All => icon::LAYOUT,
            Category::Hud => icon::GAUGE,
            Category::Pvp => icon::SWORD,
            Category::Visual => icon::SPARKLES,
            Category::Utility => icon::SETTINGS,
        }
    }
}

/// One feature on the grid: its name (language key and English), icon, category, the settings
/// page its gear opens, and how to read and set it.
pub(super) struct Tile {
    key: &'static str,
    english: &'static str,
    icon: &'static str,
    category: Category,
    pub(super) page: Page,
    pub(super) get: fn(&Extras) -> bool,
    pub(super) set: fn(&mut Extras, bool),
}
impl Tile {
    fn name(&self) -> String {
        lang::get_or(self.key, self.english)
    }
}

macro_rules! tile {
    ($key:literal, $english:literal, $icon:expr, $cat:ident, $page:ident, $e:ident => $field:expr) => {
        Tile {
            key: $key,
            english: $english,
            icon: $icon,
            category: Category::$cat,
            page: Page::$page,
            get: |$e: &Extras| $field,
            set: |$e: &mut Extras, on: bool| $field = on,
        }
    };
}

/// Every tile, in the grid's order.
pub(super) static TILES: [Tile; 26] = [
    tile!("oteria.mod.fps", "FPS", icon::GAUGE, Hud, Pvp, e => e.pvp.hud.fps),
    tile!("oteria.mod.cps", "CPS", icon::MOUSE, Hud, Pvp, e => e.pvp.hud.cps),
    tile!("oteria.mod.keystrokes", "Keystrokes", icon::KEYBOARD, Hud, Pvp, e => e.pvp.hud.keys),
    tile!("oteria.mod.armour", "Armor Status", icon::SHIELD, Hud, Pvp, e => e.pvp.hud.armour),
    tile!("oteria.mod.effects", "Potion Effects", icon::FLASK, Hud, Pvp, e => e.pvp.hud.effects),
    tile!("oteria.mod.held", "Held Item", icon::SWORD, Hud, Pvp, e => e.pvp.hud.held),
    tile!("oteria.mod.ping", "Ping", icon::WIFI, Hud, Pvp, e => e.pvp.hud.ping),
    tile!("oteria.mod.coords", "Coordinates", icon::MAP_PIN, Hud, Pvp, e => e.pvp.hud.coords),
    tile!("oteria.mod.server", "Server Address", icon::SERVER, Hud, Pvp, e => e.pvp.hud.server),
    tile!("oteria.mod.combo", "Combo Counter", icon::FLAME, Hud, Pvp, e => e.pvp.hud.combo),
    tile!("oteria.mod.clock", "Clock", icon::CLOCK, Hud, Pvp, e => e.pvp.hud.clock),
    tile!("oteria.mod.reach", "Reach Display", icon::RULER_2, Hud, Pvp, e => e.pvp.hud.reach),
    tile!("oteria.mod.speed", "Speed", icon::RUN, Hud, Pvp, e => e.pvp.hud.speed),
    tile!("oteria.mod.crosshair", "Crosshair", icon::CROSSHAIR, Pvp, Pvp, e => e.pvp.crosshair.custom),
    tile!("oteria.mod.hit", "Hit Color", icon::DROPLET, Pvp, Pvp, e => e.pvp.hit.custom),
    tile!("oteria.mod.outline", "Block Outline", icon::BOX, Pvp, Pvp, e => e.pvp.outline.custom),
    Tile {
        key: "oteria.mod.low_fire",
        english: "Low Fire",
        icon: icon::FLAME_OFF,
        category: Category::Pvp,
        page: Page::Pvp,
        get: |e: &Extras| e.pvp.tweaks.fire_lower > 0,
        // On: the fire overlay lowered by 40 % of its height, a common PvP setting.
        set: |e: &mut Extras, on: bool| e.pvp.tweaks.fire_lower = if on { 40 } else { 0 },
    },
    tile!("oteria.mod.numbers", "Enchant Numbers", icon::NUMBERS, Pvp, Pvp, e => e.pvp.tweaks.numeric_enchants),
    tile!("oteria.mod.fullbright", "Fullbright", icon::SUN, Visual, Extras, e => e.fullbright),
    tile!("oteria.mod.lights", "Dynamic Lights", icon::BULB, Visual, Extras, e => e.dynamic_lights),
    tile!("oteria.mod.water", "Clear Water", icon::RIPPLE, Visual, Pvp, e => e.pvp.tweaks.clear_water),
    tile!("oteria.mod.particles", "No Block Particles", icon::SPARKLES, Visual, Extras, e => e.hide_block_particles),
    tile!("oteria.mod.minimap", "Minimap", icon::MAP, Utility, Extras, e => e.minimap),
    tile!("oteria.mod.mouse", "Mouse Tweaks", icon::POINTER, Utility, Extras, e => e.mouse_tweaks),
    tile!("oteria.mod.reconnect", "Auto Reconnect", icon::REFRESH, Utility, Extras, e => e.auto_reconnect),
    tile!("oteria.mod.nametag", "Own Name Tag", icon::USER, Utility, Extras, e => e.own_name_tag),
];

/// What a click on the page asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Toggle(usize),
    Options(usize),
    Settings,
    Close,
    EditHud,
}

/// The page's state: its filter, search, scroll and the lights that ease.
pub(super) struct Mods {
    category: Category,
    search: String,
    pub(super) typing: bool,
    scroll: f32,
    target: f32,
    /// The search field's caret blinks once a second.
    blink: f32,
    /// The scroll's room: how far down the grid goes past its view (dp).
    room: f32,
    hover: Vec<f32>,
    /// Each tile's switch, eased from 0 (off) to 1 (on).
    switch: Vec<Option<f32>>,
    /// Where things were drawn (GUI pixels), for clicks.
    hits: Vec<([f32; 4], Hit)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Category(Category),
    Search,
    Settings,
    Close,
    EditHud,
    Toggle(usize),
    Gear(usize),
}
impl Default for Mods {
    fn default() -> Self {
        Self {
            category: Category::All,
            search: String::new(),
            typing: false,
            scroll: 0.0,
            target: 0.0,
            blink: 0.0,
            room: 0.0,
            hover: vec![0.0; TILES.len()],
            switch: vec![None; TILES.len()],
            hits: Vec::new(),
        }
    }
}

fn argb(rgb: u32, alpha: f32) -> u32 {
    super::oteria_menu::argb(rgb, alpha)
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
/// Between `a` and `b` (`0xRRGGBB`) by `t`.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, s: u32| ((c >> s) & 0xFF) as f32;
    let one = |s: u32| (ch(a, s) + (ch(b, s) - ch(a, s)) * t.clamp(0.0, 1.0)).round() as u32;
    one(16) << 16 | one(8) << 8 | one(0)
}

impl Mods {
    /// The tiles the filter and the search leave, by index.
    fn shown(&self) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        (0..TILES.len())
            .filter(|&i| self.category == Category::All || TILES[i].category == self.category)
            .filter(|&i| query.is_empty() || TILES[i].name().to_lowercase().contains(&query))
            .collect()
    }

    /// A click at `cursor` (GUI pixels): what the game should do, if anything.
    pub(super) fn click(&mut self, cursor: (f32, f32)) -> Option<Action> {
        let hit = self.hits.iter().rev().find(|(r, _)| inside(*r, cursor)).map(|h| h.1);
        self.typing = hit == Some(Hit::Search);
        match hit? {
            Hit::Category(c) => {
                self.category = c;
                self.target = 0.0;
                None
            }
            Hit::Search => None,
            Hit::Settings => Some(Action::Settings),
            Hit::Close => Some(Action::Close),
            Hit::EditHud => Some(Action::EditHud),
            Hit::Toggle(i) => Some(Action::Toggle(i)),
            Hit::Gear(i) => Some(Action::Options(i)),
        }
    }

    /// The wheel turned by `lines` (positive: up).
    pub(super) fn wheel(&mut self, lines: f32) {
        self.target = (self.target - lines * 48.0).clamp(0.0, self.room);
    }

    /// A key while the search field has the keyboard: typed text, or Backspace. True when taken.
    pub(super) fn type_key(&mut self, text: Option<&str>, backspace: bool) -> bool {
        if !self.typing {
            return false;
        }
        if backspace {
            self.search.pop();
        } else if let Some(text) = text {
            self.search
                .extend(text.chars().filter(|c| !c.is_control()).take(32 - self.search.len().min(32)));
        }
        self.target = 0.0;
        true
    }

    /// Draws the page over `ui` at `alpha`, `rise` dp below its place (its way in); `dt` is the
    /// frame's time step for the easings, `gui` turns dp into GUI pixels.
    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        extras: &Extras,
        alpha: f32,
        rise: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        self.hits.clear();
        let a = alpha;
        let d = ui.density;
        let follow = 1.0 - (-dt * 16.0).exp();
        let pointer = ui.cursor;
        let (w, h) = (ui.w, ui.h);
        let pw = (w - 48.0).min(860.0);
        let ph = (h - 48.0).min(520.0);
        let (px, py) = ((w - pw) / 2.0, (h - ph) / 2.0 + rise);
        let hit = |hits: &mut Vec<([f32; 4], Hit)>, r: [f32; 4], what: Hit| {
            let g = r.map(gui);
            horizon_client::widgets::widget_cursor(inside(r, pointer), true);
            hits.push((g, what));
        };
        // An icon centred on (cx, cy), one texel to a pixel: the tiles' at 32 texels, the
        // chrome's (`small`) at 20.
        let icon_at = |ui: &mut Ui, name: &str, cx: f32, cy: f32, colour: u32, big: bool| {
            let small = icon::small(name);
            let (name, texels) = if big { (name, 32.0) } else { (small.as_str(), 20.0) };
            if let Some(r) = ui.atlas.sprites.get(name).copied() {
                let s = texels / d;
                // On whole screen pixels, so no texel falls between two.
                let k = ui.batch.scale;
                let snap = |v: f32| (gui(v) * k).round() / k;
                ui.batch.rect_uv(snap(cx - s / 2.0), snap(cy - s / 2.0), gui(s), gui(s), r, colour);
            }
        };

        // The glass panel.
        ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.80 * a));
        ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * a));

        // The header: logo, name, tabs, close.
        let hh = 50.0;
        if let Some(r) = ui.atlas.sprites.get(name_badge::SPRITE).copied() {
            let s = 32.0 / d;
            ui.batch.rect_uv(gui(px + 18.0), gui(py + (hh - s) / 2.0), gui(s), gui(s), r, argb(WHITE, a));
        }
        let tx = px + 18.0 + 32.0 / d + 8.0;
        let bw = ui.measure(Face::SemiBold, 14.0, "OTERIA");
        ui.text(Face::SemiBold, 14.0, tx, py + (hh - 20.0) / 2.0, argb(WHITE, a), "OTERIA");
        ui.text(Face::Regular, 14.0, tx + bw, py + (hh - 20.0) / 2.0, argb(WHITE, 0.75 * a), " CLIENT");
        let tabs = [
            (lang::get_or("oteria.mods.tab", "MODS"), true),
            (lang::get_or("oteria.mods.settings", "SETTINGS"), false),
        ];
        let mut tab_x = px + 200.0;
        for (label, on) in &tabs {
            let lw = ui.measure(Face::Medium, 11.0, label) + 28.0;
            let r = [tab_x, py + (hh - 28.0) / 2.0, lw, 28.0];
            let over = inside(r, pointer);
            let fill = if *on { argb(ACCENT, 0.24 * a) } else { argb(WHITE, if over { 0.10 } else { 0.04 } * a) };
            ui.rounded(r[0], r[1], r[2], r[3], 7.0, fill);
            if *on {
                ui.border(r[0], r[1], r[2], r[3], 7.0, argb(ACCENT, 0.55 * a));
            }
            ui.text(Face::Medium, 11.0, r[0] + 14.0, r[1] + (28.0 - 16.0) / 2.0, argb(WHITE, a), label);
            if !*on {
                hit(&mut self.hits, r, Hit::Settings);
            }
            tab_x += lw + 8.0;
        }
        let close = [px + pw - 18.0 - 32.0, py + (hh - 32.0) / 2.0, 32.0, 32.0];
        let over = inside(close, pointer);
        ui.rounded(close[0], close[1], close[2], close[3], 8.0, argb(if over { OFF } else { WHITE }, if over { 0.85 } else { 0.06 } * a));
        icon_at(ui, icon::X, close[0] + 16.0, close[1] + 16.0, argb(WHITE, a), false);
        hit(&mut self.hits, close, Hit::Close);
        ui.rounded(px + 14.0, py + hh, pw - 28.0, 1.0, 0.0, argb(WHITE, 0.08 * a));

        // The categories down the left, Edit HUD at their foot.
        let (lx, ly, lw) = (px + 14.0, py + hh + 12.0, 158.0);
        for (k, category) in CATEGORIES.iter().enumerate() {
            let r = [lx, ly + k as f32 * 36.0, lw, 32.0];
            let on = *category == self.category;
            let over = inside(r, pointer);
            if on {
                ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(ACCENT, 0.22 * a));
                ui.rounded(r[0], r[1] + 8.0, 3.0, 16.0, 1.5, argb(ACCENT, a));
            } else if over {
                ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(WHITE, 0.06 * a));
            }
            icon_at(ui, category.icon(), r[0] + 22.0, r[1] + 16.0, argb(WHITE, if on || over { 1.0 } else { 0.7 } * a), false);
            let colour = argb(WHITE, if on || over { 1.0 } else { 0.72 } * a);
            ui.text(Face::Medium, 12.0, r[0] + 44.0, r[1] + (32.0 - 17.0) / 2.0, colour, &category.name());
            hit(&mut self.hits, r, Hit::Category(*category));
        }
        let edit = [lx, py + ph - 14.0 - 36.0, lw, 36.0];
        let over = inside(edit, pointer);
        ui.rounded(edit[0], edit[1], edit[2], edit[3], 9.0, argb(ACCENT, if over { 0.95 } else { 0.75 } * a));
        icon_at(ui, icon::LAYOUT, edit[0] + 22.0, edit[1] + 18.0, argb(WHITE, a), false);
        let edit_label = lang::get_or("oteria.mods.edit_hud", "Edit HUD");
        ui.text(Face::Medium, 12.0, edit[0] + 44.0, edit[1] + (36.0 - 17.0) / 2.0, argb(WHITE, a), &edit_label);
        hit(&mut self.hits, edit, Hit::EditHud);

        // The search field, and the count of what shows.
        let ax = lx + lw + 14.0;
        let aw = px + pw - 14.0 - ax;
        let search = [ax + aw - 240.0, ly, 240.0, 32.0];
        ui.rounded(search[0], search[1], search[2], search[3], 9.0, argb(WHITE, if self.typing { 0.12 } else { 0.06 } * a));
        if self.typing {
            ui.border(search[0], search[1], search[2], search[3], 9.0, argb(ACCENT, 0.7 * a));
        }
        icon_at(ui, icon::SEARCH, search[0] + 18.0, search[1] + 16.0, argb(WHITE, 0.7 * a), false);
        self.blink = (self.blink + dt) % 1.0;
        let caret = if self.typing && self.blink < 0.55 { "|" } else { "" };
        let (text, colour) = if self.search.is_empty() && !self.typing {
            (lang::get_or("oteria.mods.search", "Search…"), argb(WHITE, 0.45 * a))
        } else {
            (format!("{}{caret}", self.search), argb(WHITE, a))
        };
        ui.text(Face::Regular, 12.0, search[0] + 36.0, search[1] + (32.0 - 17.0) / 2.0, colour, &text);
        hit(&mut self.hits, search, Hit::Search);
        let shown = self.shown();
        let count = format!("{} mods", shown.len());
        ui.text(Face::Medium, 12.0, ax + 2.0, ly + (32.0 - 17.0) / 2.0, argb(WHITE, 0.55 * a), &count);

        // The grid: as many columns as fit, scrolled smoothly, clipped to its view.
        let (gx, gy) = (ax, ly + 44.0);
        let gh = py + ph - 14.0 - gy;
        let gap = 10.0;
        let cols = (((aw + gap) / (176.0 + gap)).floor() as usize).clamp(2, 5);
        let tw = (aw - gap * (cols - 1) as f32) / cols as f32;
        let th = 116.0;
        let rows = shown.len().div_ceil(cols);
        let content = rows as f32 * (th + gap) - gap;
        self.room = (content - gh).max(0.0);
        self.target = self.target.clamp(0.0, self.room);
        self.scroll += (self.target - self.scroll) * (1.0 - (-dt * 18.0).exp());
        let start = ui.batch.vertices.len();
        for (n, &i) in shown.iter().enumerate() {
            let (col, row) = (n % cols, n / cols);
            let r = [gx + col as f32 * (tw + gap), gy + row as f32 * (th + gap) - self.scroll, tw, th];
            if r[1] + th < gy || r[1] > gy + gh {
                continue;
            }
            let tile = &TILES[i];
            let on = (tile.get)(extras);
            let over = inside(r, pointer) && pointer.1 >= gy && pointer.1 < gy + gh;
            self.hover[i] += ((if over { 1.0 } else { 0.0 }) - self.hover[i]) * follow;
            let sw = self.switch[i].get_or_insert(if on { 1.0 } else { 0.0 });
            *sw += ((if on { 1.0 } else { 0.0 }) - *sw) * (1.0 - (-dt * 14.0).exp());
            let (hv, sv) = (self.hover[i], *sw);
            ui.rounded(r[0], r[1], r[2], r[3], 10.0, argb(WHITE, (0.05 + 0.04 * hv) * a));
            ui.border(r[0], r[1], r[2], r[3], 10.0, argb(WHITE, (0.07 + 0.14 * hv) * a));
            let cx = r[0] + tw / 2.0;
            icon_at(ui, tile.icon, cx, r[1] + 30.0, argb(WHITE, (0.75 + 0.25 * hv) * a), true);
            let name = tile.name();
            let nw = ui.measure(Face::Medium, 13.0, &name);
            ui.text(Face::Medium, 13.0, cx - nw / 2.0, r[1] + 50.0, argb(WHITE, (0.82 + 0.18 * hv) * a), &name);
            let by = r[1] + th - 10.0 - 30.0;
            let gear = [r[0] + 10.0, by, 30.0, 30.0];
            let gear_over = over && inside(gear, pointer);
            ui.rounded(gear[0], gear[1], gear[2], gear[3], 7.0, argb(WHITE, if gear_over { 0.16 } else { 0.07 } * a));
            icon_at(ui, icon::SETTINGS, gear[0] + 15.0, gear[1] + 15.0, argb(WHITE, 0.85 * a), false);
            let bar = [gear[0] + 36.0, by, r[0] + tw - 10.0 - (gear[0] + 36.0), 30.0];
            let bar_over = over && inside(bar, pointer);
            let colour = mix(OFF, ON, sv);
            ui.rounded(bar[0], bar[1], bar[2], bar[3], 7.0, argb(colour, (if bar_over { 0.95 } else { 0.80 }) * a));
            let label = if on {
                lang::get_or("oteria.mods.enabled", "ENABLED")
            } else {
                lang::get_or("oteria.mods.disabled", "DISABLED")
            };
            let lw = ui.measure(Face::Medium, 11.0, &label);
            ui.text(Face::Medium, 11.0, bar[0] + (bar[2] - lw) / 2.0, bar[1] + (30.0 - 16.0) / 2.0, argb(WHITE, a), &label);
            if r[1] >= gy - th && r[1] <= gy + gh {
                let visible = |b: [f32; 4]| [b[0], b[1].max(gy), b[2], (b[1] + b[3]).min(gy + gh) - b[1].max(gy)];
                hit(&mut self.hits, visible(bar), Hit::Toggle(i));
                // Named for the tools that drive the game (`horizon-ctl click`).
                horizon_client::widgets::record(tile.key, visible(bar).map(gui), true);
                hit(&mut self.hits, visible(gear), Hit::Gear(i));
                // A press anywhere else on the tile turns it on or off too.
                let body = visible([r[0], r[1], r[2], by - r[1]]);
                if body[3] > 0.0 {
                    hit(&mut self.hits, body, Hit::Toggle(i));
                }
            }
        }
        ui.batch.clip_since(start, gui(gx - 2.0), gui(gy), gui(gx + aw + 2.0), gui(gy + gh));
        if shown.is_empty() {
            let none = lang::get_or("oteria.mods.none", "No mod matches.");
            let nw = ui.measure(Face::Regular, 13.0, &none);
            ui.text(Face::Regular, 13.0, gx + (aw - nw) / 2.0, gy + 40.0, argb(WHITE, 0.55 * a), &none);
        }
        // A thin scrollbar when the grid goes past its view.
        if self.room > 0.0 {
            let track = gh;
            let thumb = (gh / content * track).max(24.0);
            let ty = gy + (self.scroll / self.room) * (track - thumb);
            ui.rounded(px + pw - 9.0, ty, 3.0, thumb, 1.5, argb(WHITE, 0.25 * a));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_turns_its_own_feature_on_and_off() {
        for (i, tile) in TILES.iter().enumerate() {
            let mut e = Extras::default();
            (tile.set)(&mut e, true);
            assert!((tile.get)(&e), "{} turns on", tile.english);
            // It is its own switch: the other tiles read what they did before.
            for (j, other) in TILES.iter().enumerate() {
                if j != i && other.key != tile.key {
                    let fresh = Extras::default();
                    let mut only = Extras::default();
                    (tile.set)(&mut only, true);
                    if (other.get)(&only) != (other.get)(&fresh) {
                        panic!("{} also changed {}", tile.english, other.english);
                    }
                }
            }
            (tile.set)(&mut e, false);
            assert!(!(tile.get)(&e), "{} turns off", tile.english);
        }
    }

    #[test]
    fn the_filters_and_the_search_narrow_the_grid() {
        let mut m = Mods::default();
        assert_eq!(m.shown().len(), TILES.len());
        m.category = Category::Hud;
        assert!(m.shown().iter().all(|&i| TILES[i].category == Category::Hud));
        m.category = Category::All;
        m.search = "fps".into();
        assert!(m.shown().iter().any(|&i| TILES[i].english == "FPS"));
        m.search = "zzzz".into();
        assert!(m.shown().is_empty());
    }

    #[test]
    fn typing_goes_to_the_search_only_while_it_has_the_keyboard() {
        let mut m = Mods::default();
        assert!(!m.type_key(Some("a"), false));
        m.typing = true;
        assert!(m.type_key(Some("ab"), false));
        assert!(m.type_key(None, true));
        assert_eq!(m.search, "a");
    }

    #[test]
    fn colours_mix_from_off_to_on() {
        assert_eq!(mix(OFF, ON, 0.0), OFF);
        assert_eq!(mix(OFF, ON, 1.0), ON);
    }
}
