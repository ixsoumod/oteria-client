//! The voice chat's menu (V in game), laid out as Simple Voice Chat's and drawn in Oteria's
//! style: the main page (Settings, Group, and the microphone, sound and icons switches), the
//! groups (join one, with its password if it has one, or make one), the group's members, the
//! settings (volumes, microphone, noise suppression, activation and its level with a live
//! meter, a microphone test, 3D audio), each player's volume, and the devices.
//!
//! It is a page of Oteria's menu (`oteria_menu`): it fades in over the game like the mods page,
//! and Escape steps back a page, then closes.

use super::oteria_voice::proto::{Group, GroupKind};
use crate::ui_theme::{Face, Ui};
use horizon_client::lang;
use horizon_client::oteria_icons as icon;
use horizon_client::pvp::Opt;

const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const ON: u32 = 0x23_A35C;
const OFF: u32 = 0xB3_3346;

fn argb(rgb: u32, alpha: f32) -> u32 {
    super::oteria_menu::argb(rgb, alpha)
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
fn tr(key: &str, english: &str) -> String {
    lang::get_or(key, english)
}

/// The connection, as the menu says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    /// This server runs no voice chat (or it is off).
    Unavailable,
    Connecting,
    Connected,
    NoAudio,
}

/// A player as the menu shows them.
#[derive(Clone, Debug)]
pub(super) struct Player {
    pub uuid: u128,
    pub name: String,
    pub face: Option<String>,
    pub talking: bool,
    pub sound_off: bool,
    pub volume: u8,
}

/// What the menu shows, read from the game each frame (`Game::voice_view`).
pub(super) struct VoiceView {
    pub status: Status,
    pub muted: bool,
    pub deafened: bool,
    pub hide_icons: bool,
    pub groups: Vec<Group>,
    pub group: Option<Group>,
    pub wrong_password: bool,
    pub members: Vec<Player>,
    pub others: Vec<Player>,
    /// Our microphone's level (dB).
    pub level: f32,
    pub loopback: bool,
    pub mics: Vec<String>,
    pub speakers: Vec<String>,
}

/// What the menu asks of the game (`Game::voice_action`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum VAction {
    ToggleMute,
    ToggleDeafen,
    ToggleIcons,
    Cycle(Opt, bool),
    /// A slider's value: `VoiceVolume`, `VoiceMic` (percent) or `VoiceThreshold` (dB).
    Set(Opt, u8),
    PlayerVolume(u128, u8),
    Device { mic: bool, name: String },
    MicTest(bool),
    CreateGroup { name: String, password: String, kind: GroupKind },
    JoinGroup(u128, Option<String>),
    LeaveGroup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Main,
    /// The groups, or the members of ours.
    Group,
    Create,
    Password(u128),
    Settings,
    Volumes,
    /// The microphones (true) or the speakers.
    Devices(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    Password,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slider {
    Volume,
    Mic,
    Threshold,
    Player(u128),
}
impl Slider {
    /// Its range.
    fn range(self) -> (u8, u8) {
        match self {
            Slider::Threshold => (20, 70),
            _ => (0, 200),
        }
    }
    fn action(self, value: u8) -> VAction {
        match self {
            Slider::Volume => VAction::Set(Opt::VoiceVolume, value),
            Slider::Mic => VAction::Set(Opt::VoiceMic, value),
            Slider::Threshold => VAction::Set(Opt::VoiceThreshold, value),
            Slider::Player(uuid) => VAction::PlayerVolume(uuid, value),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Hit {
    Go(Page),
    Back,
    Act(VAction),
    Field(Field),
    Slider(Slider, [f32; 4]),
    Kind,
    Create,
    Join(u128, bool),
    ConfirmPassword(u128),
}

pub(super) struct VoiceMenu {
    page: Page,
    hits: Vec<([f32; 4], Hit)>,
    name: String,
    password: String,
    kind: GroupKind,
    typing: Option<Field>,
    drag: Option<(Slider, [f32; 4])>,
    /// The last value sent by the slider held (sent again only when it changes).
    sent: Option<u8>,
    pending: Vec<VAction>,
    blink: f32,
    scroll: f32,
}

impl VoiceMenu {
    /// The menu, on its main page or straight on the group's.
    pub(super) fn new(group: bool) -> Self {
        Self {
            page: if group { Page::Group } else { Page::Main },
            hits: Vec::new(),
            name: String::new(),
            password: String::new(),
            kind: GroupKind::Normal,
            typing: None,
            drag: None,
            sent: None,
            pending: Vec::new(),
            blink: 0.0,
            scroll: 0.0,
        }
    }

    /// Escape: out of a field, then a page back (never past the main page: `at_main` closes).
    pub(super) fn back(&mut self) -> Option<VAction> {
        if self.typing.take().is_some() {
            return None;
        }
        let to = match self.page {
            Page::Main => return None,
            Page::Group | Page::Settings => Page::Main,
            Page::Create | Page::Password(_) => Page::Group,
            Page::Volumes | Page::Devices(_) => Page::Settings,
        };
        let leaving_settings = self.page == Page::Settings;
        self.page = to;
        self.scroll = 0.0;
        leaving_settings.then_some(VAction::MicTest(false))
    }
    /// Whether Escape closes the menu (the main page).
    pub(super) fn at_main(&self) -> bool {
        self.page == Page::Main && self.typing.is_none()
    }
    pub(super) fn wants_keys(&self) -> bool {
        self.typing.is_some()
    }
    /// A key while a field has the keyboard: text, Backspace, or Enter (`enter`).
    pub(super) fn type_key(&mut self, text: Option<&str>, backspace: bool, enter: bool) -> Option<VAction> {
        let field = self.typing?;
        if enter {
            self.typing = None;
            return match self.page {
                Page::Password(id) => Some(self.join_with_password(id)),
                Page::Create if field == Field::Password || !self.name.trim().is_empty() => self.create(),
                _ => None,
            };
        }
        let target = match field {
            Field::Name => &mut self.name,
            Field::Password => &mut self.password,
        };
        if backspace {
            target.pop();
        } else if let Some(text) = text {
            let room = 24usize.saturating_sub(target.chars().count());
            target.extend(text.chars().filter(|c| !c.is_control()).take(room));
        }
        None
    }
    fn create(&mut self) -> Option<VAction> {
        let name = self.name.trim().to_owned();
        if name.is_empty() {
            return None;
        }
        self.page = Page::Group;
        Some(VAction::CreateGroup { name, password: std::mem::take(&mut self.password), kind: self.kind })
    }
    fn join_with_password(&mut self, id: u128) -> VAction {
        self.page = Page::Group;
        VAction::JoinGroup(id, Some(std::mem::take(&mut self.password)))
    }
    /// The slider value under `x` on `track` (both in GUI pixels).
    fn slider_value(slider: Slider, [tx, _, tw, _]: [f32; 4], x: f32) -> u8 {
        let (lo, hi) = slider.range();
        let t = ((x - tx) / tw.max(1.0)).clamp(0.0, 1.0);
        if slider == Slider::Threshold {
            // The level's scale: -70 dB at the left, 0 at the right; its value is dB under 0.
            return (70.0 - 70.0 * t).round().clamp(f32::from(lo), f32::from(hi)) as u8;
        }
        // Whole fives for the percents.
        ((f32::from(lo) + t * f32::from(hi - lo)) / 5.0).round() as u8 * 5
    }

    /// A press at `cursor` (GUI pixels).
    pub(super) fn click(&mut self, cursor: (f32, f32)) -> Option<VAction> {
        let hit = self.hits.iter().rev().find(|(r, _)| inside(*r, cursor)).map(|h| h.1.clone());
        self.typing = match hit {
            Some(Hit::Field(f)) => Some(f),
            _ => None,
        };
        match hit? {
            Hit::Go(page) => {
                let leaving = self.page == Page::Settings && !matches!(page, Page::Volumes | Page::Devices(_));
                self.page = page;
                self.scroll = 0.0;
                if page == Page::Create {
                    self.name.clear();
                    self.password.clear();
                    self.typing = Some(Field::Name);
                }
                leaving.then_some(VAction::MicTest(false))
            }
            Hit::Back => self.back(),
            Hit::Act(a) => Some(a),
            Hit::Field(_) => None,
            Hit::Slider(s, track) => {
                self.drag = Some((s, track));
                let v = Self::slider_value(s, track, cursor.0);
                self.sent = Some(v);
                Some(s.action(v))
            }
            Hit::Kind => {
                let i = GroupKind::ALL.iter().position(|k| *k == self.kind).unwrap_or(0);
                self.kind = GroupKind::ALL[(i + 1) % 3];
                None
            }
            Hit::Create => self.create(),
            Hit::Join(id, false) => Some(VAction::JoinGroup(id, None)),
            Hit::Join(id, true) => {
                self.page = Page::Password(id);
                self.password.clear();
                self.typing = Some(Field::Password);
                None
            }
            Hit::ConfirmPassword(id) => Some(self.join_with_password(id)),
        }
    }
    pub(super) fn release(&mut self) {
        self.drag = None;
        self.sent = None;
    }
    pub(super) fn wheel(&mut self, lines: f32) {
        self.scroll = (self.scroll - lines * 30.0).max(0.0);
    }
    /// What dragging a slider asked for since the last frame.
    pub(super) fn take_pending(&mut self) -> Vec<VAction> {
        std::mem::take(&mut self.pending)
    }

    /// Draws the page at `alpha`, `rise` dp under its place; `cursor` in GUI pixels.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        view: &VoiceView,
        pvp: &horizon_client::pvp::Pvp,
        alpha: f32,
        rise: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
        cursor: (f32, f32),
    ) {
        self.blink = (self.blink + dt) % 1.0;
        // A slider held follows the cursor (its track kept in GUI pixels, as the cursor).
        if let Some((s, track)) = self.drag {
            let v = Self::slider_value(s, track, cursor.0);
            if self.sent != Some(v) {
                self.sent = Some(v);
                self.pending.push(s.action(v));
            }
        }
        let mut p = Painter {
            ui,
            a: alpha,
            gui,
            pointer: (0.0, 0.0),
            hits: Vec::new(),
            faces: Vec::new(),
            marks: Vec::new(),
        };
        p.pointer = p.ui.cursor;
        let (w, h) = (p.ui.w, p.ui.h);
        let (pw, ph) = match self.page {
            Page::Main => (380.0, 168.0),
            Page::Settings => (440.0, 452.0),
            Page::Create => (380.0, 290.0),
            Page::Password(_) => (380.0, 200.0),
            _ => (420.0, (h - 80.0).min(420.0)),
        };
        let (px, py) = ((w - pw) / 2.0, (h - ph) / 2.0 + rise);
        p.ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.86 * alpha));
        p.ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * alpha));
        let title = match self.page {
            Page::Main => tr("oteria.voice.title", "Voice Chat"),
            Page::Group => view.group.as_ref().map_or_else(|| tr("oteria.voice.groups", "Groups"), |g| g.name.clone()),
            Page::Create => tr("oteria.voice.create_title", "Create a group"),
            Page::Password(_) => tr("oteria.voice.password_title", "This group has a password"),
            Page::Settings => tr("oteria.voice.settings_title", "Voice Chat Settings"),
            Page::Volumes => tr("oteria.voice.volumes", "Player volumes"),
            Page::Devices(true) => tr("oteria.voice.mics", "Select microphone"),
            Page::Devices(false) => tr("oteria.voice.speakers", "Select speaker"),
        };
        p.ui.text_centered(Face::SemiBold, 15.0, px + pw / 2.0, py + 14.0, argb(WHITE, alpha), &title);
        if self.page != Page::Main {
            let back = [px + 12.0, py + 10.0, 28.0, 28.0];
            p.button(back, "‹", false, WHITE, Hit::Back);
        }
        let body = [px + 18.0, py + 48.0, pw - 36.0, ph - 48.0 - 16.0];
        match self.page {
            Page::Main => self.draw_main(&mut p, view, body),
            Page::Group if view.group.is_some() => self.draw_members(&mut p, view, body),
            Page::Group => self.draw_groups(&mut p, view, body),
            Page::Create => self.draw_create(&mut p, body),
            Page::Password(id) => self.draw_password(&mut p, view, body, id),
            Page::Settings => self.draw_settings(&mut p, view, pvp, body),
            Page::Volumes => self.draw_volumes(&mut p, view, body),
            Page::Devices(mic) => self.draw_devices(&mut p, view, pvp, body, mic),
        }
        let faces = std::mem::take(&mut p.faces);
        horizon_client::hud_layout::paint(p.ui.batch, p.ui.atlas, &faces);
        for (mx, my) in std::mem::take(&mut p.marks) {
            p.icon(icon::VOLUME_OFF, mx, my, argb(0xFF5A_5A, alpha));
        }
        self.hits = p.hits;
    }

    /// The bottom row's switches: microphone, sound, and (`eye`) the icons.
    fn switches(p: &mut Painter, view: &VoiceView, x: f32, y: f32, eye_x: Option<f32>) {
        let mic = if view.muted { icon::MICROPHONE_OFF } else { icon::MICROPHONE };
        p.icon_button([x, y, 36.0, 36.0], mic, view.muted, Hit::Act(VAction::ToggleMute));
        let sound = if view.deafened { icon::VOLUME_OFF } else { icon::VOLUME };
        p.icon_button([x + 44.0, y, 36.0, 36.0], sound, view.deafened, Hit::Act(VAction::ToggleDeafen));
        if let Some(ex) = eye_x {
            let eye = if view.hide_icons { icon::EYE_OFF } else { icon::EYE };
            p.icon_button([ex, y, 36.0, 36.0], eye, view.hide_icons, Hit::Act(VAction::ToggleIcons));
        }
    }

    fn draw_main(&mut self, p: &mut Painter, view: &VoiceView, [x, y, w, h]: [f32; 4]) {
        let (text, colour) = match view.status {
            Status::Unavailable => (tr("oteria.voice.unavailable", "This server has no voice chat."), 0xFFB0_5A),
            Status::Connecting => (tr("oteria.voice.connecting", "Connecting to the voice chat…"), 0xFFD8_6A),
            Status::Connected => (tr("oteria.voice.connected", "Connected"), 0x6BE0_8A),
            Status::NoAudio => (tr("oteria.voice.no_audio", "Connected, but no microphone or speaker"), 0xFF8A_6A),
        };
        p.ui.text_centered(Face::Regular, 11.0, x + w / 2.0, y - 8.0, argb(colour, p.a), &text);
        let bw = (w - 12.0) / 2.0;
        p.text_button([x, y + 14.0, bw, 36.0], &tr("oteria.voice.settings", "Settings"), false, Hit::Go(Page::Settings));
        p.text_button([x + bw + 12.0, y + 14.0, bw, 36.0], &tr("oteria.voice.group", "Group"), false, Hit::Go(Page::Group));
        Self::switches(p, view, x, y + h - 40.0, Some(x + w - 36.0));
    }

    fn draw_groups(&mut self, p: &mut Painter, view: &VoiceView, [x, y, w, h]: [f32; 4]) {
        let list_h = h - 48.0;
        let mut ry = y - self.scroll;
        let start = p.ui.batch.vertices.len();
        if view.groups.is_empty() {
            let none = tr("oteria.voice.no_groups", "No group yet: make one.");
            p.ui.text_centered(Face::Regular, 12.0, x + w / 2.0, y + 20.0, argb(WHITE, 0.55 * p.a), &none);
        }
        for g in &view.groups {
            let r = [x, ry, w, 40.0];
            if ry + 40.0 > y && ry < y + list_h {
                let over = inside(r, p.pointer);
                p.ui.rounded(r[0], r[1], r[2], r[3], 9.0, argb(WHITE, if over { 0.10 } else { 0.05 } * p.a));
                p.icon(icon::USERS, x + 20.0, ry + 20.0, argb(WHITE, 0.85 * p.a));
                p.ui.text(Face::Medium, 13.0, x + 40.0, ry + 11.0, argb(WHITE, p.a), &g.name);
                let kind = kind_label(g.kind);
                let kw = p.ui.measure(Face::Regular, 11.0, &kind);
                p.ui.text(Face::Regular, 11.0, x + w - 14.0 - kw, ry + 13.0, argb(WHITE, 0.55 * p.a), &kind);
                if g.password {
                    p.icon(icon::LOCK, x + w - 30.0 - kw, ry + 20.0, argb(WHITE, 0.7 * p.a));
                }
                p.hit(clip(r, y, list_h), Hit::Join(g.id, g.password));
            }
            ry += 46.0;
        }
        let gx0 = (p.gui)(x - 2.0);
        let gx1 = (p.gui)(x + w + 2.0);
        p.ui.batch.clip_since(start, gx0, (p.gui)(y), gx1, (p.gui)(y + list_h));
        let create = [x + w - 190.0, y + h - 38.0, 190.0, 36.0];
        p.colour_button(create, &tr("oteria.voice.create", "Create group"), ON, Some(icon::PLUS), Hit::Go(Page::Create));
    }

    fn draw_members(&mut self, p: &mut Painter, view: &VoiceView, [x, y, w, h]: [f32; 4]) {
        let list_h = h - 48.0;
        p.ui.rounded(x, y, w, list_h, 10.0, argb(0x00_0000, 0.25 * p.a));
        let mut ry = y + 6.0 - self.scroll;
        let start = p.ui.batch.vertices.len();
        for m in &view.members {
            if ry + 40.0 > y && ry < y + list_h {
                let r = [x + 6.0, ry, w - 12.0, 40.0];
                p.ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(WHITE, 0.05 * p.a));
                p.face(m, x + 14.0, ry + 6.0, 28.0);
                p.ui.text(Face::Medium, 13.0, x + 52.0, ry + 11.0, argb(WHITE, p.a), &m.name);
            }
            ry += 46.0;
        }
        p.ui.batch.clip_since(start, (p.gui)(x), (p.gui)(y), (p.gui)(x + w), (p.gui)(y + list_h));
        Self::switches(p, view, x, y + h - 40.0, Some(x + 88.0));
        let leave = [x + w - 36.0, y + h - 40.0, 36.0, 36.0];
        p.icon_button(leave, icon::LOGOUT, true, Hit::Act(VAction::LeaveGroup));
    }

    fn draw_create(&mut self, p: &mut Painter, [x, y, w, h]: [f32; 4]) {
        let name_label = tr("oteria.voice.group_name", "Group name");
        p.ui.text(Face::Medium, 11.0, x, y, argb(WHITE, 0.6 * p.a), &name_label);
        p.field([x, y + 16.0, w, 34.0], &self.name, self.typing == Some(Field::Name), self.blink, Field::Name, false);
        let pass_label = tr("oteria.voice.group_password", "Password (optional)");
        p.ui.text(Face::Medium, 11.0, x, y + 60.0, argb(WHITE, 0.6 * p.a), &pass_label);
        p.field([x, y + 76.0, w, 34.0], &self.password, self.typing == Some(Field::Password), self.blink, Field::Password, true);
        let kind_r = [x, y + 122.0, w, 34.0];
        let over = inside(kind_r, p.pointer);
        p.ui.rounded(kind_r[0], kind_r[1], kind_r[2], kind_r[3], 8.0, argb(WHITE, if over { 0.12 } else { 0.06 } * p.a));
        let label = format!("{}: {}", tr("oteria.voice.kind", "Type"), kind_label(self.kind));
        p.ui.text_centered(Face::Medium, 12.0, x + w / 2.0, kind_r[1] + 9.0, argb(WHITE, p.a), &label);
        p.hit(kind_r, Hit::Kind);
        let about = fit(p.ui, &kind_about(self.kind), w);
        p.ui.text(Face::Regular, 11.0, x, y + 162.0, argb(WHITE, 0.5 * p.a), &about);
        let ready = !self.name.trim().is_empty();
        let create = [x, y + h - 38.0, w, 36.0];
        let colour = if ready { ACCENT } else { 0x4A_5260 };
        p.colour_button(create, &tr("oteria.voice.create_button", "Create"), colour, None, Hit::Create);
    }

    fn draw_password(&mut self, p: &mut Painter, view: &VoiceView, [x, y, w, h]: [f32; 4], id: u128) {
        p.field([x, y + 6.0, w, 34.0], &self.password, self.typing == Some(Field::Password), self.blink, Field::Password, true);
        if view.wrong_password {
            let wrong = tr("oteria.voice.wrong_password", "Wrong password.");
            p.ui.text(Face::Regular, 11.0, x, y + 48.0, argb(0xFF6A_6A, p.a), &wrong);
        }
        let join = [x, y + h - 38.0, w, 36.0];
        p.colour_button(join, &tr("oteria.voice.join", "Join"), ACCENT, None, Hit::ConfirmPassword(id));
    }

    fn draw_settings(&mut self, p: &mut Painter, view: &VoiceView, pvp: &horizon_client::pvp::Pvp, [x, y, w, _]: [f32; 4]) {
        let v = &pvp.voice;
        let row = 38.0;
        let mut ry = y;
        let percent = |s: &str, n: u8| format!("{s}: {n}%");
        p.slider([x, ry, w, 32.0], Slider::Volume, v.volume, &percent(&tr("oteria.voice.volume", "Voice chat volume"), v.volume), None);
        ry += row;
        p.slider([x, ry, w, 32.0], Slider::Mic, v.mic, &percent(&tr("oteria.voice.amplification", "Microphone amplification"), v.mic), None);
        ry += row;
        let switch = |on: bool| if on { tr("oteria.voice.on", "On") } else { tr("oteria.voice.off", "Off") };
        let noise = format!("{}: {}", tr("oteria.voice.noise", "Noise suppression"), switch(v.noise));
        p.text_button([x, ry, w, 32.0], &noise, false, Hit::Act(VAction::Cycle(Opt::VoiceNoise, false)));
        ry += row;
        let kind = if v.activation { tr("horizon.pvp.voice.voice", "Voice") } else { tr("horizon.pvp.voice.ptt", "Push to talk") };
        let activation = format!("{}: {kind}", tr("oteria.voice.activation", "Activation type"));
        p.text_button([x, ry, w, 32.0], &activation, false, Hit::Act(VAction::Cycle(Opt::VoiceActivation, false)));
        ry += row;
        // The level, with what the microphone hears behind it.
        let threshold = format!("{}: -{} dB", tr("oteria.voice.threshold", "Voice activation threshold"), v.threshold);
        p.slider([x, ry, w, 32.0], Slider::Threshold, v.threshold, &threshold, Some(view.level));
        ry += row;
        let test = if view.loopback {
            tr("oteria.voice.test_stop", "Stop testing the microphone")
        } else {
            tr("oteria.voice.test", "Test the microphone")
        };
        p.text_button([x, ry, w, 32.0], &test, view.loopback, Hit::Act(VAction::MicTest(!view.loopback)));
        ry += row;
        let spatial = format!("{}: {}", tr("oteria.voice.spatial", "3D audio"), switch(v.spatial));
        p.text_button([x, ry, w, 32.0], &spatial, false, Hit::Act(VAction::Cycle(Opt::VoiceSpatial, false)));
        ry += row;
        p.text_button([x, ry, w, 32.0], &tr("oteria.voice.volumes", "Player volumes"), false, Hit::Go(Page::Volumes));
        ry += row;
        let half = (w - 10.0) / 2.0;
        p.text_button([x, ry, half, 32.0], &tr("oteria.voice.mics", "Select microphone"), false, Hit::Go(Page::Devices(true)));
        p.text_button([x + half + 10.0, ry, half, 32.0], &tr("oteria.voice.speakers", "Select speaker"), false, Hit::Go(Page::Devices(false)));
    }

    fn draw_volumes(&mut self, p: &mut Painter, view: &VoiceView, [x, y, w, h]: [f32; 4]) {
        let start = p.ui.batch.vertices.len();
        let mut ry = y - self.scroll;
        if view.others.is_empty() {
            let none = tr("oteria.voice.no_players", "No other player with the voice chat here.");
            p.ui.text_centered(Face::Regular, 12.0, x + w / 2.0, y + 20.0, argb(WHITE, 0.55 * p.a), &none);
        }
        for o in &view.others {
            if ry + 40.0 > y && ry < y + h {
                p.face(o, x + 2.0, ry + 4.0, 28.0);
                let label = format!("{}: {}%", o.name, o.volume);
                p.slider([x + 40.0, ry, w - 40.0, 36.0], Slider::Player(o.uuid), o.volume, &label, None);
            }
            ry += 44.0;
        }
        p.ui.batch.clip_since(start, (p.gui)(x - 2.0), (p.gui)(y), (p.gui)(x + w + 2.0), (p.gui)(y + h));
    }

    fn draw_devices(&mut self, p: &mut Painter, view: &VoiceView, pvp: &horizon_client::pvp::Pvp, [x, y, w, h]: [f32; 4], mic: bool) {
        let chosen = if mic { &pvp.voice.mic_device } else { &pvp.voice.speaker_device };
        let names = if mic { &view.mics } else { &view.speakers };
        let start = p.ui.batch.vertices.len();
        let mut ry = y - self.scroll;
        let default = tr("oteria.voice.default_device", "System default");
        for (name, label) in std::iter::once((String::new(), default)).chain(names.iter().map(|n| (n.clone(), n.clone()))) {
            if ry + 34.0 > y && ry < y + h {
                let on = *chosen == name;
                let r = [x, ry, w, 34.0];
                let over = inside(r, p.pointer);
                let fill = if on { argb(ACCENT, 0.25 * p.a) } else { argb(WHITE, if over { 0.10 } else { 0.05 } * p.a) };
                p.ui.rounded(r[0], r[1], r[2], r[3], 8.0, fill);
                if on {
                    p.ui.border(r[0], r[1], r[2], r[3], 8.0, argb(ACCENT, 0.7 * p.a));
                }
                let label = fit(p.ui, &label, w - 24.0);
                p.ui.text(Face::Medium, 12.0, x + 12.0, ry + 9.0, argb(WHITE, p.a), &label);
                p.hit(clip(r, y, h), Hit::Act(VAction::Device { mic, name }));
            }
            ry += 40.0;
        }
        p.ui.batch.clip_since(start, (p.gui)(x - 2.0), (p.gui)(y), (p.gui)(x + w + 2.0), (p.gui)(y + h));
    }
}

fn kind_label(kind: GroupKind) -> String {
    match kind {
        GroupKind::Normal => tr("oteria.voice.kind.normal", "Normal"),
        GroupKind::Open => tr("oteria.voice.kind.open", "Open"),
        GroupKind::Isolated => tr("oteria.voice.kind.isolated", "Isolated"),
    }
}
fn kind_about(kind: GroupKind) -> String {
    match kind {
        GroupKind::Normal => tr("oteria.voice.kind.normal.about", "Members hear each other, and the players near them."),
        GroupKind::Open => tr("oteria.voice.kind.open.about", "Players near the members hear them too."),
        GroupKind::Isolated => tr("oteria.voice.kind.isolated.about", "Members only hear each other."),
    }
}
/// `r` cut to the list's `y..y + h`.
fn clip(r: [f32; 4], y: f32, h: f32) -> [f32; 4] {
    [r[0], r[1].max(y), r[2], (r[1] + r[3]).min(y + h) - r[1].max(y)]
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

/// The page's drawing tools: the pointer (dp), the dp-to-GUI scale, the clickable places and the
/// faces drawn last (the skins are their own textures).
struct Painter<'u, 'a, 'g> {
    ui: &'u mut Ui<'a>,
    a: f32,
    gui: &'g dyn Fn(f32) -> f32,
    pointer: (f32, f32),
    hits: Vec<([f32; 4], Hit)>,
    faces: Vec<horizon_client::hud_layout::GuiDraw>,
    /// The sound-off marks, drawn over the faces (dp).
    marks: Vec<(f32, f32)>,
}
impl Painter<'_, '_, '_> {
    fn hit(&mut self, r: [f32; 4], what: Hit) {
        if r[3] <= 0.0 {
            return;
        }
        horizon_client::widgets::widget_cursor(inside(r, self.pointer), true);
        self.hits.push((r.map(self.gui), what));
    }
    fn icon(&mut self, name: &str, cx: f32, cy: f32, colour: u32) {
        let small = icon::small(name);
        if let Some(r) = self.ui.atlas.sprites.get(small.as_str()).copied() {
            let d = self.ui.density;
            let s = 20.0 / d;
            let k = self.ui.batch.scale;
            let gui = self.gui;
            let snap = |v: f32| (gui(v) * k).round() / k;
            self.ui.batch.rect_uv(snap(cx - s / 2.0), snap(cy - s / 2.0), gui(s), gui(s), r, colour);
        }
    }
    fn text_button(&mut self, r: [f32; 4], label: &str, lit: bool, what: Hit) {
        let over = inside(r, self.pointer);
        let fill = if lit { argb(ACCENT, 0.35 * self.a) } else { argb(WHITE, if over { 0.13 } else { 0.06 } * self.a) };
        self.ui.rounded(r[0], r[1], r[2], r[3], 8.0, fill);
        self.ui.border(r[0], r[1], r[2], r[3], 8.0, argb(WHITE, if over { 0.18 } else { 0.07 } * self.a));
        self.ui.text_centered(Face::Medium, 12.0, r[0] + r[2] / 2.0, r[1] + (r[3] - 17.0) / 2.0, argb(WHITE, self.a), label);
        self.hit(r, what);
    }
    fn colour_button(&mut self, r: [f32; 4], label: &str, colour: u32, sprite: Option<&str>, what: Hit) {
        let over = inside(r, self.pointer);
        self.ui.rounded(r[0], r[1], r[2], r[3], 9.0, argb(colour, if over { 1.0 } else { 0.82 } * self.a));
        let lw = self.ui.measure(Face::Medium, 12.0, label);
        let iw = if sprite.is_some() { 24.0 } else { 0.0 };
        let lx = r[0] + (r[2] - lw - iw) / 2.0 + iw;
        if let Some(s) = sprite {
            self.icon(s, lx - 14.0, r[1] + r[3] / 2.0, argb(WHITE, self.a));
        }
        self.ui.text(Face::Medium, 12.0, lx, r[1] + (r[3] - 17.0) / 2.0, argb(WHITE, self.a), label);
        self.hit(r, what);
    }
    fn button(&mut self, r: [f32; 4], label: &str, lit: bool, _colour: u32, what: Hit) {
        let over = inside(r, self.pointer);
        let fill = if lit { argb(OFF, 0.8 * self.a) } else { argb(WHITE, if over { 0.14 } else { 0.06 } * self.a) };
        self.ui.rounded(r[0], r[1], r[2], r[3], 8.0, fill);
        self.ui.text_centered(Face::Medium, 16.0, r[0] + r[2] / 2.0, r[1] + 4.0, argb(WHITE, self.a), label);
        self.hit(r, what);
    }
    /// A square switch: red while `off` (muted, sound off, icons hidden).
    fn icon_button(&mut self, r: [f32; 4], sprite: &str, off: bool, what: Hit) {
        let over = inside(r, self.pointer);
        let fill = if off { argb(OFF, if over { 0.95 } else { 0.75 } * self.a) } else { argb(WHITE, if over { 0.15 } else { 0.07 } * self.a) };
        self.ui.rounded(r[0], r[1], r[2], r[3], 8.0, fill);
        self.icon(sprite, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, argb(WHITE, self.a));
        self.hit(r, what);
    }
    fn field(&mut self, r: [f32; 4], text: &str, typing: bool, blink: f32, which: Field, secret: bool) {
        self.ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(WHITE, if typing { 0.12 } else { 0.07 } * self.a));
        if typing {
            self.ui.border(r[0], r[1], r[2], r[3], 8.0, argb(ACCENT, 0.8 * self.a));
        }
        let shown: String = if secret { "•".repeat(text.chars().count()) } else { text.to_owned() };
        let caret = if typing && blink < 0.55 { "|" } else { "" };
        self.ui.text(Face::Regular, 12.0, r[0] + 12.0, r[1] + (r[3] - 17.0) / 2.0, argb(WHITE, self.a), &format!("{shown}{caret}"));
        self.hit(r, Hit::Field(which));
    }
    /// A slider: the track filled to `value`, its label over it; with `level`, what the
    /// microphone hears drawn behind (green to red).
    fn slider(&mut self, r: [f32; 4], slider: Slider, value: u8, label: &str, level: Option<f32>) {
        let (lo, hi) = slider.range();
        self.ui.rounded(r[0], r[1], r[2], r[3], 8.0, argb(0x00_0000, 0.35 * self.a));
        if let Some(db) = level {
            // -70 dB at the left to 0 at the right, as the threshold's scale (inverted: its
            // value is dB below full scale).
            let t = ((db + 70.0) / 70.0).clamp(0.0, 1.0);
            let loud = t > 0.85;
            let colour = if loud { 0xE0_5A3A } else if t > 0.6 { 0xE0_C83A } else { 0x2E_C25A };
            self.ui.rounded(r[0], r[1], r[2] * t, r[3], 8.0, argb(colour, 0.75 * self.a));
            // The threshold's mark: a level of -value dB.
            let mark = ((70.0 - f32::from(value)) / 70.0).clamp(0.0, 1.0);
            self.ui.rounded(r[0] + r[2] * mark - 2.0, r[1] + 2.0, 4.0, r[3] - 4.0, 2.0, argb(WHITE, self.a));
        } else {
            let t = (f32::from(value.clamp(lo, hi) - lo) / f32::from(hi - lo)).clamp(0.0, 1.0);
            self.ui.rounded(r[0], r[1], r[2] * t, r[3], 8.0, argb(ACCENT, 0.45 * self.a));
            self.ui.rounded(r[0] + r[2] * t - 3.0, r[1] + 3.0, 6.0, r[3] - 6.0, 3.0, argb(WHITE, self.a));
        }
        self.ui.text_centered(Face::Medium, 12.0, r[0] + r[2] / 2.0, r[1] + (r[3] - 17.0) / 2.0, argb(WHITE, self.a), label);
        // The track in GUI pixels, as the cursor the slider follows.
        if r[3] > 0.0 {
            horizon_client::widgets::widget_cursor(inside(r, self.pointer), true);
            let track = r.map(self.gui);
            self.hits.push((track, Hit::Slider(slider, track)));
        }
    }
    /// A player's face, `size` dp square, outlined while they talk, their sound off marked.
    fn face(&mut self, player: &Player, x: f32, y: f32, size: f32) {
        let gui = self.gui;
        if player.talking {
            self.ui.rounded(x - 2.0, y - 2.0, size + 4.0, size + 4.0, 4.0, argb(WHITE, 0.9 * self.a));
        }
        let skin = player.face.as_deref().unwrap_or("entity/player/wide/steve");
        let (gx, gy, gs) = (gui(x).round() as i32, gui(y).round() as i32, gui(size).round() as i32);
        horizon_client::tab_list::face(&mut self.faces, skin, gx, gy, gs, true, false, argb(WHITE, self.a));
        if player.sound_off {
            self.marks.push((x + size - 4.0, y + size - 4.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_steps_back_a_page_then_closes() {
        let mut m = VoiceMenu::new(false);
        assert!(m.at_main());
        m.page = Page::Devices(true);
        assert_eq!(m.back(), None);
        assert_eq!(m.page, Page::Settings);
        // Leaving the settings stops the microphone test.
        assert_eq!(m.back(), Some(VAction::MicTest(false)));
        assert!(m.at_main());
    }

    #[test]
    fn a_group_is_made_from_what_was_typed() {
        let mut m = VoiceMenu::new(true);
        m.page = Page::Create;
        m.typing = Some(Field::Name);
        m.type_key(Some("Team"), false, false);
        m.typing = Some(Field::Password);
        m.type_key(Some("pw"), false, false);
        let made = m.type_key(None, false, true);
        assert_eq!(
            made,
            Some(VAction::CreateGroup { name: "Team".into(), password: "pw".into(), kind: GroupKind::Normal })
        );
        assert_eq!(m.page, Page::Group);
    }

    #[test]
    fn sliders_snap_to_their_steps() {
        let track = [0.0, 0.0, 200.0, 10.0];
        assert_eq!(VoiceMenu::slider_value(Slider::Volume, track, 100.0), 100);
        assert_eq!(VoiceMenu::slider_value(Slider::Volume, track, 51.0), 50);
        assert_eq!(VoiceMenu::slider_value(Slider::Threshold, track, 0.0), 70);
        assert_eq!(VoiceMenu::slider_value(Slider::Threshold, track, 999.0), 20);
    }
}
