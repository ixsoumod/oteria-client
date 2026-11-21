//! The Mods page of Oteria's menu (`oteria_menu`), as Lunar's and Feather's: a glass panel with
//! the header (logo, MODS and PROFILES tabs, close), the categories down the left with Edit HUD
//! at their foot, a search field, and a grid of tiles. Each tile is one of the engine's own
//! features (`Extras` and its PvP part): its icon, its name, a gear that opens its settings, and a
//! bar that turns it on or off, eased from red to green.
//!
//! A tile's gear opens its settings in the same panel, in the same style: its switch, then each
//! of its options (`extras::Item`) as a switch or a value stepped with arrows. The crosshair's
//! adds five ready-made crosshairs and a grid to draw one pixel by pixel.
//!
//! The PROFILES tab lists the player's profiles (`oteria_profiles`): Create, and on each card
//! Use, Rename (typed in place), Duplicate and Delete (a second click confirms).
//!
//! A colour option opens a picker under its row, eased open: the ready-made colours, a
//! saturation and brightness square with a hue bar beside it (dragged, the colour is put on as
//! it moves), and a field to type `#RRGGBB`. A number option's value is a field that takes
//! digits only, with its arrows either side.
//!
//! As the menu, it moves on the clock: the hover lights, the switches' colours, the scroll and
//! the way between the grid and a tile's settings all ease towards where they are going.

use super::oteria_profiles::{NAME_MAX, Profiles};
use crate::ui_theme::{Face, Ui};
use horizon_client::extras::{Extras, Item};
use horizon_client::lang;
use horizon_client::name_badge;
use horizon_client::oteria_icons as icon;
use horizon_client::pvp::{self, Colour, DRAWN, Opt};

/// The menu's colours, as `0xRRGGBB` (`argb` makes them the batch's).
const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const ON: u32 = 0x23_A35C;
const OFF: u32 = 0xB3_3346;
const GREY: u32 = 0x4A_5260;

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

/// One feature on the grid: its name (language key and English), icon, category, its settings
/// (shown by its gear), and how to read and set it.
pub(super) struct Tile {
    pub(super) key: &'static str,
    english: &'static str,
    icon: &'static str,
    category: Category,
    settings: &'static [Item],
    pub(super) get: fn(&Extras) -> bool,
    pub(super) set: fn(&mut Extras, bool),
}
impl Tile {
    fn name(&self) -> String {
        lang::get_or(self.key, self.english)
    }
    /// The crosshair's tile: its settings add the presets and the drawing grid.
    fn is_crosshair(&self) -> bool {
        self.key == "oteria.mod.crosshair"
    }
}

macro_rules! tile {
    ($key:literal, $english:literal, $icon:expr, $cat:ident, $settings:expr, $e:ident => $field:expr) => {
        Tile {
            key: $key,
            english: $english,
            icon: $icon,
            category: Category::$cat,
            settings: $settings,
            get: |$e: &Extras| $field,
            set: |$e: &mut Extras, on: bool| $field = on,
        }
    };
}
/// A HUD widget's settings: its corner, then the style every widget shares.
macro_rules! hud {
    ($($corner:ident),*) => {
        &[
            $(Item::Pvp(Opt::$corner),)*
            Item::Pvp(Opt::HudBackground),
            Item::Pvp(Opt::HudOpacity),
            Item::Pvp(Opt::HudTextColour),
            Item::Pvp(Opt::HudShadow),
        ]
    };
}

/// Every tile, in the grid's order.
pub(super) static TILES: [Tile; 46] = [
    tile!("oteria.mod.fps", "FPS", icon::GAUGE, Hud, hud!(HudFpsCorner), e => e.pvp.hud.fps),
    tile!("oteria.mod.cps", "CPS", icon::MOUSE, Hud, hud!(HudCpsCorner), e => e.pvp.hud.cps),
    tile!("oteria.mod.keystrokes", "Keystrokes", icon::KEYBOARD, Hud, hud!(HudKeysCorner), e => e.pvp.hud.keys),
    tile!("oteria.mod.armour", "Armor Status", icon::SHIELD, Hud, hud!(HudArmourCorner, HudArmourPercent), e => e.pvp.hud.armour),
    tile!("oteria.mod.effects", "Potion Effects", icon::FLASK, Hud, hud!(HudEffectsCorner), e => e.pvp.hud.effects),
    tile!("oteria.mod.held", "Held Item", icon::SWORD, Hud, hud!(HudHeldCorner), e => e.pvp.hud.held),
    tile!("oteria.mod.ping", "Ping", icon::WIFI, Hud, hud!(HudPingCorner), e => e.pvp.hud.ping),
    tile!("oteria.mod.coords", "Coordinates", icon::MAP_PIN, Hud, hud!(HudCoordsCorner), e => e.pvp.hud.coords),
    tile!("oteria.mod.server", "Server Address", icon::SERVER, Hud, hud!(HudServerCorner), e => e.pvp.hud.server),
    tile!("oteria.mod.combo", "Combo Counter", icon::FLAME, Hud, hud!(HudComboCorner), e => e.pvp.hud.combo),
    tile!("oteria.mod.clock", "Clock", icon::CLOCK, Hud, hud!(HudClockCorner), e => e.pvp.hud.clock),
    tile!("oteria.mod.reach", "Reach Display", icon::RULER_2, Hud, hud!(HudReachCorner), e => e.pvp.hud.reach),
    tile!("oteria.mod.speed", "Speed", icon::RUN, Hud, hud!(HudSpeedCorner), e => e.pvp.hud.speed),
    tile!("oteria.mod.cooldowns", "Cooldowns", icon::CLOCK, Hud, hud!(HudCooldownsCorner), e => e.pvp.hud.cooldowns),
    tile!("oteria.mod.totems", "Totem Counter", icon::CROWN, Hud, hud!(HudTotemsCorner), e => e.pvp.hud.totems),
    tile!("oteria.mod.pots", "Pot Counter", icon::FLASK, Hud, hud!(HudPotsCorner), e => e.pvp.hud.pots),
    tile!("oteria.mod.saturation", "AppleSkin", icon::FLASK, Hud, &[
        Item::Pvp(Opt::SaturationHeld),
        Item::Pvp(Opt::SaturationExhaustion),
        Item::Pvp(Opt::SaturationTooltip),
    ], e => e.pvp.saturation.enabled),
    tile!("oteria.mod.crystal", "Crystal Optimizer", icon::SPARKLES, Pvp,
        &[Item::Pvp(Opt::CrystalKeep)], e => e.pvp.crystal.enabled),
    tile!("oteria.mod.anchor", "Anchor Optimizer", icon::BULB, Pvp,
        &[Item::Pvp(Opt::AnchorCharge)], e => e.pvp.anchor.enabled),
    tile!("oteria.mod.pops", "Pop Counter", icon::FLAME, Pvp, &[], e => e.pvp.pops),
    tile!("oteria.mod.hitboxes", "Hitboxes", icon::BOX, Pvp, &[
        Item::Pvp(Opt::HitboxPlayers),
        Item::Pvp(Opt::HitboxHostile),
        Item::Pvp(Opt::HitboxPassive),
        Item::Pvp(Opt::HitboxItems),
        Item::Pvp(Opt::HitboxProjectiles),
        Item::Pvp(Opt::HitboxCrystals),
        Item::Pvp(Opt::HitboxOthers),
        Item::Pvp(Opt::HitboxEyes),
        Item::Pvp(Opt::HitboxLook),
        Item::Pvp(Opt::HitboxWidth),
        Item::Pvp(Opt::HitboxOwn),
        Item::Pvp(Opt::HitboxTarget),
        Item::Pvp(Opt::HitboxHurt),
        Item::Pvp(Opt::HitboxEyeColour),
        Item::Pvp(Opt::HitboxLookColour),
        Item::Pvp(Opt::HitboxOutline),
        Item::Pvp(Opt::HitboxOutlineSize),
        Item::Pvp(Opt::HitboxFarWidth),
        Item::Pvp(Opt::HitboxFarDistance),
        Item::Pvp(Opt::HitboxFireworks),
        Item::Pvp(Opt::HitboxStuck),
    ], e => e.pvp.hitboxes.enabled),
    Tile {
        key: "oteria.mod.secret",
        english: "Super Secret Settings",
        icon: icon::EYE,
        category: Category::Visual,
        settings: &[Item::Pvp(Opt::Secret)],
        get: |e: &Extras| e.pvp.secret.effect != 0,
        // On: Notch's, the first effect; the gear or the Options button picks another.
        set: |e: &mut Extras, on: bool| e.pvp.secret.effect = u8::from(on),
    },
    tile!("oteria.mod.crosshair", "Crosshair", icon::CROSSHAIR, Pvp, &[
        Item::Pvp(Opt::CrosshairShape),
        Item::Pvp(Opt::CrosshairSize),
        Item::Pvp(Opt::CrosshairGap),
        Item::Pvp(Opt::CrosshairThickness),
        Item::Pvp(Opt::CrosshairColour),
        Item::Pvp(Opt::CrosshairOutline),
        Item::Pvp(Opt::CrosshairDot),
        Item::Pvp(Opt::CrosshairHover),
        Item::Pvp(Opt::CrosshairIndicator),
    ], e => e.pvp.crosshair.custom),
    tile!("oteria.mod.hit", "Hit Color", icon::DROPLET, Pvp,
        &[Item::Pvp(Opt::HitColour), Item::Pvp(Opt::HitOpacity)], e => e.pvp.hit.custom),
    tile!("oteria.mod.outline", "Block Outline", icon::BOX, Pvp, &[
        Item::Pvp(Opt::OutlineColour),
        Item::Pvp(Opt::OutlineOpacity),
        Item::Pvp(Opt::OutlineThickness),
        Item::Pvp(Opt::OutlineFill),
    ], e => e.pvp.outline.custom),
    Tile {
        key: "oteria.mod.low_fire",
        english: "Low Fire",
        icon: icon::FLAME_OFF,
        category: Category::Pvp,
        settings: &[Item::Pvp(Opt::FireLower)],
        get: |e: &Extras| e.pvp.tweaks.fire_lower > 0,
        // On: the fire overlay lowered by 40 % of its height, a common PvP setting.
        set: |e: &mut Extras, on: bool| e.pvp.tweaks.fire_lower = if on { 40 } else { 0 },
    },
    tile!("oteria.mod.numbers", "Enchant Numbers", icon::NUMBERS, Pvp,
        &[Item::Pvp(Opt::GlintColour)], e => e.pvp.tweaks.numeric_enchants),
    tile!("oteria.mod.fullbright", "Fullbright", icon::SUN, Visual, &[], e => e.fullbright),
    tile!("oteria.mod.lights", "Dynamic Lights", icon::BULB, Visual, &[], e => e.dynamic_lights),
    tile!("oteria.mod.water", "Clear Water", icon::RIPPLE, Visual,
        &[Item::Pvp(Opt::ClearLava)], e => e.pvp.tweaks.clear_water),
    tile!("oteria.mod.sky", "Sky Mode", icon::MOON_STARS, Visual,
        &[Item::Pvp(Opt::SkyPack)], e => e.pvp.sky.enabled),
    tile!("oteria.mod.blur", "Motion Blur", icon::RIPPLE, Visual,
        &[Item::Pvp(Opt::BlurStrength)], e => e.pvp.blur.enabled),
    tile!("oteria.mod.environment", "Environment", icon::SPARKLES, Visual, &[
        Item::Pvp(Opt::EnvGround),
        Item::Pvp(Opt::EnvGroundColour),
        Item::Pvp(Opt::EnvGroundRadius),
        Item::Pvp(Opt::EnvParticles),
        Item::Pvp(Opt::EnvParticlesColour),
        Item::Pvp(Opt::EnvParticlesAmount),
        Item::Pvp(Opt::EnvHat),
        Item::Pvp(Opt::EnvHatColour),
        Item::Pvp(Opt::EnvSky),
        Item::Pvp(Opt::EnvSkyColour),
        Item::Pvp(Opt::EnvFog),
        Item::Pvp(Opt::EnvFogColour),
        Item::Pvp(Opt::EnvFogDensity),
    ], e => e.pvp.environment.enabled),
    tile!("oteria.mod.time_changer", "Time Changer", icon::CLOCK, Visual,
        &[Item::Pvp(Opt::ClockTime)], e => e.pvp.clock.enabled),
    tile!("oteria.mod.weather", "Weather Changer", icon::DROPLET, Visual,
        &[Item::Pvp(Opt::SkyWeatherPick)], e => e.pvp.sky_weather.enabled),
    tile!("oteria.mod.particles", "No Block Particles", icon::SPARKLES, Visual, &[], e => e.hide_block_particles),
    tile!("oteria.mod.minimap", "Minimap", icon::MAP, Utility,
        &[
            Item::Pvp(Opt::MapShape),
            Item::Pvp(Opt::MapBiome),
            Item::Pvp(Opt::MapRotate),
            Item::Pvp(Opt::MapZoom),
        ], e => e.minimap),
    tile!("oteria.mod.tiers", "Tier Tagger", icon::CROWN, Pvp,
        &[Item::Pvp(Opt::TiersMode)], e => e.pvp.tiers.enabled),
    tile!("oteria.mod.barrel", "Barrel Roll", icon::FEATHER, Utility, &[
        Item::Pvp(Opt::BarrelBanking),
        Item::Pvp(Opt::BarrelRighting),
        Item::Pvp(Opt::BarrelInvert),
        Item::Pvp(Opt::BarrelSwitch),
        Item::Pvp(Opt::BarrelSensitivity),
    ], e => e.pvp.barrel.enabled),
    tile!("oteria.mod.elytra", "Elytra Indicator", icon::FEATHER, Pvp,
        &[Item::Pvp(Opt::ElytraColour)], e => e.pvp.tweaks.elytra_indicator),
    tile!("oteria.mod.voice", "Voice Chat", icon::MICROPHONE, Utility,
        &[
            Item::Pvp(Opt::VoiceMenuKey),
            Item::Pvp(Opt::VoiceActivation),
            Item::Pvp(Opt::VoiceKey),
            Item::Pvp(Opt::VoiceVolume),
            Item::Pvp(Opt::VoiceMic),
            Item::Pvp(Opt::VoiceThreshold),
            Item::Pvp(Opt::VoiceNoise),
            Item::Pvp(Opt::VoiceSpatial),
        ], e => e.pvp.voice.enabled),
    tile!("oteria.mod.mouse", "Mouse Tweaks", icon::POINTER, Utility,
        &[
            Item::Pvp(Opt::MouseRmb),
            Item::Pvp(Opt::MouseLmbItem),
            Item::Pvp(Opt::MouseLmbEmpty),
            Item::MouseTweaksWheel,
            Item::Pvp(Opt::MouseWheelOrder),
            Item::Pvp(Opt::MouseWheelDirection),
            Item::Pvp(Opt::MouseScrollScaling),
            Item::Sort,
        ], e => e.mouse_tweaks),
    tile!("oteria.mod.zoom", "Zoom", icon::SEARCH, Utility,
        &[Item::Pvp(Opt::ZoomSmooth), Item::Pvp(Opt::ZoomLevel)], e => e.pvp.zoom.enabled),
    tile!("oteria.mod.music", "Music", icon::VOLUME, Utility,
        &[Item::Pvp(Opt::MusicLyrics), Item::Pvp(Opt::MusicCard)], e => e.pvp.music.enabled),
    tile!("oteria.mod.reconnect", "Auto Reconnect", icon::REFRESH, Utility, &[], e => e.auto_reconnect),
    tile!("oteria.mod.nametag", "Own Name Tag", icon::USER, Utility, &[], e => e.own_name_tag),
];

/// The page's two tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tab {
    Mods,
    Profiles,
}

/// What the profiles tab asks of the game, by profile index (`Profiles::names`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProfileOp {
    Create,
    Use(usize),
    Rename(usize, String),
    Duplicate(usize),
    Delete(usize),
}

/// What a click on the page asks of the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Toggle(usize),
    /// An option stepped forwards, or back.
    Cycle(Item, bool),
    /// One of `pvp::crosshair_presets`.
    Preset(usize),
    /// The drawn crosshair's pixels (the player indicator's, when true), once the stroke is done.
    Pixels(bool, [u16; DRAWN]),
    /// A colour option set (`None`: off); saved when the last field is true (a drag puts each
    /// step on, and saves when let go).
    SetColour(Opt, Option<Colour>, bool),
    /// A number option typed, kept within its range by `Opt::set_number`.
    SetNumber(Opt, u32),
    /// A slider dragged (`Opt::set_slider`); saved once let go (the last field true).
    SetSlider(Opt, u32, bool),
    Profile(ProfileOp),
    /// The menu's rocket: Boost on (true) or off (`oteria_boost`).
    Boost(bool),
    /// The voice chat's menu (`oteria_voice_ui`).
    Voice(super::oteria_voice_ui::VAction),
    /// The Host page (`oteria_host`).
    Host(super::oteria_host::HostAction),
    /// The Cosmetics page (`oteria_cosmetics`).
    Cosmetics(super::oteria_cosmetics::CosmeticsAction),
    /// The Keyboard page (`oteria_keyboard`).
    Keyboard(super::oteria_keyboard::KeyboardAction),
    Close,
    EditHud,
}

/// The page's state: its filter, search, scroll, the tile whose settings are open, and the
/// lights that ease.
pub(super) struct Mods {
    pub(super) tab: Tab,
    /// How far the profiles tab has come in (0: the mods, 1: the profiles).
    tab_k: f32,
    /// The profile being renamed, and the name typed so far.
    pub(super) renaming: Option<(usize, String)>,
    /// The profile whose Delete was pressed once: a second press deletes it.
    confirm: Option<usize>,
    /// Each profile card's hover light.
    cards: Vec<f32>,
    /// The scrolled view's top and height (dp), for the scrollbar.
    track: [f32; 2],
    category: Category,
    search: String,
    pub(super) typing: bool,
    scroll: f32,
    target: f32,
    /// The search field's caret blinks once a second.
    blink: f32,
    /// The scroll's room: how far down the view's content goes past it (dp).
    room: f32,
    hover: Vec<f32>,
    /// Each tile's switch, eased from 0 (off) to 1 (on).
    switch: Vec<Option<f32>>,
    /// The tile whose settings are open; `shown` the one drawn (it stays while they fade out),
    /// and `detail` how far they have come in (0: the grid, 1: the settings).
    open: Option<usize>,
    shown: usize,
    detail: f32,
    /// The settings' rows' switches, eased as the tiles'.
    rows: Vec<f32>,
    /// The drawn crosshair while a stroke is on: its pixels, and what the stroke paints.
    pixels: Option<[u16; DRAWN]>,
    /// The grid draws the player indicator (Crosshair Indicator), not the crosshair.
    indicator: bool,
    paint: Option<bool>,
    /// The stroke's last cell: the next one is joined to it by a line, however fast it moves.
    last: Option<(usize, usize)>,
    /// The drawing grid (GUI pixels) and one cell's side, for clicks.
    grid: ([f32; 4], f32),
    /// Where things were drawn (GUI pixels), for clicks.
    hits: Vec<([f32; 4], Hit)>,
    /// The colour option whose picker is open (or closing), and how far it has opened.
    picker: Option<Picker>,
    picker_k: f32,
    /// The number option whose value is being typed, and the digits so far.
    number: Option<(Opt, String)>,
    /// What a drag or a typed value asks of the game, taken after each frame.
    pending: Vec<Action>,
    /// The slider being dragged: its option, its track (GUI pixels) and the value it is at.
    slider: Option<(Opt, [f32; 4], u32)>,
}

/// A colour option's picker.
#[derive(Clone, Debug, PartialEq)]
struct Picker {
    opt: Opt,
    /// False while it closes.
    open: bool,
    /// The colour on the square and the bar: hue (0..1), saturation, brightness.
    hsv: [f32; 3],
    /// The hex digits typed, while the field has the keyboard.
    hex: Option<String>,
    drag: Option<Drag>,
    /// The square and the hue bar (GUI pixels), for drags.
    square: [f32; 4],
    hue: [f32; 4],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drag {
    Square,
    Hue,
}
/// The picker's height (dp) once open.
const PICKER_H: f32 = 150.0;

/// `0xRRGGBB` as hue (0..1), saturation and brightness.
fn rgb_to_hsv(rgb: u32) -> [f32; 3] {
    let ch = |s: u32| ((rgb >> s) & 0xFF) as f32 / 255.0;
    let (r, g, b) = (ch(16), ch(8), ch(0));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    [h, s, max]
}
/// Hue (0..1), saturation and brightness as `0xRRGGBB`.
fn hsv_to_rgb([h, s, v]: [f32; 3]) -> u32 {
    let h6 = (h.rem_euclid(1.0)) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let ch = |t: f32| (((t + m) * 255.0).round().clamp(0.0, 255.0)) as u32;
    ch(r) << 16 | ch(g) << 8 | ch(b)
}
/// A colour as the picker shows it: off and inverted have none of their own.
fn shown_rgb(c: Option<Colour>) -> u32 {
    match c {
        None => 0x2A_2F38,
        Some(c) => c.argb() & 0xFF_FFFF,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Category(Category),
    Search,
    Tab(Tab),
    Close,
    EditHud,
    Toggle(usize),
    Gear(usize),
    Back,
    Row(Item, bool),
    Preset(usize),
    Grid,
    ClearGrid,
    /// Which drawing the grid edits: the crosshair (false) or the player indicator (true).
    GridTarget(bool),
    NewProfile,
    UseProfile(usize),
    RenameProfile(usize),
    NameField,
    CopyProfile(usize),
    DeleteProfile(usize),
    /// A colour option's row: its picker opens, or closes.
    ColourRow(Opt),
    /// A ready-made colour in the picker (`None`: off).
    Swatch(Option<Colour>),
    Square,
    HueBar,
    HexField,
    /// The picker's background: a click there keeps it open and does nothing.
    PickerPanel,
    /// A number option's value: typed.
    NumberField(Opt),
    /// A slider's track: pressed, it follows the pointer.
    Slider(Opt),
}
impl Default for Mods {
    fn default() -> Self {
        Self {
            tab: Tab::Mods,
            tab_k: 0.0,
            renaming: None,
            confirm: None,
            cards: Vec::new(),
            track: [0.0; 2],
            category: Category::All,
            search: String::new(),
            typing: false,
            scroll: 0.0,
            target: 0.0,
            blink: 0.0,
            room: 0.0,
            hover: vec![0.0; TILES.len()],
            switch: vec![None; TILES.len()],
            open: None,
            shown: 0,
            detail: 0.0,
            rows: Vec::new(),
            pixels: None,
            indicator: false,
            paint: None,
            last: None,
            grid: ([0.0; 4], 1.0),
            hits: Vec::new(),
            picker: None,
            picker_k: 0.0,
            number: None,
            pending: Vec::new(),
            slider: None,
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
/// `t` from 0 to 1, slowing to a stop.
fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}
fn caption(item: Item) -> String {
    lang::get_or(item.caption_key(), item.caption())
}
fn description(item: Item) -> String {
    lang::get_or(item.description_key(), item.description())
}
/// `text` cut with an ellipsis to fit `width` dp.
fn fit(ui: &Ui, face: Face, size: f32, text: &str, width: f32) -> String {
    if ui.measure(face, size, text) <= width {
        return text.to_owned();
    }
    let mut cut: String = text.to_owned();
    while !cut.is_empty() && ui.measure(face, size, &format!("{cut}…")) > width {
        cut.pop();
    }
    format!("{}…", cut.trim_end())
}

impl Mods {
    /// Opens `opt`'s picker, as a click on its row does (tests).
    #[cfg(test)]
    fn click_test_picker(&mut self, opt: Opt) {
        self.hits = vec![([0.0, 0.0, 10.0, 10.0], Hit::ColourRow(opt))];
        self.click((1.0, 1.0), [[0; DRAWN]; 2]);
    }

    /// The tiles the filter and the search leave, by index.
    fn shown(&self) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        (0..TILES.len())
            .filter(|&i| self.category == Category::All || TILES[i].category == self.category)
            .filter(|&i| query.is_empty() || TILES[i].name().to_lowercase().contains(&query))
            .collect()
    }

    /// Escape: out of the search, then out of a tile's settings. False when there was neither
    /// (the menu goes back to its buttons).
    pub(super) fn back(&mut self) -> bool {
        if self.number.take().is_some() {
            // The digits typed are dropped.
        } else if let Some(p) = self.picker.as_mut().filter(|p| p.hex.is_some()) {
            p.hex = None;
        } else if let Some(p) = self.picker.as_mut().filter(|p| p.open) {
            p.open = false;
        } else if self.renaming.take().is_some() {
            // The name typed is dropped; the profile keeps its own.
        } else if self.typing {
            self.typing = false;
        } else if self.open.is_some() {
            self.open = None;
            self.target = 0.0;
        } else {
            return false;
        }
        true
    }

    /// A press at `cursor` (GUI pixels): what the game should do, if anything.
    /// `drawn`: the crosshair's pixels and the player indicator's.
    pub(super) fn click(&mut self, cursor: (f32, f32), drawn: [[u16; DRAWN]; 2]) -> Option<Action> {
        let crosshair = drawn[usize::from(self.indicator)];
        let found = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| inside(*r, cursor))
            .copied();
        let hit = found.map(|h| h.1);
        // A name being typed is kept by a click anywhere else.
        if self.renaming.is_some() && hit != Some(Hit::NameField) {
            return self.commit_name();
        }
        if !matches!(hit, Some(Hit::DeleteProfile(_))) {
            self.confirm = None;
        }
        // A number being typed is put on by a click anywhere else.
        if self
            .number
            .as_ref()
            .is_some_and(|(o, _)| hit != Some(Hit::NumberField(*o)))
        {
            self.commit_number();
        }
        if let Some(p) = self.picker.as_mut()
            && hit != Some(Hit::HexField)
        {
            p.hex = None;
        }
        self.typing = hit == Some(Hit::Search);
        match hit? {
            Hit::ColourRow(opt) => {
                match self.picker.as_mut() {
                    Some(p) if p.opt == opt && p.open => p.open = false,
                    _ => {
                        self.picker = Some(Picker {
                            opt,
                            open: true,
                            hsv: [0.0, 0.0, 1.0],
                            hex: None,
                            drag: None,
                            square: [0.0; 4],
                            hue: [0.0; 4],
                        });
                        // Its colour is read on the first frame it draws.
                        self.picker_k = 0.0;
                    }
                }
                None
            }
            Hit::Swatch(colour) => {
                let p = self.picker.as_mut()?;
                if let Some(c) = colour {
                    p.hsv = rgb_to_hsv(c.argb() & 0xFF_FFFF);
                }
                Some(Action::SetColour(p.opt, colour, true))
            }
            Hit::Square | Hit::HueBar => {
                let p = self.picker.as_mut()?;
                p.drag = Some(if hit == Some(Hit::Square) {
                    Drag::Square
                } else {
                    Drag::Hue
                });
                self.drag_picker(cursor);
                None
            }
            Hit::HexField => {
                let p = self.picker.as_mut()?;
                p.hex.get_or_insert_with(String::new);
                None
            }
            Hit::PickerPanel => None,
            Hit::Slider(opt) => {
                let track = found?.0;
                self.slider = Some((opt, track, u32::MAX));
                self.drag_slider(cursor);
                None
            }
            Hit::NumberField(opt) => {
                if self.number.as_ref().is_none_or(|(o, _)| *o != opt) {
                    self.number = Some((opt, String::new()));
                }
                None
            }
            Hit::Tab(tab) => {
                if self.tab != tab {
                    self.tab = tab;
                    self.open = None;
                    (self.scroll, self.target) = (0.0, 0.0);
                }
                None
            }
            Hit::NewProfile => Some(Action::Profile(ProfileOp::Create)),
            Hit::UseProfile(i) => Some(Action::Profile(ProfileOp::Use(i))),
            Hit::RenameProfile(i) => {
                self.renaming = Some((i, String::new()));
                None
            }
            Hit::NameField => None,
            Hit::CopyProfile(i) => Some(Action::Profile(ProfileOp::Duplicate(i))),
            Hit::DeleteProfile(i) => {
                if self.confirm == Some(i) {
                    self.confirm = None;
                    Some(Action::Profile(ProfileOp::Delete(i)))
                } else {
                    self.confirm = Some(i);
                    None
                }
            }
            Hit::Category(c) => {
                self.category = c;
                self.open = None;
                self.target = 0.0;
                None
            }
            Hit::Search => {
                self.open = None;
                None
            }
            Hit::Close => Some(Action::Close),
            Hit::EditHud => Some(Action::EditHud),
            Hit::Toggle(i) => Some(Action::Toggle(i)),
            Hit::Gear(i) => {
                (self.picker, self.number) = (None, None);
                self.open = Some(i);
                self.shown = i;
                self.rows.clear();
                (self.scroll, self.target) = (0.0, 0.0);
                None
            }
            Hit::Back => {
                (self.picker, self.number) = (None, None);
                self.open = None;
                self.target = 0.0;
                None
            }
            Hit::Row(item, back) => Some(Action::Cycle(item, back)),
            Hit::Preset(k) => Some(Action::Preset(k)),
            Hit::ClearGrid => Some(Action::Pixels(self.indicator, [0; DRAWN])),
            Hit::GridTarget(indicator) => {
                self.indicator = indicator;
                None
            }
            Hit::Grid => {
                // A stroke: it paints what the first cell was not, as long as the button is down.
                let mut pixels = crosshair;
                let cell = self.cell(cursor)?;
                let on = pixels[cell.1] >> cell.0 & 1 == 0;
                set_pixel(&mut pixels, cell, on);
                self.pixels = Some(pixels);
                self.paint = Some(on);
                self.last = Some(cell);
                None
            }
        }
    }

    /// The left button let go at `cursor` (GUI pixels): the end of a stroke on the grid, if one
    /// was on.
    pub(super) fn release(&mut self, cursor: (f32, f32)) -> Option<Action> {
        if self.slider.is_some() {
            self.drag_slider(cursor);
            let (opt, _, v) = self.slider.take()?;
            return Some(Action::SetSlider(opt, v, true));
        }
        if self.picker.as_ref().is_some_and(|p| p.drag.is_some()) {
            self.drag_picker(cursor);
            let p = self.picker.as_mut()?;
            p.drag = None;
            return Some(Action::SetColour(
                p.opt,
                Some(Colour::Custom(hsv_to_rgb(p.hsv))),
                true,
            ));
        }
        self.stroke(cursor);
        self.paint = None;
        self.last = None;
        let indicator = self.indicator;
        self.pixels.take().map(|p| Action::Pixels(indicator, p))
    }

    /// The stroke goes on to the cell under `cursor`, painting every cell on the way.
    fn stroke(&mut self, cursor: (f32, f32)) {
        let (Some(on), Some(cell)) = (self.paint, self.cell(cursor)) else {
            return;
        };
        let Some(pixels) = self.pixels.as_mut() else {
            return;
        };
        let from = self.last.unwrap_or(cell);
        let steps = from.0.abs_diff(cell.0).max(from.1.abs_diff(cell.1)).max(1);
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let at = |a: usize, b: usize| (a as f32 + (b as f32 - a as f32) * t).round() as usize;
            set_pixel(pixels, (at(from.0, cell.0), at(from.1, cell.1)), on);
        }
        self.last = Some(cell);
    }

    /// The grid's cell under `cursor` (GUI pixels).
    fn cell(&self, (x, y): (f32, f32)) -> Option<(usize, usize)> {
        let ([gx, gy, gw, gh], side) = self.grid;
        if !inside([gx, gy, gw, gh], (x, y)) {
            return None;
        }
        let cx = ((x - gx) / side) as usize;
        let cy = ((y - gy) / side) as usize;
        (cx < DRAWN && cy < DRAWN).then_some((cx, cy))
    }

    /// A drag on the picker's square or hue bar goes on under `cursor` (GUI pixels): the colour
    /// is put on as it moves, saved when let go (`release`).
    fn drag_picker(&mut self, (x, y): (f32, f32)) {
        let Some(p) = self.picker.as_mut() else {
            return;
        };
        let at = |[rx, ry, rw, rh]: [f32; 4]| {
            (
                ((x - rx) / rw.max(1.0)).clamp(0.0, 1.0),
                ((y - ry) / rh.max(1.0)).clamp(0.0, 1.0),
            )
        };
        let before = p.hsv;
        match p.drag {
            Some(Drag::Square) => {
                let (s, v) = at(p.square);
                (p.hsv[1], p.hsv[2]) = (s, 1.0 - v);
            }
            Some(Drag::Hue) => p.hsv[0] = at(p.hue).1.min(0.999),
            None => return,
        }
        if p.hsv != before {
            let colour = Some(Colour::Custom(hsv_to_rgb(p.hsv)));
            self.pending.push(Action::SetColour(p.opt, colour, false));
        }
    }

    /// A slider's drag goes on under `cursor` (GUI pixels): its value is put on as it moves,
    /// saved when let go (`release`).
    fn drag_slider(&mut self, (x, _): (f32, f32)) {
        let Some((opt, [tx, _, tw, _], v)) = self.slider.as_mut() else {
            return;
        };
        let Some((lo, hi)) = opt.slider() else {
            return;
        };
        let k = ((x - *tx) / tw.max(1.0)).clamp(0.0, 1.0);
        let next = lo + ((hi - lo) as f32 * k).round() as u32;
        if next != *v {
            *v = next;
            self.pending.push(Action::SetSlider(*opt, next, false));
        }
    }

    /// The typed number put on (Enter, or a click elsewhere); nothing typed keeps the value.
    fn commit_number(&mut self) {
        if let Some((opt, digits)) = self.number.take()
            && let Ok(v) = digits.parse::<u32>()
        {
            self.pending.push(Action::SetNumber(opt, v));
        }
    }

    /// Enter: a typed number or colour put on, the search left.
    pub(super) fn enter(&mut self) -> Option<Action> {
        self.commit_number();
        if let Some(p) = self.picker.as_mut() {
            p.hex = None;
        }
        self.typing = false;
        None
    }

    /// What drags and typed values asked since the last frame.
    pub(super) fn take_pending(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.pending)
    }

    /// The name typed for a profile, to give it (Enter, or a click elsewhere). None when nothing
    /// was typed: it keeps its name.
    pub(super) fn commit_name(&mut self) -> Option<Action> {
        let (i, name) = self.renaming.take()?;
        (!name.trim().is_empty()).then_some(Action::Profile(ProfileOp::Rename(i, name)))
    }

    /// Whether the keyboard goes to the page: the search, or a profile's name.
    pub(super) fn wants_keys(&self) -> bool {
        self.typing
            || self.renaming.is_some()
            || self.number.is_some()
            || self.picker.as_ref().is_some_and(|p| p.hex.is_some())
    }

    /// The wheel turned by `lines` (positive: up).
    pub(super) fn wheel(&mut self, lines: f32) {
        self.target = (self.target - lines * 48.0).clamp(0.0, self.room);
    }

    /// A key while the search field has the keyboard: typed text, or Backspace. True when taken.
    pub(super) fn type_key(&mut self, text: Option<&str>, backspace: bool) -> bool {
        // A number: digits only, three at most.
        if let Some((_, digits)) = self.number.as_mut() {
            if backspace {
                digits.pop();
            } else if let Some(text) = text {
                let room = 3usize.saturating_sub(digits.len());
                digits.extend(text.chars().filter(char::is_ascii_digit).take(room));
            }
            return true;
        }
        // A colour: six hex digits, put on once all six are there.
        if let Some(p) = self.picker.as_mut()
            && let Some(hex) = p.hex.as_mut()
        {
            if backspace {
                hex.pop();
            } else if let Some(text) = text {
                let room = 6usize.saturating_sub(hex.len());
                hex.extend(text.chars().filter(char::is_ascii_hexdigit).take(room));
            }
            if hex.len() == 6
                && let Some(c) = Colour::parse(hex)
            {
                p.hsv = rgb_to_hsv(c.argb() & 0xFF_FFFF);
                self.pending.push(Action::SetColour(p.opt, Some(c), true));
            }
            return true;
        }
        if let Some((_, name)) = self.renaming.as_mut() {
            if backspace {
                name.pop();
            } else if let Some(text) = text {
                let room = NAME_MAX.saturating_sub(name.chars().count());
                name.extend(text.chars().filter(|c| !c.is_control()).take(room));
            }
            return true;
        }
        if !self.typing {
            return false;
        }
        if backspace {
            self.search.pop();
        } else if let Some(text) = text {
            let room = 32usize.saturating_sub(self.search.chars().count());
            self.search
                .extend(text.chars().filter(|c| !c.is_control()).take(room));
        }
        self.target = 0.0;
        true
    }

    /// Draws the page over `ui` at `alpha`, `rise` dp below its place (its way in); `dt` is the
    /// frame's time step for the easings, `gui` turns dp into GUI pixels and `cursor` is the
    /// pointer in GUI pixels (for the grid's strokes).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        extras: &Extras,
        alpha: f32,
        rise: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
        cursor: (f32, f32),
        profiles: Option<&Profiles>,
    ) {
        self.hits.clear();
        let a = alpha;
        let d = ui.density;
        let pointer = ui.cursor;
        let (w, h) = (ui.w, ui.h);
        let pw = (w - 48.0).min(880.0);
        let ph = (h - 48.0).min(540.0);
        let (px, py) = ((w - pw) / 2.0, (h - ph) / 2.0 + rise);
        let mut page = Page {
            d,
            pointer,
            gui,
            live: true,
            hits: std::mem::take(&mut self.hits),
        };

        // A stroke on the grid goes on under the pointer while the button is down, as a drag on
        // the colour picker.
        self.stroke(cursor);
        self.drag_picker(cursor);
        self.drag_slider(cursor);

        // The glass panel.
        ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.80 * a));
        ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * a));

        // The header: logo, name, tabs, close.
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
        let tabs = [
            (
                lang::get_or("oteria.mods.tab", "MODS"),
                Tab::Mods,
                "oteria-tab-mods",
            ),
            (
                lang::get_or("oteria.profiles.tab", "PROFILES"),
                Tab::Profiles,
                "oteria-tab-profiles",
            ),
        ];
        let mut tab_x = px + 200.0;
        for (label, tab, id) in &tabs {
            let on = &(self.tab == *tab);
            let lw = ui.measure(Face::Medium, 11.0, label) + 28.0;
            let r = [tab_x, py + (hh - 28.0) / 2.0, lw, 28.0];
            let over = inside(r, pointer);
            let fill = if *on {
                argb(ACCENT, 0.24 * a)
            } else {
                argb(WHITE, if over { 0.10 } else { 0.04 } * a)
            };
            ui.rounded(r[0], r[1], r[2], r[3], 7.0, fill);
            if *on {
                ui.border(r[0], r[1], r[2], r[3], 7.0, argb(ACCENT, 0.55 * a));
            }
            ui.text(
                Face::Medium,
                11.0,
                r[0] + 14.0,
                r[1] + (28.0 - 16.0) / 2.0,
                argb(WHITE, a),
                label,
            );
            if !*on {
                page.hit(r, Hit::Tab(*tab));
                horizon_client::widgets::record(id, r.map(gui), true);
            }
            tab_x += lw + 8.0;
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
        page.icon(
            ui,
            icon::X,
            close[0] + 16.0,
            close[1] + 16.0,
            argb(WHITE, a),
            false,
        );
        page.hit(close, Hit::Close);
        ui.rounded(
            px + 14.0,
            py + hh,
            pw - 28.0,
            1.0,
            0.0,
            argb(WHITE, 0.08 * a),
        );

        // The tabs' crossfade: the mods' body out as the profiles come in, and back.
        let goal = if self.tab == Tab::Profiles { 1.0 } else { 0.0 };
        self.tab_k += (goal - self.tab_k) * (1.0 - (-dt * 14.0).exp());
        if (self.tab_k - goal).abs() < 0.002 {
            self.tab_k = goal;
        }
        let k = ease(self.tab_k);
        let on_mods = self.tab == Tab::Mods;
        self.blink = (self.blink + dt) % 1.0;
        if k < 1.0 {
            let a = a * (1.0 - k);
            page.live = on_mods;
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
                page.icon(
                    ui,
                    category.icon(),
                    r[0] + 22.0,
                    r[1] + 16.0,
                    argb(WHITE, if on || over { 1.0 } else { 0.7 } * a),
                    false,
                );
                let colour = argb(WHITE, if on || over { 1.0 } else { 0.72 } * a);
                ui.text(
                    Face::Medium,
                    12.0,
                    r[0] + 44.0,
                    r[1] + (32.0 - 17.0) / 2.0,
                    colour,
                    &category.name(),
                );
                page.hit(r, Hit::Category(*category));
                horizon_client::widgets::record(&format!("oteria-cat-{k}"), r.map(gui), true);
            }
            let edit = [lx, py + ph - 14.0 - 36.0, lw, 36.0];
            let over = inside(edit, pointer);
            ui.rounded(
                edit[0],
                edit[1],
                edit[2],
                edit[3],
                9.0,
                argb(ACCENT, if over { 0.95 } else { 0.75 } * a),
            );
            page.icon(
                ui,
                icon::LAYOUT,
                edit[0] + 22.0,
                edit[1] + 18.0,
                argb(WHITE, a),
                false,
            );
            let edit_label = lang::get_or("oteria.mods.edit_hud", "Edit HUD");
            ui.text(
                Face::Medium,
                12.0,
                edit[0] + 44.0,
                edit[1] + (36.0 - 17.0) / 2.0,
                argb(WHITE, a),
                &edit_label,
            );
            page.hit(edit, Hit::EditHud);
            horizon_client::widgets::record("oteria-edit-hud", edit.map(gui), true);

            // The search field.
            let ax = lx + lw + 14.0;
            let aw = px + pw - 14.0 - ax;
            let search = [ax + aw - 240.0, ly, 240.0, 32.0];
            ui.rounded(
                search[0],
                search[1],
                search[2],
                search[3],
                9.0,
                argb(WHITE, if self.typing { 0.12 } else { 0.06 } * a),
            );
            if self.typing {
                ui.border(
                    search[0],
                    search[1],
                    search[2],
                    search[3],
                    9.0,
                    argb(ACCENT, 0.7 * a),
                );
            }
            page.icon(
                ui,
                icon::SEARCH,
                search[0] + 18.0,
                search[1] + 16.0,
                argb(WHITE, 0.7 * a),
                false,
            );
            let caret = if self.typing && self.blink < 0.55 {
                "|"
            } else {
                ""
            };
            let (text, colour) = if self.search.is_empty() && !self.typing {
                (
                    lang::get_or("oteria.mods.search", "Search…"),
                    argb(WHITE, 0.45 * a),
                )
            } else {
                (format!("{}{caret}", self.search), argb(WHITE, a))
            };
            ui.text(
                Face::Regular,
                12.0,
                search[0] + 36.0,
                search[1] + (32.0 - 17.0) / 2.0,
                colour,
                &text,
            );
            page.hit(search, Hit::Search);
            horizon_client::widgets::record("oteria-search", search.map(gui), true);

            // The view under the search: the grid, or a tile's settings, crossfading.
            let target = if self.open.is_some() { 1.0 } else { 0.0 };
            self.detail += (target - self.detail) * (1.0 - (-dt * 14.0).exp());
            if (self.detail - target).abs() < 0.002 {
                self.detail = target;
            }
            let view = [ax, ly + 44.0, aw, py + ph - 14.0 - (ly + 44.0)];
            if on_mods {
                self.track = [view[1], view[3]];
            }
            self.scroll += (self.target - self.scroll) * (1.0 - (-dt * 18.0).exp());
            let t = ease(self.detail);
            if t < 1.0 {
                self.draw_grid(
                    ui,
                    &mut page,
                    extras,
                    a * (1.0 - t),
                    dt,
                    view,
                    ly,
                    self.open.is_none() && on_mods,
                );
            }
            if t > 0.0 {
                let slide = (1.0 - t) * 24.0;
                self.draw_detail(
                    ui,
                    &mut page,
                    extras,
                    a * t,
                    dt,
                    view,
                    slide,
                    self.open.is_some() && on_mods,
                );
            }
        }
        if k > 0.0 {
            page.live = !on_mods;
            let view = [
                px + 24.0 + (1.0 - k) * 24.0,
                py + hh + 18.0,
                pw - 48.0,
                ph - hh - 18.0 - 18.0,
            ];
            self.draw_profiles(ui, &mut page, profiles, a * k, dt, view, !on_mods);
        }
        page.live = true;
        // A thin scrollbar when the view's content goes past it.
        if self.room > 0.0 {
            let [gy, gh] = self.track;
            let content = gh + self.room;
            let thumb = (gh / content * gh).max(24.0);
            let ty = gy + (self.scroll / self.room).clamp(0.0, 1.0) * (gh - thumb);
            ui.rounded(px + pw - 9.0, ty, 3.0, thumb, 1.5, argb(WHITE, 0.25 * a));
        }
        self.hits = page.hits;
    }

    /// The profiles tab: its title and Create, then a card for each profile: its name (or the
    /// field its new name is typed in), and Use, Rename, Duplicate and Delete.
    #[allow(clippy::too_many_arguments)]
    fn draw_profiles(
        &mut self,
        ui: &mut Ui,
        page: &mut Page,
        profiles: Option<&Profiles>,
        a: f32,
        dt: f32,
        view: [f32; 4],
        live: bool,
    ) {
        let [vx, vy, vw, vh] = view;
        let (pointer, gui) = (page.pointer, page.gui);
        let follow = 1.0 - (-dt * 16.0).exp();
        let title = lang::get_or("oteria.profiles.title", "Profiles");
        ui.text(Face::SemiBold, 16.0, vx, vy + 2.0, argb(WHITE, a), &title);
        let sub = lang::get_or(
            "oteria.profiles.subtitle",
            "Each profile keeps its own mods, HUD and crosshair.",
        );
        let sub = fit(ui, Face::Regular, 12.0, &sub, vw - 240.0);
        ui.text(
            Face::Regular,
            12.0,
            vx,
            vy + 26.0,
            argb(WHITE, 0.55 * a),
            &sub,
        );
        // Create: green, with its plus.
        let create = [vx + vw - 210.0, vy + 4.0, 210.0, 38.0];
        let over = live && inside(create, pointer);
        ui.rounded(
            create[0],
            create[1],
            create[2],
            create[3],
            9.0,
            argb(ON, if over { 1.0 } else { 0.82 } * a),
        );
        page.icon(
            ui,
            icon::PLUS,
            create[0] + 24.0,
            create[1] + 19.0,
            argb(WHITE, a),
            false,
        );
        let label = lang::get_or("oteria.profiles.create", "Create a profile");
        ui.text(
            Face::Medium,
            12.0,
            create[0] + 44.0,
            create[1] + (38.0 - 17.0) / 2.0,
            argb(WHITE, a),
            &label,
        );
        page.hit(create, Hit::NewProfile);
        if live {
            horizon_client::widgets::record("oteria-profile-new", create.map(gui), true);
        }
        let Some(profiles) = profiles else {
            return;
        };

        // The cards, scrolled under the title.
        let (ly, lh) = (vy + 60.0, vh - 60.0);
        let n = profiles.names.len();
        self.cards.resize(n, 0.0);
        let (ch, gap, bs) = (60.0, 10.0, 34.0);
        let scroll = if live {
            self.track = [ly, lh];
            self.room = (n as f32 * (ch + gap) - gap - lh).max(0.0);
            self.target = self.target.clamp(0.0, self.room);
            self.scroll
        } else {
            0.0
        };
        let within = |r: [f32; 4]| live && r[1] + r[3] > ly && r[1] < ly + lh;
        let clipped = |r: [f32; 4]| {
            [
                r[0],
                r[1].max(ly),
                r[2],
                (r[1] + r[3]).min(ly + lh) - r[1].max(ly),
            ]
        };
        let start = ui.batch.vertices.len();
        for (i, name) in profiles.names.iter().enumerate() {
            let r = [vx, ly + i as f32 * (ch + gap) - scroll, vw, ch];
            if r[1] + ch < ly || r[1] > ly + lh {
                continue;
            }
            let active = i == profiles.active;
            let over = within(r) && inside(r, pointer) && pointer.1 >= ly && pointer.1 < ly + lh;
            self.cards[i] += ((if over { 1.0 } else { 0.0 }) - self.cards[i]) * follow;
            let hv = self.cards[i];
            if active {
                ui.rounded(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    10.0,
                    argb(ACCENT, (0.14 + 0.04 * hv) * a),
                );
                ui.border(r[0], r[1], r[2], r[3], 10.0, argb(ACCENT, 0.6 * a));
                ui.rounded(r[0], r[1] + 16.0, 3.0, ch - 32.0, 1.5, argb(ACCENT, a));
            } else {
                ui.rounded(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    10.0,
                    argb(WHITE, (0.05 + 0.04 * hv) * a),
                );
                ui.border(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    10.0,
                    argb(WHITE, (0.07 + 0.12 * hv) * a),
                );
            }
            let (ax, ay) = (r[0] + 18.0, r[1] + (ch - 36.0) / 2.0);
            ui.rounded(
                ax,
                ay,
                36.0,
                36.0,
                18.0,
                argb(
                    if active { ACCENT } else { WHITE },
                    if active { 0.35 } else { 0.08 } * a,
                ),
            );
            page.icon(ui, icon::USER, ax + 18.0, ay + 18.0, argb(WHITE, a), false);

            // The buttons, from the right: Delete, Duplicate, Rename, and Use for the others.
            let by = r[1] + (ch - bs) / 2.0;
            let mut buttons: Vec<([f32; 4], Hit, String)> = Vec::new();
            let confirm = self.confirm == Some(i);
            let can_delete = n > 1;
            let confirm_label = lang::get_or("oteria.profiles.confirm", "Delete?");
            let dw = if confirm {
                ui.measure(Face::Medium, 11.0, &confirm_label) + 28.0
            } else {
                bs
            };
            let mut bx = r[0] + vw - 14.0 - dw;
            let del = [bx, by, dw, bs];
            let del_over = within(del) && inside(del, pointer) && can_delete;
            let fill = if confirm {
                argb(OFF, if del_over { 1.0 } else { 0.85 } * a)
            } else if del_over {
                argb(OFF, 0.8 * a)
            } else {
                argb(WHITE, 0.07 * a)
            };
            ui.rounded(del[0], del[1], del[2], del[3], 8.0, fill);
            if confirm {
                ui.text_centered(
                    Face::Medium,
                    11.0,
                    del[0] + dw / 2.0,
                    del[1] + (bs - 16.0) / 2.0,
                    argb(WHITE, a),
                    &confirm_label,
                );
            } else {
                page.icon(
                    ui,
                    icon::TRASH,
                    del[0] + bs / 2.0,
                    del[1] + bs / 2.0,
                    argb(WHITE, if can_delete { 0.85 } else { 0.3 } * a),
                    false,
                );
            }
            if can_delete {
                buttons.push((
                    del,
                    Hit::DeleteProfile(i),
                    format!("oteria-profile-{i}-delete"),
                ));
            }
            for (sprite, hit, id) in [
                (icon::COPY, Hit::CopyProfile(i), "copy"),
                (icon::PENCIL, Hit::RenameProfile(i), "rename"),
            ] {
                bx -= 8.0 + bs;
                let b = [bx, by, bs, bs];
                let over = within(b) && inside(b, pointer);
                ui.rounded(
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    8.0,
                    argb(WHITE, if over { 0.16 } else { 0.07 } * a),
                );
                page.icon(
                    ui,
                    sprite,
                    b[0] + bs / 2.0,
                    b[1] + bs / 2.0,
                    argb(WHITE, 0.85 * a),
                    false,
                );
                buttons.push((b, hit, format!("oteria-profile-{i}-{id}")));
            }
            if !active {
                let use_label = lang::get_or("oteria.profiles.use", "Use");
                let uw = ui.measure(Face::Medium, 11.0, &use_label) + 36.0;
                bx -= 8.0 + uw;
                let b = [bx, by, uw, bs];
                let over = within(b) && inside(b, pointer);
                ui.rounded(
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    8.0,
                    argb(ACCENT, if over { 0.95 } else { 0.7 } * a),
                );
                ui.text_centered(
                    Face::Medium,
                    11.0,
                    b[0] + uw / 2.0,
                    b[1] + (bs - 16.0) / 2.0,
                    argb(WHITE, a),
                    &use_label,
                );
                buttons.push((b, Hit::UseProfile(i), format!("oteria-profile-{i}-use")));
            }

            // The name, or the field its new one is typed in (the old one shown faintly until
            // something is typed).
            let nx = ax + 36.0 + 14.0;
            let room = bx - 14.0 - nx;
            match &self.renaming {
                Some((k, typed)) if *k == i => {
                    let field = [nx - 8.0, r[1] + (ch - 34.0) / 2.0, room.min(300.0), 34.0];
                    ui.rounded(
                        field[0],
                        field[1],
                        field[2],
                        field[3],
                        8.0,
                        argb(WHITE, 0.10 * a),
                    );
                    ui.border(
                        field[0],
                        field[1],
                        field[2],
                        field[3],
                        8.0,
                        argb(ACCENT, 0.8 * a),
                    );
                    let ty = field[1] + (34.0 - 20.0) / 2.0;
                    let caret_x = if typed.is_empty() {
                        ui.text(Face::Medium, 14.0, nx, ty, argb(WHITE, 0.3 * a), name);
                        nx
                    } else {
                        ui.text(Face::Medium, 14.0, nx, ty, argb(WHITE, a), typed);
                        nx + ui.measure(Face::Medium, 14.0, typed)
                    };
                    if self.blink < 0.55 {
                        ui.rounded(caret_x + 1.0, ty + 1.0, 1.5, 18.0, 0.0, argb(WHITE, a));
                    }
                    buttons.push((field, Hit::NameField, format!("oteria-profile-{i}-name")));
                }
                _ => {
                    let label = fit(ui, Face::Medium, 14.0, name, room);
                    ui.text(Face::Medium, 14.0, nx, r[1] + 11.0, argb(WHITE, a), &label);
                    let (state, colour) = if active {
                        (
                            lang::get_or("oteria.profiles.active", "In use"),
                            argb(ACCENT, a),
                        )
                    } else {
                        (
                            lang::get_or("oteria.profiles.inactive", "Click to use"),
                            argb(WHITE, 0.5 * a),
                        )
                    };
                    ui.text(Face::Regular, 11.0, nx, r[1] + 32.0, colour, &state);
                }
            }
            // The card under its buttons: a press on it puts it on.
            if within(r) {
                if !active {
                    page.hit(clipped(r), Hit::UseProfile(i));
                }
                horizon_client::widgets::record(
                    &format!("oteria-profile-{i}"),
                    clipped(r).map(gui),
                    true,
                );
                for (b, hit, id) in buttons {
                    if within(b) {
                        page.hit(clipped(b), hit);
                        horizon_client::widgets::record(&id, clipped(b).map(gui), true);
                    }
                }
            }
        }
        ui.batch.clip_since(
            start,
            gui(vx - 2.0),
            gui(ly),
            gui(vx + vw + 2.0),
            gui(ly + lh),
        );
    }

    /// The grid of tiles, with the count of what shows above it.
    #[allow(clippy::too_many_arguments)]
    fn draw_grid(
        &mut self,
        ui: &mut Ui,
        page: &mut Page,
        extras: &Extras,
        a: f32,
        dt: f32,
        view: [f32; 4],
        ly: f32,
        live: bool,
    ) {
        let [gx, gy, aw, gh] = view;
        let pointer = page.pointer;
        let gui = page.gui;
        let follow = 1.0 - (-dt * 16.0).exp();
        let shown = self.shown();
        let count = format!("{} mods", shown.len());
        ui.text(
            Face::Medium,
            12.0,
            gx + 2.0,
            ly + (32.0 - 17.0) / 2.0,
            argb(WHITE, 0.55 * a),
            &count,
        );
        let gap = 10.0;
        let cols = (((aw + gap) / (176.0 + gap)).floor() as usize).clamp(2, 5);
        let tw = (aw - gap * (cols - 1) as f32) / cols as f32;
        let th = 116.0;
        let rows = shown.len().div_ceil(cols);
        let content = rows as f32 * (th + gap) - gap;
        let scroll = if live { self.scroll } else { 0.0 };
        if live {
            self.room = (content - gh).max(0.0);
            self.target = self.target.clamp(0.0, self.room);
        }
        let start = ui.batch.vertices.len();
        for (n, &i) in shown.iter().enumerate() {
            let (col, row) = (n % cols, n / cols);
            let r = [
                gx + col as f32 * (tw + gap),
                gy + row as f32 * (th + gap) - scroll,
                tw,
                th,
            ];
            if r[1] + th < gy || r[1] > gy + gh {
                continue;
            }
            let tile = &TILES[i];
            let on = (tile.get)(extras);
            let over = live && inside(r, pointer) && pointer.1 >= gy && pointer.1 < gy + gh;
            self.hover[i] += ((if over { 1.0 } else { 0.0 }) - self.hover[i]) * follow;
            let sw = self.switch[i].get_or_insert(if on { 1.0 } else { 0.0 });
            *sw += ((if on { 1.0 } else { 0.0 }) - *sw) * (1.0 - (-dt * 14.0).exp());
            let (hv, sv) = (self.hover[i], *sw);
            ui.rounded(
                r[0],
                r[1],
                r[2],
                r[3],
                10.0,
                argb(WHITE, (0.05 + 0.04 * hv) * a),
            );
            ui.border(
                r[0],
                r[1],
                r[2],
                r[3],
                10.0,
                argb(WHITE, (0.07 + 0.14 * hv) * a),
            );
            let cx = r[0] + tw / 2.0;
            page.icon(
                ui,
                tile.icon,
                cx,
                r[1] + 30.0,
                argb(WHITE, (0.75 + 0.25 * hv) * a),
                true,
            );
            let name = tile.name();
            let nw = ui.measure(Face::Medium, 13.0, &name);
            ui.text(
                Face::Medium,
                13.0,
                cx - nw / 2.0,
                r[1] + 50.0,
                argb(WHITE, (0.82 + 0.18 * hv) * a),
                &name,
            );
            let by = r[1] + th - 10.0 - 30.0;
            let gear = [r[0] + 10.0, by, 30.0, 30.0];
            let gear_over = over && inside(gear, pointer);
            ui.rounded(
                gear[0],
                gear[1],
                gear[2],
                gear[3],
                7.0,
                argb(WHITE, if gear_over { 0.16 } else { 0.07 } * a),
            );
            page.icon(
                ui,
                icon::SETTINGS,
                gear[0] + 15.0,
                gear[1] + 15.0,
                argb(WHITE, 0.85 * a),
                false,
            );
            let bar = [
                gear[0] + 36.0,
                by,
                r[0] + tw - 10.0 - (gear[0] + 36.0),
                30.0,
            ];
            let bar_over = over && inside(bar, pointer);
            let colour = mix(OFF, ON, sv);
            ui.rounded(
                bar[0],
                bar[1],
                bar[2],
                bar[3],
                7.0,
                argb(colour, (if bar_over { 0.95 } else { 0.80 }) * a),
            );
            let label = if on {
                lang::get_or("oteria.mods.enabled", "ENABLED")
            } else {
                lang::get_or("oteria.mods.disabled", "DISABLED")
            };
            let lw = ui.measure(Face::Medium, 11.0, &label);
            ui.text(
                Face::Medium,
                11.0,
                bar[0] + (bar[2] - lw) / 2.0,
                bar[1] + (30.0 - 16.0) / 2.0,
                argb(WHITE, a),
                &label,
            );
            if live {
                let visible = |b: [f32; 4]| {
                    [
                        b[0],
                        b[1].max(gy),
                        b[2],
                        (b[1] + b[3]).min(gy + gh) - b[1].max(gy),
                    ]
                };
                page.hit(visible(bar), Hit::Toggle(i));
                page.hit(visible(gear), Hit::Gear(i));
                // Named for the tools that drive the game (`horizon-ctl click`).
                horizon_client::widgets::record(tile.key, visible(bar).map(gui), true);
                horizon_client::widgets::record(
                    &format!("{}.gear", tile.key),
                    visible(gear).map(gui),
                    true,
                );
                // A press anywhere else on the tile turns it on or off too.
                let body = visible([r[0], r[1], r[2], by - r[1]]);
                if body[3] > 0.0 {
                    page.hit(body, Hit::Toggle(i));
                }
            }
        }
        ui.batch.clip_since(
            start,
            gui(gx - 2.0),
            gui(gy),
            gui(gx + aw + 2.0),
            gui(gy + gh),
        );
        if shown.is_empty() {
            let none = lang::get_or("oteria.mods.none", "No mod matches.");
            let nw = ui.measure(Face::Regular, 13.0, &none);
            ui.text(
                Face::Regular,
                13.0,
                gx + (aw - nw) / 2.0,
                gy + 40.0,
                argb(WHITE, 0.55 * a),
                &none,
            );
        }
    }

    /// The open tile's settings: its name and switch, the crosshair's presets and grid, then
    /// its options, scrolled within the view; the hovered option's description at the foot.
    #[allow(clippy::too_many_arguments)]
    fn draw_detail(
        &mut self,
        ui: &mut Ui,
        page: &mut Page,
        extras: &Extras,
        a: f32,
        dt: f32,
        view: [f32; 4],
        slide: f32,
        live: bool,
    ) {
        let [vx, vy, vw, vh] = view;
        let (vx, pointer, gui) = (vx + slide, page.pointer, page.gui);
        let tile = &TILES[self.shown];
        let ease_k = 1.0 - (-dt * 14.0).exp();

        // The head: back, icon, name, and the tile's own switch.
        let back = [vx, vy, 36.0, 36.0];
        let over = live && inside(back, pointer);
        ui.rounded(
            back[0],
            back[1],
            back[2],
            back[3],
            9.0,
            argb(WHITE, if over { 0.14 } else { 0.06 } * a),
        );
        ui.text_centered(
            Face::Medium,
            18.0,
            back[0] + 18.0,
            back[1] + 6.0,
            argb(WHITE, a),
            "‹",
        );
        page.icon(
            ui,
            tile.icon,
            vx + 36.0 + 26.0,
            vy + 18.0,
            argb(WHITE, a),
            true,
        );
        ui.text(
            Face::SemiBold,
            16.0,
            vx + 36.0 + 48.0,
            vy + 7.0,
            argb(WHITE, a),
            &tile.name(),
        );
        let on = (tile.get)(extras);
        let sw = self.switch[self.shown].get_or_insert(if on { 1.0 } else { 0.0 });
        *sw += ((if on { 1.0 } else { 0.0 }) - *sw) * ease_k;
        let bar = [vx + vw - 150.0, vy + 3.0, 150.0, 30.0];
        let bar_over = live && inside(bar, pointer);
        ui.rounded(
            bar[0],
            bar[1],
            bar[2],
            bar[3],
            7.0,
            argb(mix(OFF, ON, *sw), if bar_over { 0.95 } else { 0.8 } * a),
        );
        let label = if on {
            lang::get_or("oteria.mods.enabled", "ENABLED")
        } else {
            lang::get_or("oteria.mods.disabled", "DISABLED")
        };
        let lw = ui.measure(Face::Medium, 11.0, &label);
        ui.text(
            Face::Medium,
            11.0,
            bar[0] + (bar[2] - lw) / 2.0,
            bar[1] + 7.0,
            argb(WHITE, a),
            &label,
        );
        if live {
            page.hit(back, Hit::Back);
            page.hit(bar, Hit::Toggle(self.shown));
            horizon_client::widgets::record("oteria-back", back.map(gui), true);
        }

        // The scrolled body, under the head and above the description's line.
        let (by, bh) = (vy + 48.0, vh - 48.0 - 30.0);
        let scroll = if live { self.scroll } else { 0.0 };
        let mut y = by - scroll;
        let start = ui.batch.vertices.len();
        let within = |r: [f32; 4]| live && r[1] + r[3] > by && r[1] < by + bh;
        let clipped = |r: [f32; 4]| {
            [
                r[0],
                r[1].max(by),
                r[2],
                (r[1] + r[3]).min(by + bh) - r[1].max(by),
            ]
        };
        let section = |ui: &mut Ui, y: f32, text: &str| {
            ui.text(
                Face::Medium,
                11.0,
                vx + 2.0,
                y,
                argb(WHITE, 0.5 * a),
                &text.to_uppercase(),
            );
        };

        if tile.is_crosshair() {
            let c = &extras.pvp.crosshair;
            // Five ready-made crosshairs, each drawn as it will look, larger.
            section(ui, y, &lang::get_or("oteria.crosshair.presets", "Presets"));
            y += 22.0;
            let presets = pvp::crosshair_presets();
            let gap = 10.0;
            let cw = ((vw - gap * 4.0) / 5.0).min(110.0);
            for (k, (key, english, preset)) in presets.iter().enumerate() {
                let r = [vx + k as f32 * (cw + gap), y, cw, 86.0];
                let chosen = c.custom
                    && (c.shape, c.size, c.gap, c.thickness, c.dot)
                        == (
                            preset.shape,
                            preset.size,
                            preset.gap,
                            preset.thickness,
                            preset.dot,
                        );
                let over = within(r) && inside(r, pointer);
                ui.rounded(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    10.0,
                    argb(
                        if chosen { ACCENT } else { WHITE },
                        if chosen {
                            0.2
                        } else if over {
                            0.1
                        } else {
                            0.05
                        } * a,
                    ),
                );
                ui.border(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    10.0,
                    argb(
                        if chosen { ACCENT } else { WHITE },
                        if chosen { 0.7 } else { 0.08 } * a,
                    ),
                );
                draw_mask(
                    ui,
                    &preset.mask(),
                    r[0] + cw / 2.0,
                    r[1] + 32.0,
                    2.0,
                    argb(WHITE, a),
                );
                let name = lang::get_or(key, english);
                let nw = ui.measure(Face::Medium, 11.0, &name);
                ui.text(
                    Face::Medium,
                    11.0,
                    r[0] + (cw - nw) / 2.0,
                    r[1] + 62.0,
                    argb(WHITE, 0.85 * a),
                    &name,
                );
                if within(r) {
                    page.hit(clipped(r), Hit::Preset(k));
                    horizon_client::widgets::record(
                        &format!("oteria-preset-{k}"),
                        clipped(r).map(gui),
                        true,
                    );
                }
            }
            y += 86.0 + 18.0;

            // The drawing grid: a click flips a pixel, a drag paints the same way.
            section(
                ui,
                y,
                &lang::get_or("oteria.crosshair.draw", "Draw your own"),
            );
            y += 22.0;
            // Which drawing: the crosshair, or the player indicator over it.
            let mut tx = vx;
            for (indicator, key, english) in [
                (false, "oteria.crosshair.grid_crosshair", "Crosshair"),
                (true, "oteria.crosshair.grid_indicator", "Player indicator"),
            ] {
                let label = lang::get_or(key, english);
                let tw = ui.measure(Face::Medium, 11.0, &label) + 24.0;
                let r = [tx, y, tw, 24.0];
                let chosen = self.indicator == indicator;
                let over = within(r) && inside(r, pointer);
                ui.rounded(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    7.0,
                    if chosen {
                        argb(ACCENT, 0.85 * a)
                    } else {
                        argb(WHITE, if over { 0.14 } else { 0.07 } * a)
                    },
                );
                ui.text(Face::Medium, 11.0, r[0] + 12.0, r[1] + 5.0, argb(WHITE, a), &label);
                if within(r) {
                    page.hit(clipped(r), Hit::GridTarget(indicator));
                    horizon_client::widgets::record(
                        if indicator { "oteria-grid-indicator" } else { "oteria-grid-crosshair" },
                        clipped(r).map(gui),
                        true,
                    );
                }
                tx += tw + 8.0;
            }
            y += 34.0;
            let own = if self.indicator { c.indicator_pixels } else { c.pixels };
            let pixels = self.pixels.unwrap_or(own);
            let cell = 13.0;
            let side = cell * DRAWN as f32;
            let grid = [vx, y, side, side];
            ui.rounded(
                grid[0] - 4.0,
                grid[1] - 4.0,
                side + 8.0,
                side + 8.0,
                8.0,
                argb(0x000000, 0.35 * a),
            );
            for (gy_, row) in pixels.iter().enumerate() {
                for gx_ in 0..DRAWN {
                    let set = row >> gx_ & 1 == 1;
                    let (x0, y0) = (grid[0] + gx_ as f32 * cell, grid[1] + gy_ as f32 * cell);
                    let middle = gx_ == DRAWN / 2 && gy_ == DRAWN / 2;
                    let fill = if set {
                        argb(WHITE, 0.95 * a)
                    } else {
                        argb(
                            if middle { ACCENT } else { WHITE },
                            if middle { 0.25 } else { 0.06 } * a,
                        )
                    };
                    ui.rounded(x0 + 0.5, y0 + 0.5, cell - 1.0, cell - 1.0, 2.0, fill);
                }
            }
            if within(grid) {
                page.hit(clipped(grid), Hit::Grid);
                horizon_client::widgets::record(
                    "oteria-crosshair-grid",
                    clipped(grid).map(gui),
                    true,
                );
            }
            self.grid = (grid.map(gui), gui(cell));
            // Beside it: how it looks at its real size, and a button to start again.
            let (sx, sy) = (vx + side + 30.0, y);
            let real = lang::get_or("oteria.crosshair.preview", "Real size");
            ui.text(Face::Regular, 11.0, sx, sy, argb(WHITE, 0.6 * a), &real);
            ui.rounded(sx, sy + 20.0, 64.0, 64.0, 8.0, argb(0x6E8B5A, 0.9 * a));
            // The indicator shows over the crosshair as it is; the crosshair as drawn.
            let (under, over_mask) = if self.indicator {
                let crosshair = if c.custom {
                    c.mask()
                } else {
                    pvp::pixels_mask(&pvp::plus_pixels())
                };
                (crosshair, Some(pvp::pixels_mask(&pixels)))
            } else {
                let drawn = pvp::Crosshair {
                    shape: pvp::Shape::Drawn,
                    pixels,
                    dot: false,
                    ..*c
                };
                (drawn.mask(), None)
            };
            let scale = ui_gui_scale(ui) / page.d;
            draw_mask(ui, &under, sx + 32.0, sy + 52.0, scale, argb(WHITE, a));
            if let Some(m) = &over_mask {
                draw_mask(ui, m, sx + 32.0, sy + 52.0, scale, argb(ACCENT, a));
            }
            let clear = [sx, sy + 96.0, 120.0, 30.0];
            let over = within(clear) && inside(clear, pointer);
            ui.rounded(
                clear[0],
                clear[1],
                clear[2],
                clear[3],
                7.0,
                argb(WHITE, if over { 0.14 } else { 0.07 } * a),
            );
            let clear_label = lang::get_or("oteria.crosshair.clear", "Clear");
            let cw = ui.measure(Face::Medium, 11.0, &clear_label);
            ui.text(
                Face::Medium,
                11.0,
                clear[0] + (120.0 - cw) / 2.0,
                clear[1] + 7.0,
                argb(WHITE, a),
                &clear_label,
            );
            if within(clear) {
                page.hit(clipped(clear), Hit::ClearGrid);
            }
            let hint = if self.indicator {
                lang::get_or(
                    "oteria.crosshair.hint_indicator",
                    "Shown over the crosshair while it is on a player you can hit.",
                )
            } else {
                lang::get_or(
                    "oteria.crosshair.hint",
                    "Click or drag to paint. Drawing switches the shape to Drawn.",
                )
            };
            let hint = fit(ui, Face::Regular, 11.0, &hint, vw - side - 30.0);
            ui.text(
                Face::Regular,
                11.0,
                sx,
                sy + 140.0,
                argb(WHITE, 0.5 * a),
                &hint,
            );
            y += side + 22.0;
        }

        // The options: a switch, or a value with arrows either side.
        let items: Vec<Item> = tile
            .settings
            .iter()
            .copied()
            .filter(|i| i.key_name().is_none())
            .collect();
        if self.rows.len() != items.len() {
            self.rows = items
                .iter()
                .map(|i| {
                    if i.switch(extras) == Some(true) {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect();
        }
        if !items.is_empty() {
            section(ui, y, &lang::get_or("oteria.mods.options", "Options"));
            y += 22.0;
        } else if !tile.is_crosshair() {
            let none = lang::get_or(
                "oteria.mods.no_options",
                "Nothing else to set: the switch above turns it on or off.",
            );
            ui.text(
                Face::Regular,
                12.0,
                vx + 2.0,
                y,
                argb(WHITE, 0.55 * a),
                &none,
            );
        }
        // The picker eases open and shut; once shut it goes.
        if let Some(p) = self.picker.as_ref() {
            let goal = if p.open { 1.0 } else { 0.0 };
            self.picker_k += (goal - self.picker_k) * (1.0 - (-dt * 16.0).exp());
            if !p.open && self.picker_k < 0.01 {
                self.picker = None;
                self.picker_k = 0.0;
            }
        }
        if self
            .picker
            .as_ref()
            .is_some_and(|p| !items.contains(&Item::Pvp(p.opt)))
        {
            self.picker = None;
        }
        let mut hovered = None;
        for (n, item) in items.iter().enumerate() {
            let r = [vx, y, vw, 40.0];
            let over = within(r) && inside(r, pointer);
            if over {
                hovered = Some(*item);
            }
            ui.rounded(
                r[0],
                r[1],
                r[2],
                r[3],
                8.0,
                argb(WHITE, if over { 0.08 } else { 0.04 } * a),
            );
            ui.text(
                Face::Medium,
                12.0,
                r[0] + 14.0,
                r[1] + 11.0,
                argb(WHITE, 0.92 * a),
                &caption(*item),
            );
            match item.switch(extras) {
                Some(state) => {
                    self.rows[n] += ((if state { 1.0 } else { 0.0 }) - self.rows[n]) * ease_k;
                    let k = self.rows[n];
                    let track = [r[0] + vw - 14.0 - 40.0, r[1] + 10.0, 40.0, 20.0];
                    ui.rounded(
                        track[0],
                        track[1],
                        track[2],
                        track[3],
                        10.0,
                        argb(mix(GREY, ON, k), a),
                    );
                    let knob = track[0] + 2.0 + k * 20.0;
                    ui.rounded(knob, track[1] + 2.0, 16.0, 16.0, 8.0, argb(WHITE, a));
                    if within(r) {
                        page.hit(clipped(r), Hit::Row(*item, false));
                    }
                }
                None if let Item::Pvp(opt) = *item
                    && let (Some((lo, hi)), Some(now)) =
                        (opt.slider(), opt.slider_value(&extras.pvp)) =>
                {
                    // A track with a knob, dragged; the value beside it.
                    let dragging = self.slider.filter(|(o, _, _)| *o == opt).map(|(_, _, v)| v);
                    let v = dragging.unwrap_or(now);
                    let tw = 200.0;
                    let track = [r[0] + vw - 14.0 - tw, r[1] + 18.0, tw, 4.0];
                    let k = (v.saturating_sub(lo)) as f32 / (hi - lo).max(1) as f32;
                    let grab = [track[0] - 8.0, r[1] + 6.0, tw + 16.0, 28.0];
                    let over = within(grab) && inside(grab, pointer);
                    ui.rounded(
                        track[0],
                        track[1],
                        track[2],
                        track[3],
                        2.0,
                        argb(WHITE, 0.14 * a),
                    );
                    ui.rounded(
                        track[0],
                        track[1],
                        track[2] * k,
                        track[3],
                        2.0,
                        argb(ACCENT, a),
                    );
                    let knob = if dragging.is_some() || over {
                        14.0
                    } else {
                        12.0
                    };
                    let kx = track[0] + track[2] * k;
                    ui.rounded(
                        kx - knob / 2.0,
                        track[1] + 2.0 - knob / 2.0,
                        knob,
                        knob,
                        knob / 2.0,
                        argb(WHITE, a),
                    );
                    let mut shown = extras.pvp.clone();
                    opt.set_slider(&mut shown, v);
                    let value = opt.value(&shown).unwrap_or_default();
                    let lw = ui.measure(Face::Medium, 12.0, &value);
                    ui.text(
                        Face::Medium,
                        12.0,
                        track[0] - 14.0 - lw,
                        r[1] + 11.0,
                        argb(WHITE, a),
                        &value,
                    );
                    if within(r) {
                        page.hit(clipped(grab), Hit::Slider(opt));
                    }
                }
                None if let Item::Pvp(opt) = *item
                    && let Some(colour) = opt.colour(&extras.pvp) =>
                {
                    // The colour's swatch and name, and a chevron that turns as its picker opens.
                    let open = self.picker.as_ref().is_some_and(|p| p.opt == opt);
                    let k = if open { ease(self.picker_k) } else { 0.0 };
                    let pill = [r[0] + vw - 14.0 - 170.0, r[1] + 6.0, 170.0, 28.0];
                    let over_pill = within(pill) && inside(pill, pointer);
                    ui.rounded(
                        pill[0],
                        pill[1],
                        pill[2],
                        pill[3],
                        7.0,
                        argb(WHITE, if over_pill || open { 0.12 } else { 0.07 } * a),
                    );
                    if open {
                        ui.border(
                            pill[0],
                            pill[1],
                            pill[2],
                            pill[3],
                            7.0,
                            argb(ACCENT, 0.6 * k * a),
                        );
                    }
                    swatch(
                        ui,
                        page,
                        colour,
                        [pill[0] + 8.0, pill[1] + 6.0, 16.0, 16.0],
                        a,
                    );
                    let name = colour.map_or_else(|| lang::get_or("Off", "Off"), Colour::label);
                    let name = fit(ui, Face::Medium, 12.0, &name, pill[2] - 56.0);
                    ui.text(
                        Face::Medium,
                        12.0,
                        pill[0] + 32.0,
                        pill[1] + 6.0,
                        argb(WHITE, a),
                        &name,
                    );
                    chevron(
                        ui,
                        page,
                        pill[0] + pill[2] - 14.0,
                        pill[1] + 14.0,
                        k,
                        argb(WHITE, 0.7 * a),
                    );
                    if within(r) {
                        page.hit(clipped(r), Hit::ColourRow(opt));
                    }
                }
                None if let Item::Pvp(opt) = *item
                    && let (Some((_, unit)), Some(v)) =
                        (opt.number(), opt.number_value(&extras.pvp)) =>
                {
                    // ‹ a field for the digits ›: the arrows step, the field is typed in.
                    let pill = [r[0] + vw - 14.0 - 170.0, r[1] + 6.0, 170.0, 28.0];
                    let left = [pill[0], pill[1], 30.0, pill[3]];
                    let right = [pill[0] + pill[2] - 30.0, pill[1], 30.0, pill[3]];
                    let field = [pill[0] + 32.0, pill[1] + 2.0, pill[2] - 64.0, pill[3] - 4.0];
                    let typing = self
                        .number
                        .as_ref()
                        .filter(|(o, _)| *o == opt)
                        .map(|(_, d)| d.clone());
                    let lo = within(left) && inside(left, pointer);
                    let hi = within(right) && inside(right, pointer);
                    let over_field = within(field) && inside(field, pointer);
                    ui.rounded(
                        pill[0],
                        pill[1],
                        pill[2],
                        pill[3],
                        7.0,
                        argb(WHITE, 0.07 * a),
                    );
                    ui.rounded(
                        field[0],
                        field[1],
                        field[2],
                        field[3],
                        6.0,
                        argb(
                            WHITE,
                            if typing.is_some() {
                                0.14
                            } else if over_field {
                                0.10
                            } else {
                                0.04
                            } * a,
                        ),
                    );
                    if typing.is_some() {
                        ui.border(
                            field[0],
                            field[1],
                            field[2],
                            field[3],
                            6.0,
                            argb(ACCENT, 0.75 * a),
                        );
                    }
                    ui.text_centered(
                        Face::Medium,
                        15.0,
                        pill[0] + 12.0,
                        pill[1] + 4.0,
                        argb(WHITE, if lo { 1.0 } else { 0.55 } * a),
                        "‹",
                    );
                    ui.text_centered(
                        Face::Medium,
                        15.0,
                        pill[0] + pill[2] - 12.0,
                        pill[1] + 4.0,
                        argb(WHITE, if hi { 1.0 } else { 0.55 } * a),
                        "›",
                    );
                    let text = match &typing {
                        Some(d) => {
                            let caret = if self.blink < 0.55 { "|" } else { " " };
                            format!("{d}{caret}")
                        }
                        // Low fire at 0 is off.
                        None if opt == Opt::FireLower && v == 0 => lang::get_or("Off", "Off"),
                        None => format!("{v}{unit}"),
                    };
                    ui.text_centered(
                        Face::Medium,
                        12.0,
                        field[0] + field[2] / 2.0,
                        pill[1] + 6.0,
                        argb(WHITE, a),
                        &text,
                    );
                    if within(r) {
                        page.hit(clipped(r), Hit::Row(*item, false));
                        page.hit(clipped(left), Hit::Row(*item, true));
                        page.hit(clipped(field), Hit::NumberField(opt));
                    }
                }
                None => {
                    let value = item.value(extras);
                    let pill = [r[0] + vw - 14.0 - 170.0, r[1] + 6.0, 170.0, 28.0];
                    ui.rounded(
                        pill[0],
                        pill[1],
                        pill[2],
                        pill[3],
                        7.0,
                        argb(WHITE, 0.07 * a),
                    );
                    let half = [pill[0], pill[1], pill[2] / 2.0, pill[3]];
                    let right = [pill[0] + pill[2] / 2.0, pill[1], pill[2] / 2.0, pill[3]];
                    let lo = within(half) && inside(half, pointer);
                    let hi = within(right) && inside(right, pointer);
                    ui.text_centered(
                        Face::Medium,
                        15.0,
                        pill[0] + 12.0,
                        pill[1] + 4.0,
                        argb(WHITE, if lo { 1.0 } else { 0.55 } * a),
                        "‹",
                    );
                    ui.text_centered(
                        Face::Medium,
                        15.0,
                        pill[0] + pill[2] - 12.0,
                        pill[1] + 4.0,
                        argb(WHITE, if hi { 1.0 } else { 0.55 } * a),
                        "›",
                    );
                    let value = fit(ui, Face::Medium, 12.0, &value, pill[2] - 44.0);
                    ui.text_centered(
                        Face::Medium,
                        12.0,
                        pill[0] + pill[2] / 2.0,
                        pill[1] + 6.0,
                        argb(WHITE, a),
                        &value,
                    );
                    if within(r) {
                        // The row steps forwards; the pill's left half steps back.
                        page.hit(clipped(r), Hit::Row(*item, false));
                        page.hit(clipped(half), Hit::Row(*item, true));
                    }
                }
            }
            if within(r) {
                horizon_client::widgets::record(item.caption_key(), clipped(r).map(gui), true);
            }
            y += 46.0;
            // The colour's picker, under its row.
            if let Item::Pvp(opt) = *item
                && self.picker.as_ref().is_some_and(|p| p.opt == opt)
            {
                let k = ease(self.picker_k);
                let h = PICKER_H * k;
                if h > 0.5 {
                    let panel = [vx, y - 2.0, vw, h];
                    let from = ui.batch.vertices.len();
                    self.draw_picker(ui, page, extras, panel, a * k.sqrt(), &within, &clipped);
                    // Only what has opened shows.
                    ui.batch.clip_since(
                        from,
                        gui(vx - 2.0),
                        gui(panel[1]),
                        gui(vx + vw + 2.0),
                        gui(panel[1] + h),
                    );
                }
                y += (PICKER_H + 6.0) * k;
            }
        }
        let content = y + scroll - by;
        if live {
            self.room = (content - bh).max(0.0);
            self.target = self.target.clamp(0.0, self.room);
        }
        ui.batch.clip_since(
            start,
            gui(vx - 2.0),
            gui(by),
            gui(vx + vw + 2.0),
            gui(by + bh),
        );

        // The hovered option's description, on one line at the foot.
        if let Some(item) = hovered {
            let text = fit(ui, Face::Regular, 11.0, &description(item), vw);
            ui.text(
                Face::Regular,
                11.0,
                vx + 2.0,
                by + bh + 8.0,
                argb(WHITE, 0.6 * a),
                &text,
            );
        }
    }
}

impl Mods {
    /// The open picker in `panel` (dp, its full height once open): the ready-made colours, the
    /// square and the hue bar, the colour as it is and its `#RRGGBB` field.
    #[allow(clippy::too_many_arguments)]
    fn draw_picker(
        &mut self,
        ui: &mut Ui,
        page: &mut Page,
        extras: &Extras,
        panel: [f32; 4],
        a: f32,
        within: &dyn Fn([f32; 4]) -> bool,
        clipped: &dyn Fn([f32; 4]) -> [f32; 4],
    ) {
        let Some(p) = self.picker.as_mut() else {
            return;
        };
        let (pointer, gui) = (page.pointer, page.gui);
        let opt = p.opt;
        let Some(choice) = opt.colour_choice() else {
            return;
        };
        let now = opt.colour(&extras.pvp).flatten();
        // The first frame: the square starts at the colour the option has.
        if p.square == [0.0; 4] {
            p.hsv = rgb_to_hsv(now.map_or(0xFF_FFFF, |c| c.argb() & 0xFF_FFFF));
            if now == Some(Colour::Inverted) {
                p.hsv = [0.0, 0.0, 1.0];
            }
        }
        let [px, py, pw, _] = panel;
        let full = [px, py, pw, PICKER_H];
        ui.rounded(
            full[0],
            full[1],
            full[2],
            full[3],
            10.0,
            argb(0x05_080C, 0.55 * a),
        );
        ui.border(
            full[0],
            full[1],
            full[2],
            full[3],
            10.0,
            argb(WHITE, 0.08 * a),
        );
        if within(full) {
            page.hit(clipped(full), Hit::PickerPanel);
        }
        let pad = 14.0;
        let (top, side) = (py + pad, PICKER_H - pad * 2.0);

        // The ready-made colours, six to a row.
        let label = lang::get_or("oteria.colour.presets", "Colours");
        ui.text(
            Face::Medium,
            10.0,
            px + pad,
            top - 2.0,
            argb(WHITE, 0.5 * a),
            &label.to_uppercase(),
        );
        let (cell, gap) = (24.0, 6.0);
        for (i, c) in choice.presets().into_iter().enumerate() {
            let r = [
                px + pad + (i % 6) as f32 * (cell + gap),
                top + 16.0 + (i / 6) as f32 * (cell + gap),
                cell,
                cell,
            ];
            let chosen = c == opt.colour(&extras.pvp).unwrap_or(None);
            let over = within(r) && inside(r, pointer);
            if chosen || over {
                let g = 2.0;
                ui.border(
                    r[0] - g,
                    r[1] - g,
                    r[2] + g * 2.0,
                    r[3] + g * 2.0,
                    8.0,
                    argb(
                        if chosen { ACCENT } else { WHITE },
                        if chosen { 0.95 } else { 0.4 } * a,
                    ),
                );
            }
            swatch(ui, page, c, r, a);
            if within(r) {
                page.hit(clipped(r), Hit::Swatch(c));
            }
        }

        // The square: saturation across, brightness down, in strips; the hue bar beside it.
        let sq = [px + pad + 6.0 * (cell + gap) + 12.0, top, side, side];
        let hue = p.hsv[0];
        let pure = hsv_to_rgb([hue, 1.0, 1.0]);
        let strips = 32;
        let atlas = ui.atlas;
        for i in 0..strips {
            let s = (i as f32 + 0.5) / strips as f32;
            let topc = mix(WHITE, pure, s);
            let x0 = sq[0] + sq[2] * i as f32 / strips as f32;
            let x1 = sq[0] + sq[2] * (i + 1) as f32 / strips as f32;
            ui.batch.fill_gradient(
                atlas,
                (gui(x0), gui(sq[1]), gui(x1) - gui(x0), gui(sq[3])),
                argb(topc, a),
                argb(0x00_0000, a),
            );
        }
        let knob = [sq[0] + p.hsv[1] * sq[2], sq[1] + (1.0 - p.hsv[2]) * sq[3]];
        ui.border(
            knob[0] - 6.0,
            knob[1] - 6.0,
            12.0,
            12.0,
            6.0,
            argb(0x000000, 0.6 * a),
        );
        ui.border(
            knob[0] - 5.0,
            knob[1] - 5.0,
            10.0,
            10.0,
            5.0,
            argb(WHITE, a),
        );
        let bar = [sq[0] + sq[2] + 10.0, top, 14.0, side];
        for i in 0..6 {
            let (h0, h1) = (i as f32 / 6.0, (i + 1) as f32 / 6.0);
            let y0 = bar[1] + bar[3] * h0;
            let y1 = bar[1] + bar[3] * h1;
            let (c0, c1) = (
                hsv_to_rgb([h0, 1.0, 1.0]),
                hsv_to_rgb([h1.min(0.9999), 1.0, 1.0]),
            );
            ui.batch.fill_gradient(
                atlas,
                (gui(bar[0]), gui(y0), gui(bar[2]), gui(y1) - gui(y0)),
                argb(c0, a),
                argb(c1, a),
            );
        }
        let hy = bar[1] + hue * bar[3];
        ui.rounded(
            bar[0] - 3.0,
            hy - 2.0,
            bar[2] + 6.0,
            4.0,
            2.0,
            argb(WHITE, a),
        );
        if within(sq) {
            page.hit(clipped(sq), Hit::Square);
        }
        if within(bar) {
            page.hit(clipped(bar), Hit::HueBar);
        }
        p.square = sq.map(gui);
        p.hue = bar.map(gui);

        // The colour as it is, and its field.
        let fx = bar[0] + bar[2] + 18.0;
        let fw = (px + pw - pad - fx).clamp(90.0, 150.0);
        let label = lang::get_or("oteria.colour.hex", "Hex");
        ui.text(
            Face::Medium,
            10.0,
            fx,
            top - 2.0,
            argb(WHITE, 0.5 * a),
            &label.to_uppercase(),
        );
        let preview = [fx, top + 16.0, fw, 40.0];
        swatch(ui, page, now, preview, a);
        let field = [fx, top + 66.0, fw, 30.0];
        let typing = p.hex.clone();
        let over = within(field) && inside(field, pointer);
        ui.rounded(
            field[0],
            field[1],
            field[2],
            field[3],
            7.0,
            argb(
                WHITE,
                if typing.is_some() {
                    0.14
                } else if over {
                    0.10
                } else {
                    0.06
                } * a,
            ),
        );
        if typing.is_some() {
            ui.border(
                field[0],
                field[1],
                field[2],
                field[3],
                7.0,
                argb(ACCENT, 0.75 * a),
            );
        }
        let text = match &typing {
            Some(hex) => {
                let caret = if self.blink < 0.55 { "|" } else { "" };
                format!("#{}{caret}", hex.to_uppercase())
            }
            None => match now {
                Some(Colour::Inverted) | None => "#——————".into(),
                Some(c) => format!("#{:06X}", c.argb() & 0xFF_FFFF),
            },
        };
        ui.text(
            Face::Medium,
            12.0,
            field[0] + 10.0,
            field[1] + 7.0,
            argb(WHITE, a),
            &text,
        );
        if within(field) {
            page.hit(clipped(field), Hit::HexField);
        }
        let hint = lang::get_or("oteria.colour.hint", "Type 6 digits");
        let hint = fit(ui, Face::Regular, 10.0, &hint, fw);
        ui.text(
            Face::Regular,
            10.0,
            fx,
            top + 102.0,
            argb(WHITE, 0.45 * a),
            &hint,
        );
    }
}

/// A colour's swatch in `r` (dp): off is a dark square crossed out, inverted half white and
/// half black.
fn swatch(ui: &mut Ui, page: &Page, c: Option<Colour>, r: [f32; 4], a: f32) {
    let [x, y, w, h] = r;
    let radius = (w.min(h) / 4.0).min(7.0);
    match c {
        Some(Colour::Inverted) => {
            ui.rounded(x, y, w, h, radius, argb(WHITE, a));
            ui.rounded(x + w / 2.0, y, w / 2.0, h, 0.0, argb(0x000000, a));
        }
        None => {
            ui.rounded(x, y, w, h, radius, argb(shown_rgb(None), a));
            // A red stroke from corner to corner.
            let gui = page.gui;
            let t = 1.2;
            let corners = [
                [gui(x + 3.0), gui(y + h - 3.0 - t)],
                [gui(x + w - 3.0 - t), gui(y + 3.0)],
                [gui(x + w - 3.0), gui(y + 3.0 + t)],
                [gui(x + 3.0 + t), gui(y + h - 3.0)],
            ];
            let atlas = ui.atlas;
            ui.batch.fill_quad(atlas, corners, argb(OFF, a));
        }
        Some(c) => ui.rounded(x, y, w, h, radius, argb(shown_rgb(Some(c)), a)),
    }
    ui.border(x, y, w, h, radius, argb(WHITE, 0.18 * a));
}

/// A small chevron centred on (cx, cy): pointing down when `k` is 0, up when it is 1.
fn chevron(ui: &mut Ui, page: &Page, cx: f32, cy: f32, k: f32, colour: u32) {
    let gui = page.gui;
    let dy = 2.5 * (1.0 - 2.0 * k);
    let (w, t) = (4.5, 1.3);
    // Two strokes meeting at the tip.
    let atlas = ui.atlas;
    for side in [-1.0f32, 1.0] {
        let outer = [cx + side * w, cy - dy];
        let tip = [cx, cy + dy];
        let corners = [
            [gui(outer[0]), gui(outer[1] - t)],
            [gui(tip[0]), gui(tip[1] - t)],
            [gui(tip[0]), gui(tip[1] + t)],
            [gui(outer[0]), gui(outer[1] + t)],
        ];
        ui.batch.fill_quad(atlas, corners, colour);
    }
}

/// The GUI scale (GUI pixels to screen pixels), for drawing a crosshair at its real size.
fn ui_gui_scale(ui: &Ui) -> f32 {
    ui.batch.scale
}

/// Sets or clears the drawn crosshair's pixel `(x, y)`.
fn set_pixel(pixels: &mut [u16; DRAWN], (x, y): (usize, usize), on: bool) {
    if on {
        pixels[y] |= 1 << x;
    } else {
        pixels[y] &= !(1 << x);
    }
}

/// A crosshair's pixels centred on `(cx, cy)` (dp), each `k` dp square.
fn draw_mask(ui: &mut Ui, mask: &pvp::Mask, cx: f32, cy: f32, k: f32, colour: u32) {
    for (x0, y, x1) in mask.runs() {
        let (x, yy) = (cx + (x0 as f32 - 0.5) * k, cy + (y as f32 - 0.5) * k);
        ui.rounded(x, yy, (x1 - x0) as f32 * k, k, 0.0, colour);
    }
}

/// What every part of the page draws with: the pointer (dp), the dp-to-GUI scale, and the
/// clickable places so far.
struct Page<'g> {
    d: f32,
    pointer: (f32, f32),
    gui: &'g dyn Fn(f32) -> f32,
    /// False while its part fades out: it draws, but takes no click.
    live: bool,
    hits: Vec<([f32; 4], Hit)>,
}
impl Page<'_> {
    /// `r` (dp) answers `what` to a click, and shows the hand under the pointer.
    fn hit(&mut self, r: [f32; 4], what: Hit) {
        if r[3] <= 0.0 || !self.live {
            return;
        }
        horizon_client::widgets::widget_cursor(inside(r, self.pointer), true);
        self.hits.push((r.map(self.gui), what));
    }
    /// An icon centred on (cx, cy), one texel to a pixel: the tiles' at 32 texels, the
    /// chrome's (`small`) at 20.
    fn icon(&self, ui: &mut Ui, name: &str, cx: f32, cy: f32, colour: u32, big: bool) {
        let small = icon::small(name);
        let (name, texels) = if big {
            (name, 32.0)
        } else {
            (small.as_str(), 20.0)
        };
        if let Some(r) = ui.atlas.sprites.get(name).copied() {
            let s = texels / self.d;
            // On whole screen pixels, so no texel falls between two.
            let k = ui.batch.scale;
            let gui = self.gui;
            let snap = |v: f32| (gui(v) * k).round() / k;
            ui.batch.rect_uv(
                snap(cx - s / 2.0),
                snap(cy - s / 2.0),
                gui(s),
                gui(s),
                r,
                colour,
            );
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
    fn every_setting_steps_and_shows_a_value() {
        // Two sky packs for the Sky option to step through, as a resourcepacks folder could hold.
        *horizon_client::pvp::SKY_PACKS.lock().unwrap() = vec![
            ("file/a.zip".into(), "a".into()),
            ("file/b.zip".into(), "b".into()),
        ];
        for tile in &TILES {
            for item in tile.settings {
                let mut e = Extras::default();
                let before = (item.switch(&e), item.value(&e));
                item.cycle(&mut e, false);
                assert_ne!(
                    (item.switch(&e), item.value(&e)),
                    before,
                    "{:?} of {}",
                    item,
                    tile.english
                );
                assert!(!caption(*item).is_empty());
            }
        }
    }

    #[test]
    fn the_filters_and_the_search_narrow_the_grid() {
        let mut m = Mods::default();
        assert_eq!(m.shown().len(), TILES.len());
        m.category = Category::Hud;
        assert!(
            m.shown()
                .iter()
                .all(|&i| TILES[i].category == Category::Hud)
        );
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
    fn escape_leaves_the_search_then_the_settings() {
        let mut m = Mods {
            typing: true,
            open: Some(0),
            ..Mods::default()
        };
        assert!(m.back() && !m.typing && m.open.is_some());
        assert!(m.back() && m.open.is_none());
        assert!(!m.back());
    }

    #[test]
    fn a_stroke_paints_what_its_first_cell_was_not() {
        let mut m = Mods {
            grid: ([0.0, 0.0, 150.0, 150.0], 10.0),
            hits: vec![([0.0, 0.0, 150.0, 150.0], Hit::Grid)],
            ..Mods::default()
        };
        assert_eq!(m.click((5.0, 5.0), [[0; DRAWN]; 2]), None);
        assert_eq!(m.paint, Some(true));
        // Let go far along the row: the cells between are painted too.
        let Some(Action::Pixels(false, p)) = m.release((95.0, 5.0)) else {
            panic!("a stroke ends in pixels")
        };
        assert_eq!(p[0], (1 << 10) - 1);
        assert_eq!(m.release((5.0, 5.0)), None, "only once");
    }

    #[test]
    fn a_profile_is_renamed_by_typing_and_deleted_on_a_second_press() {
        let mut m = Mods {
            tab: Tab::Profiles,
            hits: vec![
                ([0.0, 0.0, 10.0, 10.0], Hit::RenameProfile(1)),
                ([20.0, 0.0, 10.0, 10.0], Hit::DeleteProfile(1)),
                ([40.0, 0.0, 10.0, 10.0], Hit::NameField),
            ],
            ..Mods::default()
        };
        assert_eq!(m.click((5.0, 5.0), [[0; DRAWN]; 2]), None);
        assert!(m.wants_keys());
        assert!(m.type_key(Some("Bed"), false) && m.type_key(Some("wars!"), false));
        // A press in the field keeps typing; one elsewhere gives the name.
        assert_eq!(m.click((45.0, 5.0), [[0; DRAWN]; 2]), None);
        let renamed = Action::Profile(ProfileOp::Rename(1, "Bedwars!".into()));
        assert_eq!(m.click((25.0, 5.0), [[0; DRAWN]; 2]), Some(renamed));
        assert!(!m.wants_keys());
        // Nothing typed: the name stays.
        m.renaming = Some((1, String::new()));
        assert_eq!(m.commit_name(), None);
        // Escape drops a name being typed before anything else.
        m.renaming = Some((1, "x".into()));
        assert!(m.back() && m.renaming.is_none());
        // Delete asks once.
        assert_eq!(m.click((25.0, 5.0), [[0; DRAWN]; 2]), None);
        assert_eq!(
            m.click((25.0, 5.0), [[0; DRAWN]; 2]),
            Some(Action::Profile(ProfileOp::Delete(1)))
        );
        // Pressed once, then elsewhere: it asks again.
        assert_eq!(m.click((25.0, 5.0), [[0; DRAWN]; 2]), None);
        assert_eq!(m.click((100.0, 100.0), [[0; DRAWN]; 2]), None);
        assert_eq!(m.click((25.0, 5.0), [[0; DRAWN]; 2]), None);
    }

    #[test]
    fn colours_go_to_hsv_and_back() {
        for rgb in [
            0xFF_0000, 0x00_FF00, 0x00_00FF, 0x12_ABEF, 0xFF_FFFF, 0x00_0000, 0x80_8040,
        ] {
            assert_eq!(hsv_to_rgb(rgb_to_hsv(rgb)), rgb, "{rgb:06x}");
        }
    }

    #[test]
    fn a_number_field_takes_digits_only_and_puts_them_on() {
        let mut m = Mods {
            number: Some((Opt::CrosshairSize, String::new())),
            ..Mods::default()
        };
        assert!(m.wants_keys());
        assert!(m.type_key(Some("1a-2"), false));
        assert!(m.type_key(Some("34"), false));
        assert_eq!(m.number.as_ref().unwrap().1, "123");
        m.enter();
        assert!(m.number.is_none());
        assert_eq!(
            m.take_pending(),
            vec![Action::SetNumber(Opt::CrosshairSize, 123)]
        );
    }

    #[test]
    fn six_hex_digits_put_a_colour_on() {
        let mut m = Mods::default();
        m.click_test_picker(Opt::OutlineColour);
        m.picker.as_mut().unwrap().hex = Some(String::new());
        m.type_key(Some("#12zAb"), false);
        assert!(m.take_pending().is_empty());
        m.type_key(Some("eF9"), false);
        assert_eq!(
            m.take_pending(),
            vec![Action::SetColour(
                Opt::OutlineColour,
                Some(Colour::Custom(0x12ABEF)),
                true
            )]
        );
    }

    #[test]
    fn colours_mix_from_off_to_on() {
        assert_eq!(mix(OFF, ON, 0.0), OFF);
        assert_eq!(mix(OFF, ON, 1.0), ON);
    }
}
