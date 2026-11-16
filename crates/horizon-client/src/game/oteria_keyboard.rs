//! The Keyboard page of Oteria's menu (Controls' Keyboard button, or the menu's own): every key
//! binding on a keyboard drawn as one, in the 60%, 75% or 100% layout. A key is green with one
//! action on it, red when actions share it (a possible conflict), grey when nothing is on it;
//! hovered, it rises and says what is on it. Clicked, it is the key the editor below works on:
//! the actions on it, or all of them (searched), and for the action picked, Assign here, Press a
//! key (a key or a mouse button), Unbind and Reset. Every change saves at once.
//!
//! Written for Oteria after the idea of the Studious Keybinds mod; none of its code or text.

use crate::ui_theme::{Face, Ui};
use horizon_client::keybinds;
use horizon_client::lang;
use horizon_client::name_badge;
use horizon_client::oteria_icons as icon;

const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const OFF: u32 = 0xB3_3346;
const GREEN: u32 = 0x3D_D68C;
const RED: u32 = 0xE5_484D;
const UNBOUND: &str = "key.keyboard.unknown";

fn argb(rgb: u32, alpha: f32) -> u32 {
    super::oteria_menu::argb(rgb, alpha)
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
fn tr(key: &str, english: &str) -> String {
    lang::get_or(key, english)
}
fn ease(v: &mut f32, to: f32, dt: f32) {
    *v += (to - *v) * (1.0 - (-dt * 16.0).exp());
}

/// A binding as the page shows it.
#[derive(Clone, Debug)]
pub(super) struct Bind {
    /// The mapping (`key.jump`).
    pub action: &'static str,
    /// The key or button it is on (`key.keyboard.space`).
    pub key: String,
    pub category: &'static str,
}

/// What the page shows: every binding, and the layout chosen (60, 75 or 100).
pub(super) struct KeyboardView {
    pub binds: Vec<Bind>,
    pub layout: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum KeyboardAction {
    Layout(u8),
    /// This action on this key (or mouse button).
    Assign(&'static str, String),
    Unbind(&'static str),
    Reset(&'static str),
    Close,
}

/// A keycap: its label, its key's name, and its place in key units (x, y, width, height).
#[derive(Clone, Copy, Debug)]
struct Cap {
    label: &'static str,
    name: &'static str,
    at: [f32; 4],
}

/// The keyboard of `layout` (60, 75 or 100), and its size in key units.
fn layout(layout: u8) -> (Vec<Cap>, f32, f32) {
    let mut caps = Vec::new();
    let row = |caps: &mut Vec<Cap>, y: f32, x0: f32, keys: &[(&'static str, &'static str, f32)]| {
        let mut x = x0;
        for &(label, name, w) in keys {
            if !name.is_empty() {
                caps.push(Cap {
                    label,
                    name,
                    at: [x, y, w, 1.0],
                });
            }
            x += w;
        }
    };
    let k = |c: &'static str| -> &'static str {
        match c {
            "a" => "key.keyboard.a",
            "b" => "key.keyboard.b",
            "c" => "key.keyboard.c",
            "d" => "key.keyboard.d",
            "e" => "key.keyboard.e",
            "f" => "key.keyboard.f",
            "g" => "key.keyboard.g",
            "h" => "key.keyboard.h",
            "i" => "key.keyboard.i",
            "j" => "key.keyboard.j",
            "k" => "key.keyboard.k",
            "l" => "key.keyboard.l",
            "m" => "key.keyboard.m",
            "n" => "key.keyboard.n",
            "o" => "key.keyboard.o",
            "p" => "key.keyboard.p",
            "q" => "key.keyboard.q",
            "r" => "key.keyboard.r",
            "s" => "key.keyboard.s",
            "t" => "key.keyboard.t",
            "u" => "key.keyboard.u",
            "v" => "key.keyboard.v",
            "w" => "key.keyboard.w",
            "x" => "key.keyboard.x",
            "y" => "key.keyboard.y",
            "z" => "key.keyboard.z",
            "1" => "key.keyboard.1",
            "2" => "key.keyboard.2",
            "3" => "key.keyboard.3",
            "4" => "key.keyboard.4",
            "5" => "key.keyboard.5",
            "6" => "key.keyboard.6",
            "7" => "key.keyboard.7",
            "8" => "key.keyboard.8",
            "9" => "key.keyboard.9",
            "0" => "key.keyboard.0",
            _ => "",
        }
    };
    let letters = |s: &'static str| -> Vec<(&'static str, &'static str, f32)> {
        // Each character of `s` as a 1u key; its label is drawn from the key's own name.
        (0..s.len()).map(|i| ("", k(&s[i..=i]), 1.0)).collect()
    };
    let top = if layout == 60 { 0.0 } else { 1.25 };
    if layout != 60 {
        row(&mut caps, 0.0, 0.0, &[("Esc", "key.keyboard.escape", 1.0)]);
        let f = |n: usize| -> (&'static str, &'static str, f32) {
            const F: [(&str, &str); 12] = [
                ("F1", "key.keyboard.f1"),
                ("F2", "key.keyboard.f2"),
                ("F3", "key.keyboard.f3"),
                ("F4", "key.keyboard.f4"),
                ("F5", "key.keyboard.f5"),
                ("F6", "key.keyboard.f6"),
                ("F7", "key.keyboard.f7"),
                ("F8", "key.keyboard.f8"),
                ("F9", "key.keyboard.f9"),
                ("F10", "key.keyboard.f10"),
                ("F11", "key.keyboard.f11"),
                ("F12", "key.keyboard.f12"),
            ];
            (F[n].0, F[n].1, 1.0)
        };
        row(&mut caps, 0.0, 2.0, &[f(0), f(1), f(2), f(3)]);
        row(&mut caps, 0.0, 6.5, &[f(4), f(5), f(6), f(7)]);
        row(&mut caps, 0.0, 11.0, &[f(8), f(9), f(10), f(11)]);
        row(
            &mut caps,
            0.0,
            15.25,
            &[
                ("PrtSc", "key.keyboard.print.screen", 1.0),
                ("ScrLk", "key.keyboard.scroll.lock", 1.0),
                ("Pause", "key.keyboard.pause", 1.0),
            ],
        );
    } else {
        row(&mut caps, top, 0.0, &[("Esc", "key.keyboard.escape", 1.0)]);
    }
    // The main block.
    let mut r1 = vec![];
    if layout != 60 {
        r1.push(("`", "key.keyboard.grave.accent", 1.0));
    }
    r1.extend(letters("1234567890"));
    r1.extend([
        ("-", "key.keyboard.minus", 1.0),
        ("=", "key.keyboard.equal", 1.0),
        ("Backspace", "key.keyboard.backspace", 2.0),
    ]);
    row(&mut caps, top, if layout == 60 { 1.0 } else { 0.0 }, &r1);
    let mut r2 = vec![("Tab", "key.keyboard.tab", 1.5)];
    r2.extend(letters("qwertyuiop"));
    r2.extend([
        ("[", "key.keyboard.left.bracket", 1.0),
        ("]", "key.keyboard.right.bracket", 1.0),
        ("\\", "key.keyboard.backslash", 1.5),
    ]);
    row(&mut caps, top + 1.0, 0.0, &r2);
    let mut r3 = vec![("Caps", "key.keyboard.caps.lock", 1.75)];
    r3.extend(letters("asdfghjkl"));
    r3.extend([
        (";", "key.keyboard.semicolon", 1.0),
        ("'", "key.keyboard.apostrophe", 1.0),
        ("Enter", "key.keyboard.enter", 2.25),
    ]);
    row(&mut caps, top + 2.0, 0.0, &r3);
    let mut r4 = vec![("Shift", "key.keyboard.left.shift", 2.25)];
    r4.extend(letters("zxcvbnm"));
    r4.extend([
        (",", "key.keyboard.comma", 1.0),
        (".", "key.keyboard.period", 1.0),
        ("/", "key.keyboard.slash", 1.0),
        ("Shift", "key.keyboard.right.shift", 2.75),
    ]);
    row(&mut caps, top + 3.0, 0.0, &r4);
    row(
        &mut caps,
        top + 4.0,
        0.0,
        &[
            ("Ctrl", "key.keyboard.left.control", 1.25),
            ("Win", "key.keyboard.left.win", 1.25),
            ("Alt", "key.keyboard.left.alt", 1.25),
            ("Space", "key.keyboard.space", 6.25),
            ("AltGr", "key.keyboard.right.alt", 1.25),
            ("Win", "key.keyboard.right.win", 1.25),
            ("Menu", "key.keyboard.menu", 1.25),
            ("Ctrl", "key.keyboard.right.control", 1.25),
        ],
    );
    if layout == 60 {
        // The grave accent sits where Escape does on a 60%: Escape is its own key here, on the left.
        return (caps, 15.0, 5.0);
    }
    // Navigation and arrows.
    row(
        &mut caps,
        top,
        15.25,
        &[
            ("Ins", "key.keyboard.insert", 1.0),
            ("Home", "key.keyboard.home", 1.0),
            ("PgUp", "key.keyboard.page.up", 1.0),
        ],
    );
    row(
        &mut caps,
        top + 1.0,
        15.25,
        &[
            ("Del", "key.keyboard.delete", 1.0),
            ("End", "key.keyboard.end", 1.0),
            ("PgDn", "key.keyboard.page.down", 1.0),
        ],
    );
    row(
        &mut caps,
        top + 3.0,
        16.25,
        &[("\u{2191}", "key.keyboard.up", 1.0)],
    );
    row(
        &mut caps,
        top + 4.0,
        15.25,
        &[
            ("\u{2190}", "key.keyboard.left", 1.0),
            ("\u{2193}", "key.keyboard.down", 1.0),
            ("\u{2192}", "key.keyboard.right", 1.0),
        ],
    );
    if layout == 75 {
        return (caps, 18.25, top + 5.0);
    }
    // The number pad.
    let x = 18.5;
    row(
        &mut caps,
        top,
        x,
        &[
            ("Num", "key.keyboard.num.lock", 1.0),
            ("/", "key.keyboard.keypad.divide", 1.0),
            ("*", "key.keyboard.keypad.multiply", 1.0),
            ("-", "key.keyboard.keypad.subtract", 1.0),
        ],
    );
    row(
        &mut caps,
        top + 1.0,
        x,
        &[
            ("7", "key.keyboard.keypad.7", 1.0),
            ("8", "key.keyboard.keypad.8", 1.0),
            ("9", "key.keyboard.keypad.9", 1.0),
        ],
    );
    row(
        &mut caps,
        top + 2.0,
        x,
        &[
            ("4", "key.keyboard.keypad.4", 1.0),
            ("5", "key.keyboard.keypad.5", 1.0),
            ("6", "key.keyboard.keypad.6", 1.0),
        ],
    );
    row(
        &mut caps,
        top + 3.0,
        x,
        &[
            ("1", "key.keyboard.keypad.1", 1.0),
            ("2", "key.keyboard.keypad.2", 1.0),
            ("3", "key.keyboard.keypad.3", 1.0),
        ],
    );
    row(
        &mut caps,
        top + 4.0,
        x,
        &[
            ("0", "key.keyboard.keypad.0", 2.0),
            (".", "key.keyboard.keypad.period", 1.0),
        ],
    );
    caps.push(Cap {
        label: "+",
        name: "key.keyboard.keypad.add",
        at: [x + 3.0, top + 1.0, 1.0, 2.0],
    });
    caps.push(Cap {
        label: "Enter",
        name: "key.keyboard.keypad.enter",
        at: [x + 3.0, top + 3.0, 1.0, 2.0],
    });
    (caps, 22.5, top + 5.0)
}

/// The mouse's buttons, as a row of caps under the keyboard.
const MOUSE: [(&str, &str, &str); 5] = [
    ("oteria.keyboard.mouse.left", "Left", "key.mouse.left"),
    ("oteria.keyboard.mouse.right", "Right", "key.mouse.right"),
    ("oteria.keyboard.mouse.middle", "Middle", "key.mouse.middle"),
    ("oteria.keyboard.mouse.4", "Button 4", "key.mouse.4"),
    ("oteria.keyboard.mouse.5", "Button 5", "key.mouse.5"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    OnKey,
    All,
}

#[derive(Clone, Debug)]
enum Hit {
    Action(KeyboardAction),
    /// A key (or mouse button) of the keyboard: the one the editor works on.
    Key(String),
    Tab(Tab),
    Search,
    /// An action of the list: the one picked.
    Row(&'static str),
    /// Press a key: the next key or button pressed goes on the picked action.
    Capture,
}

#[derive(Default)]
pub(super) struct KeyboardPage {
    hits: Vec<([f32; 4], Hit)>,
    /// The key the editor works on.
    key: Option<String>,
    /// The action picked in the list.
    picked: Option<&'static str>,
    tab: Tab,
    search: String,
    typing: bool,
    /// Waiting for a key (or button) for the picked action.
    capturing: bool,
    /// The list's scroll (rows) and the eased one.
    scroll: f32,
    shown_scroll: f32,
    rows: usize,
    room: usize,
    /// Each cap's rise, by name.
    rise: std::collections::HashMap<String, f32>,
    blink: f32,
    /// Opened from Controls: closed, the options come back there.
    pub(super) from_options: bool,
}

impl KeyboardPage {
    pub(super) fn new(from_options: bool) -> Self {
        Self {
            from_options,
            ..Self::default()
        }
    }

    /// The keyboard takes the keys while the search field has them or a key is awaited.
    pub(super) fn wants_keys(&self) -> bool {
        self.typing || self.capturing
    }
    /// Escape: out of waiting for a key, then of the search. False when there was neither.
    pub(super) fn back(&mut self) -> bool {
        if self.capturing {
            self.capturing = false;
        } else if self.typing {
            self.typing = false;
        } else {
            return false;
        }
        true
    }
    pub(super) fn wheel(&mut self, lines: f32) {
        let most = self.rows.saturating_sub(self.room) as f32;
        self.scroll = (self.scroll - lines * 2.0).clamp(0.0, most);
    }

    /// A key while the page has the keyboard: the key awaited (its name), or the search typed.
    pub(super) fn key(
        &mut self,
        name: Option<&'static str>,
        text: Option<&str>,
        backspace: bool,
        enter: bool,
    ) -> Option<KeyboardAction> {
        if self.capturing {
            let (Some(action), Some(name)) = (self.picked, name) else {
                return None;
            };
            self.capturing = false;
            self.key = Some(name.to_owned());
            return Some(KeyboardAction::Assign(action, name.to_owned()));
        }
        if enter {
            self.typing = false;
        } else if backspace {
            self.search.pop();
        } else if let Some(text) = text {
            let room = 40usize.saturating_sub(self.search.chars().count());
            self.search
                .extend(text.chars().filter(|c| !c.is_control()).take(room));
        }
        self.scroll = 0.0;
        None
    }

    /// A mouse button pressed (`key.mouse.*`): the button awaited, else a click at `cursor`.
    pub(super) fn press(&mut self, button: &str, cursor: (f32, f32)) -> Option<KeyboardAction> {
        if self.capturing {
            let action = self.picked?;
            self.capturing = false;
            self.key = Some(button.to_owned());
            return Some(KeyboardAction::Assign(action, button.to_owned()));
        }
        if button != "key.mouse.left" {
            return None;
        }
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| inside(*r, cursor))
            .map(|h| h.1.clone());
        self.typing = matches!(hit, Some(Hit::Search));
        match hit? {
            Hit::Action(a) => Some(a),
            Hit::Key(name) => {
                self.key = Some(name);
                self.tab = Tab::OnKey;
                self.scroll = 0.0;
                None
            }
            Hit::Tab(t) => {
                self.tab = t;
                self.scroll = 0.0;
                None
            }
            Hit::Search => {
                self.tab = Tab::All;
                None
            }
            Hit::Row(action) => {
                self.picked = Some(action);
                None
            }
            Hit::Capture => {
                self.capturing = self.picked.is_some();
                None
            }
        }
    }

    fn push(&mut self, r: [f32; 4], hit: Hit, gui: &dyn Fn(f32) -> f32, pointer: (f32, f32)) {
        horizon_client::widgets::widget_cursor(inside(r, pointer), true);
        self.hits.push((r.map(gui), hit));
    }

    /// A button: `colour` filled (brighter hovered; dim and dead when not `live`).
    #[allow(clippy::too_many_arguments)]
    fn button(
        &mut self,
        ui: &mut Ui,
        gui: &dyn Fn(f32) -> f32,
        r: [f32; 4],
        label: &str,
        colour: u32,
        live: bool,
        a: f32,
        hit: Hit,
    ) {
        let over = live && inside(r, ui.cursor);
        let fill = if !live {
            0.12
        } else if over {
            0.95
        } else {
            0.75
        };
        ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(colour, fill * a));
        ui.text_centered(
            Face::Medium,
            11.0,
            r[0] + r[2] / 2.0,
            r[1] + (r[3] - 16.0) / 2.0,
            argb(WHITE, if live { 1.0 } else { 0.4 } * a),
            label,
        );
        if live {
            let pointer = ui.cursor;
            self.push(r, hit, gui, pointer);
        }
    }

    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        view: &KeyboardView,
        alpha: f32,
        rise: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        self.hits.clear();
        self.blink = (self.blink + dt) % 1.0;
        self.shown_scroll += (self.scroll - self.shown_scroll) * (1.0 - (-dt * 18.0).exp());
        let a = alpha;
        let pointer = ui.cursor;
        let d = ui.density;
        let pw = (ui.w - 48.0).min(1100.0);
        let ph = (ui.h - 48.0).min(700.0);
        let (px, py) = ((ui.w - pw) / 2.0, (ui.h - ph) / 2.0 + rise);
        ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.86 * a));
        ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * a));

        // Which actions sit on each key.
        let mut on: std::collections::HashMap<&str, Vec<&Bind>> = std::collections::HashMap::new();
        for b in &view.binds {
            if b.key != UNBOUND {
                on.entry(b.key.as_str()).or_default().push(b);
            }
        }
        let shared = on.values().filter(|v| clashing(v) > 1).count();

        // The header: logo, name, the page's tab, the layouts, close.
        let hh = 50.0;
        if let Some(r) = ui.atlas.sprites.get(name_badge::SPRITE).copied() {
            let s = 32.0 / d;
            ui.batch.rect_uv(
                gui(px + 18.0),
                gui(py + (hh - s) / 2.0),
                gui(s),
                gui(s),
                r,
                argb(WHITE, a),
            );
        }
        let tx = px + 18.0 + 32.0 / d + 8.0;
        let bw = ui.measure(Face::SemiBold, 14.0, "OTERIA");
        ui.text(
            Face::SemiBold,
            14.0,
            tx,
            py + (hh - 20.0) / 2.0,
            argb(WHITE, a),
            "OTERIA",
        );
        ui.text(
            Face::Regular,
            14.0,
            tx + bw,
            py + (hh - 20.0) / 2.0,
            argb(WHITE, 0.75 * a),
            " CLIENT",
        );
        let tab = tr("oteria.keyboard.tab", "KEYBOARD");
        let lw = ui.measure(Face::Medium, 11.0, &tab) + 28.0;
        let t = [px + 200.0, py + (hh - 28.0) / 2.0, lw, 28.0];
        ui.rounded(t[0], t[1], t[2], t[3], 7.0, argb(ACCENT, 0.24 * a));
        ui.border(t[0], t[1], t[2], t[3], 7.0, argb(ACCENT, 0.55 * a));
        ui.text(
            Face::Medium,
            11.0,
            t[0] + 14.0,
            t[1] + (28.0 - 16.0) / 2.0,
            argb(WHITE, a),
            &tab,
        );
        let mut lx = t[0] + lw + 20.0;
        for size in [60u8, 75, 100] {
            let label = format!("{size}%");
            let r = [lx, t[1], 52.0, 28.0];
            let chosen = view.layout == size;
            let over = inside(r, pointer);
            ui.rounded(
                r[0],
                r[1],
                r[2],
                r[3],
                7.0,
                argb(
                    if chosen { ACCENT } else { WHITE },
                    if chosen {
                        0.3
                    } else if over {
                        0.12
                    } else {
                        0.05
                    } * a,
                ),
            );
            ui.text_centered(
                Face::Medium,
                11.0,
                r[0] + 26.0,
                r[1] + 6.0,
                argb(WHITE, a),
                &label,
            );
            self.push(r, Hit::Action(KeyboardAction::Layout(size)), gui, pointer);
            horizon_client::widgets::record(&format!("oteria-keyboard-{size}"), r.map(gui), true);
            lx += 58.0;
        }
        let close = [px + pw - 18.0 - 32.0, py + (hh - 32.0) / 2.0, 32.0, 32.0];
        let over = inside(close, pointer);
        ui.rounded(
            close[0],
            close[1],
            close[2],
            close[3],
            8.0,
            argb(
                if over { OFF } else { WHITE },
                if over { 0.85 } else { 0.06 } * a,
            ),
        );
        if let Some(r) = ui.atlas.sprites.get(icon::small(icon::X).as_str()).copied() {
            let s = 20.0 / d;
            ui.batch.rect_uv(
                gui(close[0] + 16.0 - s / 2.0),
                gui(close[1] + 16.0 - s / 2.0),
                gui(s),
                gui(s),
                r,
                argb(WHITE, a),
            );
        }
        self.push(close, Hit::Action(KeyboardAction::Close), gui, pointer);
        horizon_client::widgets::record("oteria-keyboard-close", close.map(gui), true);
        ui.rounded(
            px + 14.0,
            py + hh,
            pw - 28.0,
            1.0,
            0.0,
            argb(WHITE, 0.08 * a),
        );

        // The keyboard, as wide as it can be within the panel and 45% of its height.
        let (caps, kw, kh) = layout(view.layout);
        let area = [px + 24.0, py + hh + 16.0, pw - 48.0, ph * 0.42];
        let unit = (area[2] / kw).min(area[3] / kh).min(46.0);
        let ox = area[0] + (area[2] - kw * unit) / 2.0;
        let oy = area[1];
        let mut hovered: Option<(Cap, [f32; 4])> = None;
        let draw_cap = |me: &mut Self, ui: &mut Ui, label: String, name: &str, r: [f32; 4]| {
            let over = inside(r, pointer);
            let chosen = me.key.as_deref() == Some(name);
            let lift = me.rise.entry(name.to_owned()).or_insert(0.0);
            ease(lift, if over || chosen { 1.0 } else { 0.0 }, dt);
            let l = *lift;
            let n = on.get(name).map_or(0, |v| v.len());
            let colour = match (n, on.get(name).map_or(0, |v| clashing(v))) {
                (0, _) => WHITE,
                (_, 0 | 1) => GREEN,
                _ => RED,
            };
            let fill = if n == 0 { 0.06 } else { 0.30 };
            let [x, y, w, h] = [r[0] + 2.0, r[1] + 2.0 - 2.0 * l, r[2] - 4.0, r[3] - 4.0];
            // Its side, under it, then its top.
            ui.rounded(x, y + 3.0, w, h, 6.0, argb(0x00_0000, 0.35 * a));
            ui.rounded(x, y, w, h, 6.0, argb(colour, (fill + 0.12 * l) * a));
            ui.border(
                x,
                y,
                w,
                h,
                6.0,
                argb(
                    if chosen { ACCENT } else { colour },
                    if chosen { 0.95 } else { 0.25 + 0.3 * l } * a,
                ),
            );
            let size = if label.chars().count() > 3 { 9.0 } else { 11.0 };
            let mut text = label;
            while !text.is_empty() && ui.measure(Face::Medium, size, &text) > w - 4.0 {
                text.pop();
            }
            ui.text_centered(
                Face::Medium,
                size,
                x + w / 2.0,
                y + (h - size * 1.45) / 2.0,
                argb(WHITE, (0.65 + 0.35 * l) * a),
                &text,
            );
            me.push(r, Hit::Key(name.to_owned()), gui, pointer);
            over
        };
        for cap in &caps {
            let r = [
                ox + cap.at[0] * unit,
                oy + cap.at[1] * unit,
                cap.at[2] * unit,
                cap.at[3] * unit,
            ];
            let label = if cap.label.is_empty() {
                keybinds::display_name(cap.name).plain().to_uppercase()
            } else {
                cap.label.to_owned()
            };
            if draw_cap(self, ui, label, cap.name, r) {
                hovered = Some((*cap, r));
            }
        }
        // The mouse's buttons, under it.
        let my = oy + kh * unit + 10.0;
        let mouse_label = tr("oteria.keyboard.mouse", "Mouse");
        ui.text(
            Face::Medium,
            10.0,
            ox,
            my + (unit * 0.8 - 14.0) / 2.0,
            argb(WHITE, 0.55 * a),
            &mouse_label.to_uppercase(),
        );
        let mut mx = ox + ui.measure(Face::Medium, 10.0, &mouse_label.to_uppercase()) + 12.0;
        for (key, english, name) in MOUSE {
            let label = tr(key, english);
            let w = (ui.measure(Face::Medium, 10.0, &label) + 20.0).max(unit * 1.2);
            let r = [mx, my, w, unit * 0.8];
            if draw_cap(self, ui, label, name, r) {
                hovered = Some((
                    Cap {
                        label: english,
                        name,
                        at: [0.0; 4],
                    },
                    r,
                ));
            }
            mx += w + 4.0;
        }
        // The legend and the shared keys' count.
        let legend = tr(
            "oteria.keyboard.legend",
            "Green: one action   Red: shared (a possible conflict)   Grey: free",
        );
        let ly = my + unit * 0.8 + 10.0;
        ui.text(Face::Regular, 10.0, ox, ly, argb(WHITE, 0.5 * a), &legend);
        let count = match shared {
            1 => tr("oteria.keyboard.shared.one", "1 key shared"),
            n => lang::get_or("oteria.keyboard.shared", "%s keys shared")
                .replace("%s", &n.to_string()),
        };
        let cw = ui.measure(Face::Medium, 10.0, &count);
        ui.text(
            Face::Medium,
            10.0,
            ox + kw * unit - cw,
            ly,
            argb(if shared > 0 { RED } else { GREEN }, a),
            &count,
        );

        // The editor, under it: the list on the left, the picked action on the right.
        let ey = ly + 22.0;
        let eh = py + ph - 16.0 - ey;
        let (ex, ew) = (px + 24.0, pw - 48.0);
        let list_w = ew * 0.6;
        self.draw_list(ui, view, &on, [ex, ey, list_w, eh], a, gui);
        self.draw_detail(
            ui,
            view,
            [ex + list_w + 16.0, ey, ew - list_w - 16.0, eh],
            a,
            gui,
        );

        // Over everything: what is on the hovered key.
        if let Some((cap, r)) = hovered {
            let binds = on.get(cap.name).map(|v| v.as_slice()).unwrap_or(&[]);
            let title = keybinds::display_name(cap.name).plain();
            let mut lines: Vec<(String, u32)> = binds
                .iter()
                .take(4)
                .map(|b| (lang::get_or(b.action, b.action), WHITE))
                .collect();
            if binds.len() > 4 {
                lines.push((
                    lang::get_or("oteria.keyboard.more", "+%s more")
                        .replace("%s", &(binds.len() - 4).to_string()),
                    ACCENT,
                ));
            }
            if lines.is_empty() {
                lines.push((tr("oteria.keyboard.free", "Nothing on this key"), 0xAA_AAAA));
            } else if clashing(binds) > 1 {
                lines.push((
                    tr(
                        "oteria.keyboard.shared_warning",
                        "Shared: these may get in each other's way.",
                    ),
                    RED,
                ));
            }
            let w = lines
                .iter()
                .map(|(l, _)| ui.measure(Face::Regular, 11.0, l))
                .fold(ui.measure(Face::SemiBold, 12.0, &title), f32::max)
                + 24.0;
            let h = 30.0 + lines.len() as f32 * 16.0;
            let tx = (r[0] + r[2] / 2.0 - w / 2.0).clamp(px + 8.0, px + pw - 8.0 - w);
            let ty = if r[1] - h - 8.0 > py + hh {
                r[1] - h - 8.0
            } else {
                r[1] + r[3] + 8.0
            };
            ui.rounded(tx, ty, w, h, 9.0, argb(0x05_080C, 0.95 * a));
            ui.border(tx, ty, w, h, 9.0, argb(WHITE, 0.12 * a));
            ui.text(
                Face::SemiBold,
                12.0,
                tx + 12.0,
                ty + 8.0,
                argb(WHITE, a),
                &title,
            );
            for (k, (line, colour)) in lines.iter().enumerate() {
                ui.text(
                    Face::Regular,
                    11.0,
                    tx + 12.0,
                    ty + 26.0 + k as f32 * 16.0,
                    argb(*colour, 0.9 * a),
                    line,
                );
            }
        }
    }

    /// The list: on the chosen key, or every action (searched).
    #[allow(clippy::too_many_arguments)]
    fn draw_list(
        &mut self,
        ui: &mut Ui,
        view: &KeyboardView,
        on: &std::collections::HashMap<&str, Vec<&Bind>>,
        [x, y, w, h]: [f32; 4],
        a: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        let pointer = ui.cursor;
        ui.rounded(x, y, w, h, 12.0, argb(WHITE, 0.03 * a));
        // The tabs and the search.
        let here = self
            .key
            .as_ref()
            .and_then(|k| on.get(k.as_str()))
            .map_or(0, |v| v.len());
        let tabs = [
            (
                Tab::OnKey,
                lang::get_or("oteria.keyboard.on_key", "On this key (%s)")
                    .replace("%s", &here.to_string()),
            ),
            (Tab::All, tr("oteria.keyboard.all", "All actions")),
        ];
        let mut tx = x + 10.0;
        for (tab, label) in tabs {
            let lw = ui.measure(Face::Medium, 11.0, &label) + 24.0;
            let r = [tx, y + 10.0, lw, 28.0];
            let chosen = self.tab == tab;
            let over = inside(r, pointer);
            ui.rounded(
                r[0],
                r[1],
                r[2],
                r[3],
                7.0,
                argb(
                    if chosen { ACCENT } else { WHITE },
                    if chosen {
                        0.24
                    } else if over {
                        0.1
                    } else {
                        0.04
                    } * a,
                ),
            );
            ui.text(
                Face::Medium,
                11.0,
                r[0] + 12.0,
                r[1] + 6.0,
                argb(WHITE, a),
                &label,
            );
            self.push(r, Hit::Tab(tab), gui, pointer);
            tx += lw + 6.0;
        }
        let search = [tx + 6.0, y + 10.0, x + w - 10.0 - tx - 6.0, 28.0];
        ui.rounded(
            search[0],
            search[1],
            search[2],
            search[3],
            7.0,
            argb(WHITE, if self.typing { 0.12 } else { 0.06 } * a),
        );
        if self.typing {
            ui.border(
                search[0],
                search[1],
                search[2],
                search[3],
                7.0,
                argb(ACCENT, 0.7 * a),
            );
        }
        let (text, colour) = if self.search.is_empty() && !self.typing {
            (
                tr(
                    "oteria.keyboard.search",
                    "Search actions, categories or keys…",
                ),
                argb(WHITE, 0.4 * a),
            )
        } else {
            let caret = if self.typing && self.blink < 0.55 {
                "|"
            } else {
                ""
            };
            (format!("{}{caret}", self.search), argb(WHITE, a))
        };
        ui.text(
            Face::Regular,
            11.0,
            search[0] + 10.0,
            search[1] + 6.0,
            colour,
            &text,
        );
        self.push(search, Hit::Search, gui, pointer);
        horizon_client::widgets::record("oteria-keyboard-search", search.map(gui), true);

        // The rows.
        let query = self.search.trim().to_lowercase();
        let rows: Vec<&Bind> = match self.tab {
            Tab::OnKey => self
                .key
                .as_ref()
                .and_then(|k| on.get(k.as_str()))
                .cloned()
                .unwrap_or_default(),
            Tab::All => view
                .binds
                .iter()
                .filter(|b| {
                    query.is_empty()
                        || lang::get_or(b.action, b.action)
                            .to_lowercase()
                            .contains(&query)
                        || category_name(b.category).to_lowercase().contains(&query)
                        || keybinds::display_name(&b.key)
                            .plain()
                            .to_lowercase()
                            .contains(&query)
                })
                .collect(),
        };
        let top = y + 48.0;
        let row_h = 30.0;
        self.rows = rows.len();
        self.room = (((y + h - 8.0 - top) / row_h).floor().max(1.0)) as usize;
        if rows.is_empty() {
            let none = match self.tab {
                Tab::OnKey if self.key.is_none() => tr(
                    "oteria.keyboard.pick_key",
                    "Click a key of the keyboard to see what is on it.",
                ),
                Tab::OnKey => tr(
                    "oteria.keyboard.none_here",
                    "Nothing on this key. Pick an action in All actions to put it here.",
                ),
                Tab::All => tr("oteria.keyboard.no_results", "No matching actions"),
            };
            ui.text(
                Face::Regular,
                11.0,
                x + 14.0,
                top + 6.0,
                argb(WHITE, 0.45 * a),
                &none,
            );
            return;
        }
        let start = ui.batch.vertices.len();
        let first = self.shown_scroll;
        for (i, b) in rows.iter().enumerate() {
            let ry = top + (i as f32 - first) * row_h;
            if ry + row_h < top || ry > y + h - 8.0 {
                continue;
            }
            let r = [x + 8.0, ry, w - 16.0, row_h - 4.0];
            let picked = self.picked == Some(b.action);
            let over = inside(r, pointer) && pointer.1 >= top;
            ui.rounded(
                r[0],
                r[1],
                r[2],
                r[3],
                7.0,
                argb(
                    if picked { ACCENT } else { WHITE },
                    if picked {
                        0.2
                    } else if over {
                        0.08
                    } else {
                        0.0
                    } * a,
                ),
            );
            ui.text(
                Face::Medium,
                11.0,
                r[0] + 10.0,
                r[1] + 5.0,
                argb(WHITE, a),
                &lang::get_or(b.action, b.action),
            );
            let cat = category_name(b.category);
            let name_w = ui.measure(Face::Medium, 11.0, &lang::get_or(b.action, b.action));
            ui.text(
                Face::Regular,
                10.0,
                r[0] + 18.0 + name_w,
                r[1] + 6.0,
                argb(WHITE, 0.4 * a),
                &cat,
            );
            // Its key, as a cap at the right; red when it is shared.
            let key = if b.key == UNBOUND {
                tr("oteria.keyboard.unbound", "None")
            } else {
                keybinds::display_name(&b.key).plain()
            };
            let shared =
                b.category != "debug" && on.get(b.key.as_str()).is_some_and(|v| clashing(v) > 1);
            let kw = ui.measure(Face::Medium, 10.0, &key) + 14.0;
            let kr = [r[0] + r[2] - kw - 6.0, r[1] + 4.0, kw, r[3] - 8.0];
            ui.rounded(
                kr[0],
                kr[1],
                kr[2],
                kr[3],
                5.0,
                argb(
                    if shared { RED } else { WHITE },
                    if shared { 0.35 } else { 0.1 } * a,
                ),
            );
            ui.text_centered(
                Face::Medium,
                10.0,
                kr[0] + kw / 2.0,
                kr[1] + 2.0,
                argb(WHITE, a),
                &key,
            );
            if r[1] >= top {
                self.push(r, Hit::Row(b.action), gui, pointer);
            }
        }
        ui.batch
            .clip_since(start, gui(x), gui(top), gui(x + w), gui(y + h - 6.0));
    }

    /// The action picked: its name, its key, and what can be done to it.
    fn draw_detail(
        &mut self,
        ui: &mut Ui,
        view: &KeyboardView,
        [x, y, w, h]: [f32; 4],
        a: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        ui.rounded(x, y, w, h, 12.0, argb(WHITE, 0.03 * a));
        let picked = self
            .picked
            .and_then(|p| view.binds.iter().find(|b| b.action == p));
        let Some(b) = picked else {
            let hint = tr(
                "oteria.keyboard.pick_action",
                "Pick an action in the list to change its key.",
            );
            ui.text(
                Face::Regular,
                11.0,
                x + 14.0,
                y + 16.0,
                argb(WHITE, 0.45 * a),
                &hint,
            );
            return;
        };
        let action = b.action;
        ui.text(
            Face::SemiBold,
            14.0,
            x + 14.0,
            y + 12.0,
            argb(WHITE, a),
            &lang::get_or(action, action),
        );
        ui.text(
            Face::Regular,
            10.0,
            x + 14.0,
            y + 32.0,
            argb(WHITE, 0.45 * a),
            &category_name(b.category),
        );
        let now = if b.key == UNBOUND {
            tr("oteria.keyboard.unbound", "None")
        } else {
            keybinds::display_name(&b.key).plain()
        };
        let default = keybinds::KeyBinds::default_of(action);
        let default = if default == UNBOUND {
            tr("oteria.keyboard.unbound", "None")
        } else {
            keybinds::display_name(default).plain()
        };
        let line = lang::get_or("oteria.keyboard.key_is", "Key: %1 (default: %2)")
            .replace("%1", &now)
            .replace("%2", &default);
        ui.text(
            Face::Regular,
            11.0,
            x + 14.0,
            y + 52.0,
            argb(WHITE, 0.8 * a),
            &line,
        );
        let bw = w - 28.0;
        let mut by = y + 78.0;
        let target = self.key.clone();
        let assign = match &target {
            Some(k) => lang::get_or("oteria.keyboard.assign", "Put on %s")
                .replace("%s", &keybinds::display_name(k).plain()),
            None => tr("oteria.keyboard.assign_none", "Put on the key chosen"),
        };
        let can_assign = target.as_ref().is_some_and(|k| *k != b.key);
        let hit = Hit::Action(KeyboardAction::Assign(action, target.unwrap_or_default()));
        self.button(
            ui,
            gui,
            [x + 14.0, by, bw, 30.0],
            &assign,
            ACCENT,
            can_assign,
            a,
            hit,
        );
        by += 36.0;
        let label = if self.capturing {
            let caret = if self.blink < 0.55 { "…" } else { "" };
            format!(
                "{}{caret}",
                tr("oteria.keyboard.capturing", "Press a key or a mouse button")
            )
        } else {
            tr("oteria.keyboard.capture", "Press a new key")
        };
        self.button(
            ui,
            gui,
            [x + 14.0, by, bw, 30.0],
            &label,
            if self.capturing { GREEN } else { WHITE },
            true,
            a * if self.capturing { 1.0 } else { 0.6 },
            Hit::Capture,
        );
        by += 36.0;
        let half = (bw - 8.0) / 2.0;
        self.button(
            ui,
            gui,
            [x + 14.0, by, half, 30.0],
            &tr("oteria.keyboard.unbind", "Unbind"),
            OFF,
            b.key != UNBOUND,
            a,
            Hit::Action(KeyboardAction::Unbind(action)),
        );
        let reset_live = b.key != keybinds::KeyBinds::default_of(action);
        self.button(
            ui,
            gui,
            [x + 14.0 + half + 8.0, by, half, 30.0],
            &tr("oteria.keyboard.reset", "Reset"),
            WHITE,
            reset_live,
            a * 0.6,
            Hit::Action(KeyboardAction::Reset(action)),
        );
        by += 40.0;
        let saved = tr("oteria.keyboard.saved", "Changes save at once.");
        ui.text(
            Face::Regular,
            10.0,
            x + 14.0,
            by.min(y + h - 22.0),
            argb(WHITE, 0.4 * a),
            &saved,
        );
    }
}

/// How many of the actions on a key can get in each other's way: the debug ones are F3 combos
/// (`F3 + key`), which never clash with the key alone.
fn clashing(binds: &[&Bind]) -> usize {
    binds.iter().filter(|b| b.category != "debug").count()
}

/// A category's name, as the controls list shows it.
fn category_name(category: &str) -> String {
    lang::get_or(&format!("key.category.minecraft.{category}"), category)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_layout_has_its_keys_once_and_no_two_caps_overlap() {
        for size in [60u8, 75, 100] {
            let (caps, w, h) = layout(size);
            let mut names = std::collections::HashSet::new();
            for c in &caps {
                assert!(names.insert(c.name), "{size}%: {} twice", c.name);
                assert!(
                    keybinds::key_code(c.name).is_some(),
                    "{size}%: {} is no key",
                    c.name
                );
                assert!(
                    c.at[0] + c.at[2] <= w + 1e-4 && c.at[1] + c.at[3] <= h + 1e-4,
                    "{size}%: {} out",
                    c.name
                );
            }
            for (i, p) in caps.iter().enumerate() {
                for q in &caps[i + 1..] {
                    let apart = p.at[0] + p.at[2] <= q.at[0] + 1e-4
                        || q.at[0] + q.at[2] <= p.at[0] + 1e-4
                        || p.at[1] + p.at[3] <= q.at[1] + 1e-4
                        || q.at[1] + q.at[3] <= p.at[1] + 1e-4;
                    assert!(apart, "{size}%: {} on {}", p.name, q.name);
                }
            }
            let has = |n: &str| names.contains(n);
            assert_eq!(has("key.keyboard.f1"), size != 60);
            assert_eq!(has("key.keyboard.up"), size != 60);
            assert_eq!(has("key.keyboard.keypad.5"), size == 100);
        }
        assert_eq!(layout(100).0.len(), 104);
    }

    #[test]
    fn a_key_awaited_goes_on_the_picked_action() {
        let mut p = KeyboardPage {
            picked: Some("key.jump"),
            capturing: true,
            ..KeyboardPage::default()
        };
        assert!(p.wants_keys());
        assert_eq!(
            p.key(Some("key.keyboard.g"), Some("g"), false, false),
            Some(KeyboardAction::Assign("key.jump", "key.keyboard.g".into()))
        );
        assert!(!p.capturing && p.key.as_deref() == Some("key.keyboard.g"));
        // A mouse button too.
        p.capturing = true;
        assert_eq!(
            p.press("key.mouse.4", (0.0, 0.0)),
            Some(KeyboardAction::Assign("key.jump", "key.mouse.4".into()))
        );
        // Escape: nothing awaited any more.
        p.capturing = true;
        assert!(p.back() && !p.capturing);
    }
}
