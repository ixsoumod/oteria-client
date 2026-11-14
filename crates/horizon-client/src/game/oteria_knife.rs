//! Oteria's butterfly knife (a cosmetic, `docs/oteria/ROADMAP.md`): with it on (the Cosmetics
//! page), every sword in the player's main hand is drawn in first person as Oteria's balisong
//! (`branding/cosmetics/make_balisong.py`), in the player's own arm, which shows as it holds it
//! and moves with every throw. Taken in hand, it opens as a real one does (a flick, a rollover
//! shut, a second flick open).
//!
//! The knife moves as a real balisong does: the hand holds one handle (the safe one); the blade
//! turns on that handle's pin, half a turn from shut to open; the other handle (the bite one)
//! turns on the blade's other pin, half a turn from lying on the blade to lying by the held
//! handle. The inspect key (² by default) plays real tricks: a basic close (the bite handle swings over
//! onto the blade, then both fold into the hand), a flick out, an aerial (the knife leaves the
//! hand, turns once in the air and is caught), the flick that opens it, then a fast close and
//! open, to metal clicks made of the game's own sounds. Only the picture changes: the sword is
//! the sword for the game and the server, and nothing is sent.

use super::{Game, Screen};
use horizon_client::mesher::Vertex;
use horizon_client::mobs::{Part, Polygon};
use std::cell::RefCell;
use std::f32::consts::PI;
use std::sync::OnceLock;
use std::time::Instant;

/// The knife drawn: its model, its texture's sprite, where the fist closes on the held handle
/// (x, model units) and its length (blade tip to handle end).
struct Knife {
    part: Part,
    sprite: &'static str,
    /// Where the fist closes on the held handle (x, model units).
    grip: f32,
    length: f32,
}

/// Oteria's own knife; with `OTERIA_KNIFE=private`, a knife converted on this computer
/// (`oteria_art::KNIFE_PRIVATE_FILE`, never in git) when there is one.
fn model() -> Option<&'static Knife> {
    static MODEL: OnceLock<Option<Knife>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let private = std::env::var("OTERIA_KNIFE").is_ok_and(|v| v == "private");
            let (part, sprite) = match private.then(private_model).flatten() {
                Some(part) => (part, horizon_client::oteria_art::KNIFE_PRIVATE),
                None => {
                    let bytes = include_bytes!("../../assets/horizon/cosmetics/oteria_balisong.json");
                    match horizon_cosmetics::Item::parse(bytes) {
                        Ok(item) => {
                            // The file's attachment (its place on a player's arm) is not the hand's.
                            let mut root = convert(&item.root);
                            root.pose = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
                            (root, horizon_client::oteria_art::KNIFE)
                        }
                        Err(e) => {
                            log::warn!("Oteria: the butterfly knife's model doesn't read: {e}");
                            return None;
                        }
                    }
                }
            };
            // Its extent along x, open: each part's corners from its pivot.
            let extent = |names: &[&str]| {
                let xs = part.children.iter().filter(|p| names.contains(&p.name.as_str())).flat_map(|p| {
                    p.polygons.iter().flat_map(move |poly| poly.vertices.iter().map(move |v| v[0] + p.pose[0]))
                });
                xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
            };
            let (lo, hi) = extent(&["blade", "handle_safe", "handle_latch"]);
            let (handle_lo, handle_hi) = extent(&["handle_safe"]);
            if lo >= hi {
                return None;
            }
            Some(Knife { part, sprite, grip: handle_lo + 0.3 * (handle_hi - handle_lo), length: hi - lo })
        })
        .as_ref()
}
/// A converted knife (`branding/cosmetics/convert_glb_balisong.py`): parts with a pivot and
/// triangles of (x, y, z, u, v) from it, in Oteria's knife's model space.
fn private_model() -> Option<Part> {
    #[derive(serde::Deserialize)]
    struct File {
        parts: Vec<Piece>,
    }
    #[derive(serde::Deserialize)]
    struct Piece {
        name: String,
        pivot: [f32; 3],
        triangles: Vec<[[f32; 5]; 3]>,
    }
    let path = horizon_client::oteria_art::knife_private_path("json")?;
    let path = path.display().to_string();
    let text = std::fs::read_to_string(&path).ok()?;
    let file: File = match serde_json::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Oteria: {path} doesn't read: {e}");
            return None;
        }
    };
    let polygon = |t: &[[f32; 5]; 3]| {
        let v = |i: usize| glam::Vec3::new(t[i][0], t[i][1], t[i][2]);
        let n = (v(1) - v(0)).cross(v(2) - v(0)).normalize_or_zero();
        // A triangle as a quad whose last corner is its third.
        Polygon { normal: n.to_array(), vertices: [t[0], t[1], t[2], t[2]], cull_back: false }
    };
    let children = file
        .parts
        .iter()
        .map(|p| Part {
            name: p.name.clone(),
            pose: [p.pivot[0], p.pivot[1], p.pivot[2], 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            polygons: p.triangles.iter().map(polygon).collect(),
            children: Vec::new(),
        })
        .collect();
    log::info!("Oteria: the butterfly knife is the one in {path}");
    Some(Part {
        name: "knife".into(),
        pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        polygons: Vec::new(),
        children,
    })
}

fn convert(p: &horizon_cosmetics::Part) -> Part {
    Part {
        name: p.name.clone(),
        pose: p.pose,
        polygons: p
            .faces
            .iter()
            .map(|f| Polygon { normal: f.normal, vertices: f.vertices, cull_back: false })
            .collect(),
        children: p.children.iter().map(convert).collect(),
    }
}

/// How a value gets from one key to the next.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ease {
    /// Slow at both ends.
    Smooth,
    /// Speeding up (a part let go, falling round its pin).
    In,
    /// Slowing down.
    Out,
    /// Steady.
    Line,
    /// Thrown from rest round its pin: it speeds up, hits its stop and bounces off it a little.
    Swing,
    /// Already moving, it hits its stop and bounces off it a little.
    Hit,
}
fn ease(e: Ease, u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    let bounce = |u: f32| 1.0 - 0.06 * ((u - 0.8) / 0.2 * PI).sin();
    match e {
        Ease::Smooth => u * u * (3.0 - 2.0 * u),
        Ease::In => u * u,
        Ease::Out => 1.0 - (1.0 - u) * (1.0 - u),
        Ease::Line => u,
        Ease::Swing if u < 0.8 => (u / 0.8).powi(2),
        Ease::Hit if u < 0.8 => u / 0.8,
        Ease::Swing | Ease::Hit => bounce(u),
    }
}
/// A channel: keys of (time s, value, how it gets there from the key before).
type Track = &'static [(f32, f32, Ease)];
fn track(t: f32, keys: Track) -> f32 {
    let Some(first) = keys.first() else {
        return 0.0;
    };
    if t <= first.0 {
        return first.1;
    }
    for w in keys.windows(2) {
        let ((t0, v0, _), (t1, v1, e)) = (w[0], w[1]);
        if t < t1 {
            return v0 + (v1 - v0) * ease(e, (t - t0) / (t1 - t0));
        }
    }
    keys[keys.len() - 1].1
}

/// The knife and the arm at one moment. The joints are angles about the pins' axis (radians):
/// `blade`, the blade's on the held handle's pin (0: open, ±π: shut between the handles), and
/// `latch`, the bite handle's, on its pin that the blade carries (0: lying by the held handle,
/// ±π: lying along the open blade, the knife a straight stick). Whole turns are the same pose.
/// `spin` turns the whole knife about the pins' axis (an aerial), `toss` lifts it out of the
/// fist along the arm (blocks), `wrist` tips it in the fist (radians). The arm: `up`, how far it
/// has brought the knife up to be looked at (0 to 1, the grip between its two sets); `shift`,
/// where it has moved (blocks: right, up, toward the eye); `tilt`, `pitch` and `roll`, how it
/// has turned (radians: in the screen's plane, up and down, about its own length).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(super) struct Pose {
    pub blade: f32,
    pub latch: f32,
    pub spin: f32,
    pub toss: f32,
    pub wrist: f32,
    pub up: f32,
    pub shift: [f32; 3],
    pub tilt: f32,
    pub pitch: f32,
    pub roll: f32,
}

/// An animation: its length, its channels and its sounds (time, event, volume, pitch).
pub(super) struct Anim {
    pub length: f32,
    /// A stretch played over and over while the key that started it is held (from, to): the
    /// pose is the same at both ends.
    repeat: Option<(f32, f32)>,
    blade: Track,
    latch: Track,
    spin: Track,
    toss: Track,
    wrist: Track,
    up: Track,
    shift: [Track; 3],
    tilt: Track,
    pitch: Track,
    roll: Track,
    sounds: &'static [(f32, &'static str, f32, f32)],
}
impl Anim {
    pub(super) fn pose(&self, t: f32) -> Pose {
        Pose {
            blade: track(t, self.blade),
            latch: track(t, self.latch),
            spin: track(t, self.spin),
            toss: track(t, self.toss),
            wrist: track(t, self.wrist),
            up: track(t, self.up),
            shift: self.shift.map(|k| track(t, k)),
            tilt: track(t, self.tilt),
            pitch: track(t, self.pitch),
            roll: track(t, self.roll),
        }
    }
}

const SWISH: &str = "minecraft:entity.player.attack.sweep";
const CLICK: &str = "minecraft:block.tripwire.click_on";
const SHUT: &str = "minecraft:block.iron_trapdoor.close";
const CATCH: &str = "minecraft:block.chain.step";
use Ease::{Hit, In, Line, Out, Smooth, Swing};
const P: f32 = PI;

/// Taking it in hand (from a real balisong's opening, watched frame by frame): the arm comes up
/// with the knife shut, the palm turns, and a flick throws the bite handle and the blade out
/// together, over the top and round into the fist shut again (a rollover); the hand dips, lifts
/// and flicks again: the blade swings up, the bite handle goes on round over the top and down
/// the other side into the palm while the arm swings across with it; open, the arm comes back.
pub(super) static DRAW: Anim = Anim {
    length: 1.2,
    repeat: None,
    blade: &[(0.0, P, Line), (0.20, P, Line), (0.33, 0.0, In), (0.50, -P, Hit), (0.56, -P, Line), (0.72, -2.0 * P, Swing)],
    latch: &[(0.0, 0.0, Line), (0.22, 0.0, Line), (0.36, -P, In), (0.52, -2.0 * P, Hit), (0.58, -2.0 * P, Line), (0.74, -3.0 * P, In), (0.93, -4.0 * P, Hit)],
    spin: &[],
    toss: &[],
    wrist: &[(0.0, 0.0, Line), (0.18, 0.30, Smooth), (0.30, -0.40, Out), (0.45, 0.0, Smooth), (0.56, 0.30, Smooth), (0.68, -0.40, Out), (0.92, 0.0, Smooth)],
    up: &[(0.0, 0.0, Line), (0.14, 0.7, Out), (0.85, 0.7, Line), (1.2, 0.0, Smooth)],
    shift: [
        &[(0.0, 0.05, Line), (0.14, -0.10, Out), (0.20, -0.08, Smooth), (0.33, -0.16, Out), (0.50, -0.07, Smooth), (0.60, -0.10, Smooth), (0.86, -0.22, Smooth), (0.96, -0.20, Smooth), (1.2, 0.0, Smooth)],
        &[(0.0, -0.50, Line), (0.14, 0.12, Out), (0.30, 0.14, Smooth), (0.50, 0.08, Smooth), (0.62, 0.16, Out), (0.86, 0.06, Smooth), (1.2, 0.0, Smooth)],
        &[],
    ],
    tilt: &[(0.0, 0.7, Line), (0.14, 0.5, Out), (0.20, 0.4, Smooth), (0.33, 0.8, Out), (0.50, 0.4, Smooth), (0.58, 0.35, Smooth), (0.72, 0.7, Out), (0.88, 0.85, Smooth), (1.0, 0.55, Smooth), (1.2, 0.0, Smooth)],
    pitch: &[(0.0, -0.6, Line), (0.14, 0.0, Out), (0.52, 0.15, Smooth), (0.62, -0.20, Out), (0.90, 0.10, Smooth), (1.2, 0.0, Smooth)],
    roll: &[(0.0, 0.0, Line), (0.10, 0.1, Smooth), (0.18, 0.25, Smooth), (0.35, 0.05, Smooth), (0.50, 0.1, Smooth), (0.85, -0.1, Smooth), (1.2, 0.0, Smooth)],
    sounds: &[
        (0.24, SWISH, 0.15, 1.9),
        (0.48, CLICK, 0.6, 1.7),
        (0.60, SWISH, 0.16, 2.0),
        (0.78, SWISH, 0.12, 1.6),
        (0.92, CLICK, 0.7, 1.9),
        (0.95, CATCH, 0.3, 1.8),
    ],
};

/// The inspect (the inspect key): brought up and turned to be seen, then real tricks. A basic
/// close (the bite handle swings over onto the blade, then both fold into the fist), a flick out
/// to a straight stick, an aerial (the arm throws it up, it turns once and is caught, the arm
/// giving with it), the flick that opens it, then a fast close and open; it goes back down.
/// Each throw is the arm's: it cocks back, snaps through and follows on.
pub(super) static INSPECT: Anim = Anim {
    length: 3.0,
    repeat: Some((1.12, 1.70)),
    blade: &[(0.0, 0.0, Line), (0.50, 0.0, Line), (0.72, P, Swing), (0.90, P, Line), (1.12, 2.0 * P, Swing), (2.20, 2.0 * P, Line), (2.36, 3.0 * P, Swing), (2.42, 3.0 * P, Line), (2.56, 4.0 * P, Swing)],
    latch: &[(0.0, 0.0, Line), (0.25, 0.0, Line), (0.45, -P, Swing), (0.50, -P, Line), (0.72, 0.0, Swing), (0.90, 0.0, Line), (1.12, P, Swing), (1.75, P, Line), (2.00, 2.0 * P, Swing), (2.05, 2.0 * P, Line), (2.20, P, Swing), (2.36, 2.0 * P, Swing), (2.42, 2.0 * P, Line), (2.56, 3.0 * P, Swing), (2.58, 3.0 * P, Line), (2.72, 4.0 * P, Swing)],
    spin: &[(1.18, 0.0, Line), (1.70, -2.0 * P, Line)],
    toss: &[(1.18, 0.0, Line), (1.44, 0.24, Out), (1.70, 0.0, In)],
    wrist: &[(0.0, 0.0, Line), (0.18, 0.0, Line), (0.25, 0.25, Smooth), (0.40, -0.25, Out), (0.50, 0.15, Smooth), (0.62, -0.30, Out), (0.82, 0.25, Smooth), (0.98, -0.35, Out), (1.12, 0.0, Smooth), (1.70, 0.0, Line), (1.75, 0.25, Smooth), (1.90, -0.30, Out), (2.05, 0.20, Smooth), (2.16, -0.25, Out), (2.36, 0.25, Smooth), (2.50, -0.25, Out), (2.65, 0.15, Smooth), (2.80, 0.0, Smooth)],
    up: &[(0.0, 0.0, Line), (0.25, 1.0, Smooth), (2.75, 1.0, Line), (3.0, 0.0, Smooth)],
    shift: [
        &[(0.0, 0.0, Line), (0.25, -0.18, Smooth), (0.40, -0.21, Out), (0.62, -0.15, Out), (0.90, -0.20, Smooth), (1.05, -0.16, Out), (1.12, -0.17, Smooth), (1.70, -0.17, Line), (1.85, -0.18, Smooth), (2.15, -0.21, Out), (2.45, -0.15, Out), (2.75, -0.18, Smooth), (3.0, 0.0, Smooth)],
        &[(0.0, 0.0, Line), (0.25, 0.14, Smooth), (1.12, 0.11, Smooth), (1.20, 0.22, Out), (1.40, 0.16, Smooth), (1.64, 0.08, Out), (1.70, 0.11, Smooth), (1.90, 0.14, Smooth), (2.75, 0.14, Line), (3.0, 0.0, Smooth)],
        &[(1.12, 0.0, Line), (1.20, -0.05, Out), (1.64, 0.03, Out), (1.70, 0.0, Smooth)],
    ],
    tilt: &[(0.0, 0.0, Line), (0.22, 0.10, Smooth), (0.40, -0.25, Out), (0.55, 0.15, Smooth), (0.70, -0.20, Out), (0.88, 0.20, Smooth), (1.05, -0.20, Out), (1.12, -0.05, Smooth), (1.70, -0.05, Line), (1.78, 0.15, Smooth), (1.95, -0.20, Out), (2.10, 0.15, Smooth), (2.30, -0.20, Out), (2.45, 0.15, Smooth), (2.65, -0.10, Out), (3.0, 0.0, Smooth)],
    pitch: &[(0.0, 0.0, Line), (0.25, 0.10, Smooth), (1.12, 0.15, Smooth), (1.20, -0.30, Out), (1.45, -0.10, Smooth), (1.64, 0.20, Out), (1.70, 0.15, Smooth), (1.90, 0.10, Smooth), (2.75, 0.10, Line), (3.0, 0.0, Smooth)],
    roll: &[(0.0, 0.0, Line), (0.25, 0.35, Smooth), (0.90, 0.20, Smooth), (1.12, 0.22, Smooth), (1.70, 0.22, Line), (1.78, 0.25, Smooth), (2.20, -0.25, Smooth), (2.75, 0.30, Smooth), (3.0, 0.0, Smooth)],
    sounds: &[
        (0.27, SWISH, 0.15, 1.9),
        (0.43, CLICK, 0.6, 1.8),
        (0.69, SHUT, 0.3, 2.0),
        (0.92, SWISH, 0.16, 2.0),
        (1.10, CLICK, 0.6, 1.6),
        (1.18, SWISH, 0.2, 1.5),
        (1.69, CATCH, 0.55, 1.7),
        (1.78, SWISH, 0.15, 2.0),
        (1.98, CLICK, 0.7, 1.9),
        (2.07, SWISH, 0.15, 1.8),
        (2.33, SHUT, 0.3, 2.0),
        (2.44, SWISH, 0.16, 2.0),
        (2.70, CLICK, 0.7, 1.8),
    ],
};

/// The animation playing, if any: which, since when, and the sounds already played.
struct Playing {
    anim: &'static Anim,
    started: Instant,
    played: usize,
    /// Whether the key that started it is still held (its `repeat` plays on meanwhile).
    holding: bool,
    /// The time spent repeating, once the key is let go (s).
    skipped: f32,
    /// Where it was last frame (a smaller time: it went round again).
    last: f32,
}
impl Playing {
    fn new(anim: &'static Anim, holding: bool) -> Self {
        Self { anim, started: Instant::now(), played: 0, holding, skipped: 0.0, last: 0.0 }
    }
    /// Where it is in the animation now.
    fn time(&self) -> f32 {
        let t = self.started.elapsed().as_secs_f32() - self.skipped;
        match self.anim.repeat {
            Some((from, to)) if self.holding && t > to => from + (t - from) % (to - from),
            _ => t,
        }
    }
    /// The key is let go: the stretch being repeated plays to its end, then the rest.
    fn let_go(&mut self) {
        let now = self.time();
        self.holding = false;
        self.skipped = self.started.elapsed().as_secs_f32() - now;
    }
}
#[derive(Default)]
struct State {
    playing: Option<Playing>,
    /// The slot and the item last shown as the knife: another one, and it is taken in hand.
    held: Option<(usize, String)>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
fn play(anim: &'static Anim, holding: bool) {
    STATE.with(|s| s.borrow_mut().playing = Some(Playing::new(anim, holding)));
}

/// How the knife sits against the fist's palm side, in the arm's own space (model pixels from
/// the arm part's pivot, y toward the hand; degrees; the knife's size, for a knife as long as
/// `LENGTH`): at rest, and brought up. `private/knife_pose.txt` (14 numbers, the two sets)
/// overrides it while tuning.
const GRIP_REST: [f32; 7] = [-1.0, 11.0, -2.2, 0.0, 10.0, -45.0, 0.36];
const GRIP_SHOWN: [f32; 7] = [-1.0, 11.0, -2.2, 0.0, 0.0, -30.0, 0.4];
/// The knife's length (blade tip to handle end, in its model units) the grip's size is for.
const LENGTH: f32 = 41.0;
fn grips() -> ([f32; 7], [f32; 7]) {
    let tuned = std::fs::read_to_string("private/knife_pose.txt").ok().and_then(|text| {
        let v: Vec<f32> = text.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|n| n.parse().ok()).collect();
        (v.len() == 14).then(|| (v[..7].try_into().unwrap(), v[7..].try_into().unwrap()))
    });
    tuned.unwrap_or((GRIP_REST, GRIP_SHOWN))
}

impl Game<'_> {
    /// Whether `item` in the main hand is drawn as the knife.
    pub(super) fn knife_shown(&self, item: &str) -> bool {
        self.settings.extras.pvp.skins.knife && item.ends_with("_sword") && model().is_some()
    }

    /// The selected slot and its item, when it shows as the knife.
    fn knife_held(&self) -> Option<(usize, String)> {
        let item = self.inventory.get(self.selected).and_then(|s| s.as_ref()).map(|s| s.item.to_string())?;
        self.knife_shown(&item).then_some((self.selected, item))
    }

    /// The inspect key, in play with a sword shown as the knife. True when taken.
    pub(super) fn knife_key_press(&mut self, name: &str) -> bool {
        if self.screen != Screen::Playing || name != self.settings.extras.pvp.skins.inspect_key {
            return false;
        }
        if self.knife_held().is_none() {
            return false;
        }
        // The key repeating while held doesn't start it again.
        let holding = STATE.with(|s| s.borrow().playing.as_ref().is_some_and(|p| p.holding));
        if !holding {
            play(&INSPECT, true);
        }
        true
    }

    /// Once a frame: the opening when the knife comes into the hand, the inspect's aerial
    /// over and over while its key is held, and the playing animation's sounds as it reaches
    /// them; it stops when the knife goes.
    pub(super) fn knife_frame(&mut self) {
        let held = self.knife_held();
        let key_down = horizon_client::keybinds::key_code(&self.settings.extras.pvp.skins.inspect_key)
            .is_some_and(|k| self.physical_keys.contains(&k));
        let due: Vec<(&str, f32, f32)> = STATE.with(|s| {
            let mut s = s.borrow_mut();
            if held != s.held {
                s.held = held.clone();
                s.playing = held.is_some().then(|| Playing::new(&DRAW, false));
            }
            let Some(p) = s.playing.as_mut() else {
                return Vec::new();
            };
            if p.holding && !key_down {
                p.let_go();
            }
            let t = p.time();
            if t < p.last {
                // Round again: the repeated stretch's sounds again.
                let from = p.anim.repeat.map_or(0.0, |r| r.0);
                p.played = p.anim.sounds.iter().take_while(|s| s.0 < from).count();
            }
            p.last = t;
            let mut due = Vec::new();
            while p.played < p.anim.sounds.len() && p.anim.sounds[p.played].0 <= t {
                let (_, event, volume, pitch) = p.anim.sounds[p.played];
                due.push((event, volume, pitch));
                p.played += 1;
            }
            if t >= p.anim.length {
                s.playing = None;
            }
            due
        });
        for (event, volume, pitch) in due {
            self.audio.play(event, None, volume, pitch);
        }
    }

    /// The arm that holds the knife and the knife in its fist, in first person: `to_world` is the
    /// hands' view space, `inverse_height` and `attack` the hand's equip height and swing (the
    /// arm swings as an empty hand does, the knife with it). The vertices where the knife starts
    /// are returned (the arm before them).
    pub(super) fn draw_knife(
        &self,
        to_world: glam::Mat4,
        left: bool,
        inverse_height: f32,
        attack: f32,
        light: u32,
        out: &mut Vec<Vertex>,
    ) -> usize {
        use glam::{EulerRot, Mat4, Quat, Vec3};
        // While tuning, `private/knife_t.txt` ("draw 0.3", "inspect 1.2") holds one moment.
        let frozen = std::fs::read_to_string("private/knife_t.txt").ok().and_then(|text| {
            let (kind, t) = text.trim().split_once(' ')?;
            let anim = if kind == "draw" { &DRAW } else { &INSPECT };
            Some((anim.pose(t.parse().ok()?), kind == "draw"))
        });
        let (pose, drawing) = frozen
            .or_else(|| STATE.with(|s| {
                let s = s.borrow();
                let p = s.playing.as_ref()?;
                Some((p.anim.pose(p.time()), std::ptr::eq(p.anim, &DRAW)))
            }))
            .unwrap_or_default();
        // The opening brings the arm up itself, in place of the game's own lowered hand.
        let inverse_height = if drawing { 0.0 } else { inverse_height };
        let side = if left { -1.0 } else { 1.0 };
        // The arm moves and turns about where the hand is, in the view's own axes (mirrored for
        // the left hand).
        let [sx, sy, sz] = pose.shift;
        let at = Vec3::new(0.64 * side, -0.6, -0.72);
        let moved = Mat4::from_translation(Vec3::new(sx * side, sy, sz) + at)
            * Mat4::from_rotation_z(pose.tilt * side)
            * Mat4::from_rotation_x(pose.pitch)
            * Mat4::from_translation(-at);
        let slim = self.player_model.as_ref().is_some_and(|m| m.slim);
        let pivot = self.res.player_arm(slim, left).as_ref().map_or([-5.0 * side, 2.0, 0.0], |a| {
            [a.pose[0], a.pose[1], a.pose[2]]
        });
        let pivot = Vec3::from(pivot) / 16.0;
        // The forearm rolls about its own length (y, in the arm part's space).
        let arm = to_world
            * moved
            * horizon_client::first_person::player_arm_pose(left, inverse_height, attack)
            * Mat4::from_translation(pivot)
            * Mat4::from_rotation_y(pose.roll * side)
            * Mat4::from_translation(-pivot);
        if !self.own_invisible() {
            self.first_person_arm(left, arm, light, out);
        }
        let start = out.len();
        let Some(knife) = model() else {
            return start;
        };
        let Some(r) = self.gui.atlas.sprites.get(knife.sprite).copied() else {
            return start;
        };
        let part = &knife.part;
        // The arm part's own space, as `first_person_arm` poses it: its pivot, turned outwards.
        let hand = arm * Mat4::from_translation(pivot) * Mat4::from_rotation_z(0.1 * side);
        let (rest, shown) = grips();
        let g: [f32; 7] = std::array::from_fn(|k| rest[k] + (shown[k] - rest[k]) * pose.up);
        let pin = |name: &str| {
            part.children.iter().find(|p| p.name == name).map_or(Vec3::ZERO, |p| Vec3::new(p.pose[0], p.pose[1], p.pose[2]))
        };
        let (safe_pin, bite_pin) = (pin("handle_safe"), pin("handle_latch"));
        // The fist closes on the held handle near its pin, the rest of it out of the fist.
        let grip = Vec3::new(knife.grip, safe_pin.y, safe_pin.z);
        let base = hand
            // The aerial: out of the fist, on along the arm.
            * Mat4::from_translation(Vec3::new(0.0, pose.toss, 0.0))
            * Mat4::from_translation(Vec3::new(g[0] * side, g[1], g[2]) / 16.0)
            * Mat4::from_quat(Quat::from_euler(
                EulerRot::XYZ,
                g[3].to_radians(),
                (g[4] * side).to_radians(),
                (g[5] * side).to_radians(),
            ))
            // The wrist's flicks, and the aerial's turn about the pins' axis.
            * Mat4::from_rotation_z(pose.wrist)
            * Mat4::from_scale(Vec3::splat(g[6] * LENGTH / knife.length))
            * Mat4::from_rotation_z(pose.spin)
            * Mat4::from_translation(-grip / 16.0);
        // The joints: the blade turns on the held handle's pin; the bite handle turns on its own
        // pin, which the blade carries round.
        let about = |centre: Vec3, angle: f32, point: Vec3| {
            centre + Mat4::from_rotation_z(angle).transform_vector3(point - centre)
        };
        let pose_of = |p: &Part| {
            let mut q = p.pose;
            match p.name.as_str() {
                "blade" => {
                    let at = about(safe_pin, pose.blade, Vec3::new(q[0], q[1], q[2]));
                    (q[0], q[1], q[5]) = (at.x, at.y, pose.blade);
                }
                "handle_latch" => {
                    let at = about(safe_pin, pose.blade, bite_pin);
                    (q[0], q[1], q[5]) = (at.x, at.y, pose.latch);
                }
                _ => {}
            }
            q
        };
        horizon_client::mobs::render_model(part, base, &pose_of, [r.u0, r.v0, r.u1, r.v1], light, 0, out);
        start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_reads_and_has_its_moving_parts() {
        let knife = model().expect("the knife's model reads");
        let names: Vec<&str> = knife.part.children.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["blade", "handle_safe", "handle_latch"]);
        assert!(knife.length > 20.0 && knife.grip > 0.0, "{} {}", knife.length, knife.grip);
    }

    /// Whole turns are the same pose.
    fn same_angle(a: f32, b: f32) -> bool {
        let d = (a - b).rem_euclid(2.0 * PI);
        d < 1e-3 || 2.0 * PI - d < 1e-3
    }

    #[test]
    fn the_animations_start_and_end_as_a_balisong_does() {
        // The opening starts shut, low; the inspect starts open. Both end open, at rest.
        let draw = DRAW.pose(0.0);
        assert!(same_angle(draw.blade, PI) && same_angle(draw.latch, 0.0) && draw.shift[1] < -0.3);
        let inspect = INSPECT.pose(0.0);
        assert!(same_angle(inspect.blade, 0.0) && same_angle(inspect.latch, 0.0));
        for anim in [&DRAW, &INSPECT] {
            let end = anim.pose(anim.length);
            assert!(same_angle(end.blade, 0.0) && same_angle(end.latch, 0.0), "{end:?}");
            assert!(same_angle(end.spin, 0.0));
            let still = [end.toss, end.wrist, end.up, end.tilt, end.pitch, end.roll];
            assert!(still.iter().chain(&end.shift).all(|v| v.abs() < 1e-4), "{end:?}");
            assert!(anim.sounds.windows(2).all(|w| w[0].0 <= w[1].0), "the sounds in order");
            assert!(anim.sounds.last().is_some_and(|s| s.0 < anim.length));
        }
        // The rollover: shut again halfway through the opening.
        let shut = DRAW.pose(0.53);
        assert!(same_angle(shut.blade, PI) && same_angle(shut.latch, 0.0), "{shut:?}");
        // The inspect's aerial: a straight stick, out of the fist.
        let air = INSPECT.pose(1.44);
        assert!(same_angle(air.blade, 0.0) && same_angle(air.latch, PI) && air.toss > 0.2, "{air:?}");
    }

    #[test]
    fn the_aerial_repeats_without_a_jump() {
        let (from, to) = INSPECT.repeat.expect("the inspect repeats its aerial");
        let (a, b) = (INSPECT.pose(from), INSPECT.pose(to));
        assert!(same_angle(a.blade, b.blade) && same_angle(a.latch, b.latch) && same_angle(a.spin, b.spin));
        let near = |x: f32, y: f32| (x - y).abs() < 1e-3;
        assert!(near(a.toss, b.toss) && near(a.wrist, b.wrist) && near(a.up, b.up), "{a:?} {b:?}");
        assert!(near(a.tilt, b.tilt) && near(a.pitch, b.pitch) && near(a.roll, b.roll), "{a:?} {b:?}");
        assert!((0..3).all(|k| near(a.shift[k], b.shift[k])), "{a:?} {b:?}");
        assert!(DRAW.repeat.is_none());
    }

    #[test]
    fn a_track_holds_its_ends_and_lands_on_its_keys() {
        let keys: Track = &[(1.0, 2.0, Line), (2.0, 4.0, Swing), (3.0, 0.0, Smooth)];
        assert_eq!(track(0.0, keys), 2.0);
        assert_eq!(track(2.0, keys), 4.0);
        assert!((track(1.5, keys) - 2.0).abs() > 0.1);
        assert_eq!(track(9.0, keys), 0.0);
        assert_eq!(track(5.0, &[]), 0.0);
    }
}
