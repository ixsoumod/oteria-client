//! PvP and HUD options (`docs/pvp.md`): a custom crosshair, block outline, hit colour and HUD
//! widgets (`pvp_hud`). Kept in
//! `oteria-extras.txt` with the other extras ([`crate::extras::Extras::pvp`]) and shown on
//! Horizon Settings > PvP & HUD. Every option is off by default, so the game looks like
//! vanilla until the player changes one, and none of them shows anything vanilla doesn't
//! already tell the player.

/// Colours the PvP options pick from. `Inverted` is vanilla's crosshair blend: the colour
/// under it, inverted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    Inverted,
    White,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Magenta,
    Black,
    /// Any colour, `0xRRGGBB` (the picker's wheel or a typed `#RRGGBB`).
    Custom(u32),
}
impl Colour {
    pub const ALL: [Colour; 10] = [
        Colour::Inverted,
        Colour::White,
        Colour::Red,
        Colour::Orange,
        Colour::Yellow,
        Colour::Green,
        Colour::Cyan,
        Colour::Blue,
        Colour::Magenta,
        Colour::Black,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Colour::Inverted => "inverted",
            Colour::White => "white",
            Colour::Red => "red",
            Colour::Orange => "orange",
            Colour::Yellow => "yellow",
            Colour::Green => "green",
            Colour::Cyan => "cyan",
            Colour::Blue => "blue",
            Colour::Magenta => "magenta",
            Colour::Black => "black",
            Colour::Custom(_) => "custom",
        }
    }
    /// The saved form: the name, or `#rrggbb` for a custom colour.
    pub fn save(self) -> String {
        match self {
            Colour::Custom(rgb) => format!("#{:06x}", rgb & 0xFF_FFFF),
            c => c.name().to_owned(),
        }
    }
    fn english(self) -> &'static str {
        match self {
            Colour::Inverted => "Inverted",
            Colour::White => "White",
            Colour::Red => "Red",
            Colour::Orange => "Orange",
            Colour::Yellow => "Yellow",
            Colour::Green => "Green",
            Colour::Cyan => "Cyan",
            Colour::Blue => "Blue",
            Colour::Magenta => "Magenta",
            Colour::Black => "Black",
            Colour::Custom(_) => "Custom",
        }
    }
    /// The name in the current language; a custom colour shows as `#RRGGBB`.
    pub fn label(self) -> String {
        if let Colour::Custom(rgb) = self {
            return format!("#{:06X}", rgb & 0xFF_FFFF);
        }
        crate::lang::get_or(
            &format!("horizon.pvp.colour.{}", self.name()),
            self.english(),
        )
    }
    /// Opaque ARGB; white for `Inverted` (the blend makes it the inverse).
    pub fn argb(self) -> u32 {
        match self {
            Colour::Inverted | Colour::White => 0xFFFF_FFFF,
            Colour::Red => 0xFFFF_0000,
            Colour::Orange => 0xFFFF_A020,
            Colour::Yellow => 0xFFFF_FF40,
            Colour::Green => 0xFF40_FF40,
            Colour::Cyan => 0xFF40_FFFF,
            Colour::Blue => 0xFF40_80FF,
            Colour::Magenta => 0xFFFF_40FF,
            Colour::Black => 0xFF00_0000,
            Colour::Custom(rgb) => 0xFF00_0000 | rgb & 0xFF_FFFF,
        }
    }
    /// A name, or `#rrggbb` (`rrggbb` alone too).
    pub fn parse(s: &str) -> Option<Colour> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return u32::from_str_radix(hex, 16).ok().map(Colour::Custom);
        }
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
    fn step(self, back: bool) -> Colour {
        step(&Self::ALL, self, back)
    }
    /// The next solid colour (`Inverted` left out).
    fn step_solid(self, back: bool) -> Colour {
        step(&Self::ALL[1..], self, back)
    }
    /// The colour at `percent` opacity, as ARGB.
    pub fn with_opacity(self, percent: u8) -> u32 {
        self.argb() & 0x00FF_FFFF | alpha(percent) << 24
    }
}
/// `percent` of 255, rounded.
fn alpha(percent: u8) -> u32 {
    (u32::from(percent.min(100)) * 255 + 50) / 100
}

/// The next (or previous) value of `all` after `v`, wrapping.
fn step<T: Copy + PartialEq>(all: &[T], v: T, back: bool) -> T {
    let i = all.iter().position(|&x| x == v).unwrap_or(0);
    let n = all.len();
    all[if back { (i + n - 1) % n } else { (i + 1) % n }]
}
/// `v` one step up or down within `lo..=hi`, wrapping.
fn step_num(v: u8, lo: u8, hi: u8, back: bool) -> u8 {
    step_by(v, (lo, hi), 1, back)
}
/// `v` `by` up or down within `lo..=hi`, wrapping.
fn step_by(v: u8, (lo, hi): (u8, u8), by: u8, back: bool) -> u8 {
    match (back, v) {
        (false, v) if v >= hi => lo,
        (false, v) => (v + by).min(hi),
        (true, v) if v <= lo => hi,
        (true, v) => v.saturating_sub(by).max(lo),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Four arms, as vanilla's.
    Cross,
    /// No arm above.
    T,
    /// Four diagonal arms.
    X,
    Circle,
    Square,
    /// Only the dot.
    Dot,
    /// The player's own pixels (`Crosshair::pixels`), drawn on the mods menu's grid.
    Drawn,
}
impl Shape {
    pub const ALL: [Shape; 7] = [
        Shape::Cross,
        Shape::T,
        Shape::X,
        Shape::Circle,
        Shape::Square,
        Shape::Dot,
        Shape::Drawn,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Shape::Cross => "cross",
            Shape::T => "t",
            Shape::X => "x",
            Shape::Circle => "circle",
            Shape::Square => "square",
            Shape::Dot => "dot",
            Shape::Drawn => "drawn",
        }
    }
    fn english(self) -> &'static str {
        match self {
            Shape::Cross => "Cross",
            Shape::T => "T",
            Shape::X => "X",
            Shape::Circle => "Circle",
            Shape::Square => "Square",
            Shape::Dot => "Dot",
            Shape::Drawn => "Drawn",
        }
    }
    pub fn label(self) -> String {
        crate::lang::get_or(
            &format!("horizon.pvp.shape.{}", self.name()),
            self.english(),
        )
    }
    fn parse(s: &str) -> Option<Shape> {
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
}

/// A time of day this player's view can stay at (`ViewTime::day_time` in vanilla's ticks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewTime {
    Morning,
    Noon,
    Sunset,
    Midnight,
}
impl ViewTime {
    pub const ALL: [ViewTime; 4] = [
        ViewTime::Morning,
        ViewTime::Noon,
        ViewTime::Sunset,
        ViewTime::Midnight,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ViewTime::Morning => "morning",
            ViewTime::Noon => "noon",
            ViewTime::Sunset => "sunset",
            ViewTime::Midnight => "midnight",
        }
    }
    fn english(self) -> &'static str {
        match self {
            ViewTime::Morning => "Morning",
            ViewTime::Noon => "Noon",
            ViewTime::Sunset => "Sunset",
            ViewTime::Midnight => "Midnight",
        }
    }
    /// The time of day it shows, as `/time set` takes it.
    pub fn day_time(self) -> i64 {
        match self {
            ViewTime::Morning => 1000,
            ViewTime::Noon => 6000,
            ViewTime::Sunset => 12500,
            ViewTime::Midnight => 18000,
        }
    }
    pub fn label(self) -> String {
        crate::lang::get_or(&format!("horizon.pvp.time.{}", self.name()), self.english())
    }
    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
}
/// The weather this player's view can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewWeather {
    Clear,
    Rain,
    Thunder,
}
impl ViewWeather {
    pub const ALL: [ViewWeather; 3] = [ViewWeather::Clear, ViewWeather::Rain, ViewWeather::Thunder];
    pub fn name(self) -> &'static str {
        match self {
            ViewWeather::Clear => "clear",
            ViewWeather::Rain => "rain",
            ViewWeather::Thunder => "thunder",
        }
    }
    fn english(self) -> &'static str {
        match self {
            ViewWeather::Clear => "Clear",
            ViewWeather::Rain => "Rain",
            ViewWeather::Thunder => "Thunder",
        }
    }
    /// `Level.getRainLevel` and `getThunderLevel` as `/weather` leaves them.
    pub fn levels(self) -> (f32, f32) {
        match self {
            ViewWeather::Clear => (0.0, 0.0),
            ViewWeather::Rain => (1.0, 0.0),
            ViewWeather::Thunder => (1.0, 1.0),
        }
    }
    pub fn label(self) -> String {
        crate::lang::get_or(
            &format!("horizon.pvp.weather.{}", self.name()),
            self.english(),
        )
    }
    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
}
/// The day time this player sees with `fixed` set: the same day (the moon's phase follows the
/// world's), at that time.
pub fn view_day_time(world: i64, fixed: Option<ViewTime>) -> i64 {
    match fixed {
        Some(t) => world.div_euclid(24000) * 24000 + t.day_time(),
        None => world,
    }
}

/// The crosshair's look. Its defaults draw vanilla's: a 15-pixel cross, one pixel thick,
/// inverting what is under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crosshair {
    /// Off: vanilla's crosshair sprite (a resource pack's included).
    pub custom: bool,
    pub shape: Shape,
    /// Arm length in GUI pixels (the circle's and square's radius past the gap).
    pub size: u8,
    pub gap: u8,
    pub thickness: u8,
    pub colour: Colour,
    /// A one-pixel black rim around every part.
    pub outline: bool,
    /// A dot in the middle.
    pub dot: bool,
    /// The colour while the crosshair is over a living entity; `None` keeps `colour`.
    pub hover: Option<Colour>,
    /// The drawn shape's pixels: a 15x15 grid as vanilla's sprite, bit `x` of row `y`, the
    /// middle pixel at (7, 7).
    pub pixels: [u16; DRAWN],
    /// Crosshair Indicator (CC0): while the crosshair is on a player in reach, these pixels are
    /// drawn over it too, on the same grid as `pixels` (vanilla's crosshair or the custom one).
    pub indicator: bool,
    pub indicator_pixels: [u16; DRAWN],
}
/// The drawn crosshair's grid side, in GUI pixels.
pub const DRAWN: usize = 15;
pub const SIZES: (u8, u8) = (1, 12);
pub const GAPS: (u8, u8) = (0, 8);
pub const THICKNESSES: (u8, u8) = (1, 4);
impl Default for Crosshair {
    fn default() -> Self {
        Self {
            custom: false,
            shape: Shape::Cross,
            size: 7,
            gap: 0,
            thickness: 1,
            colour: Colour::Inverted,
            outline: false,
            dot: false,
            hover: None,
            pixels: plus_pixels(),
            indicator: false,
            indicator_pixels: indicator_pixels(),
        }
    }
}
/// Crosshair Indicator's sprite: four small corners around the middle, the box a player's
/// hitbox makes in the sights.
pub fn indicator_pixels() -> [u16; DRAWN] {
    let mut p = [0u16; DRAWN];
    let corner = 1 << 4 | 1 << 10;
    let pair = 1 << 4 | 1 << 5 | 1 << 9 | 1 << 10;
    (p[4], p[5], p[9], p[10]) = (pair, corner, corner, pair);
    p
}
/// The mask of a 15x15 grid of pixels, centred as the crosshair's.
pub fn pixels_mask(pixels: &[u16; DRAWN]) -> Mask {
    let mut m = Mask([0; 64]);
    let mid = (DRAWN / 2) as i32;
    for (y, row) in pixels.iter().enumerate() {
        for x in 0..DRAWN {
            if row >> x & 1 == 1 {
                m.set(x as i32 - mid, y as i32 - mid);
            }
        }
    }
    m
}
/// A plus to start drawing from: vanilla's crosshair.
pub fn plus_pixels() -> [u16; DRAWN] {
    let mut p = [0u16; DRAWN];
    for (y, row) in p.iter_mut().enumerate() {
        *row = if y == DRAWN / 2 { (1 << DRAWN) - 1 } else { 1 << (DRAWN / 2) };
    }
    p
}

/// The five ready-made crosshairs of the mods menu, by name: shape and sizes (the colour stays
/// the player's).
pub fn crosshair_presets() -> [(&'static str, &'static str, Crosshair); 5] {
    let base = Crosshair {
        custom: true,
        ..Crosshair::default()
    };
    [
        ("oteria.crosshair.classic", "Classic", Crosshair { size: 5, ..base }),
        ("oteria.crosshair.dot", "Dot", Crosshair { shape: Shape::Dot, thickness: 2, outline: true, ..base }),
        ("oteria.crosshair.gap", "Gap", Crosshair { size: 4, gap: 2, dot: true, ..base }),
        ("oteria.crosshair.circle", "Circle", Crosshair { shape: Shape::Circle, size: 4, gap: 1, dot: true, ..base }),
        ("oteria.crosshair.x", "X", Crosshair { shape: Shape::X, size: 3, gap: 1, ..base }),
    ]
}

/// The crosshair's pixels on a 64x64 grid centred on bit 32 of row 32 (the middle pixel of
/// vanilla's 15x15 sprite): bit `x` of row `y` is the pixel `x - 32`, `y - 32` from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mask(pub [u64; 64]);
pub const MASK_CENTRE: i32 = 32;
impl Mask {
    fn set(&mut self, x: i32, y: i32) {
        let (gx, gy) = (x + MASK_CENTRE, y + MASK_CENTRE);
        if (0..64).contains(&gx) && (0..64).contains(&gy) {
            self.0[gy as usize] |= 1 << gx;
        }
    }
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y);
            }
        }
    }
    pub fn get(&self, x: i32, y: i32) -> bool {
        let (gx, gy) = (x + MASK_CENTRE, y + MASK_CENTRE);
        (0..64).contains(&gx) && (0..64).contains(&gy) && self.0[gy as usize] >> gx & 1 == 1
    }
    /// The pixels around this mask's (eight neighbours) that are not in it.
    pub fn rim(&self) -> Mask {
        let mut grown = [0u64; 64];
        for (y, &row) in self.0.iter().enumerate() {
            let spread = row | row << 1 | row >> 1;
            for dy in [y.wrapping_sub(1), y, y + 1] {
                if let Some(g) = grown.get_mut(dy) {
                    *g |= spread;
                }
            }
        }
        for (g, &row) in grown.iter_mut().zip(&self.0) {
            *g &= !row;
        }
        Mask(grown)
    }
    /// Each row's runs of set pixels as (x0, y, x1) offsets from the centre, `x1` exclusive.
    pub fn runs(&self) -> impl Iterator<Item = (i32, i32, i32)> + '_ {
        self.0.iter().enumerate().flat_map(|(y, &row)| {
            let mut bits = row;
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let start = bits.trailing_zeros();
                let len = (bits >> start).trailing_ones();
                bits &= !((((1u128 << len) - 1) as u64) << start);
                let y = y as i32 - MASK_CENTRE;
                let x0 = start as i32 - MASK_CENTRE;
                Some((x0, y, x0 + len as i32))
            })
        })
    }
}
impl Crosshair {
    /// The pixels of the shape and its dot.
    pub fn mask(&self) -> Mask {
        let mut m = Mask([0; 64]);
        let t = i32::from(self.thickness.clamp(THICKNESSES.0, THICKNESSES.1));
        // A thickness band centred on the middle pixel (the extra pixel of an even one
        // below and right).
        let (lo, hi) = (-(t - 1) / 2, -(t - 1) / 2 + t - 1);
        let size = i32::from(self.size.clamp(SIZES.0, SIZES.1));
        let gap = i32::from(self.gap.min(GAPS.1));
        let (near, far) = (hi + 1 + gap, hi + gap + size);
        let (near_neg, far_neg) = (lo - 1 - gap, lo - gap - size);
        match self.shape {
            Shape::Cross | Shape::T => {
                m.rect(near, lo, far, hi);
                m.rect(far_neg, lo, near_neg, hi);
                m.rect(lo, near, hi, far);
                if self.shape == Shape::Cross {
                    m.rect(lo, far_neg, hi, near_neg);
                }
                if gap == 0 {
                    m.rect(lo, lo, hi, hi);
                }
            }
            Shape::X => {
                for d in gap + 1..=gap + size {
                    for (sx, sy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                        let (x, y) = (sx * d, sy * d);
                        m.rect(x + lo, y + lo, x + hi, y + hi);
                    }
                }
                if gap == 0 {
                    m.rect(lo, lo, hi, hi);
                }
            }
            Shape::Circle | Shape::Square => {
                let outer = gap + size;
                let inner = outer - t;
                for y in -outer..=outer {
                    for x in -outer..=outer {
                        let inside = |r: i32| match self.shape {
                            Shape::Circle => {
                                let d = ((x * x + y * y) as f32).sqrt();
                                d <= r as f32 + 0.5
                            }
                            _ => x.abs().max(y.abs()) <= r,
                        };
                        if inside(outer) && !inside(inner) {
                            m.set(x, y);
                        }
                    }
                }
            }
            Shape::Dot => {}
            Shape::Drawn => {
                let mid = (DRAWN / 2) as i32;
                for (y, row) in self.pixels.iter().enumerate() {
                    for x in 0..DRAWN {
                        if row >> x & 1 == 1 {
                            m.set(x as i32 - mid, y as i32 - mid);
                        }
                    }
                }
            }
        }
        if self.dot || self.shape == Shape::Dot {
            m.rect(lo, lo, hi, hi);
        }
        m
    }
}

/// The crosshair's draw list, cached: rebuilt only when its settings change.
#[derive(Default)]
pub struct CrosshairCache {
    key: Option<Crosshair>,
    shape: Vec<(i32, i32, i32)>,
    rim: Vec<(i32, i32, i32)>,
}
impl CrosshairCache {
    /// The shape's runs and its outline's (empty without one), as [`Mask::runs`].
    pub fn runs(&mut self, c: &Crosshair) -> (&[(i32, i32, i32)], &[(i32, i32, i32)]) {
        if self.key != Some(*c) {
            let mask = c.mask();
            self.shape.clear();
            self.shape.extend(mask.runs());
            self.rim.clear();
            if c.outline {
                self.rim.extend(mask.rim().runs());
            }
            self.key = Some(*c);
        }
        (&self.shape, &self.rim)
    }
}

/// The block outline (`LevelRenderer.submitBlockOutline`). Off, vanilla's: black at 40%, or the
/// accessibility option's high contrast one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outline {
    pub custom: bool,
    /// A solid colour, at `opacity` percent.
    pub colour: Colour,
    pub opacity: u8,
    /// Times vanilla's width (`Window.getAppropriateLineWidth`).
    pub thickness: u8,
    /// The block's faces tinted with this colour, lightly.
    pub fill: Option<Colour>,
}
pub const OPACITIES: (u8, u8) = (10, 100);
pub const OUTLINE_THICKNESSES: (u8, u8) = (1, 4);
/// How much the fill covers the block's faces, in percent.
pub const FILL_OPACITY: u8 = 25;
impl Default for Outline {
    fn default() -> Self {
        Self {
            custom: false,
            colour: Colour::Black,
            opacity: 40,
            thickness: 1,
            fill: None,
        }
    }
}
impl Outline {
    /// The stroke's ARGB and its width as a multiple of vanilla's; none for vanilla's.
    pub fn stroke(&self) -> Option<(u32, f32)> {
        self.custom.then(|| {
            (
                self.colour.with_opacity(self.opacity),
                f32::from(self.thickness),
            )
        })
    }
    /// The fill's ARGB, when there is one.
    pub fn fill(&self) -> Option<u32> {
        self.fill
            .filter(|_| self.custom)
            .map(|c| c.with_opacity(FILL_OPACITY))
    }
}
/// The tint a hurt entity flashes (`OverlayTexture`'s red rows). Off, vanilla's: red over 30%.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub custom: bool,
    pub colour: Colour,
    /// How much of the colour covers the entity, in percent.
    pub opacity: u8,
}
pub const HIT_OPACITIES: (u8, u8) = (0, 100);
impl Default for Hit {
    fn default() -> Self {
        Self {
            custom: false,
            colour: Colour::Red,
            opacity: 30,
        }
    }
}
impl Hit {
    /// The overlay texel as RGBA bytes in a little-endian u32 (WGSL's `unpack4x8unorm`), its alpha
    /// what of the entity's own colour stays (`entity.fsh` mixes by it); 0 for vanilla's texel.
    pub fn texel(&self) -> u32 {
        if !self.custom {
            return 0;
        }
        let [_, r, g, b] = self.colour.argb().to_be_bytes();
        u32::from_le_bytes([r, g, b, (255 - alpha(self.opacity)) as u8])
    }
}

/// What the renderer reads of these options each frame (`FrameInputs::pvp`); the default is
/// vanilla's look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The block outline's stroke ([`Outline::stroke`]).
    pub outline: Option<(u32, f32)>,
    /// The hurt overlay texel ([`Hit::texel`]), 0 for vanilla's.
    pub hurt_texel: u32,
    /// The glint's colour ([`Tweaks::glint_texel`]), 0 for vanilla's.
    pub glint: u32,
}
impl Pvp {
    pub fn frame(&self) -> Frame {
        Frame {
            outline: self.outline.stroke(),
            hurt_texel: self.hit.texel(),
            glint: self.tweaks.glint_texel(),
        }
    }
}

/// The HUD widgets (`pvp_hud`), each in a corner of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hud {
    /// The armour worn, head first, with each piece's durability.
    pub armour: bool,
    pub armour_corner: Corner,
    pub armour_percent: bool,
    /// The active effects: icon, name and level, time left. They replace vanilla's icons.
    pub effects: bool,
    pub effects_corner: Corner,
    /// How many of the held item the inventory holds (arrows with a bow or crossbow).
    pub held: bool,
    pub held_corner: Corner,
    /// The latency the server reports for this player (the tab list's bars), in multiplayer.
    pub ping: bool,
    pub ping_corner: Corner,
    /// The block position and the direction faced, as F3 shows them; hidden when F3 is
    /// reduced (`reducedDebugInfo`).
    pub coords: bool,
    pub coords_corner: Corner,
    /// The server's address, in multiplayer.
    pub server: bool,
    pub server_corner: Corner,
    /// Hits in a row the server confirmed, until this player is hurt or 3 seconds pass.
    pub combo: bool,
    pub combo_corner: Corner,
    /// Frames drawn in the last second.
    pub fps: bool,
    pub fps_corner: Corner,
    /// Left and right clicks in the last second.
    pub cps: bool,
    pub cps_corner: Corner,
    /// The movement keys, mouse buttons and jump, lit while held, with clicks per second.
    pub keys: bool,
    pub keys_corner: Corner,
    /// The computer's local time.
    pub clock: bool,
    pub clock_corner: Corner,
    /// How far the last hit landed from the eyes, for 3 seconds; it only measures.
    pub reach: bool,
    pub reach_corner: Corner,
    /// Horizontal speed in blocks per second.
    pub speed: bool,
    pub speed_corner: Corner,
    /// The cooldowns of the items in hand and the hotbar (ender pearl, chorus fruit, wind
    /// charge, shield...), each with the seconds left.
    pub cooldowns: bool,
    pub cooldowns_corner: Corner,
    /// How many totems of undying the inventory holds, and how many you popped.
    pub totems: bool,
    pub totems_corner: Corner,
    /// How many healing splash potions the inventory holds, and the seconds since you threw one.
    pub pots: bool,
    pub pots_corner: Corner,
    /// A dark box behind each widget.
    pub background: bool,
    /// The boxes' opacity, in percent.
    pub opacity: u8,
    /// The widgets' text colour (durability keeps its own).
    pub text: Colour,
    /// A shadow under the widgets' text, as vanilla's HUD text has.
    pub shadow: bool,
}
impl Hud {
    /// Whether any widget is on: with none, the HUD draws nothing and reads nothing.
    pub fn any(&self) -> bool {
        [
            self.armour,
            self.effects,
            self.held,
            self.ping,
            self.coords,
            self.server,
            self.combo,
            self.fps,
            self.cps,
            self.keys,
            self.clock,
            self.reach,
            self.speed,
            self.cooldowns,
            self.totems,
            self.pots,
        ]
        .contains(&true)
    }
}
impl Default for Hud {
    fn default() -> Self {
        Self {
            armour: false,
            armour_corner: Corner::BottomRight,
            armour_percent: false,
            effects: false,
            effects_corner: Corner::TopRight,
            held: false,
            held_corner: Corner::BottomRight,
            ping: false,
            ping_corner: Corner::TopLeft,
            coords: false,
            coords_corner: Corner::TopLeft,
            server: false,
            server_corner: Corner::TopLeft,
            combo: false,
            combo_corner: Corner::TopLeft,
            fps: false,
            fps_corner: Corner::TopLeft,
            cps: false,
            cps_corner: Corner::TopLeft,
            keys: false,
            keys_corner: Corner::BottomLeft,
            clock: false,
            clock_corner: Corner::TopRight,
            reach: false,
            reach_corner: Corner::TopLeft,
            speed: false,
            speed_corner: Corner::TopLeft,
            cooldowns: false,
            cooldowns_corner: Corner::BottomLeft,
            totems: false,
            totems_corner: Corner::BottomLeft,
            pots: false,
            pots_corner: Corner::BottomLeft,
            background: true,
            opacity: HUD_OPACITY,
            text: Colour::White,
            shadow: true,
        }
    }
}
pub use crate::extras::Corner;
/// The widgets' boxes at first: the extras' 0x78 alpha, 47%.
pub const HUD_OPACITY: u8 = 47;
pub const HUD_OPACITIES: (u8, u8) = (0, 100);
impl Hud {
    /// The boxes' colour, ARGB.
    pub fn background_argb(&self) -> u32 {
        alpha(self.opacity) << 24
    }
}
/// The value of the view options while they follow the world.
fn world_label() -> String {
    crate::lang::get_or("horizon.pvp.view.world", "World's")
}
fn corner_label(c: Corner) -> String {
    crate::lang::get_or(&format!("horizon.pvp.corner.{}", c.name()), c.label())
}

/// Smaller changes to what vanilla shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tweaks {
    /// Enchantment levels as numbers in tooltips ("Sharpness 5").
    pub numeric_enchants: bool,
    /// How far down the first-person fire overlay moves, in percent of its height (0: vanilla).
    pub fire_lower: u8,
    /// The first-person shield's size, in percent (100: vanilla).
    pub shield_size: u8,
    /// The totem animation's size, in percent (100: vanilla).
    pub totem_size: u8,
    /// The enchantment glint's colour; `None` is vanilla's purple.
    pub glint: Option<Colour>,
    /// Water fog as after 30 seconds under water (vanilla's clearest), from the first moment.
    pub clear_water: bool,
    /// Lava fog as with Fire Resistance (vanilla's clearest), without the effect.
    pub clear_lava: bool,
    /// A small elytra under the crosshair while one is worn (Oteria's Elytra Indicator); placed
    /// and sized in the HUD editor (`pvp_hud::WidgetId::Elytra`).
    pub elytra_indicator: bool,
    /// Tints the elytra icon; `None` follows the crosshair's colour, or white without one.
    pub elytra_colour: Option<Colour>,
    /// The time of day this player sees, `None` for the world's. Never sent to the server.
    pub time: Option<ViewTime>,
    /// The weather this player sees, `None` for the world's. Never sent to the server.
    pub weather: Option<ViewWeather>,
}
impl Default for Tweaks {
    fn default() -> Self {
        Self {
            numeric_enchants: false,
            fire_lower: 0,
            shield_size: 100,
            totem_size: 100,
            glint: None,
            clear_water: false,
            clear_lava: false,
            elytra_indicator: false,
            elytra_colour: None,
            time: None,
            weather: None,
        }
    }
}
pub const FIRE_LOWER: (u8, u8) = (0, 80);
pub const SHIELD_SIZES: (u8, u8) = (50, 100);
pub const TOTEM_SIZES: (u8, u8) = (25, 100);
impl Tweaks {
    /// `colour` as the glint shaders read it: `0` for vanilla's, else `0xFF` alpha over RGB in
    /// little-endian bytes (`unpack4x8unorm`'s order).
    pub fn glint_texel(&self) -> u32 {
        self.glint.map_or(0, |c| {
            let [_, r, g, b] = c.argb().to_be_bytes();
            u32::from_le_bytes([r, g, b, 255])
        })
    }
}

/// Chat options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatOptions {
    /// The local time before each message.
    pub timestamps: bool,
    /// A message repeated in a row shows once with its count.
    pub compact: bool,
    /// A click on a message (where it has no click action) copies it.
    pub copy: bool,
    /// Ctrl+F in the chat searches the chat's messages.
    pub search: bool,
}
impl ChatOptions {
    /// What `chat::Chat` lays its lines out with.
    pub fn qol(&self) -> crate::chat::ChatQol {
        crate::chat::ChatQol {
            timestamps: self.timestamps,
            compact: self.compact,
        }
    }
}

/// Combat feel (`docs/pvp.md`): how fights look. Legacy draws them as 1.8.9 did; what the
/// game sends to a server never changes, so a vanilla server and its anticheat see 26.3.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Combat {
    /// Legacy (1.8.9) instead of Modern.
    pub legacy: bool,
    /// Keep the attack indicator under Legacy (1.8.9 had none).
    pub indicator: bool,
    /// Under Legacy, silence the attack sounds 1.9 added (1.8.9 played none on a hit).
    pub sounds: bool,
    /// The worlds this game hosts (single player and LAN) fight by 1.8.9's rules: no attack
    /// cooldown, no sweep, 1.8 weapon damage, effects, enchantments and critical hits
    /// (`horizon_client::legacy_combat`). Never used as a client of a server.
    pub rules: bool,
}
impl Combat {
    /// Whether this client leaves out sound `name` a server sent: under Legacy with its
    /// sounds, the `entity.player.attack.*` family (strong, weak, crit, sweep, knockback,
    /// nodamage), which 1.9 brought with the attack cooldown.
    pub fn mutes_sound(&self, name: &str) -> bool {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        self.legacy && self.sounds && name.starts_with("entity.player.attack.")
    }
    /// Whether this client leaves out particles of `ty` a server sent: under Legacy, the
    /// sweep and the damage hearts, which 1.9 brought (1.8.9's hits show crits only).
    pub fn hides_particle(&self, ty: crate::particles::ParticleType) -> bool {
        use crate::particles::ParticleType as T;
        self.legacy && matches!(ty, T::SweepAttack | T::DamageIndicator)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pvp {
    pub crosshair: Crosshair,
    pub outline: Outline,
    pub hit: Hit,
    pub hud: Hud,
    pub tweaks: Tweaks,
    pub chat: ChatOptions,
    /// Where each HUD widget sits and its size.
    pub layout: crate::pvp_hud::Layout,
    /// The key that opens the HUD editor from the game (unbound at first).
    pub editor_key: String,
    pub combat: Combat,
    pub voice: Voice,
    pub mouse: MouseTweaks,
    pub minimap: Minimap,
    pub skins: Skins,
    pub sky: Sky,
    pub zoom: Zoom,
    pub tiers: Tiers,
    pub blur: MotionBlur,
    pub environment: Environment,
    pub clock: Clock,
    pub sky_weather: SkyWeather,
    pub barrel: Barrel,
    pub music: Music,
    /// The Keyboard page's layout: 60, 75 or 100 (percent of a full keyboard).
    pub keyboard_layout: u8,
    pub crystal: CrystalOpt,
    pub anchor: AnchorOpt,
    pub hitboxes: Hitboxes,
    pub secret: Secret,
    /// Pop Counter: a chat line each time a player near you uses a totem, and when one dies
    /// after popping (`game/oteria_pvp.rs`).
    pub pops: bool,
    /// Food Saturation (after the Fabric mod AppleSkin): a saturation overlay on the hunger bar
    /// and food values in tooltips.
    pub saturation: Saturation,
    /// Viewmodel: where the items in hand sit in first person.
    pub viewmodel: Viewmodel,
    /// Scoreboard: the sidebar's look, place and size.
    pub scoreboard: ScoreboardStyle,
    /// Clear View: screen effects this player doesn't want drawn.
    pub clear_view: ClearView,
    /// Fast Totem: totems into the off hand.
    pub fast_totem: FastTotem,
    /// HurtCam: how much the camera shakes when the player is hurt.
    pub hurtcam: HurtCam,
    /// Whether Oteria's video defaults (VSync off, unlimited frames) were put on these settings
    /// once (`Settings::load`).
    pub video_defaults: bool,
}

/// Oteria's Viewmodel (`game/frame_geometry.rs`): each hand's item moved in first person, along
/// the screen's right (X), up (Y) and towards the player (Z). Each value is hundredths of a
/// block from 0 to 200, 100 being vanilla's place. Off by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewmodel {
    pub enabled: bool,
    pub main: [u8; 3],
    pub off: [u8; 3],
}
impl Default for Viewmodel {
    fn default() -> Self {
        Self {
            enabled: false,
            main: [100; 3],
            off: [100; 3],
        }
    }
}
/// A viewmodel value's ends; the middle is vanilla's place.
pub const VIEWMODEL_RANGE: (u32, u32) = (0, 200);
impl Viewmodel {
    /// The offset of one hand, blocks, in view space (x right, y up, z towards the player).
    pub fn offset(&self, main: bool) -> [f32; 3] {
        let v = if main { self.main } else { self.off };
        v.map(|c| (f32::from(c) - 100.0) / 100.0)
    }
}

/// Oteria's Scoreboard (`game/scoreboard.rs`): the sidebar sized and moved in the HUD editor
/// (`WidgetId::Scoreboard`), with its own background, rounded or not, and the red numbers shown
/// or not. Off: vanilla's sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreboardStyle {
    pub enabled: bool,
    /// The background's colour; `None` draws no background.
    pub background: Option<Colour>,
    /// The background's opacity, percent.
    pub opacity: u8,
    pub rounded: bool,
    /// The scores (the red numbers on the right).
    pub numbers: bool,
}
impl Default for ScoreboardStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            background: Some(Colour::Black),
            opacity: 30,
            rounded: false,
            numbers: true,
        }
    }
}
pub const SCOREBOARD_OPACITIES: (u8, u8) = (0, 100);
impl ScoreboardStyle {
    /// The body's colour, ARGB; the title's band a little darker. `None`: no background.
    pub fn colours(&self) -> Option<(u32, u32)> {
        let rgb = self.background?.argb() & 0x00FF_FFFF;
        let a = |pct: f32| ((pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u32) << 24;
        let body = f32::from(self.opacity);
        Some((a(body) | rgb, a(body + 10.0) | rgb))
    }
}

/// Oteria's Clear View: what this player's screen leaves out, each its own switch, all on once
/// the mod is (`game/fov.rs`, `game/sculk.rs`, `game/render_tick.rs`). Only the picture changes:
/// the effects still act as vanilla's (Blindness still stops sprinting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearView {
    pub enabled: bool,
    /// The carved pumpkin's overlay worn on the head.
    pub pumpkin: bool,
    /// The Warden's Darkness: its fog and its pulsing dark.
    pub darkness: bool,
    /// Blindness' fog.
    pub blindness: bool,
    /// Nausea's wobble and its green overlay.
    pub nausea: bool,
}
impl Default for ClearView {
    fn default() -> Self {
        Self {
            enabled: false,
            pumpkin: true,
            darkness: true,
            blindness: true,
            nausea: true,
        }
    }
}
impl ClearView {
    pub fn hides_pumpkin(&self) -> bool {
        self.enabled && self.pumpkin
    }
    pub fn hides_darkness(&self) -> bool {
        self.enabled && self.darkness
    }
    pub fn hides_blindness(&self) -> bool {
        self.enabled && self.blindness
    }
    pub fn hides_nausea(&self) -> bool {
        self.enabled && self.nausea
    }
}

/// Oteria's Fast Totem (`game/oteria_totem.rs`): a totem of undying from the inventory into the
/// off hand, by itself when the off hand is empty (after a pop), and with Shift + click on a
/// totem in the inventory. Each is a SWAP with the off hand, as the F key over a slot sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastTotem {
    pub enabled: bool,
    /// An empty off hand takes a totem from the inventory by itself.
    pub auto: bool,
    /// How long the off hand stays empty before, ticks (`TOTEM_DELAYS`).
    pub delay: u8,
    /// Shift + click on a totem in the inventory puts it in the off hand.
    pub shift_click: bool,
}
impl Default for FastTotem {
    fn default() -> Self {
        Self {
            enabled: false,
            auto: true,
            delay: 2,
            shift_click: true,
        }
    }
}
pub const TOTEM_DELAYS: (u8, u8) = (0, 20);

/// Oteria's HurtCam (`game/fov.rs`): how much the camera shakes when the player is hurt, from 0
/// (none) to 10. 4 is vanilla's tilt; above it the tilt grows and a shake is added. Off: vanilla's
/// Damage Tilt option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HurtCam {
    pub enabled: bool,
    /// `HURTCAM_STRENGTHS`.
    pub strength: u8,
}
impl Default for HurtCam {
    fn default() -> Self {
        Self {
            enabled: false,
            strength: 4,
        }
    }
}
pub const HURTCAM_STRENGTHS: (u8, u8) = (0, 10);
impl HurtCam {
    /// The hurt tilt's strength (`bobHurt`'s `damageTiltStrength`), given vanilla's option.
    pub fn tilt(&self, vanilla: f64) -> f64 {
        if self.enabled {
            f64::from(self.strength.min(HURTCAM_STRENGTHS.1)) / 4.0
        } else {
            vanilla
        }
    }
    /// How much the camera shakes on top of the tilt, 0 to 1 (only above vanilla's 4).
    pub fn shake(&self) -> f32 {
        if self.enabled && self.strength > 4 {
            f32::from(self.strength.min(HURTCAM_STRENGTHS.1) - 4) / 6.0
        } else {
            0.0
        }
    }
}

/// Oteria's AppleSkin, after the Fabric mod AppleSkin (public domain, `THIRD_PARTY_NOTICES.md`):
/// what vanilla's bars and tooltips don't tell the player (`game/appleskin.rs`). Off by default;
/// once on, each part is on as in AppleSkin's own defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saturation {
    /// The saturation's gold outlines over the hunger icons.
    pub enabled: bool,
    /// The food in hand: the hunger, saturation and health it would give, flashing on the bars.
    pub held: bool,
    /// The exhaustion bar behind the hunger icons.
    pub exhaustion: bool,
    /// Hunger and saturation points on food items' tooltips.
    pub tooltip: bool,
}
impl Default for Saturation {
    fn default() -> Self {
        Self {
            enabled: false,
            held: true,
            exhaustion: true,
            tooltip: true,
        }
    }
}

/// Oteria's Crystal Optimizer, after Marlow's Crystal Optimizer (MIT): an End crystal this
/// player hits with a blow strong enough to break it leaves this client at once, so the next
/// one can go where it stood without waiting for the server; it comes back if the server
/// hasn't removed it 30 ticks later (`game/oteria_pvp.rs`). Off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CrystalOpt {
    pub enabled: bool,
    /// The broken crystal stays drawn until the server removes it (it no longer blocks).
    pub keep_render: bool,
}

/// Oteria's Anchor Optimizer: a charged respawn anchor this player sets off outside the Nether
/// disappears at once, ahead of the server, which stays the judge: the block comes back when it
/// says so (`BlockStatePredictionHandler`). Off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnchorOpt {
    pub enabled: bool,
    /// Glowstone charges the anchor at once too.
    pub charge: bool,
}

/// Oteria's hitboxes: every entity's box in a colour by its kind (`None`: not drawn), with its
/// eyes and where it looks, as F3+B but always on and coloured (`game/oteria_pvp.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hitboxes {
    pub enabled: bool,
    pub players: Option<Colour>,
    pub hostile: Option<Colour>,
    pub passive: Option<Colour>,
    pub items: Option<Colour>,
    pub projectiles: Option<Colour>,
    pub crystals: Option<Colour>,
    pub others: Option<Colour>,
    /// The red line at the eyes.
    pub eyes: bool,
    /// The blue arrow where it looks.
    pub look: bool,
    /// The lines' width (`HITBOX_WIDTHS`), in tenths of vanilla's.
    pub width: u8,
    /// Your own box, in third person.
    pub own: bool,
    /// Combat Hitbox (Apache-2.0): the box of the entity under the crosshair, in this colour.
    pub target: Option<Colour>,
    /// The box of an entity that was just hit (its hurt time), in this colour.
    pub hurt: Option<Colour>,
    pub eye_colour: Colour,
    pub look_colour: Colour,
    /// A thicker line behind each box, in this colour.
    pub outline: Option<Colour>,
    /// The outline's width, in tenths of the box's (`HITBOX_OUTLINE_SIZES`).
    pub outline_size: u8,
    /// The lines' width past `far_distance` blocks, as `width`.
    pub far_width: u8,
    pub far_distance: u8,
    /// Fireworks' boxes.
    pub fireworks: bool,
    /// Hides the arrows and bee stingers stuck in players.
    pub hide_stuck: bool,
}
impl Default for Hitboxes {
    fn default() -> Self {
        Self {
            enabled: false,
            players: Some(Colour::Red),
            hostile: Some(Colour::Orange),
            passive: Some(Colour::Green),
            items: Some(Colour::Yellow),
            projectiles: Some(Colour::Cyan),
            crystals: Some(Colour::Magenta),
            others: Some(Colour::White),
            eyes: true,
            look: true,
            width: 10,
            own: false,
            target: Some(Colour::Yellow),
            hurt: None,
            eye_colour: Colour::Red,
            look_colour: Colour::Blue,
            outline: None,
            outline_size: 20,
            far_width: 10,
            far_distance: 32,
            fireworks: true,
            hide_stuck: false,
        }
    }
}
pub const HITBOX_WIDTHS: (u8, u8) = (4, 30);
pub const HITBOX_OUTLINE_SIZES: (u8, u8) = (12, 40);
pub const HITBOX_FAR: (u8, u8) = (4, 64);

/// Super Secret Settings (the 1.8 button, after fooxy's mod, MIT): a fun effect over the whole
/// screen, picked at random by the button of Options or its key (`key.oteria.secret`), or chosen
/// here (`gfx/oteria_secret`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Secret {
    /// The effect (`SECRET_EFFECTS`), 0 off.
    pub effect: u8,
}
/// Super Secret Settings' effects: their language key's end and English name; the index is what
/// the shader draws (`oteria_secret.wgsl`), 0 none.
pub const SECRET_EFFECTS: [(&str, &str); 22] = [
    ("none", "None"),
    ("notch", "Notch"),
    ("fxaa", "FXAA"),
    ("art", "Art"),
    ("bumpy", "Bumpy"),
    ("blobs", "Blobs"),
    ("pencil", "Pencil"),
    ("color_convolve", "Color Convolve"),
    ("deconverge", "Deconverge"),
    ("flip", "Flip"),
    ("invert", "Invert"),
    ("ntsc", "NTSC"),
    ("outline", "Outline"),
    ("scan_pincushion", "Scan Pincushion"),
    ("sobel", "Sobel"),
    ("bits", "Bits"),
    ("desaturate", "Desaturate"),
    ("green", "Green"),
    ("blur", "Blur"),
    ("wobble", "Wobble"),
    ("creeper", "Creeper"),
    ("spider", "Spider"),
];
pub fn secret_label(effect: u8) -> String {
    let (id, english) = SECRET_EFFECTS[usize::from(effect).min(SECRET_EFFECTS.len() - 1)];
    crate::lang::get_or(&format!("horizon.pvp.secret.{id}"), english)
}

/// Oteria's music (`game/oteria_music.rs`): the song playing on this computer and its lyrics.
/// Off by default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Music {
    pub enabled: bool,
    /// The lyrics floating in the air ahead of the player.
    pub lyrics: bool,
    /// The song's card at the top right.
    pub card: bool,
    /// How far the floating lyrics land, blocks (`LYRICS_DISTANCES`): the words spread from a
    /// little nearer to a little farther than this.
    pub distance: u8,
    /// The keys: play/pause, next, previous (unbound at first, set in Controls).
    pub play_key: String,
    pub next_key: String,
    pub previous_key: String,
}
impl Default for Music {
    fn default() -> Self {
        Self {
            enabled: false,
            lyrics: true,
            card: true,
            distance: 6,
            play_key: "key.keyboard.unknown".into(),
            next_key: "key.keyboard.unknown".into(),
            previous_key: "key.keyboard.unknown".into(),
        }
    }
}

/// Oteria's barrel roll (`game/oteria_barrel.rs`): an elytra flown as a plane. Off by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Barrel {
    pub enabled: bool,
    /// Rolled, the plane turns towards the lower wing.
    pub banking: bool,
    /// A roll under 90 degrees comes back level by itself.
    pub righting: bool,
    pub invert_pitch: bool,
    /// The mouse's left and right turn (yaw) instead of rolling.
    pub switch_roll_yaw: bool,
    /// The mouse's sensitivity in flight, percent (`BARREL_SENSITIVITIES`).
    pub sensitivity: u8,
}
impl Default for Barrel {
    fn default() -> Self {
        Self {
            enabled: false,
            banking: true,
            righting: false,
            invert_pitch: false,
            switch_roll_yaw: false,
            sensitivity: 100,
        }
    }
}
pub const BARREL_SENSITIVITIES: (u8, u8) = (25, 250);
pub const LYRICS_DISTANCES: (u8, u8) = (2, 24);

/// Oteria's motion blur (`gfx/motion_blur.rs`): off by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionBlur {
    pub enabled: bool,
    /// How long the trail lasts, in percent (`BLUR_STRENGTHS`).
    pub strength: u8,
}
impl Default for MotionBlur {
    fn default() -> Self {
        Self { enabled: false, strength: 50 }
    }
}
pub const BLUR_STRENGTHS: (u8, u8) = (10, 100);

/// Oteria's environment (`gfx/oteria_environment.rs`): the world dressed as a dream, off by
/// default. Each part has its own switch and colour: waves of tiles running out from the player
/// over the ground, flakes and sparkles floating in the air, a China hat over the player's head
/// in third person, an aurora sky with stars and shooting stars, and a fog on the distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Environment {
    pub enabled: bool,
    pub ground: bool,
    pub ground_colour: Colour,
    /// How far the waves run, in blocks (`ENV_RADII`).
    pub ground_radius: u8,
    pub particles: bool,
    pub particles_colour: Colour,
    /// How many there are, in percent (`ENV_AMOUNTS`).
    pub particles_amount: u8,
    /// Their shape: the flakes and sparkles, a ready-made one or the player's drawing.
    pub particles_shape: ParticleShape,
    /// How big the shaped ones are, in percent (`ENV_SIZES`).
    pub particles_size: u8,
    /// The drawn shape's pixels, on the crosshair's grid (`DRAWN`).
    pub particles_pixels: [u16; DRAWN],
    pub hat: bool,
    pub hat_colour: Colour,
    pub sky: bool,
    pub sky_colour: Colour,
    pub fog: bool,
    pub fog_colour: Colour,
    /// How thick the fog is, in percent (`ENV_AMOUNTS`).
    pub fog_density: u8,
}
impl Default for Environment {
    fn default() -> Self {
        // The colours of the reference: pink tiles and hat, a purple sky and fog.
        Self {
            enabled: false,
            ground: true,
            ground_colour: Colour::Custom(0xFF_8EC6),
            ground_radius: 9,
            particles: true,
            particles_colour: Colour::Custom(0xFF_9ADB),
            particles_amount: 50,
            particles_shape: ParticleShape::Classic,
            particles_size: 100,
            particles_pixels: ParticleShape::Heart.pixels([0; DRAWN]),
            hat: true,
            hat_colour: Colour::Custom(0xFF_9AD0),
            sky: true,
            sky_colour: Colour::Custom(0x9B_3A8C),
            fog: true,
            fog_colour: Colour::Custom(0x84_3474),
            fog_density: 50,
        }
    }
}
pub const ENV_RADII: (u8, u8) = (4, 24);
pub const ENV_AMOUNTS: (u8, u8) = (10, 100);
pub const ENV_SIZES: (u8, u8) = (50, 250);

/// The shape of the Environment's particles: the flakes and sparkles as before, a ready-made
/// shape, or one the player drew on the mods menu's grid. Every shape but the flakes is a
/// picture on the crosshair's grid (`DRAWN` by `DRAWN`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleShape {
    Classic,
    Heart,
    Star,
    Square,
    Circle,
    Diamond,
    Plus,
    Drawn,
}
impl ParticleShape {
    pub const ALL: [ParticleShape; 8] = [
        ParticleShape::Classic,
        ParticleShape::Heart,
        ParticleShape::Star,
        ParticleShape::Square,
        ParticleShape::Circle,
        ParticleShape::Diamond,
        ParticleShape::Plus,
        ParticleShape::Drawn,
    ];
    /// The ready-made ones, as the mods menu shows them (all but the flakes and the drawing).
    pub const PRESETS: [ParticleShape; 6] = [
        ParticleShape::Heart,
        ParticleShape::Star,
        ParticleShape::Square,
        ParticleShape::Circle,
        ParticleShape::Diamond,
        ParticleShape::Plus,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ParticleShape::Classic => "classic",
            ParticleShape::Heart => "heart",
            ParticleShape::Star => "star",
            ParticleShape::Square => "square",
            ParticleShape::Circle => "circle",
            ParticleShape::Diamond => "diamond",
            ParticleShape::Plus => "plus",
            ParticleShape::Drawn => "drawn",
        }
    }
    fn english(self) -> &'static str {
        match self {
            ParticleShape::Classic => "Flakes",
            ParticleShape::Heart => "Heart",
            ParticleShape::Star => "Star",
            ParticleShape::Square => "Square",
            ParticleShape::Circle => "Circle",
            ParticleShape::Diamond => "Diamond",
            ParticleShape::Plus => "Plus",
            ParticleShape::Drawn => "Drawn",
        }
    }
    pub fn label(self) -> String {
        crate::lang::get_or(
            &format!("horizon.pvp.particle_shape.{}", self.name()),
            self.english(),
        )
    }
    fn parse(s: &str) -> Option<ParticleShape> {
        Self::ALL.into_iter().find(|c| c.name() == s)
    }
    /// Its picture, bit `x` of row `y` (row 0 at the top); `drawn` for the player's own, and
    /// nothing for the flakes.
    pub fn pixels(self, drawn: [u16; DRAWN]) -> [u16; DRAWN] {
        const HEART: [&str; DRAWN] = [
            "...............",
            "..###.....###..",
            ".#####...#####.",
            "###############",
            "###############",
            "###############",
            "###############",
            ".#############.",
            "..###########..",
            "...#########...",
            "....#######....",
            ".....#####.....",
            "......###......",
            ".......#.......",
            "...............",
        ];
        const STAR: [&str; DRAWN] = [
            ".......#.......",
            ".......#.......",
            "......###......",
            "......###......",
            ".....#####.....",
            "###############",
            ".#############.",
            "..###########..",
            "...#########...",
            "...#########...",
            "..#####.#####..",
            "..####...####..",
            ".####.....####.",
            ".###.......###.",
            ".#...........#.",
        ];
        let from = |rows: [&str; DRAWN]| {
            let mut p = [0u16; DRAWN];
            for (y, row) in rows.iter().enumerate() {
                for (x, c) in row.bytes().enumerate() {
                    if c == b'#' {
                        p[y] |= 1 << x;
                    }
                }
            }
            p
        };
        let by = |inside: &dyn Fn(i32, i32) -> bool| {
            let mut p = [0u16; DRAWN];
            let mid = (DRAWN / 2) as i32;
            for (y, row) in p.iter_mut().enumerate() {
                for x in 0..DRAWN {
                    if inside(x as i32 - mid, y as i32 - mid) {
                        *row |= 1 << x;
                    }
                }
            }
            p
        };
        match self {
            ParticleShape::Classic => [0; DRAWN],
            ParticleShape::Heart => from(HEART),
            ParticleShape::Star => from(STAR),
            ParticleShape::Square => by(&|x, y| x.abs() <= 5 && y.abs() <= 5),
            ParticleShape::Circle => by(&|x, y| x * x + y * y <= 42),
            ParticleShape::Diamond => by(&|x, y| x.abs() + y.abs() <= 7),
            ParticleShape::Plus => {
                by(&|x, y| (x.abs() <= 1 && y.abs() <= 6) || (y.abs() <= 1 && x.abs() <= 6))
            }
            ParticleShape::Drawn => drawn,
        }
    }
}
impl Environment {
    /// The particles' picture this frame (`None`: the flakes and sparkles).
    pub fn particle_pixels(&self) -> Option<[u16; DRAWN]> {
        (self.particles_shape != ParticleShape::Classic)
            .then(|| self.particles_shape.pixels(self.particles_pixels))
    }
}

/// Oteria's time changer: the time of day this player sees, fixed where the slider puts it.
/// Never sent to the server; the world's time and what plays by it stay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub enabled: bool,
    /// In the day's ticks, as `/time set` takes them (0: 6 in the morning, 6000: noon).
    pub time: u16,
}
impl Default for Clock {
    fn default() -> Self {
        Self { enabled: false, time: 6000 }
    }
}
/// The slider's ends: a day's ticks.
pub const DAY_TICKS: (u32, u32) = (0, 23_999);
/// A day time in ticks as the clock reads it, `HH:MM` (tick 0 is 06:00).
pub fn clock_label(ticks: u32) -> String {
    let t = ticks % 24_000;
    let hours = (t / 1000 + 6) % 24;
    let minutes = (t % 1000) * 60 / 1000;
    format!("{hours:02}:{minutes:02}")
}

/// Oteria's weather changer: the weather this player sees, fixed. Never sent to the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkyWeather {
    pub enabled: bool,
    pub weather: ViewWeather,
}
impl Default for SkyWeather {
    fn default() -> Self {
        Self { enabled: false, weather: ViewWeather::Clear }
    }
}

/// Oteria's Tier Tagger: each player's PvP tier from MCTiers before their name, over their head
/// and in the player list (`game/oteria_tiers.rs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tiers {
    pub enabled: bool,
    /// The game mode shown (`TIER_MODES`); empty: the player's best.
    pub mode: String,
}
/// The game modes MCTiers ranks (its slugs), with their names; "" is the best of them.
pub const TIER_MODES: [(&str, &str); 9] = [
    ("", "Best"),
    ("sword", "Sword"),
    ("vanilla", "Vanilla"),
    ("uhc", "UHC"),
    ("pot", "Pot"),
    ("nethop", "NethPot"),
    ("smp", "SMP"),
    ("axe", "Axe"),
    ("mace", "Mace"),
];

/// Oteria's zoom, as OptiFine's: the view narrowed while its key is held (unbound at first, set
/// in Controls), the wheel zooming in and out while it is, eased or at once
/// (`game/oteria_zoom.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zoom {
    pub enabled: bool,
    /// The key to hold (`key.oteria.zoom`).
    pub key: String,
    /// The view eases in and out (else it narrows at once).
    pub smooth: bool,
    /// How many times the view is magnified when the key is pressed (`ZOOM_LEVELS`).
    pub level: u8,
}
impl Default for Zoom {
    fn default() -> Self {
        Self { enabled: true, key: "key.keyboard.unknown".into(), smooth: true, level: 4 }
    }
}
/// The magnifications the key starts at.
pub const ZOOM_LEVELS: (u8, u8) = (2, 10);

/// Oteria's sky mode: a sky pack (OptiFine's custom sky format) from the player's resourcepacks
/// folder, put on top of their packs while it is on (`game/oteria_sky.rs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sky {
    pub enabled: bool,
    /// The pack's id (`file/<name>`); empty: the first one found.
    pub pack: String,
}

/// The sky packs found in the resourcepacks folder (id, title), which `Opt::SkyPack` steps
/// through: the game fills it when it looks (`game/oteria_sky.rs`).
pub static SKY_PACKS: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

/// Oteria's cosmetics this client draws for its own player (the Cosmetics page).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skins {
    /// Swords in the main hand drawn as Oteria's butterfly knife (`game/oteria_knife.rs`).
    pub knife: bool,
    /// Which knife (`oteria_art::KNIFE_RIGS`).
    pub knife_skin: String,
    /// The key that plays the knife's inspect.
    pub inspect_key: String,
    /// The skin this player wears in Oteria (`game/oteria_skins.rs`, by its id in the skin
    /// history); empty: the account's own.
    pub player: String,
    /// Oteria's cape, wings and aura worn (`oteria_art::CAPES`, `WINGS`, `AURAS` ids); empty:
    /// none (`game/oteria_wear.rs`).
    pub cape: String,
    pub wings: String,
    pub aura: String,
}
impl Default for Skins {
    fn default() -> Self {
        Self {
            knife: false,
            knife_skin: "butterfly".into(),
            inspect_key: "key.keyboard.grave.accent".into(),
            player: String::new(),
            cape: String::new(),
            wings: String::new(),
            aura: String::new(),
        }
    }
}

/// Oteria's minimap: its shape, the biome under it, whether it turns, and its zoom (its size
/// and place are the HUD editor's, `Layout`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Minimap {
    pub round: bool,
    pub biome: bool,
    pub rotate: bool,
    /// Index into `MINIMAP_ZOOMS`.
    pub zoom: u8,
}
impl Default for Minimap {
    fn default() -> Self {
        Self { round: false, biome: true, rotate: false, zoom: 1 }
    }
}
/// The minimap's zooms, in blocks per pixel.
pub const MINIMAP_ZOOMS: [f32; 4] = [0.5, 1.0, 2.0, 4.0];

/// Mouse Tweaks' own options (its `Config`, with its defaults); the switch and the wheel's are
/// `Extras::mouse_tweaks` and `mouse_tweaks_wheel` (`game/tweaks.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseTweaks {
    /// Right-drag holding a stack: one item into each slot entered.
    pub rmb: bool,
    /// Left-drag holding a stack: the same items gathered (Shift: moved).
    pub lmb_item: bool,
    /// Shift + left-drag with an empty hand: every slot entered moved.
    pub lmb_empty: bool,
    /// The wheel looks for the items to pull from the first slot (else from the last).
    pub first_to_last: bool,
    /// What the wheel's directions do (`WHEEL_DIRECTIONS`).
    pub direction: u8,
    /// One item a wheel step whatever the step's size (else in proportion to it).
    pub always_one: bool,
}
impl Default for MouseTweaks {
    fn default() -> Self {
        Self {
            rmb: true,
            lmb_item: true,
            lmb_empty: true,
            first_to_last: false,
            direction: 0,
            always_one: false,
        }
    }
}
/// The wheel's directions, as Mouse Tweaks names them: down pushes and up pulls; the other way;
/// towards the other inventory pushes; away from it pushes.
pub const WHEEL_DIRECTIONS: [(&str, &str); 4] = [
    ("horizon.pvp.mouse.normal", "Normal"),
    ("horizon.pvp.mouse.inverted", "Inverted"),
    ("horizon.pvp.mouse.aware", "Inventory position aware"),
    ("horizon.pvp.mouse.aware_inverted", "Inventory position aware, inverted"),
];

/// Oteria's voice chat (`game/oteria_voice`), on Simple Voice Chat's protocol: players on Fabric
/// with that mod hear us, and we hear them.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub enabled: bool,
    /// Talk whenever the microphone hears speech (true), or while `key` is held (false).
    pub activation: bool,
    /// The push-to-talk key (`VOICE_KEYS`).
    pub key: String,
    /// The others' voices, in percent.
    pub volume: u8,
    /// The microphone's gain, in percent.
    pub mic: u8,
    /// Voice activation's level, in dB below full scale: a larger number sends quieter speech.
    pub threshold: u8,
    /// Our microphone off; the others not heard (Simple Voice Chat's "disabled").
    pub muted: bool,
    pub deafened: bool,
    /// The icons by the players' names and the group's faces hidden.
    pub hide_icons: bool,
    /// Background noise taken out of the microphone.
    pub noise: bool,
    /// Voices placed where the players are.
    pub spatial: bool,
    /// The microphone and the speakers by name; empty: the system's.
    pub mic_device: String,
    pub speaker_device: String,
    /// The key that opens the voice chat's menu (`VOICE_MENU_KEYS`).
    pub menu_key: String,
    /// Each player's volume in percent, by UUID (100 is left out).
    pub volumes: std::collections::BTreeMap<u128, u8>,
}
impl Default for Voice {
    fn default() -> Self {
        Self {
            enabled: false,
            activation: false,
            // Simple Voice Chat's own default.
            key: VOICE_KEYS[0].into(),
            volume: 100,
            mic: 100,
            threshold: 40,
            muted: false,
            deafened: false,
            hide_icons: false,
            noise: true,
            spatial: true,
            mic_device: String::new(),
            speaker_device: String::new(),
            menu_key: VOICE_MENU_KEYS[0].into(),
            volumes: Default::default(),
        }
    }
}
/// The push-to-talk keys to choose from.
pub const VOICE_KEYS: [&str; 5] = [
    "key.keyboard.caps.lock",
    "key.keyboard.v",
    "key.keyboard.b",
    "key.keyboard.g",
    "key.keyboard.left.alt",
];
/// The keys that can open the voice chat's menu: V first, as Simple Voice Chat.
pub const VOICE_MENU_KEYS: [&str; 4] = ["key.keyboard.v", "key.keyboard.j", "key.keyboard.k", "key.keyboard.u"];
const VOICE_PERCENT: (u8, u8) = (0, 200);
const VOICE_THRESHOLD: (u8, u8) = (20, 70);
/// The HUD editor key's name for the controls (`Extras::set_key`).
pub const HUD_EDITOR_KEY: &str = "horizon.hud_editor";
impl Default for Pvp {
    fn default() -> Self {
        Self {
            crosshair: Crosshair::default(),
            outline: Outline::default(),
            hit: Hit::default(),
            hud: Hud::default(),
            tweaks: Tweaks::default(),
            chat: ChatOptions::default(),
            layout: crate::pvp_hud::Layout::default(),
            editor_key: "key.keyboard.unknown".into(),
            combat: Combat::default(),
            voice: Voice::default(),
            mouse: MouseTweaks::default(),
            minimap: Minimap::default(),
            skins: Skins::default(),
            sky: Sky::default(),
            zoom: Zoom::default(),
            tiers: Tiers::default(),
            blur: MotionBlur::default(),
            environment: Environment::default(),
            clock: Clock::default(),
            sky_weather: SkyWeather::default(),
            barrel: Barrel::default(),
            music: Music::default(),
            keyboard_layout: 100,
            crystal: CrystalOpt::default(),
            anchor: AnchorOpt::default(),
            hitboxes: Hitboxes::default(),
            secret: Secret::default(),
            pops: false,
            saturation: Saturation::default(),
            viewmodel: Viewmodel::default(),
            scoreboard: ScoreboardStyle::default(),
            clear_view: ClearView::default(),
            fast_totem: FastTotem::default(),
            hurtcam: HurtCam::default(),
            video_defaults: false,
        }
    }
}
fn flag(value: &str, d: bool) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => d,
    }
}
fn num(value: &str, (lo, hi): (u8, u8), d: u8) -> u8 {
    value.parse::<u8>().map_or(d, |v| v.clamp(lo, hi))
}
impl Pvp {
    /// One `key:value` line of `oteria-extras.txt`; false when the key isn't a PvP one.
    pub fn parse_line(&mut self, key: &str, value: &str) -> bool {
        let c = &mut self.crosshair;
        let (o, h, u) = (&mut self.outline, &mut self.hit, &mut self.hud);
        let t = &mut self.tweaks;
        match key {
            "crosshair" => c.custom = flag(value, c.custom),
            "crosshairShape" => c.shape = Shape::parse(value).unwrap_or(c.shape),
            "crosshairSize" => c.size = num(value, SIZES, c.size),
            "crosshairGap" => c.gap = num(value, GAPS, c.gap),
            "crosshairThickness" => c.thickness = num(value, THICKNESSES, c.thickness),
            "crosshairColour" => c.colour = Colour::parse(value).unwrap_or(c.colour),
            "crosshairOutline" => c.outline = flag(value, c.outline),
            "crosshairDot" => c.dot = flag(value, c.dot),
            "crosshairPixels" => {
                let rows: Vec<u16> = value
                    .split(',')
                    .filter_map(|v| u16::from_str_radix(v, 16).ok())
                    .collect();
                if rows.len() == DRAWN {
                    c.pixels.copy_from_slice(&rows);
                }
            }
            "crosshairIndicator" => c.indicator = flag(value, c.indicator),
            "crosshairIndicatorPixels" => {
                let rows: Vec<u16> = value
                    .split(',')
                    .filter_map(|v| u16::from_str_radix(v, 16).ok())
                    .collect();
                if rows.len() == DRAWN {
                    c.indicator_pixels.copy_from_slice(&rows);
                }
            }
            "crosshairHover" => {
                c.hover = match value {
                    "off" => None,
                    v => Colour::parse(v).or(c.hover),
                }
            }
            "outline" => o.custom = flag(value, o.custom),
            "outlineColour" => o.colour = Colour::parse(value).unwrap_or(o.colour),
            "outlineOpacity" => o.opacity = num(value, OPACITIES, o.opacity),
            "outlineThickness" => o.thickness = num(value, OUTLINE_THICKNESSES, o.thickness),
            "outlineFill" => {
                o.fill = match value {
                    "off" => None,
                    v => Colour::parse(v).or(o.fill),
                }
            }
            "hit" => h.custom = flag(value, h.custom),
            "hitColour" => h.colour = Colour::parse(value).unwrap_or(h.colour),
            "hitOpacity" => h.opacity = num(value, HIT_OPACITIES, h.opacity),
            "hudArmour" => u.armour = flag(value, u.armour),
            "hudArmourCorner" => u.armour_corner = Corner::parse(value, u.armour_corner),
            "hudArmourPercent" => u.armour_percent = flag(value, u.armour_percent),
            "hudEffects" => u.effects = flag(value, u.effects),
            "hudEffectsCorner" => u.effects_corner = Corner::parse(value, u.effects_corner),
            "hudHeld" => u.held = flag(value, u.held),
            "hudHeldCorner" => u.held_corner = Corner::parse(value, u.held_corner),
            "hudBackground" => u.background = flag(value, u.background),
            "hudOpacity" => u.opacity = num(value, HUD_OPACITIES, u.opacity),
            "hudTextColour" => {
                u.text = Colour::parse(value)
                    .filter(|c| *c != Colour::Inverted)
                    .unwrap_or(u.text)
            }
            "hudShadow" => u.shadow = flag(value, u.shadow),
            "hudPlace" => self.layout.parse(value),
            "hudEditorKey" if value.starts_with("key.") => self.editor_key = value.to_owned(),
            "hudPing" => u.ping = flag(value, u.ping),
            "hudPingCorner" => u.ping_corner = Corner::parse(value, u.ping_corner),
            "hudCoords" => u.coords = flag(value, u.coords),
            "hudCoordsCorner" => u.coords_corner = Corner::parse(value, u.coords_corner),
            "hudServer" => u.server = flag(value, u.server),
            "hudServerCorner" => u.server_corner = Corner::parse(value, u.server_corner),
            "hudCombo" => u.combo = flag(value, u.combo),
            "hudComboCorner" => u.combo_corner = Corner::parse(value, u.combo_corner),
            "hudFps" => u.fps = flag(value, u.fps),
            "hudFpsCorner" => u.fps_corner = Corner::parse(value, u.fps_corner),
            "hudCps" => u.cps = flag(value, u.cps),
            "hudCpsCorner" => u.cps_corner = Corner::parse(value, u.cps_corner),
            "hudKeys" => u.keys = flag(value, u.keys),
            "hudKeysCorner" => u.keys_corner = Corner::parse(value, u.keys_corner),
            "hudClock" => u.clock = flag(value, u.clock),
            "hudClockCorner" => u.clock_corner = Corner::parse(value, u.clock_corner),
            "hudReach" => u.reach = flag(value, u.reach),
            "hudReachCorner" => u.reach_corner = Corner::parse(value, u.reach_corner),
            "hudSpeed" => u.speed = flag(value, u.speed),
            "hudSpeedCorner" => u.speed_corner = Corner::parse(value, u.speed_corner),
            "hudCooldowns" => u.cooldowns = flag(value, u.cooldowns),
            "hudCooldownsCorner" => u.cooldowns_corner = Corner::parse(value, u.cooldowns_corner),
            "hudTotems" => u.totems = flag(value, u.totems),
            "hudTotemsCorner" => u.totems_corner = Corner::parse(value, u.totems_corner),
            "hudPots" => u.pots = flag(value, u.pots),
            "hudPotsCorner" => u.pots_corner = Corner::parse(value, u.pots_corner),
            "oteriaCrystal" => self.crystal.enabled = flag(value, self.crystal.enabled),
            "oteriaCrystalKeep" => self.crystal.keep_render = flag(value, self.crystal.keep_render),
            "oteriaAnchor" => self.anchor.enabled = flag(value, self.anchor.enabled),
            "oteriaAnchorCharge" => self.anchor.charge = flag(value, self.anchor.charge),
            "oteriaPops" => self.pops = flag(value, self.pops),
            "oteriaSecret" => {
                self.secret.effect = num(value, (0, SECRET_EFFECTS.len() as u8 - 1), self.secret.effect)
            }
            "hitboxes" => self.hitboxes.enabled = flag(value, self.hitboxes.enabled),
            "hitboxesEyes" => self.hitboxes.eyes = flag(value, self.hitboxes.eyes),
            "hitboxesLook" => self.hitboxes.look = flag(value, self.hitboxes.look),
            "hitboxesOwn" => self.hitboxes.own = flag(value, self.hitboxes.own),
            "hitboxesWidth" => self.hitboxes.width = num(value, HITBOX_WIDTHS, self.hitboxes.width),
            "hitboxesFarWidth" => {
                self.hitboxes.far_width = num(value, HITBOX_WIDTHS, self.hitboxes.far_width)
            }
            "hitboxesFarDistance" => {
                self.hitboxes.far_distance = num(value, HITBOX_FAR, self.hitboxes.far_distance)
            }
            "hitboxesOutlineSize" => {
                self.hitboxes.outline_size = num(value, HITBOX_OUTLINE_SIZES, self.hitboxes.outline_size)
            }
            "hitboxesFireworks" => self.hitboxes.fireworks = flag(value, self.hitboxes.fireworks),
            "hitboxesHideStuck" => self.hitboxes.hide_stuck = flag(value, self.hitboxes.hide_stuck),
            "hitboxesEyeColour" => {
                self.hitboxes.eye_colour = Colour::parse(value).unwrap_or(self.hitboxes.eye_colour)
            }
            "hitboxesLookColour" => {
                self.hitboxes.look_colour = Colour::parse(value).unwrap_or(self.hitboxes.look_colour)
            }
            k if k.starts_with("hitboxes") => {
                let off_or = |v: Option<Colour>| match value {
                    "off" => None,
                    v2 => Colour::parse(v2).or(v),
                };
                let h = &mut self.hitboxes;
                match k {
                    "hitboxesPlayers" => h.players = off_or(h.players),
                    "hitboxesHostile" => h.hostile = off_or(h.hostile),
                    "hitboxesPassive" => h.passive = off_or(h.passive),
                    "hitboxesItems" => h.items = off_or(h.items),
                    "hitboxesProjectiles" => h.projectiles = off_or(h.projectiles),
                    "hitboxesCrystals" => h.crystals = off_or(h.crystals),
                    "hitboxesOthers" => h.others = off_or(h.others),
                    "hitboxesTarget" => h.target = off_or(h.target),
                    "hitboxesHurt" => h.hurt = off_or(h.hurt),
                    "hitboxesOutline" => h.outline = off_or(h.outline),
                    _ => return false,
                }
            }
            "numericEnchants" => t.numeric_enchants = flag(value, t.numeric_enchants),
            "lowFire" => t.fire_lower = num(value, FIRE_LOWER, t.fire_lower),
            "shieldSize" => t.shield_size = num(value, SHIELD_SIZES, t.shield_size),
            "totemSize" => t.totem_size = num(value, TOTEM_SIZES, t.totem_size),
            "glintColour" => {
                t.glint = match value {
                    "off" => None,
                    v => Colour::parse(v).or(t.glint),
                }
            }
            "clearWater" => t.clear_water = flag(value, t.clear_water),
            "clearLava" => t.clear_lava = flag(value, t.clear_lava),
            "elytraIndicator" => t.elytra_indicator = flag(value, t.elytra_indicator),
            "elytraColour" => {
                t.elytra_colour = match value {
                    "off" => None,
                    v => Colour::parse(v).or(t.elytra_colour),
                }
            }
            "oteriaSaturation" => self.saturation.enabled = flag(value, self.saturation.enabled),
            "oteriaSaturationHeld" => self.saturation.held = flag(value, self.saturation.held),
            "oteriaSaturationExhaustion" => {
                self.saturation.exhaustion = flag(value, self.saturation.exhaustion)
            }
            "oteriaSaturationTooltip" => {
                self.saturation.tooltip = flag(value, self.saturation.tooltip)
            }
            "mouseRmb" => self.mouse.rmb = flag(value, self.mouse.rmb),
            "skinKnife" => self.skins.knife = flag(value, self.skins.knife),
            "knifeSkin" if crate::oteria_art::KNIFE_RIGS.contains(&value) => self.skins.knife_skin = value.to_owned(),
            "oteriaCape" if value.is_empty() || crate::oteria_art::wear_style(&crate::oteria_art::CAPES, value).is_some() => {
                self.skins.cape = value.to_owned()
            }
            "oteriaWings" if value.is_empty() || crate::oteria_art::wear_style(&crate::oteria_art::WINGS, value).is_some() => {
                self.skins.wings = value.to_owned()
            }
            "oteriaAura" if value.is_empty() || crate::oteria_art::wear_style(&crate::oteria_art::AURAS, value).is_some() => {
                self.skins.aura = value.to_owned()
            }
            "knifeInspectKey" if value.starts_with("key.") => self.skins.inspect_key = value.to_owned(),
            "playerSkin" if value.chars().all(|c| c.is_ascii_hexdigit()) => self.skins.player = value.to_owned(),
            "skyMode" => self.sky.enabled = flag(value, self.sky.enabled),
            "skyPack" => self.sky.pack = value.to_owned(),
            "keyboardLayout" => {
                if let Ok(v @ (60 | 75 | 100)) = value.parse::<u8>() {
                    self.keyboard_layout = v;
                }
            }
            "music" => self.music.enabled = flag(value, self.music.enabled),
            "musicLyrics" => self.music.lyrics = flag(value, self.music.lyrics),
            "musicCard" => self.music.card = flag(value, self.music.card),
            "musicDistance" => self.music.distance = num(value, LYRICS_DISTANCES, self.music.distance),
            "viewmodel" => self.viewmodel.enabled = flag(value, self.viewmodel.enabled),
            "viewmodelMain" | "viewmodelOff" => {
                let parts: Vec<u8> = value
                    .split(',')
                    .filter_map(|v| v.trim().parse::<u8>().ok())
                    .map(|v| v.min(VIEWMODEL_RANGE.1 as u8))
                    .collect();
                if let [x, y, z] = parts[..] {
                    if key == "viewmodelMain" {
                        self.viewmodel.main = [x, y, z];
                    } else {
                        self.viewmodel.off = [x, y, z];
                    }
                }
            }
            "scoreboard" => self.scoreboard.enabled = flag(value, self.scoreboard.enabled),
            "scoreboardBackground" => {
                self.scoreboard.background = match value {
                    "off" => None,
                    v => Colour::parse(v)
                        .filter(|c| *c != Colour::Inverted)
                        .or(self.scoreboard.background),
                }
            }
            "scoreboardOpacity" => {
                self.scoreboard.opacity = num(value, SCOREBOARD_OPACITIES, self.scoreboard.opacity)
            }
            "scoreboardRounded" => self.scoreboard.rounded = flag(value, self.scoreboard.rounded),
            "scoreboardNumbers" => self.scoreboard.numbers = flag(value, self.scoreboard.numbers),
            "clearView" => self.clear_view.enabled = flag(value, self.clear_view.enabled),
            "clearViewPumpkin" => self.clear_view.pumpkin = flag(value, self.clear_view.pumpkin),
            "clearViewDarkness" => self.clear_view.darkness = flag(value, self.clear_view.darkness),
            "clearViewBlindness" => self.clear_view.blindness = flag(value, self.clear_view.blindness),
            "clearViewNausea" => self.clear_view.nausea = flag(value, self.clear_view.nausea),
            "fastTotem" => self.fast_totem.enabled = flag(value, self.fast_totem.enabled),
            "fastTotemAuto" => self.fast_totem.auto = flag(value, self.fast_totem.auto),
            "fastTotemDelay" => self.fast_totem.delay = num(value, TOTEM_DELAYS, self.fast_totem.delay),
            "fastTotemShiftClick" => self.fast_totem.shift_click = flag(value, self.fast_totem.shift_click),
            "hurtCam" => self.hurtcam.enabled = flag(value, self.hurtcam.enabled),
            "hurtCamStrength" => self.hurtcam.strength = num(value, HURTCAM_STRENGTHS, self.hurtcam.strength),
            "oteriaVideoDefaults" => self.video_defaults = flag(value, self.video_defaults),
            "musicPlayKey" if value.starts_with("key.") => self.music.play_key = value.to_owned(),
            "musicNextKey" if value.starts_with("key.") => self.music.next_key = value.to_owned(),
            "musicPreviousKey" if value.starts_with("key.") => self.music.previous_key = value.to_owned(),
            "barrelRoll" => self.barrel.enabled = flag(value, self.barrel.enabled),
            "barrelBanking" => self.barrel.banking = flag(value, self.barrel.banking),
            "barrelRighting" => self.barrel.righting = flag(value, self.barrel.righting),
            "barrelInvertPitch" => self.barrel.invert_pitch = flag(value, self.barrel.invert_pitch),
            "barrelSwitchRollYaw" => self.barrel.switch_roll_yaw = flag(value, self.barrel.switch_roll_yaw),
            "barrelSensitivity" => {
                self.barrel.sensitivity = num(value, BARREL_SENSITIVITIES, self.barrel.sensitivity)
            }
            "motionBlur" => self.blur.enabled = flag(value, self.blur.enabled),
            "environment" => self.environment.enabled = flag(value, self.environment.enabled),
            "environmentGround" => self.environment.ground = flag(value, self.environment.ground),
            "environmentGroundColour" => {
                self.environment.ground_colour = Colour::parse(value).unwrap_or(self.environment.ground_colour)
            }
            "environmentGroundRadius" => {
                self.environment.ground_radius = num(value, ENV_RADII, self.environment.ground_radius)
            }
            "environmentParticles" => self.environment.particles = flag(value, self.environment.particles),
            "environmentParticlesColour" => {
                self.environment.particles_colour =
                    Colour::parse(value).unwrap_or(self.environment.particles_colour)
            }
            "environmentParticlesAmount" => {
                self.environment.particles_amount = num(value, ENV_AMOUNTS, self.environment.particles_amount)
            }
            "environmentParticlesShape" => {
                self.environment.particles_shape =
                    ParticleShape::parse(value).unwrap_or(self.environment.particles_shape)
            }
            "environmentParticlesSize" => {
                self.environment.particles_size = num(value, ENV_SIZES, self.environment.particles_size)
            }
            "environmentParticlesPixels" => {
                let rows: Vec<u16> = value
                    .split(',')
                    .filter_map(|v| u16::from_str_radix(v, 16).ok())
                    .collect();
                if rows.len() == DRAWN {
                    self.environment.particles_pixels.copy_from_slice(&rows);
                }
            }
            "environmentHat" => self.environment.hat = flag(value, self.environment.hat),
            "environmentHatColour" => {
                self.environment.hat_colour = Colour::parse(value).unwrap_or(self.environment.hat_colour)
            }
            "environmentSky" => self.environment.sky = flag(value, self.environment.sky),
            "environmentSkyColour" => {
                self.environment.sky_colour = Colour::parse(value).unwrap_or(self.environment.sky_colour)
            }
            "environmentFog" => self.environment.fog = flag(value, self.environment.fog),
            "environmentFogColour" => {
                self.environment.fog_colour = Colour::parse(value).unwrap_or(self.environment.fog_colour)
            }
            "environmentFogDensity" => {
                self.environment.fog_density = num(value, ENV_AMOUNTS, self.environment.fog_density)
            }
            "motionBlurStrength" => self.blur.strength = num(value, BLUR_STRENGTHS, self.blur.strength),
            "timeChanger" => self.clock.enabled = flag(value, self.clock.enabled),
            "timeChangerAt" => {
                if let Ok(v) = value.parse::<u32>() {
                    self.clock.time = v.min(DAY_TICKS.1) as u16;
                }
            }
            "weatherChanger" => self.sky_weather.enabled = flag(value, self.sky_weather.enabled),
            "weatherChangerAt" => {
                if let Some(w) = ViewWeather::parse(value) {
                    self.sky_weather.weather = w;
                }
            }
            "tierTagger" => self.tiers.enabled = flag(value, self.tiers.enabled),
            "tierMode" if TIER_MODES.iter().any(|(m, _)| *m == value) => self.tiers.mode = value.to_owned(),
            "oteriaZoom" => self.zoom.enabled = flag(value, self.zoom.enabled),
            "oteriaZoomKey" if value.starts_with("key.") => self.zoom.key = value.to_owned(),
            "oteriaZoomSmooth" => self.zoom.smooth = flag(value, self.zoom.smooth),
            "oteriaZoomLevel" => self.zoom.level = num(value, ZOOM_LEVELS, self.zoom.level),
            "mapRound" => self.minimap.round = flag(value, self.minimap.round),
            "mapBiome" => self.minimap.biome = flag(value, self.minimap.biome),
            "mapRotate" => self.minimap.rotate = flag(value, self.minimap.rotate),
            "mapZoom" => self.minimap.zoom = num(value, (0, 3), self.minimap.zoom),
            "mouseLmbItem" => self.mouse.lmb_item = flag(value, self.mouse.lmb_item),
            "mouseLmbEmpty" => self.mouse.lmb_empty = flag(value, self.mouse.lmb_empty),
            "mouseWheelOrder" => match value {
                "first" => self.mouse.first_to_last = true,
                "last" => self.mouse.first_to_last = false,
                _ => {}
            },
            "mouseWheelDirection" => {
                self.mouse.direction = num(value, (0, 3), self.mouse.direction)
            }
            "mouseScrollScaling" => match value {
                "one" => self.mouse.always_one = true,
                "proportional" => self.mouse.always_one = false,
                _ => {}
            },
            "voiceChat" => self.voice.enabled = flag(value, self.voice.enabled),
            "voiceActivation" => match value {
                "voice" => self.voice.activation = true,
                "ptt" => self.voice.activation = false,
                _ => {}
            },
            "voiceKey" if value.starts_with("key.") => self.voice.key = value.to_owned(),
            "voiceVolume" => self.voice.volume = num(value, VOICE_PERCENT, self.voice.volume),
            "voiceMic" => self.voice.mic = num(value, VOICE_PERCENT, self.voice.mic),
            "voiceMuted" => self.voice.muted = flag(value, self.voice.muted),
            "voiceDeafened" => self.voice.deafened = flag(value, self.voice.deafened),
            "voiceHideIcons" => self.voice.hide_icons = flag(value, self.voice.hide_icons),
            "voiceNoise" => self.voice.noise = flag(value, self.voice.noise),
            "voiceSpatial" => self.voice.spatial = flag(value, self.voice.spatial),
            "voiceMicDevice" => self.voice.mic_device = value.to_owned(),
            "voiceSpeakerDevice" => self.voice.speaker_device = value.to_owned(),
            "voiceMenuKey" if value.starts_with("key.") => self.voice.menu_key = value.to_owned(),
            "voiceVolumes" => {
                self.voice.volumes = value
                    .split(',')
                    .filter_map(|e| {
                        let (id, pct) = e.split_once('=')?;
                        Some((u128::from_str_radix(id, 16).ok()?, pct.parse::<u8>().ok()?.min(200)))
                    })
                    .collect();
            }
            "voiceThreshold" => {
                self.voice.threshold = num(value, VOICE_THRESHOLD, self.voice.threshold)
            }
            "chatTimestamps" => self.chat.timestamps = flag(value, self.chat.timestamps),
            "chatCompact" => self.chat.compact = flag(value, self.chat.compact),
            "chatCopy" => self.chat.copy = flag(value, self.chat.copy),
            "chatSearch" => self.chat.search = flag(value, self.chat.search),
            "combatFeel" => match value {
                "legacy" => self.combat.legacy = true,
                "modern" => self.combat.legacy = false,
                _ => {}
            },
            "combatIndicator" => self.combat.indicator = flag(value, self.combat.indicator),
            "combatSounds" => self.combat.sounds = flag(value, self.combat.sounds),
            "combatRules" => self.combat.rules = flag(value, self.combat.rules),
            "viewTime" => {
                t.time = match value {
                    "off" => None,
                    v => ViewTime::parse(v).or(t.time),
                }
            }
            "viewWeather" => {
                t.weather = match value {
                    "off" => None,
                    v => ViewWeather::parse(v).or(t.weather),
                }
            }
            _ => return false,
        }
        true
    }
    /// The PvP lines of `oteria-extras.txt`.
    pub fn serialize(&self, out: &mut String) {
        use std::fmt::Write;
        let c = &self.crosshair;
        let _ = write!(
            out,
            "crosshair:{}\ncrosshairShape:{}\ncrosshairSize:{}\ncrosshairGap:{}\ncrosshairThickness:{}\ncrosshairColour:{}\ncrosshairOutline:{}\ncrosshairDot:{}\ncrosshairHover:{}\n",
            c.custom,
            c.shape.name(),
            c.size,
            c.gap,
            c.thickness,
            c.colour.save(),
            c.outline,
            c.dot,
            c.hover.map_or_else(|| "off".into(), Colour::save),
        );
        let pixels: Vec<String> = c.pixels.iter().map(|row| format!("{row:x}")).collect();
        let _ = writeln!(out, "crosshairPixels:{}", pixels.join(","));
        let indicator: Vec<String> = c.indicator_pixels.iter().map(|row| format!("{row:x}")).collect();
        let _ = writeln!(out, "crosshairIndicator:{}", c.indicator);
        let _ = writeln!(out, "crosshairIndicatorPixels:{}", indicator.join(","));
        let (o, h) = (&self.outline, &self.hit);
        let _ = write!(
            out,
            "outline:{}\noutlineColour:{}\noutlineOpacity:{}\noutlineThickness:{}\noutlineFill:{}\nhit:{}\nhitColour:{}\nhitOpacity:{}\n",
            o.custom,
            o.colour.save(),
            o.opacity,
            o.thickness,
            o.fill.map_or_else(|| "off".into(), Colour::save),
            h.custom,
            h.colour.save(),
            h.opacity,
        );
        let u = &self.hud;
        let _ = write!(
            out,
            "hudArmour:{}\nhudArmourCorner:{}\nhudArmourPercent:{}\nhudEffects:{}\nhudEffectsCorner:{}\nhudHeld:{}\nhudHeldCorner:{}\nhudBackground:{}\n",
            u.armour,
            u.armour_corner.name(),
            u.armour_percent,
            u.effects,
            u.effects_corner.name(),
            u.held,
            u.held_corner.name(),
            u.background,
        );
        let _ = write!(
            out,
            "hudPing:{}\nhudPingCorner:{}\nhudCoords:{}\nhudCoordsCorner:{}\nhudServer:{}\nhudServerCorner:{}\nhudCombo:{}\nhudComboCorner:{}\n",
            u.ping,
            u.ping_corner.name(),
            u.coords,
            u.coords_corner.name(),
            u.server,
            u.server_corner.name(),
            u.combo,
            u.combo_corner.name(),
        );
        let _ = write!(
            out,
            "hudFps:{}\nhudFpsCorner:{}\nhudCps:{}\nhudCpsCorner:{}\nhudKeys:{}\nhudKeysCorner:{}\n",
            u.fps,
            u.fps_corner.name(),
            u.cps,
            u.cps_corner.name(),
            u.keys,
            u.keys_corner.name(),
        );
        let _ = write!(
            out,
            "hudClock:{}\nhudClockCorner:{}\nhudReach:{}\nhudReachCorner:{}\nhudSpeed:{}\nhudSpeedCorner:{}\n",
            u.clock,
            u.clock_corner.name(),
            u.reach,
            u.reach_corner.name(),
            u.speed,
            u.speed_corner.name(),
        );
        let _ = write!(
            out,
            "hudOpacity:{}\nhudTextColour:{}\nhudShadow:{}\n",
            u.opacity,
            u.text.save(),
            u.shadow,
        );
        self.layout.serialize(out);
        let _ = writeln!(out, "hudEditorKey:{}", self.editor_key);
        let t = &self.tweaks;
        let _ = write!(
            out,
            "numericEnchants:{}\nlowFire:{}\nshieldSize:{}\ntotemSize:{}\nglintColour:{}\nclearWater:{}\nclearLava:{}\n",
            t.numeric_enchants,
            t.fire_lower,
            t.shield_size,
            t.totem_size,
            t.glint.map_or_else(|| "off".into(), Colour::save),
            t.clear_water,
            t.clear_lava,
        );
        let _ = write!(
            out,
            "viewTime:{}\nviewWeather:{}\n",
            t.time.map_or("off", ViewTime::name),
            t.weather.map_or("off", ViewWeather::name),
        );
        let c = &self.chat;
        let _ = write!(
            out,
            "chatTimestamps:{}\nchatCompact:{}\nchatCopy:{}\nchatSearch:{}\n",
            c.timestamps, c.compact, c.copy, c.search,
        );
        let c = &self.combat;
        let feel = if c.legacy { "legacy" } else { "modern" };
        let _ = write!(
            out,
            "combatFeel:{feel}\ncombatIndicator:{}\ncombatSounds:{}\ncombatRules:{}\n",
            c.indicator, c.sounds, c.rules
        );
        let _ = writeln!(out, "elytraIndicator:{}", self.tweaks.elytra_indicator);
        let _ = writeln!(
            out,
            "elytraColour:{}",
            self.tweaks
                .elytra_colour
                .map_or_else(|| "off".into(), Colour::save),
        );
        let s = &self.saturation;
        let _ = write!(
            out,
            "oteriaSaturation:{}\noteriaSaturationHeld:{}\noteriaSaturationExhaustion:{}\noteriaSaturationTooltip:{}\n",
            s.enabled, s.held, s.exhaustion, s.tooltip,
        );
        let off = |c: Option<Colour>| c.map_or_else(|| "off".into(), Colour::save);
        let lines: [(&str, String); 34] = [
            ("hudCooldowns", u.cooldowns.to_string()),
            ("hudCooldownsCorner", u.cooldowns_corner.name().into()),
            ("hudTotems", u.totems.to_string()),
            ("hudTotemsCorner", u.totems_corner.name().into()),
            ("hudPots", u.pots.to_string()),
            ("hudPotsCorner", u.pots_corner.name().into()),
            ("oteriaCrystal", self.crystal.enabled.to_string()),
            ("oteriaCrystalKeep", self.crystal.keep_render.to_string()),
            ("oteriaAnchor", self.anchor.enabled.to_string()),
            ("oteriaAnchorCharge", self.anchor.charge.to_string()),
            ("oteriaPops", self.pops.to_string()),
            ("oteriaSecret", self.secret.effect.to_string()),
            ("hitboxes", self.hitboxes.enabled.to_string()),
            ("hitboxesEyes", self.hitboxes.eyes.to_string()),
            ("hitboxesLook", self.hitboxes.look.to_string()),
            ("hitboxesOwn", self.hitboxes.own.to_string()),
            ("hitboxesWidth", self.hitboxes.width.to_string()),
            ("hitboxesPlayers", off(self.hitboxes.players)),
            ("hitboxesHostile", off(self.hitboxes.hostile)),
            ("hitboxesPassive", off(self.hitboxes.passive)),
            ("hitboxesItems", off(self.hitboxes.items)),
            ("hitboxesProjectiles", off(self.hitboxes.projectiles)),
            ("hitboxesCrystals", off(self.hitboxes.crystals)),
            ("hitboxesOthers", off(self.hitboxes.others)),
            ("hitboxesTarget", off(self.hitboxes.target)),
            ("hitboxesHurt", off(self.hitboxes.hurt)),
            ("hitboxesOutline", off(self.hitboxes.outline)),
            ("hitboxesOutlineSize", self.hitboxes.outline_size.to_string()),
            ("hitboxesFarWidth", self.hitboxes.far_width.to_string()),
            ("hitboxesFarDistance", self.hitboxes.far_distance.to_string()),
            ("hitboxesFireworks", self.hitboxes.fireworks.to_string()),
            ("hitboxesHideStuck", self.hitboxes.hide_stuck.to_string()),
            ("hitboxesEyeColour", self.hitboxes.eye_colour.save()),
            ("hitboxesLookColour", self.hitboxes.look_colour.save()),
        ];
        for (key, value) in lines {
            let _ = writeln!(out, "{key}:{value}");
        }
        let _ = write!(
            out,
            "skinKnife:{}\nknifeSkin:{}\nknifeInspectKey:{}\nplayerSkin:{}\n",
            self.skins.knife, self.skins.knife_skin, self.skins.inspect_key, self.skins.player
        );
        let _ = write!(
            out,
            "oteriaCape:{}\noteriaWings:{}\noteriaAura:{}\n",
            self.skins.cape, self.skins.wings, self.skins.aura
        );
        let _ = write!(out, "skyMode:{}\nskyPack:{}\n", self.sky.enabled, self.sky.pack);
        let _ = write!(out, "tierTagger:{}\ntierMode:{}\n", self.tiers.enabled, self.tiers.mode);
        let _ = writeln!(out, "keyboardLayout:{}", self.keyboard_layout);
        let m = &self.music;
        let _ = write!(
            out,
            "music:{}\nmusicLyrics:{}\nmusicCard:{}\nmusicPlayKey:{}\nmusicNextKey:{}\nmusicPreviousKey:{}\n",
            m.enabled, m.lyrics, m.card, m.play_key, m.next_key, m.previous_key
        );
        let _ = writeln!(out, "musicDistance:{}", m.distance);
        let vm = &self.viewmodel;
        let triple = |v: [u8; 3]| format!("{},{},{}", v[0], v[1], v[2]);
        let _ = write!(
            out,
            "viewmodel:{}\nviewmodelMain:{}\nviewmodelOff:{}\n",
            vm.enabled,
            triple(vm.main),
            triple(vm.off)
        );
        let sb = &self.scoreboard;
        let _ = write!(
            out,
            "scoreboard:{}\nscoreboardBackground:{}\nscoreboardOpacity:{}\nscoreboardRounded:{}\nscoreboardNumbers:{}\n",
            sb.enabled,
            sb.background.map_or_else(|| "off".into(), Colour::save),
            sb.opacity,
            sb.rounded,
            sb.numbers,
        );
        let cv = &self.clear_view;
        let _ = write!(
            out,
            "clearView:{}\nclearViewPumpkin:{}\nclearViewDarkness:{}\nclearViewBlindness:{}\nclearViewNausea:{}\n",
            cv.enabled, cv.pumpkin, cv.darkness, cv.blindness, cv.nausea,
        );
        let ft = &self.fast_totem;
        let _ = write!(
            out,
            "fastTotem:{}\nfastTotemAuto:{}\nfastTotemDelay:{}\nfastTotemShiftClick:{}\nhurtCam:{}\nhurtCamStrength:{}\n",
            ft.enabled, ft.auto, ft.delay, ft.shift_click, self.hurtcam.enabled, self.hurtcam.strength,
        );
        let e = &self.environment;
        let ep: Vec<String> = e.particles_pixels.iter().map(|row| format!("{row:x}")).collect();
        let _ = write!(
            out,
            "environmentParticlesShape:{}\nenvironmentParticlesSize:{}\nenvironmentParticlesPixels:{}\n",
            e.particles_shape.name(),
            e.particles_size,
            ep.join(","),
        );
        let _ = writeln!(out, "oteriaVideoDefaults:{}", self.video_defaults);
        let b = &self.barrel;
        let _ = write!(
            out,
            "barrelRoll:{}\nbarrelBanking:{}\nbarrelRighting:{}\nbarrelInvertPitch:{}\nbarrelSwitchRollYaw:{}\nbarrelSensitivity:{}\n",
            b.enabled, b.banking, b.righting, b.invert_pitch, b.switch_roll_yaw, b.sensitivity
        );
        let _ = write!(
            out,
            "motionBlur:{}\nmotionBlurStrength:{}\ntimeChanger:{}\ntimeChangerAt:{}\nweatherChanger:{}\nweatherChangerAt:{}\n",
            self.blur.enabled,
            self.blur.strength,
            self.clock.enabled,
            self.clock.time,
            self.sky_weather.enabled,
            self.sky_weather.weather.name(),
        );
        let e = &self.environment;
        let _ = write!(
            out,
            "environment:{}\nenvironmentGround:{}\nenvironmentGroundColour:{}\nenvironmentGroundRadius:{}\nenvironmentParticles:{}\nenvironmentParticlesColour:{}\nenvironmentParticlesAmount:{}\n",
            e.enabled,
            e.ground,
            e.ground_colour.save(),
            e.ground_radius,
            e.particles,
            e.particles_colour.save(),
            e.particles_amount,
        );
        let _ = write!(
            out,
            "environmentHat:{}\nenvironmentHatColour:{}\nenvironmentSky:{}\nenvironmentSkyColour:{}\nenvironmentFog:{}\nenvironmentFogColour:{}\nenvironmentFogDensity:{}\n",
            e.hat,
            e.hat_colour.save(),
            e.sky,
            e.sky_colour.save(),
            e.fog,
            e.fog_colour.save(),
            e.fog_density,
        );
        let z = &self.zoom;
        let _ = write!(
            out,
            "oteriaZoom:{}\noteriaZoomKey:{}\noteriaZoomSmooth:{}\noteriaZoomLevel:{}\n",
            z.enabled, z.key, z.smooth, z.level
        );
        let mm = &self.minimap;
        let _ = write!(
            out,
            "mapRound:{}\nmapBiome:{}\nmapRotate:{}\nmapZoom:{}\n",
            mm.round, mm.biome, mm.rotate, mm.zoom
        );
        let m = &self.mouse;
        let _ = write!(
            out,
            "mouseRmb:{}\nmouseLmbItem:{}\nmouseLmbEmpty:{}\nmouseWheelOrder:{}\nmouseWheelDirection:{}\nmouseScrollScaling:{}\n",
            m.rmb,
            m.lmb_item,
            m.lmb_empty,
            if m.first_to_last { "first" } else { "last" },
            m.direction,
            if m.always_one { "one" } else { "proportional" },
        );
        let v = &self.voice;
        let _ = write!(
            out,
            "voiceChat:{}\nvoiceActivation:{}\nvoiceKey:{}\nvoiceVolume:{}\nvoiceMic:{}\nvoiceThreshold:{}\n",
            v.enabled,
            if v.activation { "voice" } else { "ptt" },
            v.key,
            v.volume,
            v.mic,
            v.threshold,
        );
        let volumes: Vec<String> = v.volumes.iter().map(|(id, pct)| format!("{id:x}={pct}")).collect();
        let _ = write!(
            out,
            "voiceMuted:{}\nvoiceDeafened:{}\nvoiceHideIcons:{}\nvoiceNoise:{}\nvoiceSpatial:{}\nvoiceMicDevice:{}\nvoiceSpeakerDevice:{}\nvoiceMenuKey:{}\nvoiceVolumes:{}\n",
            v.muted,
            v.deafened,
            v.hide_icons,
            v.noise,
            v.spatial,
            v.mic_device,
            v.speaker_device,
            v.menu_key,
            volumes.join(","),
        );
    }
}

/// The official mods the PvP preset turns on when they are installed: the FPS counter and the
/// keystrokes with clicks per second.
pub const PRESET_MODS: [&str; 2] = ["fps_overlay", "keystrokes"];
/// Fire overlay's place under the PvP preset (percent of its height down).
const PRESET_FIRE_LOWER: u8 = 40;
/// The PvP preset: the custom crosshair, the armour and effects widgets, lower fire, and the FPS and keystrokes mods when installed. It only turns
/// things on; each keeps its own settings, so it can be undone option by option.
pub fn apply_preset(s: &mut crate::settings::Settings) {
    let p = &mut s.extras.pvp;
    p.crosshair.custom = true;
    p.hud.armour = true;
    p.hud.effects = true;
    if p.tweaks.fire_lower == 0 {
        p.tweaks.fire_lower = PRESET_FIRE_LOWER;
    }
    for id in PRESET_MODS {
        s.mods.turn_on(id);
    }
}

/// The options of the PvP & HUD page: each one's id (its language keys are `horizon.pvp.<id>`
/// and `horizon.pvp.<id>.tooltip`), English caption and description (en_us.json has the same
/// text; a test keeps them equal).
macro_rules! opts {
    ($($name:ident, $id:literal, $caption:literal, $description:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Opt { $($name),* }
        impl Opt {
            pub const ALL: &'static [Opt] = &[$(Opt::$name),*];
            pub fn caption_key(self) -> &'static str {
                match self { $(Opt::$name => concat!("horizon.pvp", ".", $id)),* }
            }
            pub fn description_key(self) -> &'static str {
                match self { $(Opt::$name => concat!("horizon.pvp", ".", $id, ".tooltip")),* }
            }
            pub fn caption(self) -> &'static str {
                match self { $(Opt::$name => $caption),* }
            }
            pub fn description(self) -> &'static str {
                match self { $(Opt::$name => $description),* }
            }
        }
    };
}
opts! {
    Crosshair, "crosshair", "Custom crosshair",
        "Draws your own crosshair instead of vanilla's. Its first settings look like vanilla's.";
    CrosshairShape, "crosshair_shape", "Shape", "A cross, a T, an X, a circle, a square or only a dot.";
    CrosshairSize, "crosshair_size", "Size", "The length of each arm, in GUI pixels.";
    CrosshairGap, "crosshair_gap", "Gap", "The empty space in the middle.";
    CrosshairThickness, "crosshair_thickness", "Thickness", "How wide each line is.";
    CrosshairColour, "crosshair_colour", "Colour",
        "Inverted shows the colour under it inverted, as vanilla's crosshair does.";
    CrosshairOutline, "crosshair_outline", "Outline", "A thin black edge, so it shows on any background.";
    CrosshairDot, "crosshair_dot", "Centre dot", "A dot in the middle of the crosshair.";
    CrosshairHover, "crosshair_hover", "Colour on entities",
        "Another colour while the crosshair is over a mob or a player you could hit, as vanilla's attack indicator knows it.";
    CrosshairIndicator, "crosshair_indicator", "Player indicator",
        "While the crosshair is on a player you can hit, a second drawing shows over it (Crosshair Indicator). Draw it pixel by pixel below the crosshair's grid.";
    Outline, "outline", "Custom block outline", "Changes the outline around the block you look at.";
    OutlineColour, "outline_colour", "Colour", "The outline's colour. Black at 40% is vanilla's.";
    OutlineOpacity, "outline_opacity", "Opacity", "How solid the outline is.";
    OutlineThickness, "outline_thickness", "Thickness", "The outline's width, in times vanilla's.";
    OutlineFill, "outline_fill", "Fill", "Tints the faces of the block you look at, lightly.";
    Hit, "hit", "Custom hit colour",
        "Changes the colour mobs and players flash when they are hurt. Red at 30% is vanilla's.";
    HitColour, "hit_colour", "Colour", "The colour hurt mobs and players flash.";
    HitOpacity, "hit_opacity", "Opacity", "How much of the colour covers them.";
    HudArmour, "hud_armour", "Armour",
        "The armour you wear, head first, with how many uses each piece has left.";
    HudArmourCorner, "hud_armour_corner", "Armour corner", "Where the armour shows.";
    HudArmourPercent, "hud_armour_percent", "Durability in %",
        "Uses left as a percent of the piece's maximum instead of a number.";
    HudEffects, "hud_effects", "Effects",
        "Your effects with their names, levels and time left, in place of the icons at the top right.";
    HudEffectsCorner, "hud_effects_corner", "Effects corner", "Where the effects show.";
    HudHeld, "hud_held", "Held item count",
        "How many of the item in your hand your inventory holds; arrows with a bow or crossbow.";
    HudHeldCorner, "hud_held_corner", "Held count corner", "Where the count shows.";
    HudPing, "hud_ping", "Ping", "The latency the server reports for you, in milliseconds. Multiplayer only.";
    HudPingCorner, "hud_ping_corner", "Ping corner", "Where the ping shows.";
    HudCoords, "hud_coords", "Coordinates",
        "Your block position and the direction you face, as F3 shows them. Hidden when the world or server reduces F3.";
    HudCoordsCorner, "hud_coords_corner", "Coordinates corner", "Where the coordinates show.";
    HudServer, "hud_server", "Server address", "The address of the server you play on. Multiplayer only.";
    HudServerCorner, "hud_server_corner", "Server corner", "Where the address shows.";
    HudCombo, "hud_combo", "Combo counter",
        "Hits in a row the server confirmed, until you are hurt or 3 seconds pass without one.";
    HudComboCorner, "hud_combo_corner", "Combo corner", "Where the combo shows.";
    HudFps, "hud_fps", "FPS", "Frames drawn in the last second.";
    HudFpsCorner, "hud_fps_corner", "FPS corner", "Where the FPS shows.";
    HudCps, "hud_cps", "Clicks per second", "Your left and right clicks in the last second.";
    HudCpsCorner, "hud_cps_corner", "CPS corner", "Where the clicks per second show.";
    HudKeys, "hud_keys", "Keystrokes",
        "Your movement keys, mouse buttons and jump, lit while you hold them, with clicks per second.";
    HudKeysCorner, "hud_keys_corner", "Keystrokes corner", "Where the keystrokes show.";
    HudClock, "hud_clock", "Clock", "Your computer's time.";
    HudClockCorner, "hud_clock_corner", "Clock corner", "Where the clock shows.";
    HudReach, "hud_reach", "Reach",
        "How far from your eyes your last hit landed, for 3 seconds. It only measures: your reach stays vanilla's.";
    HudReachCorner, "hud_reach_corner", "Reach corner", "Where the reach shows.";
    HudSpeed, "hud_speed", "Speed", "How fast you move across the ground, in blocks per second.";
    HudSpeedCorner, "hud_speed_corner", "Speed corner", "Where the speed shows.";
    HudBackground, "hud_background", "Widget backgrounds", "A dark box behind each widget.";
    HudOpacity, "hud_opacity", "Background opacity", "How dark the boxes behind the widgets are.";
    HudTextColour, "hud_text_colour", "Text colour",
        "The widgets' text colour. Durability keeps its green to red.";
    HudShadow, "hud_shadow", "Text shadow", "A shadow under the widgets' text, as vanilla's HUD text has.";
    HudEditorKey, "hud_editor_key", "HUD editor key",
        "Opens the HUD editor from the game. Click, then press a key; Escape unbinds it.";
    NumericEnchants, "numeric_enchants", "Enchantment levels as numbers",
        "Tooltips and the enchanting table show Sharpness 5 instead of Sharpness V.";
    FireLower, "fire_lower", "Lower fire",
        "Moves the flames you see while burning down, so they cover less of the screen.";
    ShieldSize, "shield_size", "Shield size", "The size of the shield in your hand, in first person.";
    TotemSize, "totem_size", "Totem animation size",
        "The size of the totem that fills the screen when it saves you.";
    GlintColour, "glint_colour", "Enchantment glint colour",
        "The colour of the shimmer on enchanted items and armour. Off is vanilla's purple.";
    ClearWater, "clear_water", "Clear water",
        "Under water, see as far as vanilla lets you after 30 seconds there, from the start.";
    ClearLava, "clear_lava", "Clear lava",
        "In lava, see as far as vanilla's Fire Resistance lets you, without the effect.";
    ViewTime, "view_time", "Time of day",
        "The time of day you see: the sky and its light only. The world's time goes on and nothing is sent to the server.";
    ViewWeather, "view_weather", "Weather",
        "The weather you see: rain, its light and fog only. The world's weather goes on and nothing is sent to the server.";
    ChatTimestamps, "chat_timestamps", "Timestamps", "The time each message came in, before it.";
    ChatCompact, "chat_compact", "Compact chat",
        "A message repeated in a row shows once, with how many times it came: (x3).";
    ChatCopy, "chat_copy", "Copy on click",
        "Clicking a message in the open chat copies its text, unless the click does something else.";
    ChatSearch, "chat_search", "Chat search",
        "Ctrl+F in the open chat: what you type shows only the messages holding it. Enter or Ctrl+F again stops.";
    CombatFeel, "combat_feel", "Combat feel",
        "Legacy draws fights as 1.8.9 did: the item in your hand, its swing, eating, the bow and a sword held up while you hold use, without the dip after each hit or the attack indicator. Only the look: what you send a server stays the same.";
    CombatIndicator, "combat_indicator", "Attack indicator in Legacy",
        "Keeps the attack indicator under Legacy, which 1.8.9 didn't have.";
    CombatSounds, "combat_sounds", "1.8 hit sounds",
        "Under Legacy, leaves out the attack sounds 1.9 added (strong, weak, sweep, critical...). Hurt sounds stay.";
    CombatRules, "combat_rules", "1.8 combat in worlds you host",
        "Single player and the worlds you open to LAN fight by 1.8.9's rules: no attack cooldown or sweep, 1.8 weapon damage, Strength, Sharpness and critical hits. Servers you join keep their own rules.";
    ElytraIndicator, "elytra_indicator", "Elytra indicator",
        "A small elytra under the crosshair while you wear one, with how worn it is.";
    ElytraColour, "elytra_colour", "Colour",
        "Tints the elytra icon. Off follows your crosshair's colour, or white without a custom one. Move and size it in Edit HUD.";
    Saturation, "saturation", "Food Saturation",
        "Your saturation as gold outlines over the hunger bar.";
    SaturationHeld, "saturation_held", "Food in hand",
        "Holding food, the hunger, saturation and health it would give you flash on the bars.";
    SaturationExhaustion, "saturation_exhaustion", "Exhaustion",
        "A bar behind the hunger icons fills as you tire; once full, you lose a point of saturation, or of hunger.";
    SaturationTooltip, "saturation_tooltip", "Food values in tooltips",
        "Hunger and saturation points shown on food items' tooltips.";
    Voice, "voice", "Voice chat",
        "Talk with the players near you on servers running Simple Voice Chat, Fabric players included.";
    VoiceActivation, "voice_activation", "Talk",
        "Push to talk sends your voice while the key is held; Voice sends it whenever the microphone hears you.";
    VoiceKey, "voice_key", "Push-to-talk key", "The key to hold to talk.";
    VoiceVolume, "voice_volume", "Voices", "How loud the other players are.";
    VoiceMic, "voice_mic", "Microphone", "How loud you are to the others.";
    MapShape, "map_shape", "Shape", "A square map, or a round one.";
    MapBiome, "map_biome", "Biome name", "The name of the biome you are in, under the map.";
    MapRotate, "map_rotate", "Turn with you",
        "The map turns so that where you look is up, with N on its rim. Off: north is always up.";
    MapZoom, "map_zoom", "Zoom", "How many blocks each pixel of the map shows.";
    MouseRmb, "mouse_rmb", "RMB tweak",
        "Holding a stack, right-drag over slots: one item goes into each slot you pass over.";
    MouseLmbItem, "mouse_lmb_item", "LMB tweak with item",
        "Holding a stack, left-drag over slots: the same items are gathered into your hand. With Shift they are moved to the other inventory.";
    MouseLmbEmpty, "mouse_lmb_empty", "LMB tweak without item",
        "With an empty hand, hold Shift and left-drag over slots: each stack you pass over moves to the other inventory, as a Shift click.";
    MouseWheelOrder, "mouse_wheel_order", "Wheel search order",
        "Where the wheel looks first for the items it pulls into a slot.";
    MouseWheelDirection, "mouse_wheel_direction", "Wheel scroll direction",
        "Normal: down moves one item to the other inventory, up brings one back. Inverted: the other way. Position aware: scrolling towards the other inventory moves items there.";
    MouseScrollScaling, "mouse_scroll_scaling", "Scroll item scaling",
        "Proportional: a big wheel step moves more items. Always one: each step moves exactly one item.";
    VoiceMenuKey, "voice_menu_key", "Menu key",
        "Opens the voice chat's menu in game: microphone, sound, group and settings.";
    VoiceNoise, "voice_noise", "Noise suppression",
        "Takes the steady background noise (a fan, a hum) out of your microphone.";
    VoiceSpatial, "voice_spatial", "3D audio",
        "Voices come from where the players are, left or right. Off: everyone in the middle.";
    Sky, "sky", "Sky mode",
        "Draws a sky pack from your resourcepacks folder (OptiFine's custom sky format: nebulae, stars, a skybox) in place of the plain sky.";
    SkyPack, "sky_pack", "Sky",
        "Which sky pack, among those in your resourcepacks folder.";
    VoiceThreshold, "voice_threshold", "Voice activation level",
        "With Voice: how quiet your speech can be and still be sent. A higher number sends quieter sounds.";
    Zoom, "zoom", "Zoom",
        "Hold the zoom key to look far, as OptiFine's; the wheel zooms in and out while it is held. Its key is unbound at first: set it in Controls, under Oteria.";
    ZoomSmooth, "zoom_smooth", "Smooth zoom", "The view eases in and out. Off: it narrows at once.";
    ZoomLevel, "zoom_level", "Magnification", "How many times the view is magnified when you press the key.";
    Tiers, "tiers", "Tier Tagger",
        "Each player's PvP tier from MCTiers (HT1 to LT5) before their name, over their head and in the player list.";
    TiersMode, "tiers_mode", "Game mode", "Which game mode's tier to show; Best shows each player's best one.";
    Blur, "blur", "Motion blur", "Each frame keeps a trail of the ones before it, so fast turns blur.";
    BlurStrength, "blur_strength", "Strength", "How long the trail lasts.";
    Env, "environment", "Environment",
        "Dresses the world: waves of tiles over the ground, particles in the air, a China hat, an aurora sky and a coloured fog. Only your view changes.";
    EnvGround, "environment_ground", "Ground waves", "Waves of glowing tiles run out from under your feet over the ground.";
    EnvGroundColour, "environment_ground_colour", "Ground colour", "The tiles' and their edges' colour.";
    EnvGroundRadius, "environment_ground_radius", "Wave reach", "How far the waves run, in blocks.";
    EnvParticles, "environment_particles", "Particles", "Flakes and sparkles floating in the air around you.";
    EnvParticlesColour, "environment_particles_colour", "Particle colour", "The flakes' and sparkles' colour.";
    EnvParticlesAmount, "environment_particles_amount", "Particle amount", "How many particles float in the air.";
    EnvParticlesShape, "environment_particles_shape", "Particle shape", "Flakes and sparkles, a ready-made shape (heart, star, square...) or one you draw.";
    EnvParticlesSize, "environment_particles_size", "Particle size", "How big the shaped particles are.";
    EnvHat, "environment_hat", "China hat", "A glowing China hat and a halo over your head, in third person.";
    EnvHatColour, "environment_hat_colour", "Hat colour", "The hat's and its halo's colour.";
    EnvSky, "environment_sky", "Aurora sky", "Waves of light across the sky, with twinkling stars and shooting stars.";
    EnvSkyColour, "environment_sky_colour", "Sky colour", "The sky's colour; its waves are lighter, the horizon darker.";
    EnvFog, "environment_fog", "Fog", "A coloured fog over the distance.";
    EnvFogColour, "environment_fog_colour", "Fog colour", "The fog's colour.";
    EnvFogDensity, "environment_fog_density", "Fog density", "How close the fog comes.";
    Clock, "clock", "Time changer",
        "Fixes the time of day you see, wherever the slider puts it. Only your view changes: the server's time stays.";
    ClockTime, "clock_time", "Time", "The time of day you see. Drag the slider.";
    SkyWeather, "sky_weather", "Weather changer",
        "Fixes the weather you see. Only your view changes: the server's weather stays.";
    SkyWeatherPick, "sky_weather_pick", "Weather", "The weather you see.";
    Barrel, "barrel", "Barrel roll",
        "Flying with an elytra, the view turns as a plane: the mouse pitches and rolls, Left and Right are the rudder. Loops and barrel rolls go all the way round.";
    BarrelBanking, "barrel_banking", "Banking", "Rolled, you turn towards the lower wing, as a plane in a turn.";
    BarrelRighting, "barrel_righting", "Automatic righting", "A roll of less than 90 degrees comes back level by itself.";
    BarrelInvert, "barrel_invert", "Invert pitch", "Mouse up points the nose down.";
    BarrelSwitch, "barrel_switch", "Switch roll and yaw",
        "The mouse's left and right turn you as in vanilla instead of rolling; Left and Right still steer.";
    BarrelSensitivity, "barrel_sensitivity", "Sensitivity", "How fast the mouse turns the plane.";
    Music, "music", "Music",
        "The song playing on this computer (Spotify, or any player Windows' media controls know) and its synced lyrics, from LRCLIB. Play/pause, next and previous have keys in Controls, under Oteria.";
    MusicLyrics, "music_lyrics", "Floating lyrics", "The line sung now floats in the air ahead of you, the next one under it.";
    MusicCard, "music_card", "Song card", "The song, its artist and how far it has played, at the top right.";
    MusicDistance, "music_distance", "Lyrics distance",
        "How far from you the floating lyrics land, in blocks: the words spread a little nearer and a little farther than this.";
    Viewmodel, "viewmodel", "Viewmodel",
        "Moves the items in your hands in first person, each hand on its own: left and right (X), up and down (Y), nearer and farther (Z). 0 is vanilla's place.";
    ViewMainX, "view_main_x", "Main hand X", "The main hand's item to the left (below 0) or the right (above 0).";
    ViewMainY, "view_main_y", "Main hand Y", "The main hand's item down (below 0) or up (above 0).";
    ViewMainZ, "view_main_z", "Main hand Z", "The main hand's item farther (below 0) or nearer (above 0).";
    ViewOffX, "view_off_x", "Off hand X", "The off hand's item to the left (below 0) or the right (above 0).";
    ViewOffY, "view_off_y", "Off hand Y", "The off hand's item down (below 0) or up (above 0).";
    ViewOffZ, "view_off_z", "Off hand Z", "The off hand's item farther (below 0) or nearer (above 0).";
    Scoreboard, "scoreboard", "Custom scoreboard",
        "Your own look for the scoreboard on the right of the screen. Move it and size it in Edit HUD, or with Size here.";
    ScoreboardSize, "scoreboard_size", "Size", "How big the scoreboard is.";
    ScoreboardBackground, "scoreboard_background", "Background", "The background's colour, or none.";
    ScoreboardOpacity, "scoreboard_opacity", "Opacity", "How solid the background is.";
    ScoreboardRounded, "scoreboard_rounded", "Rounded corners", "The background with rounded corners.";
    ScoreboardNumbers, "scoreboard_numbers", "Numbers", "The red scores on the right of each line.";
    ClearView, "clear_view", "Clear View",
        "Leaves screen effects out of your view, each with its own switch. Only the picture changes: the effects still act.";
    ClearPumpkin, "clear_pumpkin", "Pumpkin overlay", "No carved pumpkin over the screen when you wear one on your head.";
    ClearDarkness, "clear_darkness", "Darkness", "No Warden's Darkness: neither its fog nor its pulsing dark.";
    ClearBlindness, "clear_blindness", "Blindness", "No Blindness fog.";
    ClearNausea, "clear_nausea", "Nausea", "No Nausea wobble nor its overlay.";
    FastTotem, "fast_totem", "Fast Totem",
        "Totems of undying into your off hand: by themselves when it is empty, or with Shift + click on a totem in your inventory.";
    FastTotemAuto, "fast_totem_auto", "Refill by itself", "An empty off hand (after a pop) takes a totem from your inventory.";
    FastTotemDelay, "fast_totem_delay", "Delay", "How many ticks the off hand stays empty before the totem comes (20 ticks: one second).";
    FastTotemShift, "fast_totem_shift", "Shift + click", "Shift + click on a totem in your inventory puts it in your off hand.";
    HurtCam, "hurtcam", "HurtCam",
        "How much the camera shakes when you are hit or hurt: 0 never, 4 as vanilla, 10 a lot.";
    HurtCamStrength, "hurtcam_strength", "Strength", "0: no shake at all; 4: vanilla; 10: the strongest.";
    HudCooldowns, "hud_cooldowns", "Cooldowns",
        "The items cooling down in your hotbar and hands (ender pearl, chorus fruit, wind charge, shield...), with a bar and the seconds left.";
    HudCooldownsCorner, "hud_cooldowns_corner", "Cooldowns corner", "Where the cooldowns show.";
    HudTotems, "hud_totems", "Totem counter",
        "How many totems of undying you carry, and how many you popped since you last died.";
    HudTotemsCorner, "hud_totems_corner", "Totems corner", "Where the totems show.";
    HudPots, "hud_pots", "Pot counter",
        "How many healing splash potions you carry, and the seconds since you threw your last potion.";
    HudPotsCorner, "hud_pots_corner", "Pots corner", "Where the potions show.";
    Crystal, "crystal", "Crystal Optimizer",
        "A crystal you hit hard enough to break leaves at once, so the next one goes where it stood without waiting for the server. If the server didn't break it, it comes back 1.5 seconds later. Nothing is sent that vanilla wouldn't send.";
    CrystalKeep, "crystal_keep", "Keep render",
        "The broken crystal stays drawn until the server removes it; you can still place the next one there.";
    Anchor, "anchor", "Anchor Optimizer",
        "A charged respawn anchor you set off disappears at once, without waiting for the server. The server stays the judge: if it didn't explode, the anchor comes back.";
    AnchorCharge, "anchor_charge", "Instant charge", "Glowstone charges the anchor at once too.";
    Hitboxes, "hitboxes", "Hitboxes",
        "Every entity's hitbox, in a colour by its kind, with its eyes and where it looks, as F3+B. Off for a kind: its boxes aren't drawn.";
    HitboxPlayers, "hitbox_players", "Players", "The colour of other players' boxes.";
    HitboxHostile, "hitbox_hostile", "Hostile mobs", "The colour of monsters' boxes.";
    HitboxPassive, "hitbox_passive", "Passive mobs", "The colour of animals' and villagers' boxes.";
    HitboxItems, "hitbox_items", "Items", "The colour of dropped items' and experience orbs' boxes.";
    HitboxProjectiles, "hitbox_projectiles", "Projectiles", "The colour of arrows', pearls', potions' and tridents' boxes.";
    HitboxCrystals, "hitbox_crystals", "End crystals", "The colour of End crystals' boxes.";
    HitboxOthers, "hitbox_others", "Others", "The colour of boats', minecarts', armour stands' and the other entities' boxes.";
    HitboxEyes, "hitbox_eyes", "Eye line", "A line across each living entity's box at its eyes.";
    HitboxLook, "hitbox_look", "Look direction", "An arrow from the eyes, where the entity looks.";
    HitboxWidth, "hitbox_width", "Line width", "How thick the lines are.";
    HitboxOwn, "hitbox_own", "Your own", "Your own box too, in third person.";
    HitboxTarget, "hitbox_target", "Targeted entity", "The box of the entity under your crosshair, in this colour (Combat Hitbox).";
    HitboxHurt, "hitbox_hurt", "When hit", "The box of an entity that was just hit, in this colour, to time your next hit.";
    HitboxEyeColour, "hitbox_eye_colour", "Eye line colour", "The colour of the line at the eyes.";
    HitboxLookColour, "hitbox_look_colour", "Look direction colour", "The colour of the line where the entity looks.";
    HitboxOutline, "hitbox_outline", "Outline", "A thicker line behind each box, in this colour, so it shows on any background.";
    HitboxOutlineSize, "hitbox_outline_size", "Outline size", "How much wider than the box's lines the outline is.";
    HitboxFarWidth, "hitbox_far_width", "Width far away", "The lines' width for the entities farther than the distance below.";
    HitboxFarDistance, "hitbox_far_distance", "Far away from", "From how many blocks away the far width is used.";
    HitboxFireworks, "hitbox_fireworks", "Fireworks", "Fireworks' boxes too.";
    HitboxStuck, "hitbox_stuck", "Hide stuck arrows", "Hides the arrows and bee stingers stuck in players.";
    Pops, "pops", "Pop Counter",
        "A chat line each time a player near you uses a totem, with how many they popped, and when they die after popping.";
    Secret, "secret", "Super Secret Settings",
        "A fun effect over the whole screen, as 1.8's button. The button in Options picks one at random; so does its key in Controls, under Oteria (Shift with it turns it off).";
}
/// What a colour option lets the player choose besides any solid colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColourChoice {
    /// None (the option off).
    pub off: bool,
    /// Vanilla's inverted blend.
    pub inverted: bool,
}
impl ColourChoice {
    /// The ready-made choices, in the picker's order: off, inverted, then the solid colours.
    pub fn presets(self) -> Vec<Option<Colour>> {
        let mut all = Vec::new();
        if self.off {
            all.push(None);
        }
        if self.inverted {
            all.push(Some(Colour::Inverted));
        }
        all.extend(Colour::ALL[1..].iter().copied().map(Some));
        all
    }
}
impl Opt {
    /// For a colour option: what it offers.
    pub fn colour_choice(self) -> Option<ColourChoice> {
        let (off, inverted) = match self {
            Opt::CrosshairColour => (false, true),
            Opt::CrosshairHover => (true, true),
            Opt::OutlineColour | Opt::HitColour | Opt::HudTextColour => (false, false),
            Opt::OutlineFill | Opt::GlintColour | Opt::ElytraColour => (true, false),
            Opt::EnvGroundColour
            | Opt::EnvParticlesColour
            | Opt::EnvHatColour
            | Opt::EnvSkyColour
            | Opt::EnvFogColour => (false, false),
            Opt::HitboxPlayers
            | Opt::HitboxHostile
            | Opt::HitboxPassive
            | Opt::HitboxItems
            | Opt::HitboxProjectiles
            | Opt::HitboxCrystals
            | Opt::HitboxOthers
            | Opt::HitboxTarget
            | Opt::HitboxHurt
            | Opt::HitboxOutline => (true, false),
            Opt::HitboxEyeColour | Opt::HitboxLookColour => (false, false),
            Opt::ScoreboardBackground => (true, false),
            _ => return None,
        };
        Some(ColourChoice { off, inverted })
    }
    /// A colour option's colour now (`None`: off).
    pub fn colour(self, p: &Pvp) -> Option<Option<Colour>> {
        Some(match self {
            Opt::CrosshairColour => Some(p.crosshair.colour),
            Opt::CrosshairHover => p.crosshair.hover,
            Opt::OutlineColour => Some(p.outline.colour),
            Opt::OutlineFill => p.outline.fill,
            Opt::HitColour => Some(p.hit.colour),
            Opt::HudTextColour => Some(p.hud.text),
            Opt::GlintColour => p.tweaks.glint,
            Opt::ElytraColour => p.tweaks.elytra_colour,
            Opt::EnvGroundColour => Some(p.environment.ground_colour),
            Opt::EnvParticlesColour => Some(p.environment.particles_colour),
            Opt::EnvHatColour => Some(p.environment.hat_colour),
            Opt::EnvSkyColour => Some(p.environment.sky_colour),
            Opt::EnvFogColour => Some(p.environment.fog_colour),
            Opt::HitboxPlayers => p.hitboxes.players,
            Opt::HitboxHostile => p.hitboxes.hostile,
            Opt::HitboxPassive => p.hitboxes.passive,
            Opt::HitboxItems => p.hitboxes.items,
            Opt::HitboxProjectiles => p.hitboxes.projectiles,
            Opt::HitboxCrystals => p.hitboxes.crystals,
            Opt::HitboxOthers => p.hitboxes.others,
            Opt::HitboxTarget => p.hitboxes.target,
            Opt::HitboxHurt => p.hitboxes.hurt,
            Opt::HitboxOutline => p.hitboxes.outline,
            Opt::HitboxEyeColour => Some(p.hitboxes.eye_colour),
            Opt::HitboxLookColour => Some(p.hitboxes.look_colour),
            Opt::ScoreboardBackground => p.scoreboard.background,
            _ => return None,
        })
    }
    /// Sets a colour option; a choice it doesn't offer is left out.
    pub fn set_colour(self, p: &mut Pvp, c: Option<Colour>) {
        let Some(choice) = self.colour_choice() else {
            return;
        };
        if c.is_none() && !choice.off || c == Some(Colour::Inverted) && !choice.inverted {
            return;
        }
        match (self, c) {
            (Opt::CrosshairColour, Some(c)) => p.crosshair.colour = c,
            (Opt::CrosshairHover, c) => p.crosshair.hover = c,
            (Opt::OutlineColour, Some(c)) => p.outline.colour = c,
            (Opt::OutlineFill, c) => p.outline.fill = c,
            (Opt::HitColour, Some(c)) => p.hit.colour = c,
            (Opt::HudTextColour, Some(c)) => p.hud.text = c,
            (Opt::GlintColour, c) => p.tweaks.glint = c,
            (Opt::ElytraColour, c) => p.tweaks.elytra_colour = c,
            (Opt::EnvGroundColour, Some(c)) => p.environment.ground_colour = c,
            (Opt::EnvParticlesColour, Some(c)) => p.environment.particles_colour = c,
            (Opt::EnvHatColour, Some(c)) => p.environment.hat_colour = c,
            (Opt::EnvSkyColour, Some(c)) => p.environment.sky_colour = c,
            (Opt::EnvFogColour, Some(c)) => p.environment.fog_colour = c,
            (Opt::HitboxPlayers, c) => p.hitboxes.players = c,
            (Opt::HitboxHostile, c) => p.hitboxes.hostile = c,
            (Opt::HitboxPassive, c) => p.hitboxes.passive = c,
            (Opt::HitboxItems, c) => p.hitboxes.items = c,
            (Opt::HitboxProjectiles, c) => p.hitboxes.projectiles = c,
            (Opt::HitboxCrystals, c) => p.hitboxes.crystals = c,
            (Opt::HitboxOthers, c) => p.hitboxes.others = c,
            (Opt::HitboxTarget, c) => p.hitboxes.target = c,
            (Opt::HitboxHurt, c) => p.hitboxes.hurt = c,
            (Opt::HitboxOutline, c) => p.hitboxes.outline = c,
            (Opt::HitboxEyeColour, Some(c)) => p.hitboxes.eye_colour = c,
            (Opt::HitboxLookColour, Some(c)) => p.hitboxes.look_colour = c,
            (Opt::ScoreboardBackground, c) => p.scoreboard.background = c,
            _ => {}
        }
    }
    /// For a number option: its range and the unit shown after it.
    pub fn number(self) -> Option<((u8, u8), &'static str)> {
        Some(match self {
            Opt::CrosshairSize => (SIZES, ""),
            Opt::CrosshairGap => (GAPS, ""),
            Opt::CrosshairThickness => (THICKNESSES, ""),
            Opt::OutlineOpacity => (OPACITIES, "%"),
            Opt::OutlineThickness => (OUTLINE_THICKNESSES, "x"),
            Opt::HitOpacity => (HIT_OPACITIES, "%"),
            Opt::HudOpacity => (HUD_OPACITIES, "%"),
            Opt::FireLower => (FIRE_LOWER, "%"),
            Opt::ShieldSize => (SHIELD_SIZES, "%"),
            Opt::TotemSize => (TOTEM_SIZES, "%"),
            Opt::VoiceVolume | Opt::VoiceMic => (VOICE_PERCENT, "%"),
            Opt::VoiceThreshold => (VOICE_THRESHOLD, ""),
            Opt::ZoomLevel => (ZOOM_LEVELS, "x"),
            Opt::BlurStrength => (BLUR_STRENGTHS, "%"),
            Opt::EnvGroundRadius => (ENV_RADII, ""),
            Opt::EnvParticlesAmount | Opt::EnvFogDensity => (ENV_AMOUNTS, "%"),
            Opt::EnvParticlesSize => (ENV_SIZES, "%"),
            Opt::FastTotemDelay => (TOTEM_DELAYS, ""),
            Opt::HurtCamStrength => (HURTCAM_STRENGTHS, ""),
            Opt::BarrelSensitivity => (BARREL_SENSITIVITIES, "%"),
            Opt::HitboxWidth | Opt::HitboxFarWidth => (HITBOX_WIDTHS, ""),
            Opt::HitboxOutlineSize => (HITBOX_OUTLINE_SIZES, "x"),
            Opt::HitboxFarDistance => (HITBOX_FAR, ""),
            Opt::MusicDistance => (LYRICS_DISTANCES, ""),
            Opt::ScoreboardOpacity => (SCOREBOARD_OPACITIES, "%"),
            Opt::ScoreboardSize => (crate::pvp_hud::SCALES, "%"),
            _ => return None,
        })
    }
    /// A number option's value now.
    pub fn number_value(self, p: &Pvp) -> Option<u8> {
        Some(match self {
            Opt::CrosshairSize => p.crosshair.size,
            Opt::CrosshairGap => p.crosshair.gap,
            Opt::CrosshairThickness => p.crosshair.thickness,
            Opt::OutlineOpacity => p.outline.opacity,
            Opt::OutlineThickness => p.outline.thickness,
            Opt::HitOpacity => p.hit.opacity,
            Opt::HudOpacity => p.hud.opacity,
            Opt::FireLower => p.tweaks.fire_lower,
            Opt::ShieldSize => p.tweaks.shield_size,
            Opt::TotemSize => p.tweaks.totem_size,
            Opt::VoiceVolume => p.voice.volume,
            Opt::VoiceMic => p.voice.mic,
            Opt::VoiceThreshold => p.voice.threshold,
            Opt::ZoomLevel => p.zoom.level,
            Opt::BlurStrength => p.blur.strength,
            Opt::BarrelSensitivity => p.barrel.sensitivity,
            Opt::EnvGroundRadius => p.environment.ground_radius,
            Opt::EnvParticlesAmount => p.environment.particles_amount,
            Opt::EnvParticlesSize => p.environment.particles_size,
            Opt::FastTotemDelay => p.fast_totem.delay,
            Opt::HurtCamStrength => p.hurtcam.strength,
            Opt::EnvFogDensity => p.environment.fog_density,
            Opt::HitboxWidth => p.hitboxes.width,
            Opt::HitboxFarWidth => p.hitboxes.far_width,
            Opt::HitboxOutlineSize => p.hitboxes.outline_size,
            Opt::HitboxFarDistance => p.hitboxes.far_distance,
            Opt::MusicDistance => p.music.distance,
            Opt::ScoreboardOpacity => p.scoreboard.opacity,
            Opt::ScoreboardSize => p.layout.get(crate::pvp_hud::WidgetId::Scoreboard).scale,
            _ => return None,
        })
    }
    /// A viewmodel option's hand (main or not) and axis.
    fn viewmodel_axis(self) -> Option<(bool, usize)> {
        Some(match self {
            Opt::ViewMainX => (true, 0),
            Opt::ViewMainY => (true, 1),
            Opt::ViewMainZ => (true, 2),
            Opt::ViewOffX => (false, 0),
            Opt::ViewOffY => (false, 1),
            Opt::ViewOffZ => (false, 2),
            _ => return None,
        })
    }
    /// For an option set with a slider: its ends.
    pub fn slider(self) -> Option<(u32, u32)> {
        match self {
            Opt::ClockTime => Some(DAY_TICKS),
            _ if self.viewmodel_axis().is_some() => Some(VIEWMODEL_RANGE),
            _ => None,
        }
    }
    /// A slider option's value now.
    pub fn slider_value(self, p: &Pvp) -> Option<u32> {
        if let Some((main, axis)) = self.viewmodel_axis() {
            let v = if main { p.viewmodel.main } else { p.viewmodel.off };
            return Some(u32::from(v[axis]));
        }
        match self {
            Opt::ClockTime => Some(u32::from(p.clock.time)),
            _ => None,
        }
    }
    /// Sets a slider option, kept between its ends.
    pub fn set_slider(self, p: &mut Pvp, v: u32) {
        let Some((lo, hi)) = self.slider() else {
            return;
        };
        let v = v.clamp(lo, hi);
        if let Some((main, axis)) = self.viewmodel_axis() {
            let hand = if main { &mut p.viewmodel.main } else { &mut p.viewmodel.off };
            hand[axis] = v as u8;
        } else if self == Opt::ClockTime {
            p.clock.time = v as u16;
        }
    }
    /// Sets a number option, kept within its range.
    pub fn set_number(self, p: &mut Pvp, v: u32) {
        let Some(((lo, hi), _)) = self.number() else {
            return;
        };
        let v = v.clamp(u32::from(lo), u32::from(hi)) as u8;
        if self == Opt::ScoreboardSize {
            p.layout.get_mut(crate::pvp_hud::WidgetId::Scoreboard).scale = v;
            return;
        }
        let field = match self {
            Opt::MusicDistance => &mut p.music.distance,
            Opt::ScoreboardOpacity => &mut p.scoreboard.opacity,
            Opt::CrosshairSize => &mut p.crosshair.size,
            Opt::CrosshairGap => &mut p.crosshair.gap,
            Opt::CrosshairThickness => &mut p.crosshair.thickness,
            Opt::OutlineOpacity => &mut p.outline.opacity,
            Opt::OutlineThickness => &mut p.outline.thickness,
            Opt::HitOpacity => &mut p.hit.opacity,
            Opt::HudOpacity => &mut p.hud.opacity,
            Opt::FireLower => &mut p.tweaks.fire_lower,
            Opt::ShieldSize => &mut p.tweaks.shield_size,
            Opt::TotemSize => &mut p.tweaks.totem_size,
            Opt::VoiceVolume => &mut p.voice.volume,
            Opt::VoiceMic => &mut p.voice.mic,
            Opt::VoiceThreshold => &mut p.voice.threshold,
            Opt::ZoomLevel => &mut p.zoom.level,
            Opt::BlurStrength => &mut p.blur.strength,
            Opt::BarrelSensitivity => &mut p.barrel.sensitivity,
            Opt::EnvGroundRadius => &mut p.environment.ground_radius,
            Opt::EnvParticlesAmount => &mut p.environment.particles_amount,
            Opt::EnvParticlesSize => &mut p.environment.particles_size,
            Opt::FastTotemDelay => &mut p.fast_totem.delay,
            Opt::HurtCamStrength => &mut p.hurtcam.strength,
            Opt::EnvFogDensity => &mut p.environment.fog_density,
            Opt::HitboxWidth => &mut p.hitboxes.width,
            Opt::HitboxFarWidth => &mut p.hitboxes.far_width,
            Opt::HitboxOutlineSize => &mut p.hitboxes.outline_size,
            Opt::HitboxFarDistance => &mut p.hitboxes.far_distance,
            _ => return,
        };
        *field = v;
    }
}

/// The PvP & HUD page: headers (language keys) and options.
pub fn rows() -> Vec<Result<Opt, &'static str>> {
    use Opt::*;
    let mut rows = vec![Err("horizon.pvp.header.crosshair")];
    rows.extend(
        [
            Crosshair,
            CrosshairShape,
            CrosshairSize,
            CrosshairGap,
            CrosshairThickness,
            CrosshairColour,
            CrosshairOutline,
            CrosshairDot,
            CrosshairHover,
            CrosshairIndicator,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.outline"));
    rows.extend(
        [
            Outline,
            OutlineColour,
            OutlineOpacity,
            OutlineThickness,
            OutlineFill,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.hit"));
    rows.extend([Hit, HitColour, HitOpacity].map(Ok));
    rows.push(Err("horizon.pvp.header.hud"));
    rows.extend(
        [
            HudArmour,
            HudArmourCorner,
            HudArmourPercent,
            HudEffects,
            HudEffectsCorner,
            HudHeld,
            HudHeldCorner,
            HudPing,
            HudPingCorner,
            HudCoords,
            HudCoordsCorner,
            HudServer,
            HudServerCorner,
            HudCombo,
            HudComboCorner,
            HudFps,
            HudFpsCorner,
            HudCps,
            HudCpsCorner,
            HudKeys,
            HudKeysCorner,
            HudClock,
            HudClockCorner,
            HudReach,
            HudReachCorner,
            HudSpeed,
            HudSpeedCorner,
            HudBackground,
            HudOpacity,
            HudTextColour,
            HudShadow,
            HudEditorKey,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.items"));
    rows.push(Ok(NumericEnchants));
    rows.push(Ok(GlintColour));
    rows.push(Err("horizon.pvp.header.visual"));
    rows.extend(
        [
            FireLower,
            ShieldSize,
            TotemSize,
            ClearWater,
            ClearLava,
            ElytraIndicator,
            ElytraColour,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.saturation"));
    rows.extend([Saturation, SaturationHeld, SaturationExhaustion, SaturationTooltip].map(Ok));
    rows.push(Err("horizon.pvp.header.view"));
    rows.extend([ViewTime, ViewWeather].map(Ok));
    rows.push(Err("horizon.pvp.header.chat"));
    rows.extend([ChatTimestamps, ChatCompact, ChatCopy, ChatSearch].map(Ok));
    rows.push(Err("horizon.pvp.header.combat"));
    rows.extend([CombatFeel, CombatIndicator, CombatSounds, CombatRules].map(Ok));
    rows.push(Err("horizon.pvp.header.map"));
    rows.extend([MapShape, MapBiome, MapRotate, MapZoom].map(Ok));
    rows.push(Err("horizon.pvp.header.sky"));
    rows.extend([Sky, SkyPack].map(Ok));
    rows.push(Err("horizon.pvp.header.oteria_view"));
    rows.extend([Blur, BlurStrength, Clock, ClockTime, SkyWeather, SkyWeatherPick].map(Ok));
    rows.push(Err("horizon.pvp.header.environment"));
    rows.extend(
        [
            Env,
            EnvGround,
            EnvGroundColour,
            EnvGroundRadius,
            EnvParticles,
            EnvParticlesColour,
            EnvParticlesAmount,
            EnvParticlesShape,
            EnvParticlesSize,
            EnvHat,
            EnvHatColour,
            EnvSky,
            EnvSkyColour,
            EnvFog,
            EnvFogColour,
            EnvFogDensity,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.music"));
    rows.extend([Music, MusicLyrics, MusicDistance, MusicCard].map(Ok));
    rows.push(Err("horizon.pvp.header.viewmodel"));
    rows.extend([Viewmodel, ViewMainX, ViewMainY, ViewMainZ, ViewOffX, ViewOffY, ViewOffZ].map(Ok));
    rows.push(Err("horizon.pvp.header.scoreboard"));
    rows.extend(
        [
            Scoreboard,
            ScoreboardSize,
            ScoreboardBackground,
            ScoreboardOpacity,
            ScoreboardRounded,
            ScoreboardNumbers,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.clear_view"));
    rows.extend([ClearView, ClearPumpkin, ClearDarkness, ClearBlindness, ClearNausea].map(Ok));
    rows.push(Err("horizon.pvp.header.fast_totem"));
    rows.extend([FastTotem, FastTotemAuto, FastTotemDelay, FastTotemShift].map(Ok));
    rows.push(Err("horizon.pvp.header.hurtcam"));
    rows.extend([HurtCam, HurtCamStrength].map(Ok));
    rows.push(Err("horizon.pvp.header.barrel"));
    rows.extend([Barrel, BarrelBanking, BarrelRighting, BarrelInvert, BarrelSwitch, BarrelSensitivity].map(Ok));
    rows.push(Err("horizon.pvp.header.tiers"));
    rows.extend([Tiers, TiersMode].map(Ok));
    rows.push(Err("horizon.pvp.header.zoom"));
    rows.extend([Zoom, ZoomSmooth, ZoomLevel].map(Ok));
    rows.push(Err("horizon.pvp.header.mouse"));
    rows.extend(
        [MouseRmb, MouseLmbItem, MouseLmbEmpty, MouseWheelOrder, MouseWheelDirection, MouseScrollScaling]
            .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.oteria_pvp"));
    rows.extend(
        [
            HudCooldowns,
            HudCooldownsCorner,
            HudTotems,
            HudTotemsCorner,
            HudPots,
            HudPotsCorner,
            Pops,
            Crystal,
            CrystalKeep,
            Anchor,
            AnchorCharge,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.hitboxes"));
    rows.extend(
        [
            Hitboxes,
            HitboxPlayers,
            HitboxHostile,
            HitboxPassive,
            HitboxItems,
            HitboxProjectiles,
            HitboxCrystals,
            HitboxOthers,
            HitboxEyes,
            HitboxLook,
            HitboxWidth,
            HitboxOwn,
            HitboxTarget,
            HitboxHurt,
            HitboxEyeColour,
            HitboxLookColour,
            HitboxOutline,
            HitboxOutlineSize,
            HitboxFarWidth,
            HitboxFarDistance,
            HitboxFireworks,
            HitboxStuck,
        ]
        .map(Ok),
    );
    rows.push(Err("horizon.pvp.header.secret"));
    rows.push(Ok(Secret));
    rows.push(Err("horizon.pvp.header.voice"));
    rows.extend(
        [Voice, VoiceMenuKey, VoiceActivation, VoiceKey, VoiceVolume, VoiceMic, VoiceThreshold, VoiceNoise, VoiceSpatial]
            .map(Ok),
    );
    rows
}
impl Opt {
    /// The switch this option belongs to.
    pub fn parent(self) -> Option<Opt> {
        use Opt::*;
        match self {
            Crosshair | Outline | Hit => None,
            OutlineColour | OutlineOpacity | OutlineThickness | OutlineFill => Some(Outline),
            HitColour | HitOpacity => Some(Hit),
            HudArmour | HudEffects | HudHeld | HudPing | HudCoords | HudServer | HudCombo
            | HudFps | HudCps | HudKeys | HudClock | HudReach | HudSpeed | HudBackground
            | HudTextColour | HudShadow | HudEditorKey | NumericEnchants => None,
            HudOpacity => Some(HudBackground),
            HudFpsCorner => Some(HudFps),
            HudCpsCorner => Some(HudCps),
            HudKeysCorner => Some(HudKeys),
            HudClockCorner => Some(HudClock),
            HudReachCorner => Some(HudReach),
            HudSpeedCorner => Some(HudSpeed),
            FireLower | ShieldSize | TotemSize | GlintColour | ClearWater | ClearLava => None,
            ViewTime | ViewWeather => None,
            ChatTimestamps | ChatCompact | ChatCopy | ChatSearch => None,
            CombatFeel | CombatIndicator | CombatSounds | CombatRules => None,
            ElytraIndicator | Voice | Saturation => None,
            ElytraColour => Some(ElytraIndicator),
            SaturationHeld | SaturationExhaustion | SaturationTooltip => Some(Saturation),
            MapShape | MapBiome | MapRotate | MapZoom => None,
            Sky => None,
            SkyPack => Some(Sky),
            Zoom | Tiers | Blur | Clock | SkyWeather | Barrel | Music => None,
            MusicLyrics | MusicCard => Some(Music),
            MusicDistance => Some(MusicLyrics),
            Viewmodel | Scoreboard | ClearView => None,
            ViewMainX | ViewMainY | ViewMainZ | ViewOffX | ViewOffY | ViewOffZ => Some(Viewmodel),
            ScoreboardSize | ScoreboardBackground | ScoreboardRounded | ScoreboardNumbers => {
                Some(Scoreboard)
            }
            ScoreboardOpacity => Some(ScoreboardBackground),
            ClearPumpkin | ClearDarkness | ClearBlindness | ClearNausea => Some(ClearView),
            FastTotem | HurtCam => None,
            FastTotemAuto | FastTotemShift => Some(FastTotem),
            FastTotemDelay => Some(FastTotemAuto),
            HurtCamStrength => Some(HurtCam),
            BarrelBanking | BarrelRighting | BarrelInvert | BarrelSwitch | BarrelSensitivity => Some(Barrel),
            BlurStrength => Some(Blur),
            Env => None,
            EnvGround | EnvParticles | EnvHat | EnvSky | EnvFog => Some(Env),
            EnvGroundColour | EnvGroundRadius => Some(EnvGround),
            EnvParticlesColour | EnvParticlesAmount | EnvParticlesShape => Some(EnvParticles),
            EnvParticlesSize => Some(EnvParticlesShape),
            EnvHatColour => Some(EnvHat),
            EnvSkyColour => Some(EnvSky),
            EnvFogColour | EnvFogDensity => Some(EnvFog),
            ClockTime => Some(Clock),
            SkyWeatherPick => Some(SkyWeather),
            TiersMode => Some(Tiers),
            ZoomSmooth | ZoomLevel => Some(Zoom),
            MouseRmb | MouseLmbItem | MouseLmbEmpty | MouseWheelOrder | MouseWheelDirection
            | MouseScrollScaling => None,
            VoiceActivation | VoiceKey | VoiceVolume | VoiceMic | VoiceThreshold | VoiceMenuKey
            | VoiceNoise | VoiceSpatial => Some(Voice),
            HudPingCorner => Some(HudPing),
            HudCoordsCorner => Some(HudCoords),
            HudServerCorner => Some(HudServer),
            HudComboCorner => Some(HudCombo),
            HudArmourCorner | HudArmourPercent => Some(HudArmour),
            HudEffectsCorner => Some(HudEffects),
            HudHeldCorner => Some(HudHeld),
            HudCooldowns | HudTotems | HudPots | Pops | Crystal | Anchor | Hitboxes | Secret => None,
            HudCooldownsCorner => Some(HudCooldowns),
            HudTotemsCorner => Some(HudTotems),
            HudPotsCorner => Some(HudPots),
            CrystalKeep => Some(Crystal),
            AnchorCharge => Some(Anchor),
            HitboxPlayers | HitboxHostile | HitboxPassive | HitboxItems | HitboxProjectiles
            | HitboxCrystals | HitboxOthers | HitboxEyes | HitboxLook | HitboxWidth | HitboxOwn
            | HitboxTarget | HitboxHurt | HitboxEyeColour | HitboxLookColour | HitboxOutline
            | HitboxOutlineSize | HitboxFarWidth | HitboxFarDistance | HitboxFireworks => {
                Some(Hitboxes)
            }
            // It works without the hitboxes, as its own mod.
            HitboxStuck => None,
            _ => Some(Crosshair),
        }
    }
    pub fn switch(self, p: &Pvp) -> Option<bool> {
        let c = &p.crosshair;
        Some(match self {
            Opt::Crosshair => c.custom,
            Opt::CrosshairOutline => c.outline,
            Opt::CrosshairDot => c.dot,
            Opt::Outline => p.outline.custom,
            Opt::Hit => p.hit.custom,
            Opt::HudArmour => p.hud.armour,
            Opt::HudArmourPercent => p.hud.armour_percent,
            Opt::HudEffects => p.hud.effects,
            Opt::HudHeld => p.hud.held,
            Opt::HudBackground => p.hud.background,
            Opt::HudShadow => p.hud.shadow,
            Opt::HudPing => p.hud.ping,
            Opt::HudCoords => p.hud.coords,
            Opt::HudServer => p.hud.server,
            Opt::HudCombo => p.hud.combo,
            Opt::HudFps => p.hud.fps,
            Opt::HudCps => p.hud.cps,
            Opt::HudKeys => p.hud.keys,
            Opt::HudClock => p.hud.clock,
            Opt::HudReach => p.hud.reach,
            Opt::HudSpeed => p.hud.speed,
            Opt::NumericEnchants => p.tweaks.numeric_enchants,
            Opt::ClearWater => p.tweaks.clear_water,
            Opt::ClearLava => p.tweaks.clear_lava,
            Opt::ChatTimestamps => p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact,
            Opt::ChatCopy => p.chat.copy,
            Opt::ChatSearch => p.chat.search,
            Opt::CombatIndicator => p.combat.indicator,
            Opt::CombatSounds => p.combat.sounds,
            Opt::CombatRules => p.combat.rules,
            Opt::ElytraIndicator => p.tweaks.elytra_indicator,
            Opt::Saturation => p.saturation.enabled,
            Opt::SaturationHeld => p.saturation.held,
            Opt::SaturationExhaustion => p.saturation.exhaustion,
            Opt::SaturationTooltip => p.saturation.tooltip,
            Opt::MouseRmb => p.mouse.rmb,
            Opt::MapBiome => p.minimap.biome,
            Opt::MapRotate => p.minimap.rotate,
            Opt::MouseLmbItem => p.mouse.lmb_item,
            Opt::MouseLmbEmpty => p.mouse.lmb_empty,
            Opt::Voice => p.voice.enabled,
            Opt::Sky => p.sky.enabled,
            Opt::Zoom => p.zoom.enabled,
            Opt::ZoomSmooth => p.zoom.smooth,
            Opt::Tiers => p.tiers.enabled,
            Opt::Blur => p.blur.enabled,
            Opt::Env => p.environment.enabled,
            Opt::EnvGround => p.environment.ground,
            Opt::EnvParticles => p.environment.particles,
            Opt::EnvHat => p.environment.hat,
            Opt::EnvSky => p.environment.sky,
            Opt::EnvFog => p.environment.fog,
            Opt::Barrel => p.barrel.enabled,
            Opt::Music => p.music.enabled,
            Opt::MusicLyrics => p.music.lyrics,
            Opt::MusicCard => p.music.card,
            Opt::BarrelBanking => p.barrel.banking,
            Opt::BarrelRighting => p.barrel.righting,
            Opt::BarrelInvert => p.barrel.invert_pitch,
            Opt::BarrelSwitch => p.barrel.switch_roll_yaw,
            Opt::Clock => p.clock.enabled,
            Opt::SkyWeather => p.sky_weather.enabled,
            Opt::VoiceNoise => p.voice.noise,
            Opt::VoiceSpatial => p.voice.spatial,
            Opt::HudCooldowns => p.hud.cooldowns,
            Opt::HudTotems => p.hud.totems,
            Opt::HudPots => p.hud.pots,
            Opt::Pops => p.pops,
            Opt::Crystal => p.crystal.enabled,
            Opt::CrystalKeep => p.crystal.keep_render,
            Opt::Anchor => p.anchor.enabled,
            Opt::AnchorCharge => p.anchor.charge,
            Opt::Hitboxes => p.hitboxes.enabled,
            Opt::HitboxEyes => p.hitboxes.eyes,
            Opt::HitboxLook => p.hitboxes.look,
            Opt::HitboxOwn => p.hitboxes.own,
            Opt::HitboxFireworks => p.hitboxes.fireworks,
            Opt::HitboxStuck => p.hitboxes.hide_stuck,
            Opt::CrosshairIndicator => c.indicator,
            Opt::Viewmodel => p.viewmodel.enabled,
            Opt::Scoreboard => p.scoreboard.enabled,
            Opt::ScoreboardRounded => p.scoreboard.rounded,
            Opt::ScoreboardNumbers => p.scoreboard.numbers,
            Opt::ClearView => p.clear_view.enabled,
            Opt::ClearPumpkin => p.clear_view.pumpkin,
            Opt::ClearDarkness => p.clear_view.darkness,
            Opt::ClearBlindness => p.clear_view.blindness,
            Opt::ClearNausea => p.clear_view.nausea,
            Opt::FastTotem => p.fast_totem.enabled,
            Opt::FastTotemAuto => p.fast_totem.auto,
            Opt::FastTotemShift => p.fast_totem.shift_click,
            Opt::HurtCam => p.hurtcam.enabled,
            _ => return None,
        })
    }
    /// The value its button shows, for the options that aren't switches.
    pub fn value(self, p: &Pvp) -> Option<String> {
        let c = &p.crosshair;
        Some(match self {
            Opt::CrosshairShape => c.shape.label(),
            Opt::CrosshairSize => c.size.to_string(),
            Opt::CrosshairGap => c.gap.to_string(),
            Opt::CrosshairThickness => c.thickness.to_string(),
            Opt::CrosshairColour => c.colour.label(),
            Opt::CrosshairHover => match c.hover {
                Some(colour) => colour.label(),
                None => crate::lang::get_or("Off", "Off"),
            },
            Opt::OutlineColour => p.outline.colour.label(),
            Opt::OutlineOpacity => format!("{}%", p.outline.opacity),
            Opt::OutlineThickness => format!("{}x", p.outline.thickness),
            Opt::OutlineFill => match p.outline.fill {
                Some(colour) => colour.label(),
                None => crate::lang::get_or("Off", "Off"),
            },
            Opt::HitColour => p.hit.colour.label(),
            Opt::HitOpacity => format!("{}%", p.hit.opacity),
            Opt::HudArmourCorner => corner_label(p.hud.armour_corner),
            Opt::HudEffectsCorner => corner_label(p.hud.effects_corner),
            Opt::HudHeldCorner => corner_label(p.hud.held_corner),
            Opt::HudPingCorner => corner_label(p.hud.ping_corner),
            Opt::HudCoordsCorner => corner_label(p.hud.coords_corner),
            Opt::HudServerCorner => corner_label(p.hud.server_corner),
            Opt::HudComboCorner => corner_label(p.hud.combo_corner),
            Opt::HudFpsCorner => corner_label(p.hud.fps_corner),
            Opt::HudCpsCorner => corner_label(p.hud.cps_corner),
            Opt::HudKeysCorner => corner_label(p.hud.keys_corner),
            Opt::HudClockCorner => corner_label(p.hud.clock_corner),
            Opt::HudReachCorner => corner_label(p.hud.reach_corner),
            Opt::HudSpeedCorner => corner_label(p.hud.speed_corner),
            Opt::HudOpacity => format!("{}%", p.hud.opacity),
            Opt::HudTextColour => p.hud.text.label(),
            Opt::HudEditorKey => crate::keybinds::display_name(&p.editor_key).plain(),
            Opt::FireLower => match p.tweaks.fire_lower {
                0 => crate::lang::get_or("Off", "Off"),
                v => format!("{v}%"),
            },
            Opt::ShieldSize => format!("{}%", p.tweaks.shield_size),
            Opt::TotemSize => format!("{}%", p.tweaks.totem_size),
            Opt::ElytraColour => match p.tweaks.elytra_colour {
                Some(colour) => colour.label(),
                None => crate::lang::get_or("Off", "Off"),
            },
            Opt::GlintColour => match p.tweaks.glint {
                Some(colour) => colour.label(),
                None => crate::lang::get_or("Off", "Off"),
            },
            Opt::CombatFeel => match p.combat.legacy {
                true => crate::lang::get_or("horizon.pvp.combat_feel.legacy", "Legacy (1.8.9)"),
                false => crate::lang::get_or("horizon.pvp.combat_feel.modern", "Modern"),
            },
            Opt::ViewTime => p.tweaks.time.map_or_else(world_label, ViewTime::label),
            Opt::ViewWeather => p
                .tweaks
                .weather
                .map_or_else(world_label, ViewWeather::label),
            Opt::VoiceActivation => match p.voice.activation {
                true => crate::lang::get_or("horizon.pvp.voice.voice", "Voice"),
                false => crate::lang::get_or("horizon.pvp.voice.ptt", "Push to talk"),
            },
            Opt::VoiceKey => crate::keybinds::display_name(&p.voice.key).plain(),
            Opt::VoiceMenuKey => crate::keybinds::display_name(&p.voice.menu_key).plain(),
            Opt::SkyPack => {
                let packs = SKY_PACKS.lock().map(|l| l.clone()).unwrap_or_default();
                match packs.iter().find(|(id, _)| *id == p.sky.pack).or(packs.first()) {
                    Some((_, title)) => title.clone(),
                    None => crate::lang::get_or("horizon.pvp.sky.none", "None found"),
                }
            }
            Opt::MapShape => match p.minimap.round {
                true => crate::lang::get_or("horizon.pvp.map.round", "Round"),
                false => crate::lang::get_or("horizon.pvp.map.square", "Square"),
            },
            Opt::ZoomLevel => format!("x{}", p.zoom.level),
            Opt::BlurStrength => format!("{}%", p.blur.strength),
            Opt::EnvGroundColour => p.environment.ground_colour.label(),
            Opt::EnvParticlesColour => p.environment.particles_colour.label(),
            Opt::EnvHatColour => p.environment.hat_colour.label(),
            Opt::EnvSkyColour => p.environment.sky_colour.label(),
            Opt::EnvFogColour => p.environment.fog_colour.label(),
            Opt::EnvGroundRadius => p.environment.ground_radius.to_string(),
            Opt::EnvParticlesAmount => format!("{}%", p.environment.particles_amount),
            Opt::EnvParticlesShape => p.environment.particles_shape.label(),
            Opt::EnvParticlesSize => format!("{}%", p.environment.particles_size),
            Opt::FastTotemDelay => p.fast_totem.delay.to_string(),
            Opt::HurtCamStrength => p.hurtcam.strength.to_string(),
            Opt::EnvFogDensity => format!("{}%", p.environment.fog_density),
            Opt::BarrelSensitivity => format!("{}%", p.barrel.sensitivity),
            Opt::ClockTime => clock_label(u32::from(p.clock.time)),
            Opt::SkyWeatherPick => p.sky_weather.weather.label(),
            Opt::TiersMode => {
                let (slug, english) =
                    TIER_MODES.iter().find(|(m, _)| *m == p.tiers.mode).copied().unwrap_or(TIER_MODES[0]);
                let key = if slug.is_empty() { "horizon.pvp.tiers.best".to_owned() } else { format!("horizon.pvp.tiers.{slug}") };
                crate::lang::get_or(&key, english)
            }
            Opt::MapZoom => {
                let z = MINIMAP_ZOOMS[usize::from(p.minimap.zoom.min(3))];
                format!("{z} {}", crate::lang::get_or("horizon.pvp.map.blocks", "blocks/px"))
            }
            Opt::MouseWheelOrder => match p.mouse.first_to_last {
                true => crate::lang::get_or("horizon.pvp.mouse.first", "First to last"),
                false => crate::lang::get_or("horizon.pvp.mouse.last", "Last to first"),
            },
            Opt::MouseWheelDirection => {
                let (key, english) = WHEEL_DIRECTIONS[usize::from(p.mouse.direction.min(3))];
                crate::lang::get_or(key, english)
            }
            Opt::MouseScrollScaling => match p.mouse.always_one {
                true => crate::lang::get_or("horizon.pvp.mouse.one", "Always exactly one item"),
                false => crate::lang::get_or("horizon.pvp.mouse.proportional", "Proportional"),
            },
            Opt::VoiceVolume => format!("{}%", p.voice.volume),
            Opt::VoiceMic => format!("{}%", p.voice.mic),
            Opt::VoiceThreshold => format!("-{} dB", p.voice.threshold),
            Opt::HudCooldownsCorner => corner_label(p.hud.cooldowns_corner),
            Opt::HudTotemsCorner => corner_label(p.hud.totems_corner),
            Opt::HudPotsCorner => corner_label(p.hud.pots_corner),
            Opt::HitboxWidth => format!("{:.1}", f32::from(p.hitboxes.width) / 10.0),
            Opt::HitboxFarWidth => format!("{:.1}", f32::from(p.hitboxes.far_width) / 10.0),
            Opt::HitboxOutlineSize => format!("{:.1}x", f32::from(p.hitboxes.outline_size) / 10.0),
            Opt::HitboxFarDistance => format!("{}", p.hitboxes.far_distance),
            Opt::Secret => secret_label(p.secret.effect),
            Opt::MusicDistance => format!("{}", p.music.distance),
            Opt::ScoreboardOpacity => format!("{}%", p.scoreboard.opacity),
            Opt::ScoreboardSize => format!(
                "{}%",
                p.layout.get(crate::pvp_hud::WidgetId::Scoreboard).scale
            ),
            Opt::ScoreboardBackground => match p.scoreboard.background {
                Some(colour) => colour.label(),
                None => crate::lang::get_or("Off", "Off"),
            },
            o if o.viewmodel_axis().is_some() => {
                let v = o.slider_value(p).unwrap_or(100) as i32 - 100;
                format!("{:+.2}", v as f32 / 100.0)
            }
            o @ (Opt::HitboxPlayers
            | Opt::HitboxHostile
            | Opt::HitboxPassive
            | Opt::HitboxItems
            | Opt::HitboxProjectiles
            | Opt::HitboxCrystals
            | Opt::HitboxOthers
            | Opt::HitboxTarget
            | Opt::HitboxHurt
            | Opt::HitboxOutline
            | Opt::HitboxEyeColour
            | Opt::HitboxLookColour) => match o.colour(p).flatten() {
                Some(colour) => colour.label(),
                None => crate::lang::get_or("Off", "Off"),
            },
            _ => return None,
        })
    }
    /// A click; `back` (right button, wheel up) goes the other way.
    pub fn cycle(self, p: &mut Pvp, back: bool) {
        let c = &mut p.crosshair;
        let (o, h, u) = (&mut p.outline, &mut p.hit, &mut p.hud);
        let t = &mut p.tweaks;
        let corner = |c: Corner| step(&Corner::ALL, c, back);
        // Off, then every solid colour.
        let solid_or_off = |v: Option<Colour>| {
            let all = [None]
                .into_iter()
                .chain(Colour::ALL[1..].iter().copied().map(Some));
            step(&all.collect::<Vec<_>>(), v, back)
        };
        match self {
            Opt::Crosshair => c.custom = !c.custom,
            Opt::CrosshairShape => c.shape = step(&Shape::ALL, c.shape, back),
            Opt::CrosshairSize => c.size = step_num(c.size, SIZES.0, SIZES.1, back),
            Opt::CrosshairGap => c.gap = step_num(c.gap, GAPS.0, GAPS.1, back),
            Opt::CrosshairThickness => {
                c.thickness = step_num(c.thickness, THICKNESSES.0, THICKNESSES.1, back)
            }
            Opt::CrosshairColour => c.colour = c.colour.step(back),
            Opt::CrosshairOutline => c.outline = !c.outline,
            Opt::CrosshairDot => c.dot = !c.dot,
            Opt::CrosshairIndicator => c.indicator = !c.indicator,
            Opt::CrosshairHover => {
                // Off, then every colour.
                let all: Vec<Option<Colour>> = std::iter::once(None)
                    .chain(Colour::ALL.into_iter().map(Some))
                    .collect();
                c.hover = step(&all, c.hover, back);
            }
            Opt::Outline => o.custom = !o.custom,
            Opt::OutlineColour => o.colour = o.colour.step_solid(back),
            Opt::OutlineOpacity => o.opacity = step_by(o.opacity, OPACITIES, 10, back),
            Opt::OutlineThickness => {
                o.thickness = step_by(o.thickness, OUTLINE_THICKNESSES, 1, back)
            }
            Opt::OutlineFill => o.fill = solid_or_off(o.fill),
            Opt::Hit => h.custom = !h.custom,
            Opt::HitColour => h.colour = h.colour.step_solid(back),
            Opt::HitOpacity => h.opacity = step_by(h.opacity, HIT_OPACITIES, 10, back),
            Opt::HudArmour => u.armour = !u.armour,
            Opt::HudArmourCorner => u.armour_corner = corner(u.armour_corner),
            Opt::HudArmourPercent => u.armour_percent = !u.armour_percent,
            Opt::HudEffects => u.effects = !u.effects,
            Opt::HudEffectsCorner => u.effects_corner = corner(u.effects_corner),
            Opt::HudHeld => u.held = !u.held,
            Opt::HudHeldCorner => u.held_corner = corner(u.held_corner),
            Opt::HudBackground => u.background = !u.background,
            Opt::HudOpacity => u.opacity = step_by(u.opacity, HUD_OPACITIES, 10, back),
            Opt::HudTextColour => u.text = u.text.step_solid(back),
            Opt::HudShadow => u.shadow = !u.shadow,
            // Bound through the controls' key capture (`Item::key_name`).
            Opt::HudEditorKey => {}
            Opt::HudPing => u.ping = !u.ping,
            Opt::HudPingCorner => u.ping_corner = corner(u.ping_corner),
            Opt::HudCoords => u.coords = !u.coords,
            Opt::HudCoordsCorner => u.coords_corner = corner(u.coords_corner),
            Opt::HudServer => u.server = !u.server,
            Opt::HudServerCorner => u.server_corner = corner(u.server_corner),
            Opt::HudCombo => u.combo = !u.combo,
            Opt::HudComboCorner => u.combo_corner = corner(u.combo_corner),
            Opt::HudFps => u.fps = !u.fps,
            Opt::HudFpsCorner => u.fps_corner = corner(u.fps_corner),
            Opt::HudCps => u.cps = !u.cps,
            Opt::HudCpsCorner => u.cps_corner = corner(u.cps_corner),
            Opt::HudKeys => u.keys = !u.keys,
            Opt::HudKeysCorner => u.keys_corner = corner(u.keys_corner),
            Opt::HudClock => u.clock = !u.clock,
            Opt::HudClockCorner => u.clock_corner = corner(u.clock_corner),
            Opt::HudReach => u.reach = !u.reach,
            Opt::HudReachCorner => u.reach_corner = corner(u.reach_corner),
            Opt::HudSpeed => u.speed = !u.speed,
            Opt::HudSpeedCorner => u.speed_corner = corner(u.speed_corner),
            Opt::NumericEnchants => p.tweaks.numeric_enchants = !p.tweaks.numeric_enchants,
            Opt::FireLower => t.fire_lower = step_by(t.fire_lower, FIRE_LOWER, 10, back),
            Opt::ShieldSize => t.shield_size = step_by(t.shield_size, SHIELD_SIZES, 10, back),
            Opt::TotemSize => t.totem_size = step_by(t.totem_size, TOTEM_SIZES, 25, back),
            Opt::GlintColour => t.glint = solid_or_off(t.glint),
            Opt::ClearWater => t.clear_water = !t.clear_water,
            Opt::ClearLava => t.clear_lava = !t.clear_lava,
            Opt::ElytraIndicator => t.elytra_indicator = !t.elytra_indicator,
            Opt::ElytraColour => t.elytra_colour = solid_or_off(t.elytra_colour),
            Opt::Saturation => p.saturation.enabled = !p.saturation.enabled,
            Opt::SaturationHeld => p.saturation.held = !p.saturation.held,
            Opt::SaturationExhaustion => p.saturation.exhaustion = !p.saturation.exhaustion,
            Opt::SaturationTooltip => p.saturation.tooltip = !p.saturation.tooltip,
            Opt::MouseRmb => p.mouse.rmb = !p.mouse.rmb,
            Opt::Sky => p.sky.enabled = !p.sky.enabled,
            Opt::Zoom => p.zoom.enabled = !p.zoom.enabled,
            Opt::Tiers => p.tiers.enabled = !p.tiers.enabled,
            Opt::Blur => p.blur.enabled = !p.blur.enabled,
            Opt::Barrel => p.barrel.enabled = !p.barrel.enabled,
            Opt::Music => p.music.enabled = !p.music.enabled,
            Opt::MusicLyrics => p.music.lyrics = !p.music.lyrics,
            Opt::MusicCard => p.music.card = !p.music.card,
            Opt::MusicDistance => p.music.distance = step_by(p.music.distance, LYRICS_DISTANCES, 1, back),
            Opt::Viewmodel => p.viewmodel.enabled = !p.viewmodel.enabled,
            o @ (Opt::ViewMainX
            | Opt::ViewMainY
            | Opt::ViewMainZ
            | Opt::ViewOffX
            | Opt::ViewOffY
            | Opt::ViewOffZ) => {
                // Five hundredths a click.
                let v = o.slider_value(p).unwrap_or(100) as i32 + if back { -5 } else { 5 };
                o.set_slider(p, v.max(0) as u32);
            }
            Opt::Scoreboard => p.scoreboard.enabled = !p.scoreboard.enabled,
            Opt::ScoreboardSize => {
                let place = p.layout.get_mut(crate::pvp_hud::WidgetId::Scoreboard);
                place.scale = step_by(place.scale, crate::pvp_hud::SCALES, 10, back);
            }
            Opt::ScoreboardBackground => p.scoreboard.background = solid_or_off(p.scoreboard.background),
            Opt::ScoreboardOpacity => {
                p.scoreboard.opacity = step_by(p.scoreboard.opacity, SCOREBOARD_OPACITIES, 10, back)
            }
            Opt::ScoreboardRounded => p.scoreboard.rounded = !p.scoreboard.rounded,
            Opt::ScoreboardNumbers => p.scoreboard.numbers = !p.scoreboard.numbers,
            Opt::ClearView => p.clear_view.enabled = !p.clear_view.enabled,
            Opt::ClearPumpkin => p.clear_view.pumpkin = !p.clear_view.pumpkin,
            Opt::ClearDarkness => p.clear_view.darkness = !p.clear_view.darkness,
            Opt::ClearBlindness => p.clear_view.blindness = !p.clear_view.blindness,
            Opt::ClearNausea => p.clear_view.nausea = !p.clear_view.nausea,
            Opt::FastTotem => p.fast_totem.enabled = !p.fast_totem.enabled,
            Opt::FastTotemAuto => p.fast_totem.auto = !p.fast_totem.auto,
            Opt::FastTotemShift => p.fast_totem.shift_click = !p.fast_totem.shift_click,
            Opt::FastTotemDelay => p.fast_totem.delay = step_by(p.fast_totem.delay, TOTEM_DELAYS, 1, back),
            Opt::HurtCam => p.hurtcam.enabled = !p.hurtcam.enabled,
            Opt::HurtCamStrength => {
                p.hurtcam.strength = step_by(p.hurtcam.strength, HURTCAM_STRENGTHS, 1, back)
            }
            Opt::EnvParticlesShape => {
                p.environment.particles_shape = step(&ParticleShape::ALL, p.environment.particles_shape, back)
            }
            Opt::EnvParticlesSize => {
                p.environment.particles_size = step_by(p.environment.particles_size, ENV_SIZES, 10, back)
            }
            Opt::BarrelBanking => p.barrel.banking = !p.barrel.banking,
            Opt::BarrelRighting => p.barrel.righting = !p.barrel.righting,
            Opt::BarrelInvert => p.barrel.invert_pitch = !p.barrel.invert_pitch,
            Opt::BarrelSwitch => p.barrel.switch_roll_yaw = !p.barrel.switch_roll_yaw,
            Opt::BarrelSensitivity => {
                p.barrel.sensitivity = step_by(p.barrel.sensitivity, BARREL_SENSITIVITIES, 25, back)
            }
            Opt::BlurStrength => p.blur.strength = step_by(p.blur.strength, BLUR_STRENGTHS, 10, back),
            Opt::Env => p.environment.enabled = !p.environment.enabled,
            Opt::EnvGround => p.environment.ground = !p.environment.ground,
            Opt::EnvParticles => p.environment.particles = !p.environment.particles,
            Opt::EnvHat => p.environment.hat = !p.environment.hat,
            Opt::EnvSky => p.environment.sky = !p.environment.sky,
            Opt::EnvFog => p.environment.fog = !p.environment.fog,
            Opt::EnvGroundColour => p.environment.ground_colour = p.environment.ground_colour.step_solid(back),
            Opt::EnvParticlesColour => {
                p.environment.particles_colour = p.environment.particles_colour.step_solid(back)
            }
            Opt::EnvHatColour => p.environment.hat_colour = p.environment.hat_colour.step_solid(back),
            Opt::EnvSkyColour => p.environment.sky_colour = p.environment.sky_colour.step_solid(back),
            Opt::EnvFogColour => p.environment.fog_colour = p.environment.fog_colour.step_solid(back),
            Opt::EnvGroundRadius => {
                p.environment.ground_radius = step_by(p.environment.ground_radius, ENV_RADII, 1, back)
            }
            Opt::EnvParticlesAmount => {
                p.environment.particles_amount = step_by(p.environment.particles_amount, ENV_AMOUNTS, 10, back)
            }
            Opt::EnvFogDensity => {
                p.environment.fog_density = step_by(p.environment.fog_density, ENV_AMOUNTS, 10, back)
            }
            Opt::Clock => p.clock.enabled = !p.clock.enabled,
            // An hour a step.
            Opt::ClockTime => {
                let t = i32::from(p.clock.time) + if back { -1000 } else { 1000 };
                p.clock.time = t.rem_euclid(24_000) as u16;
            }
            Opt::SkyWeather => p.sky_weather.enabled = !p.sky_weather.enabled,
            Opt::SkyWeatherPick => p.sky_weather.weather = step(&ViewWeather::ALL, p.sky_weather.weather, back),
            Opt::TiersMode => {
                let slugs: Vec<&str> = TIER_MODES.iter().map(|(m, _)| *m).collect();
                p.tiers.mode = step(&slugs, p.tiers.mode.as_str(), back).to_owned();
            }
            Opt::ZoomSmooth => p.zoom.smooth = !p.zoom.smooth,
            Opt::ZoomLevel => p.zoom.level = step_num(p.zoom.level, ZOOM_LEVELS.0, ZOOM_LEVELS.1, back),
            Opt::SkyPack => {
                let ids: Vec<String> =
                    SKY_PACKS.lock().map(|l| l.iter().map(|(id, _)| id.clone()).collect()).unwrap_or_default();
                if !ids.is_empty() {
                    let i = ids.iter().position(|id| *id == p.sky.pack).unwrap_or(0);
                    let k = step(&(0..ids.len()).collect::<Vec<_>>(), i, back);
                    p.sky.pack = ids[k].clone();
                }
            }
            Opt::MapShape => p.minimap.round = !p.minimap.round,
            Opt::MapBiome => p.minimap.biome = !p.minimap.biome,
            Opt::MapRotate => p.minimap.rotate = !p.minimap.rotate,
            Opt::MapZoom => {
                p.minimap.zoom = if back { (p.minimap.zoom + 3) % 4 } else { (p.minimap.zoom + 1) % 4 }
            }
            Opt::MouseLmbItem => p.mouse.lmb_item = !p.mouse.lmb_item,
            Opt::MouseLmbEmpty => p.mouse.lmb_empty = !p.mouse.lmb_empty,
            Opt::MouseWheelOrder => p.mouse.first_to_last = !p.mouse.first_to_last,
            Opt::MouseWheelDirection => {
                p.mouse.direction = if back { (p.mouse.direction + 3) % 4 } else { (p.mouse.direction + 1) % 4 }
            }
            Opt::MouseScrollScaling => p.mouse.always_one = !p.mouse.always_one,
            Opt::Voice => p.voice.enabled = !p.voice.enabled,
            Opt::VoiceNoise => p.voice.noise = !p.voice.noise,
            Opt::VoiceSpatial => p.voice.spatial = !p.voice.spatial,
            Opt::VoiceMenuKey => {
                let v = &mut p.voice;
                let i = VOICE_MENU_KEYS.iter().position(|k| *k == v.menu_key).unwrap_or(0);
                let n = VOICE_MENU_KEYS.len();
                v.menu_key = VOICE_MENU_KEYS[if back { (i + n - 1) % n } else { (i + 1) % n }].into();
            }
            Opt::VoiceActivation => p.voice.activation = !p.voice.activation,
            Opt::VoiceKey => {
                let v = &mut p.voice;
                let i = VOICE_KEYS.iter().position(|k| *k == v.key);
                let n = VOICE_KEYS.len();
                let next = match (i, back) {
                    (None, _) => 0,
                    (Some(i), false) => (i + 1) % n,
                    (Some(i), true) => (i + n - 1) % n,
                };
                v.key = VOICE_KEYS[next].into();
            }
            Opt::VoiceVolume => p.voice.volume = step_by(p.voice.volume, VOICE_PERCENT, 10, back),
            Opt::VoiceMic => p.voice.mic = step_by(p.voice.mic, VOICE_PERCENT, 10, back),
            Opt::VoiceThreshold => {
                p.voice.threshold = step_by(p.voice.threshold, VOICE_THRESHOLD, 5, back)
            }
            Opt::ViewTime => {
                let all: Vec<_> = std::iter::once(None)
                    .chain(ViewTime::ALL.map(Some))
                    .collect();
                t.time = step(&all, t.time, back);
            }
            Opt::ChatTimestamps => p.chat.timestamps = !p.chat.timestamps,
            Opt::ChatCompact => p.chat.compact = !p.chat.compact,
            Opt::ChatCopy => p.chat.copy = !p.chat.copy,
            Opt::ChatSearch => p.chat.search = !p.chat.search,
            Opt::CombatFeel => p.combat.legacy = !p.combat.legacy,
            Opt::CombatIndicator => p.combat.indicator = !p.combat.indicator,
            Opt::CombatSounds => p.combat.sounds = !p.combat.sounds,
            Opt::CombatRules => p.combat.rules = !p.combat.rules,
            Opt::ViewWeather => {
                let all: Vec<_> = std::iter::once(None)
                    .chain(ViewWeather::ALL.map(Some))
                    .collect();
                t.weather = step(&all, t.weather, back);
            }
            Opt::HudCooldowns => p.hud.cooldowns = !p.hud.cooldowns,
            Opt::HudCooldownsCorner => p.hud.cooldowns_corner = step(&Corner::ALL, p.hud.cooldowns_corner, back),
            Opt::HudTotems => p.hud.totems = !p.hud.totems,
            Opt::HudTotemsCorner => p.hud.totems_corner = step(&Corner::ALL, p.hud.totems_corner, back),
            Opt::HudPots => p.hud.pots = !p.hud.pots,
            Opt::HudPotsCorner => p.hud.pots_corner = step(&Corner::ALL, p.hud.pots_corner, back),
            Opt::Pops => p.pops = !p.pops,
            Opt::Crystal => p.crystal.enabled = !p.crystal.enabled,
            Opt::CrystalKeep => p.crystal.keep_render = !p.crystal.keep_render,
            Opt::Anchor => p.anchor.enabled = !p.anchor.enabled,
            Opt::AnchorCharge => p.anchor.charge = !p.anchor.charge,
            Opt::Hitboxes => p.hitboxes.enabled = !p.hitboxes.enabled,
            Opt::HitboxEyes => p.hitboxes.eyes = !p.hitboxes.eyes,
            Opt::HitboxLook => p.hitboxes.look = !p.hitboxes.look,
            Opt::HitboxOwn => p.hitboxes.own = !p.hitboxes.own,
            Opt::HitboxWidth => p.hitboxes.width = step_by(p.hitboxes.width, HITBOX_WIDTHS, 2, back),
            Opt::HitboxFarWidth => {
                p.hitboxes.far_width = step_by(p.hitboxes.far_width, HITBOX_WIDTHS, 2, back)
            }
            Opt::HitboxOutlineSize => {
                p.hitboxes.outline_size = step_by(p.hitboxes.outline_size, HITBOX_OUTLINE_SIZES, 2, back)
            }
            Opt::HitboxFarDistance => {
                p.hitboxes.far_distance = step_by(p.hitboxes.far_distance, HITBOX_FAR, 4, back)
            }
            Opt::HitboxFireworks => p.hitboxes.fireworks = !p.hitboxes.fireworks,
            Opt::HitboxStuck => p.hitboxes.hide_stuck = !p.hitboxes.hide_stuck,
            Opt::HitboxEyeColour => p.hitboxes.eye_colour = p.hitboxes.eye_colour.step_solid(back),
            Opt::HitboxLookColour => p.hitboxes.look_colour = p.hitboxes.look_colour.step_solid(back),
            o @ (Opt::HitboxPlayers
            | Opt::HitboxHostile
            | Opt::HitboxPassive
            | Opt::HitboxItems
            | Opt::HitboxProjectiles
            | Opt::HitboxCrystals
            | Opt::HitboxOthers
            | Opt::HitboxTarget
            | Opt::HitboxHurt
            | Opt::HitboxOutline) => {
                let next = solid_or_off(o.colour(p).flatten());
                o.set_colour(p, next);
            }
            Opt::Secret => {
                let n = SECRET_EFFECTS.len() as u8;
                p.secret.effect = if back { (p.secret.effect + n - 1) % n } else { (p.secret.effect + 1) % n };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_colour_is_saved_and_read_back() {
        let mut p = Pvp::default();
        Opt::OutlineColour.set_colour(&mut p, Colour::parse("#12aBef"));
        Opt::GlintColour.set_colour(&mut p, Some(Colour::Custom(0x00FF80)));
        // Inverted isn't the outline's to take, nor off.
        Opt::OutlineColour.set_colour(&mut p, Some(Colour::Inverted));
        Opt::OutlineColour.set_colour(&mut p, None);
        assert_eq!(p.outline.colour, Colour::Custom(0x12ABEF));
        assert_eq!(p.outline.colour.label(), "#12ABEF");
        let mut text = String::new();
        p.serialize(&mut text);
        let mut back = Pvp::default();
        for line in text.lines() {
            let (k, v) = line.split_once(':').unwrap();
            back.parse_line(k, v);
        }
        assert_eq!(back.outline.colour, Colour::Custom(0x12ABEF));
        assert_eq!(back.tweaks.glint, Some(Colour::Custom(0x00FF80)));
    }

    #[test]
    fn the_clock_reads_ticks_as_a_day_does() {
        assert_eq!(clock_label(0), "06:00");
        assert_eq!(clock_label(6000), "12:00");
        assert_eq!(clock_label(18_000), "00:00");
        assert_eq!(clock_label(23_999), "05:59");
        let mut p = Pvp::default();
        Opt::ClockTime.set_slider(&mut p, 99_999);
        assert_eq!(p.clock.time, 23_999);
    }

    #[test]
    fn a_typed_number_stays_in_its_range() {
        let mut p = Pvp::default();
        Opt::CrosshairSize.set_number(&mut p, 999);
        assert_eq!(p.crosshair.size, SIZES.1);
        Opt::ShieldSize.set_number(&mut p, 3);
        assert_eq!(p.tweaks.shield_size, SHIELD_SIZES.0);
        for &o in Opt::ALL {
            assert_eq!(o.number().is_some(), o.number_value(&p).is_some(), "{o:?}");
            assert_eq!(o.colour_choice().is_some(), o.colour(&p).is_some(), "{o:?}");
        }
    }

    /// The mask drawn as rows of `#` and `.` over `-r..=r`.
    fn picture(m: &Mask, r: i32) -> Vec<String> {
        (-r..=r)
            .map(|y| {
                (-r..=r)
                    .map(|x| if m.get(x, y) { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_default_crosshair_covers_vanillas_sprite() {
        // hud/crosshair.png: a 15x15 plus sign, one pixel wide, through the middle.
        let m = Crosshair::default().mask();
        for y in -7..=7 {
            for x in -7..=7 {
                assert_eq!(m.get(x, y), x == 0 || y == 0, "({x}, {y})");
            }
        }
        assert!(!m.get(8, 0) && !m.get(0, -8));
        assert_eq!(
            m.runs().count(),
            15,
            "one run per row: 14 single pixels and the bar"
        );
    }

    #[test]
    fn gap_thickness_shape_and_dot_change_the_pixels() {
        let c = Crosshair {
            size: 2,
            gap: 1,
            ..Default::default()
        };
        assert_eq!(
            picture(&c.mask(), 3),
            [
                "...#...", "...#...", ".......", "##...##", ".......", "...#...", "...#...",
            ]
        );
        let dotted = Crosshair { dot: true, ..c };
        assert!(dotted.mask().get(0, 0));
        let t = Crosshair {
            shape: Shape::T,
            thickness: 3,
            size: 1,
            ..Default::default()
        };
        assert_eq!(
            picture(&t.mask(), 2),
            [".....", "#####", "#####", "#####", ".###."]
        );
        let x = Crosshair {
            shape: Shape::X,
            size: 2,
            ..Default::default()
        };
        assert_eq!(
            picture(&x.mask(), 2),
            ["#...#", ".#.#.", "..#..", ".#.#.", "#...#"]
        );
        let square = Crosshair {
            shape: Shape::Square,
            size: 2,
            ..Default::default()
        };
        assert_eq!(
            picture(&square.mask(), 2),
            ["#####", "#...#", "#...#", "#...#", "#####"]
        );
        let ring = Crosshair {
            shape: Shape::Circle,
            size: 3,
            ..Default::default()
        };
        let p = picture(&ring.mask(), 3);
        assert_eq!(p[3], "#.....#", "{p:?}");
        assert_eq!(p[0], "..###..", "{p:?}");
        assert!(!ring.mask().get(0, 0));
        let dot = Crosshair {
            shape: Shape::Dot,
            thickness: 2,
            ..Default::default()
        };
        assert_eq!(picture(&dot.mask(), 1), ["...", ".##", ".##"]);
    }

    #[test]
    fn the_outline_rings_the_shape_without_covering_it() {
        let dot = Crosshair {
            shape: Shape::Dot,
            ..Default::default()
        };
        assert_eq!(
            picture(&dot.mask().rim(), 2),
            [".....", ".###.", ".#.#.", ".###.", "....."]
        );
        let mut cache = CrosshairCache::default();
        let (shape, rim) = cache.runs(&Crosshair {
            outline: true,
            ..Crosshair::default()
        });
        assert_eq!(shape.len(), 15);
        // Rows -8 and 8 (one run each), and the rows beside the bar: three runs each
        // (left of it, between, right of it) except where the arms meet it.
        assert!(rim.iter().all(|&(x0, _, x1)| x1 > x0));
        assert!(rim.iter().any(|&(_, y, _)| y == -8) && rim.iter().any(|&(_, y, _)| y == 8));
    }

    #[test]
    fn options_round_trip_and_cycle_both_ways() {
        let d = Pvp::default();
        let mut text = String::new();
        d.serialize(&mut text);
        let mut back = Pvp::default();
        for line in text.lines() {
            let (k, v) = line.split_once(':').unwrap();
            assert!(back.parse_line(k, v), "{k}");
        }
        assert_eq!(back, d);
        let mut p = Pvp::default();
        for opt in rows().into_iter().flatten() {
            opt.cycle(&mut p, false);
        }
        assert!(p.crosshair.custom && p.crosshair.outline && p.crosshair.dot);
        assert_eq!(p.crosshair.shape, Shape::T);
        assert_eq!(p.crosshair.size, 8);
        assert_eq!(p.crosshair.hover, Some(Colour::Inverted));
        // From vanilla's values, one click: the overlays' sizes wrap to their smallest.
        let t = p.tweaks;
        assert_eq!((t.fire_lower, t.shield_size, t.totem_size), (10, 50, 25));
        assert_eq!(t.glint, Some(Colour::White));
        assert!(t.clear_water && t.clear_lava);
        assert_eq!(
            (t.time, t.weather),
            (Some(ViewTime::Morning), Some(ViewWeather::Clear))
        );
        let c = p.chat;
        assert!(c.timestamps && c.compact && c.copy && c.search);
        let mut text = String::new();
        p.serialize(&mut text);
        let mut read = Pvp::default();
        text.lines().for_each(|l| {
            let (k, v) = l.split_once(':').unwrap();
            read.parse_line(k, v);
        });
        assert_eq!(read, p);
        // Down from the lowest wraps to the highest; bad values keep what was there.
        Opt::CrosshairThickness.cycle(&mut p, true);
        Opt::CrosshairThickness.cycle(&mut p, true);
        assert_eq!(p.crosshair.thickness, THICKNESSES.1);
        Opt::CrosshairHover.cycle(&mut p, true);
        assert_eq!(p.crosshair.hover, None);
        assert!(p.parse_line("crosshairSize", "99") && p.crosshair.size == SIZES.1);
        assert!(p.parse_line("crosshairColour", "plaid") && p.crosshair.colour == Colour::White);
        assert!(!p.parse_line("fullbright", "true"));
        assert!(p.parse_line("totemSize", "5") && p.tweaks.totem_size == TOTEM_SIZES.0);
    }

    #[test]
    fn the_pvp_preset_turns_its_options_on_and_keeps_their_settings() {
        use crate::mod_list::{Installed, NativePage};
        let mut s = crate::settings::Settings::default();
        s.extras.pvp.crosshair.colour = Colour::Green;
        s.extras.pvp.tweaks.fire_lower = 60;
        let installed = |id: &str| Installed {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            problem: None,
            dev: false,
            native: Some(NativePage::default()),
        };
        s.mods.installed = vec![installed("fps_overlay"), installed("zoom")];
        s.mods.disabled.insert("fps_overlay".into());
        apply_preset(&mut s);
        let p = &s.extras.pvp;
        assert!(p.crosshair.custom && p.hud.armour && p.hud.effects);
        // Sprint and sneak stay held, as vanilla's defaults: toggled, they caught players out.
        assert!(!s.toggle_sprint && !s.toggle_crouch);
        // What the player had chosen stays: the crosshair's colour, a lower fire already set.
        assert_eq!(p.crosshair.colour, Colour::Green);
        assert_eq!(p.tweaks.fire_lower, 60);
        // The FPS counter the player had turned off is on; keystrokes isn't installed: nothing.
        assert!(!s.mods.disabled.contains("fps_overlay"));
        assert!(s.mods.enabled.contains("fps_overlay") && !s.mods.enabled.contains("keystrokes"));
        let mut fresh = crate::settings::Settings::default();
        apply_preset(&mut fresh);
        assert_eq!(fresh.extras.pvp.tweaks.fire_lower, PRESET_FIRE_LOWER);
    }

    #[test]
    fn a_fixed_time_keeps_the_worlds_day() {
        // Day 9 at 5000: the moon's phase (the day number) stays, the time of day is the chosen.
        let world = 9 * 24000 + 5000;
        assert_eq!(view_day_time(world, None), world);
        assert_eq!(
            view_day_time(world, Some(ViewTime::Midnight)),
            9 * 24000 + 18000
        );
        assert_eq!(view_day_time(-100, Some(ViewTime::Noon)), -24000 + 6000);
    }

    #[test]
    fn the_glint_colour_reaches_the_shaders_as_rgb_bytes() {
        let mut t = Tweaks::default();
        assert_eq!(t.glint_texel(), 0, "vanilla's glint");
        t.glint = Some(Colour::Red);
        // unpack4x8unorm reads the low byte first: r = 1, g = 0, b = 0, a = 1.
        assert_eq!(t.glint_texel().to_le_bytes(), [255, 0, 0, 255]);
    }

    #[test]
    fn outline_and_hit_colours_default_to_vanillas() {
        // Turned on with their first settings, they give vanilla's colours: the outline's
        // 0x66000000 (LevelRenderer) and OverlayTexture's hurt texel 0xB2FF0000.
        let o = Outline {
            custom: true,
            ..Default::default()
        };
        assert_eq!(o.stroke(), Some((0x6600_0000, 1.0)));
        assert_eq!(o.fill(), None);
        let [r, g, b, a] = crate::overlay::HURT_TEXEL;
        let hit = Hit {
            custom: true,
            ..Default::default()
        };
        assert_eq!(hit.texel(), u32::from_le_bytes([r, g, b, a]));
        assert_eq!(
            (Outline::default().stroke(), Hit::default().texel()),
            (None, 0)
        );
        // Fully opaque cyan covers the entity entirely: none of its own colour stays.
        let cyan = Hit {
            custom: true,
            colour: Colour::Cyan,
            opacity: 100,
        };
        assert_eq!(cyan.texel().to_le_bytes(), [0x40, 0xFF, 0xFF, 0]);
        let mut p = Pvp::default();
        Opt::Outline.cycle(&mut p, false);
        Opt::OutlineFill.cycle(&mut p, false);
        assert_eq!(p.outline.fill(), Some(0x40FF_FFFF), "white at 25%");
        Opt::OutlineOpacity.cycle(&mut p, true);
        Opt::OutlineOpacity.cycle(&mut p, true);
        Opt::OutlineOpacity.cycle(&mut p, true);
        Opt::OutlineOpacity.cycle(&mut p, true);
        assert_eq!(p.outline.opacity, 100, "down from 10% wraps to 100%");
        Opt::OutlineColour.cycle(&mut p, true);
        assert_eq!(p.outline.colour, Colour::Magenta, "never Inverted");
        let mut text = String::new();
        p.serialize(&mut text);
        let mut read = Pvp::default();
        for line in text.lines() {
            let (k, v) = line.split_once(':').unwrap();
            assert!(read.parse_line(k, v), "{k}");
        }
        assert_eq!(read, p);
    }

    #[test]
    fn every_option_and_value_has_its_english_text() {
        let en = crate::lang::Language::load(&[], "en_us");
        for opt in Opt::ALL {
            assert_eq!(en.get(opt.caption_key()), Some(opt.caption()), "{opt:?}");
            assert_eq!(
                en.get(opt.description_key()),
                Some(opt.description()),
                "{opt:?}"
            );
        }
        for header in rows().into_iter().filter_map(Result::err) {
            assert!(en.get(header).is_some(), "{header}");
        }
        for c in Corner::ALL {
            let key = format!("horizon.pvp.corner.{}", c.name());
            assert_eq!(en.get(&key), Some(c.label()), "{key}");
        }
        let names = Colour::ALL.map(|c| (format!("horizon.pvp.colour.{}", c.name()), c.english()));
        let shapes = Shape::ALL.map(|c| (format!("horizon.pvp.shape.{}", c.name()), c.english()));
        let times = ViewTime::ALL.map(|c| (format!("horizon.pvp.time.{}", c.name()), c.english()));
        let weathers =
            ViewWeather::ALL.map(|c| (format!("horizon.pvp.weather.{}", c.name()), c.english()));
        assert_eq!(en.get("horizon.pvp.view.world"), Some("World's"));
        for (key, english) in names.iter().chain(&shapes).chain(&times).chain(&weathers) {
            assert_eq!(en.get(key), Some(*english), "{key}");
        }
    }

    #[test]
    fn legacy_leaves_out_what_1_9_added_to_a_hit() {
        use crate::particles::ParticleType as T;
        let modern = Combat::default();
        let legacy = Combat {
            legacy: true,
            ..Combat::default()
        };
        let sounds = Combat {
            sounds: true,
            ..legacy
        };
        for name in [
            "minecraft:entity.player.attack.strong",
            "entity.player.attack.sweep",
            "minecraft:entity.player.attack.nodamage",
        ] {
            assert!(sounds.mutes_sound(name), "{name}");
            assert!(!legacy.mutes_sound(name), "only with the sounds option");
            assert!(!modern.mutes_sound(name));
        }
        // 1.8.9 had the hurt sound: it stays.
        assert!(!sounds.mutes_sound("minecraft:entity.player.hurt"));
        assert!(legacy.hides_particle(T::SweepAttack) && legacy.hides_particle(T::DamageIndicator));
        assert!(!legacy.hides_particle(T::Crit) && !legacy.hides_particle(T::EnchantedHit));
        assert!(!modern.hides_particle(T::SweepAttack));
    }

    #[test]
    fn oteria_pvp_options_are_saved_and_read_back() {
        let mut p = Pvp::default();
        p.hud.cooldowns = true;
        p.hud.totems_corner = Corner::TopRight;
        p.crystal = CrystalOpt { enabled: true, keep_render: true };
        p.anchor.enabled = true;
        p.pops = true;
        p.secret.effect = 7;
        p.hitboxes.enabled = true;
        p.hitboxes.passive = None;
        p.hitboxes.players = Some(Colour::Custom(0x12AB34));
        p.hitboxes.width = 20;
        let mut text = String::new();
        p.serialize(&mut text);
        let mut back = Pvp::default();
        for line in text.lines() {
            let (k, v) = line.split_once(':').unwrap();
            back.parse_line(k, v);
        }
        assert_eq!(back.hud, p.hud);
        assert_eq!(back.crystal, p.crystal);
        assert_eq!(back.anchor, p.anchor);
        assert!(back.pops);
        assert_eq!(back.secret, p.secret);
        assert_eq!(back.hitboxes, p.hitboxes);
    }

    #[test]
    fn a_hitbox_colour_cycles_through_off() {
        let mut p = Pvp::default();
        p.hitboxes.items = Some(Colour::Black);
        Opt::HitboxItems.cycle(&mut p, false);
        assert_eq!(p.hitboxes.items, None);
        Opt::HitboxItems.cycle(&mut p, false);
        assert_eq!(p.hitboxes.items, Some(Colour::White));
    }
}
