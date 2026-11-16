//! Oteria's own wearables (the Cosmetics page's Capes, Wings and Auras), drawn on the window's
//! own player through the player hooks (`horizon_hooks::Hooks::player_layers`), in every view:
//!
//! - **A cape** that is cloth, simulated as Cloth-Cloaks (pheological, CC0) does: a grid of
//!   points held at its top corners, springs between them, gravity, a push out behind as the
//!   player runs and a lift as they fall, and the back it can't pass through. The grid is drawn
//!   as it bends, each frame.
//! - **Wings** of two hinged parts each, beating slowly at rest and fast in the air.
//! - **An aura**: motes rising in a spiral around the player, sparkles, and a ring at the feet,
//!   glowing.
//!
//! Every picture is drawn here, in code (no file): one 64×64 sprite per style worn. Only this
//! window's player wears them; nothing is sent anywhere.

use super::oteria_cosmetics::WearKind;
use glam::{DVec3, Mat4, Vec3};
use horizon_client::oteria_art::{AURAS, CAPES, WINGS, WearStyle, wear_style};
use horizon_hooks::players::{
    Bone, Frame, Kind, Light, Look, Mesh, PartPoses, Particle, Pass, PlayerDraw, PlayerModel,
    Quad, SpriteStore,
};
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};

/// The hooks Oteria's wearables draw through (`player_hooks::register`).
pub(super) struct Wear;
pub(super) static HOOKS: Wear = Wear;

/// What is worn, and what moves between frames.
#[derive(Default)]
struct State {
    cape: Option<&'static WearStyle>,
    wings: Option<&'static WearStyle>,
    aura: Option<&'static WearStyle>,
    cloth: Option<Cloth>,
    /// The cloth's mesh as it last stepped.
    cape_mesh: Option<Arc<Mesh>>,
    flight: Flight,
    /// The style tried on in the Cosmetics page's preview (its pictures only), and the last
    /// one tried, whose sprite stays stored.
    trying: Option<(WearKind, &'static WearStyle)>,
    tried: Option<(WearKind, &'static WearStyle)>,
    /// The cloth in the pictures (the preview, the inventory): fluttering as in a breeze.
    still_cloth: Option<Cloth>,
    still_mesh: Option<Arc<Mesh>>,
    /// The cloth the Cosmetics page's cards show, alone, in the lightest breeze.
    card_cloth: Option<Cloth>,
    card_mesh: Option<Arc<Mesh>>,
    /// The Cosmetics page shows: every style's sprite is kept.
    showing: bool,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);
fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the player wears now, from their settings (each frame; nothing when unchanged).
pub(super) fn sync(skins: &horizon_client::pvp::Skins) {
    let mut s = state();
    s.cape = wear_style(&CAPES, &skins.cape);
    s.wings = wear_style(&WINGS, &skins.wings);
    s.aura = wear_style(&AURAS, &skins.aura);
    if s.cape.is_none() {
        s.cloth = None;
        s.cape_mesh = None;
    }
}

/// The Cosmetics page's preview tries `trying` on (a cape, wings or aura, by its id) over what
/// is worn, in the pictures only; None: what is worn.
pub(super) fn try_on(trying: Option<(WearKind, &str)>) {
    let mut s = state();
    s.trying = trying.and_then(|(kind, id)| Some((kind, wear_style(kind.styles(), id)?)));
    if s.trying.is_some() {
        s.tried = s.trying;
    }
}

/// The Cosmetics page shows (each frame it does): every style's picture is kept in the atlas,
/// for its card.
pub(super) fn showing(on: bool) {
    state().showing = on;
}

/// The sprites' ids: a kind and the style's place in its list.
const SPRITE: u64 = 0x07E7_1A00_0000_0000;
fn sprite_id(kind: u64, all: &[WearStyle], style: &WearStyle) -> u64 {
    let i = all.iter().position(|s| s.id == style.id).unwrap_or(0) as u64;
    SPRITE | kind << 8 | i
}
/// Style `style`'s sprite (one of `kind`'s).
pub(super) fn sprite_of(kind: WearKind, style: &WearStyle) -> u64 {
    match kind {
        WearKind::Cape => sprite_id(CAPE_SPRITE, &CAPES, style),
        WearKind::Wings => sprite_id(WINGS_SPRITE, &WINGS, style),
        WearKind::Aura => sprite_id(AURA_SPRITE, &AURAS, style),
    }
}
const CAPE_SPRITE: u64 = 1;
const WINGS_SPRITE: u64 = 2;
const AURA_SPRITE: u64 = 3;

impl horizon_hooks::Hooks for Wear {
    fn frame(&self, _frame: &Frame, sprites: &mut dyn SpriteStore) {
        let (mut cape, mut wings, mut aura) = {
            let s = state();
            (s.cape, s.wings, s.aura)
        };
        // The style the preview tries on: its sprite too, but nothing worn instead of it.
        if let Some((kind, style)) = state().tried {
            let slot = match kind {
                WearKind::Cape => &mut cape,
                WearKind::Wings => &mut wings,
                WearKind::Aura => &mut aura,
            };
            if slot.is_none() {
                *slot = Some(style);
            } else if slot.is_some_and(|w| w.id != style.id) {
                let (id, make): (u64, Box<dyn Fn() -> Vec<u8>>) = match kind {
                    WearKind::Cape => (
                        sprite_id(CAPE_SPRITE, &CAPES, style),
                        Box::new(|| cape_texture(style)),
                    ),
                    WearKind::Wings => (
                        sprite_id(WINGS_SPRITE, &WINGS, style),
                        Box::new(|| wings_texture(style)),
                    ),
                    WearKind::Aura => (
                        sprite_id(AURA_SPRITE, &AURAS, style),
                        Box::new(|| aura_texture(style)),
                    ),
                };
                if !sprites.has(id) {
                    sprites.put(id, make());
                }
            }
        }
        let mut put = |id: u64, make: &dyn Fn() -> Vec<u8>| {
            if !sprites.has(id) {
                sprites.put(id, make());
            }
        };
        if let Some(c) = cape {
            put(sprite_id(CAPE_SPRITE, &CAPES, c), &|| cape_texture(c));
        }
        if let Some(w) = wings {
            put(sprite_id(WINGS_SPRITE, &WINGS, w), &|| wings_texture(w));
        }
        if let Some(a) = aura {
            put(sprite_id(AURA_SPRITE, &AURAS, a), &|| aura_texture(a));
        }
        // The page's cards: every style.
        if state().showing {
            for c in &CAPES {
                put(sprite_id(CAPE_SPRITE, &CAPES, c), &|| cape_texture(c));
            }
            for w in &WINGS {
                put(sprite_id(WINGS_SPRITE, &WINGS, w), &|| wings_texture(w));
            }
            for a in &AURAS {
                put(sprite_id(AURA_SPRITE, &AURAS, a), &|| aura_texture(a));
            }
        }
    }

    fn player_layers(&self, model: &PlayerModel, draw: &mut dyn PlayerDraw) {
        if model.kind != Kind::Own || model.invisible {
            return;
        }
        let mut s = state();
        let ticks = draw.ticks();
        // In a picture, the style the preview tries on in its place.
        let picture = draw.pass() != Pass::World;
        let tried = |kind: WearKind, worn: Option<&'static WearStyle>| match s.trying {
            Some((k, style)) if picture && k == kind => Some(style),
            _ => worn,
        };
        let (cape, wings) = (
            tried(WearKind::Cape, s.cape),
            tried(WearKind::Wings, s.wings),
        );
        if let Some(style) = cape {
            draw_cape(&mut s, style, draw, ticks);
        }
        if let Some(style) = wings {
            draw_wings(&mut s, style, draw, ticks);
        }
        if let (Some(style), Pass::World) = (s.aura, draw.pass()) {
            draw_aura(style, draw, ticks);
        }
    }

    /// In first person the body is out of view, the aura around the feet is not.
    fn first_person(&self, own: &PlayerModel, draw: &mut dyn PlayerDraw) {
        if own.invisible {
            return;
        }
        let aura = state().aura;
        if let Some(style) = aura {
            let ticks = draw.ticks();
            draw_aura(style, draw, ticks);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The cape's cloth.
//
// As Cloth-Cloaks (pheological, CC0) simulates a cape: a grid of `COLS` × `ROWS` points on the
// body, its two top corners held, structural springs between neighbours solved `PASSES` times a
// step, a hundred steps a second. Gravity pulls it down; moving pushes it out behind in
// proportion to the speed (only going forward: backing up lays it on the back); falling lifts
// it; it never passes in front of the back's plane. The grid is drawn as it is, a cloth of
// some thickness that bends anywhere, not hinged strips.

const COLS: usize = 5;
const ROWS: usize = 8;
/// The grid's spacing, in its own units.
const SPACING: f32 = 0.1;
const SPRING: f32 = 0.8;
const GRAVITY: f32 = 0.0001;
/// The movement read (blocks a tick) scaled up.
const INPUT_SCALE: f32 = 1.2;
const BASE_FORCE_Y: f32 = 0.0005;
/// The speed (blocks a tick) that pushes it out by `MOTION_FORCE_MAX`.
const MOTION_REFERENCE: f32 = 0.153;
const MOTION_FORCE_MAX: f32 = 0.0005;
/// Falling (blocks a tick, down) from which it lifts, and at which it lifts fully.
const FALL_START: f32 = 0.02;
const FALL_FULL: f32 = 0.17;
const FALL_FORCE_Y: f32 = -0.001;
const FALL_FORCE_Z: f32 = 0.001;
/// Nothing goes nearer the body than this (model units, out from where it hangs).
const BACK_PLANE: f32 = -0.01;
const DAMPING: f32 = 0.975;
const PASSES: usize = 50;
/// A step, in seconds.
const STEP: f32 = 0.01;
/// From the grid to the model (16 units a block): a cape 10 pixels wide, 16 down.
const X_SCALE: f32 = 0.625 / ((COLS - 1) as f32 * SPACING);
const Y_SCALE: f32 = 1.0 / ((ROWS - 1) as f32 * SPACING);
/// The cloth set out along its normal, and half its thickness (model units).
const NORMAL_OFFSET: f32 = 0.05;
const HALF_THICKNESS: f32 = 1.0 / 32.0;
/// Where the grid hangs on the body (model units): at the neck, out from the back.
const HUNG: Vec3 = Vec3::new(0.0, 0.0, 0.14);

/// The body's back, in the world.
#[derive(Clone, Copy)]
struct Axes {
    z: DVec3,
}

impl Axes {
    fn of(m: Mat4) -> Self {
        let n = |v: glam::Vec4| v.truncate().normalize_or_zero().as_dvec3();
        Self { z: n(m.z_axis) }
    }
}

/// How the player moves, as the cloth reads it (blocks a tick, scaled by `INPUT_SCALE`).
#[derive(Clone, Copy, Default)]
struct Moving {
    /// Across the ground, how fast.
    across: f32,
    /// Along where the body faces (forward positive).
    forward: f32,
    /// Up (falling negative).
    up: f32,
}

#[derive(Clone)]
struct Cloth {
    /// Each point, `[column][row]`, x across, y down, z out from the back.
    at: [[Vec3; ROWS]; COLS],
    before: [[Vec3; ROWS]; COLS],
    /// The push the last step left for the next (down, out).
    force: (f32, f32),
    moving: Moving,
    /// The model's age it was moved on to, and the time not yet stepped (seconds).
    ticks: f32,
    left: f32,
    feet: DVec3,
}

fn locked(c: usize, r: usize) -> bool {
    r == 0 && (c == 0 || c == COLS - 1)
}

fn rest(c: usize, r: usize) -> Vec3 {
    Vec3::new(-0.25 + c as f32 * SPACING, -0.4 + r as f32 * SPACING, 0.0)
}

impl Cloth {
    /// Hanging flat, still.
    fn hanging(feet: DVec3, ticks: f32) -> Self {
        let at = std::array::from_fn(|c| std::array::from_fn(|r| rest(c, r)));
        Self {
            at,
            before: at,
            force: (0.0, 0.0),
            moving: Moving::default(),
            ticks,
            left: 0.0,
            feet,
        }
    }

    /// Moved on to `ticks`, the feet at `feet`, the body's axes `axes`: whether it stepped.
    fn advance(&mut self, feet: DVec3, axes: Axes, ticks: f32) -> bool {
        let dt = (ticks - self.ticks) / 20.0;
        self.ticks = ticks;
        // A teleport, a respawn, or a long pause: hang again.
        if !(0.0..=0.25).contains(&dt) || feet.distance(self.feet) > 4.0 {
            *self = Self::hanging(feet, ticks);
            return true;
        }
        if dt > 0.0 {
            let moved = ((feet - self.feet) * f64::from(0.05 / dt)).as_vec3() * INPUT_SCALE;
            self.feet = feet;
            let facing = (-axes.z).with_y(0.0).normalize_or_zero().as_vec3();
            self.moving = Moving {
                across: moved.with_y(0.0).length(),
                forward: moved.dot(facing),
                up: moved.y,
            };
        }
        self.run(dt)
    }

    /// Moved on to `ticks` as in a breeze that comes and goes (a picture), `strength` its
    /// strength (1: a breeze, small: a breath): whether it stepped.
    fn flutter(&mut self, ticks: f32, strength: f32) -> bool {
        let dt = (ticks - self.ticks) / 20.0;
        self.ticks = ticks;
        if !(0.0..=0.25).contains(&dt) {
            return false;
        }
        let t = ticks / 20.0;
        let gust = 0.5 + 0.5 * (t * 0.9).sin() * (t * 0.37 + 1.0).sin();
        self.moving = Moving {
            across: (0.05 + 0.16 * gust) * strength,
            forward: 1.0,
            up: 0.0,
        };
        self.run(dt)
    }

    /// `dt` seconds of steps: whether there was one.
    fn run(&mut self, dt: f32) -> bool {
        self.left += dt;
        let mut steps = 0;
        while self.left >= STEP {
            self.left -= STEP;
            self.update();
            steps += 1;
            // Far behind (a slow frame): no catching up.
            if steps == 25 {
                self.left = 0.0;
            }
        }
        steps > 0
    }

    /// A step.
    fn update(&mut self) {
        let (down, out) = self.force;
        for c in 0..COLS {
            for r in 0..ROWS {
                if locked(c, r) {
                    continue;
                }
                let p = self.at[c][r];
                let v = (p - self.before[c][r]) * DAMPING;
                self.before[c][r] = p;
                self.at[c][r] = p + v + Vec3::new(0.0, GRAVITY + down, out);
            }
        }
        self.constrain();
        for pass in 0..PASSES {
            for c in 0..COLS {
                for r in 0..ROWS {
                    if c + 1 < COLS {
                        self.spring((c, r), (c + 1, r));
                    }
                    if r + 1 < ROWS {
                        self.spring((c, r), (c, r + 1));
                    }
                }
            }
            if pass == PASSES - 1 {
                self.constrain();
            }
        }

        // The push for the next step, from how the player moves.
        let m = self.moving;
        let fall = ((-m.up - FALL_START) / (FALL_FULL - FALL_START)).clamp(0.0, 1.0);
        let push = (m.across / MOTION_REFERENCE * MOTION_FORCE_MAX).max(FALL_FORCE_Z * fall);
        let direction = if fall > 0.0 || m.forward >= -1.0e-4 {
            1.0
        } else {
            -1.0
        };
        let down = BASE_FORCE_Y + (FALL_FORCE_Y - BASE_FORCE_Y) * fall;
        let out = if push > 0.0008 {
            push * 0.8
        } else {
            push * 0.5
        } * direction;
        self.force = (down, out);
    }

    fn spring(&mut self, (ac, ar): (usize, usize), (bc, br): (usize, usize)) {
        let d = self.at[bc][br] - self.at[ac][ar];
        let distance = d.length() + 1.0e-6;
        let fix = d * ((distance - SPACING) / distance * SPRING * 0.5);
        if !locked(ac, ar) {
            self.at[ac][ar] += fix;
        }
        if !locked(bc, br) {
            self.at[bc][br] -= fix;
        }
    }

    /// The held corners where they hang; nothing in front of the back.
    fn constrain(&mut self) {
        for c in 0..COLS {
            for r in 0..ROWS {
                if locked(c, r) {
                    self.at[c][r] = rest(c, r);
                    self.before[c][r] = rest(c, r);
                } else if self.at[c][r].z < BACK_PLANE {
                    self.at[c][r].z = BACK_PLANE;
                    self.before[c][r].z = self.before[c][r].z.max(BACK_PLANE);
                }
            }
        }
    }

    /// The cloth as drawn on the body's bone (pixels): each point, its normal (out), the cloth
    /// set out along it.
    fn surface(&self) -> ([[Vec3; ROWS]; COLS], [[Vec3; ROWS]; COLS]) {
        let mut at: [[Vec3; ROWS]; COLS] = std::array::from_fn(|c| {
            std::array::from_fn(|r| {
                let p = self.at[c][r];
                Vec3::new((p.x + 0.05) * X_SCALE, (p.y + 0.4) * Y_SCALE, p.z) + HUNG
            })
        });
        let normal: [[Vec3; ROWS]; COLS] = std::array::from_fn(|c| {
            std::array::from_fn(|r| {
                let across = at[(c + 1).min(COLS - 1)][r] - at[c.saturating_sub(1)][r];
                let down = at[c][(r + 1).min(ROWS - 1)] - at[c][r.saturating_sub(1)];
                across.cross(down).try_normalize().unwrap_or(Vec3::Z)
            })
        });
        for c in 0..COLS {
            for r in 0..ROWS {
                at[c][r] = (at[c][r] + normal[c][r] * NORMAL_OFFSET) * 16.0;
            }
        }
        (at, normal)
    }

    /// The cloth's mesh: its outside (the texture's left half), its inside (the right), and its
    /// four edges (the trim), each corner a side of the cloth.
    fn mesh(&self) -> Mesh {
        let (at, normal) = self.surface();
        let side =
            |c: usize, r: usize, s: f32| at[c][r] + normal[c][r] * (HALF_THICKNESS * 16.0 * s);
        let v = |r: usize| r as f32 / (ROWS - 1) as f32;
        let t = |c: usize| c as f32 / (COLS - 1) as f32;
        let corner = |p: Vec3, u: f32, v: f32| [p.x, p.y, p.z, u, v];
        let mut quads = Vec::with_capacity((COLS - 1) * (ROWS - 1) * 2 + 2 * (COLS + ROWS));
        for c in 0..COLS - 1 {
            for r in 0..ROWS - 1 {
                let n = (normal[c][r] + normal[c + 1][r] + normal[c + 1][r + 1] + normal[c][r + 1])
                    .normalize_or_zero();
                let cells = [(c, r), (c + 1, r), (c + 1, r + 1), (c, r + 1)];
                // Outside: seen from behind, the texture's left half runs the other way.
                quads.push(Quad::new(
                    n.to_array(),
                    cells.map(|(c, r)| corner(side(c, r, 1.0), 0.5 * (1.0 - t(c)), v(r))),
                ));
                quads.push(Quad::new(
                    (-n).to_array(),
                    cells.map(|(c, r)| corner(side(c, r, -1.0), 0.5 + 0.5 * t(c), v(r))),
                ));
            }
        }
        // The edges, all the way round.
        let mut rim: Vec<(usize, usize)> = (0..COLS).map(|c| (c, 0)).collect();
        rim.extend((1..ROWS).map(|r| (COLS - 1, r)));
        rim.extend((0..COLS - 1).rev().map(|c| (c, ROWS - 1)));
        rim.extend((1..ROWS - 1).rev().map(|r| (0, r)));
        let middle = rim.iter().map(|&(c, r)| at[c][r]).sum::<Vec3>() / rim.len() as f32;
        for (i, &(ac, ar)) in rim.iter().enumerate() {
            let (bc, br) = rim[(i + 1) % rim.len()];
            let (a, b) = (at[ac][ar], at[bc][br]);
            let mut n = (b - a)
                .cross(normal[ac][ar] + normal[bc][br])
                .normalize_or_zero();
            if n.dot((a + b) / 2.0 - middle) < 0.0 {
                n = -n;
            }
            let (va, vb) = (v(ar), v(br));
            quads.push(Quad::new(
                n.to_array(),
                [
                    corner(side(ac, ar, 1.0), 0.0, va),
                    corner(side(ac, ar, -1.0), 0.03, va),
                    corner(side(bc, br, -1.0), 0.03, vb),
                    corner(side(bc, br, 1.0), 0.0, vb),
                ],
            ));
        }
        Mesh::new(
            "oteria_cape".to_owned(),
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            quads,
            Vec::new(),
        )
    }
}

fn draw_cape(s: &mut State, style: &'static WearStyle, draw: &mut dyn PlayerDraw, ticks: f32) {
    let Some(uv) = draw.sprite(sprite_id(CAPE_SPRITE, &CAPES, style)) else {
        return;
    };
    let Some(body) = draw.bone(Bone::Body) else {
        return;
    };
    // In the world: the cloth moves, and its mesh is made again each step (the engine forgets
    // the one before once it is let go). In the inventory's picture: it hangs still.
    let mesh = if draw.pass() == Pass::World {
        let feet = DVec3::from_array(draw.motion().feet);
        let axes = Axes::of(Mat4::from_cols_array_2d(&body));
        let cloth = s.cloth.get_or_insert_with(|| Cloth::hanging(feet, ticks));
        let stepped = cloth.advance(feet, axes, ticks);
        if stepped || s.cape_mesh.is_none() {
            s.cape_mesh = Some(Arc::new(cloth.mesh()));
        }
        s.cape_mesh.clone().expect("made")
    } else {
        // In a picture: it flutters, as in a breeze, on the clock (a picture's model is still).
        static START: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);
        let now = START.elapsed().as_secs_f32() * 20.0;
        let cloth = s
            .still_cloth
            .get_or_insert_with(|| Cloth::hanging(DVec3::ZERO, now));
        if cloth.flutter(now, 1.0) || s.still_mesh.is_none() {
            s.still_mesh = Some(Arc::new(cloth.mesh()));
        }
        s.still_mesh.clone().expect("made")
    };
    draw.draw(&mesh, body, Look::new(uv).changing(), &mut |_| {});
}

// ---------------------------------------------------------------------------------------------
// The wings.

/// How the wings beat: their phase, how much the player is in the air (0 to 1), and the feet's
/// height and time a frame ago.
#[derive(Default)]
struct Flight {
    phase: f32,
    air: f32,
    last: Option<(f64, f32)>,
}

fn draw_wings(s: &mut State, style: &'static WearStyle, draw: &mut dyn PlayerDraw, ticks: f32) {
    let Some(uv) = draw.sprite(sprite_id(WINGS_SPRITE, &WINGS, style)) else {
        return;
    };
    let Some(body) = draw.bone(Bone::Body) else {
        return;
    };
    let motion = draw.motion();
    let f = &mut s.flight;
    if draw.pass() == Pass::World {
        let dt = f
            .last
            .map_or(0.0, |(_, t)| ((ticks - t) / 20.0).clamp(0.0, 0.25));
        let rising = f.last.map_or(0.0, |(y, _)| {
            if dt > 0.0 {
                (motion.feet[1] - y) / f64::from(dt)
            } else {
                0.0
            }
        });
        f.last = Some((motion.feet[1], ticks));
        // In the air: flying, or going up or down faster than a step.
        let aloft = motion.flying || rising.abs() > 2.5;
        let to = if aloft { 1.0 } else { 0.0 };
        f.air += (to - f.air) * (1.0 - (-dt * 4.0).exp());
        let beats = 0.5 + 1.9 * f.air;
        f.phase = (f.phase + dt * beats * TAU) % TAU;
    }
    let beat = f.phase.sin();
    let reach = 0.16 + 0.6 * f.air;
    let mesh = wings_mesh();
    draw.draw(mesh, body, Look::new(uv), &mut |p| wing_poses(beat, reach, p));
}

/// The wings' parts posed at `beat` (-1 to 1) of a beat reaching `reach`.
fn wing_poses(beat: f32, reach: f32, p: &mut dyn PartPoses) {
    for (side, sign) in [("l", 1.0f32), ("r", -1.0)] {
        let wing = p.pose(if side == "l" {
            "oteria_wing_l"
        } else {
            "oteria_wing_r"
        });
        // Spread back and up, beating forward and back.
        wing[4] = sign * (-0.55 + beat * reach);
        wing[5] = sign * (-0.32 - 0.18 * beat * reach);
        let tip = p.pose(if side == "l" {
            "oteria_wing_l_tip"
        } else {
            "oteria_wing_r_tip"
        });
        // The outer part follows a little late, folding on the way back.
        tip[4] = sign * (-0.2 - 0.45 * beat * reach);
    }
}

fn wings_mesh() -> &'static Arc<Mesh> {
    static MESH: OnceLock<Arc<Mesh>> = OnceLock::new();
    MESH.get_or_init(|| {
        let wing = |sign: f32, side: &str| {
            // Inner part: a bone along the top, the membrane under it; outer part the same.
            let part = |name: String,
                        at: [f32; 3],
                        len: f32,
                        drop: f32,
                        u: (f32, f32),
                        children: Vec<Mesh>| {
                let (x0, x1) = if sign > 0.0 { (0.0, len) } else { (-len, 0.0) };
                let mut quads = cuboid(
                    [x0, -0.9, -0.45],
                    [len, 1.0, 0.9],
                    [Some([0.0, 0.89, 1.0, 0.98]); 6],
                );
                // The membrane, both sides; its texture runs out from the body.
                let (ua, ub) = if sign > 0.0 { u } else { (u.1, u.0) };
                quads.extend(membrane(x0, x1, 0.0, drop, ua, ub, 0.875));
                Mesh::new(
                    name,
                    [at[0], at[1], at[2], 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                    quads,
                    children,
                )
            };
            let tip = part(
                format!("oteria_wing_{side}_tip"),
                [sign * 10.0, 0.0, 0.0],
                10.0,
                13.0,
                (0.5, 1.0),
                Vec::new(),
            );
            part(
                format!("oteria_wing_{side}"),
                [sign * 1.0, 0.0, 0.0],
                10.0,
                13.0,
                (0.0, 0.5),
                vec![tip],
            )
        };
        Arc::new(Mesh::new(
            "oteria_wings".into(),
            [0.0, 2.0, 2.6, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            Vec::new(),
            vec![wing(1.0, "l"), wing(-1.0, "r")],
        ))
    })
}

/// A flat quad from x0 to x1 and y 0 to `drop`, at z = 0, seen from both sides: u runs from
/// `ua` at x0 to `ub` at x1, v from 0 to `v1`.
fn membrane(x0: f32, x1: f32, y0: f32, drop: f32, ua: f32, ub: f32, v1: f32) -> [Quad; 2] {
    let y1 = y0 + drop;
    let back = Quad::new(
        [0.0, 0.0, 1.0],
        [
            [x0, y0, 0.0, ua, 0.0],
            [x1, y0, 0.0, ub, 0.0],
            [x1, y1, 0.0, ub, v1],
            [x0, y1, 0.0, ua, v1],
        ],
    );
    let front = Quad::new(
        [0.0, 0.0, -1.0],
        [
            [x1, y0, 0.0, ub, 0.0],
            [x0, y0, 0.0, ua, 0.0],
            [x0, y1, 0.0, ua, v1],
            [x1, y1, 0.0, ub, v1],
        ],
    );
    [back, front]
}

/// A box at `o` of `size` (pixels), its faces (in vanilla's order: up, down, west, north, east,
/// south, as `ModelPart.Cube` lists them) each textured by a part of the sprite (u0, v0, u1, v1)
/// or left out.
fn cuboid(o: [f32; 3], size: [f32; 3], faces: [Option<[f32; 4]>; 6]) -> Vec<Quad> {
    let [x0, y0, z0] = o;
    let (x1, y1, z1) = (x0 + size[0], y0 + size[1], z0 + size[2]);
    let t0 = [x0, y0, z0];
    let t1 = [x1, y0, z0];
    let t2 = [x1, y1, z0];
    let t3 = [x0, y1, z0];
    let l0 = [x0, y0, z1];
    let l1 = [x1, y0, z1];
    let l2 = [x1, y1, z1];
    let l3 = [x0, y1, z1];
    let all: [([[f32; 3]; 4], [f32; 3]); 6] = [
        ([l1, l0, t0, t1], [0.0, -1.0, 0.0]),
        ([t2, t3, l3, l2], [0.0, 1.0, 0.0]),
        ([t0, l0, l3, t3], [-1.0, 0.0, 0.0]),
        ([t1, t0, t3, t2], [0.0, 0.0, -1.0]),
        ([l1, t1, t2, l2], [1.0, 0.0, 0.0]),
        ([l0, l1, l2, l3], [0.0, 0.0, 1.0]),
    ];
    all.into_iter()
        .zip(faces)
        .filter_map(|((corners, normal), uv)| {
            let [a, b, c, e] = uv?;
            // As `ModelPart.Polygon` maps them: (u1, v0), (u0, v0), (u0, v1), (u1, v1).
            let uvs = [[c, b], [a, b], [a, e], [c, e]];
            let vertices = std::array::from_fn(|i| {
                let [x, y, z] = corners[i];
                [x, y, z, uvs[i][0], uvs[i][1]]
            });
            Some(Quad::new(normal, vertices))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The aura.

fn draw_aura(style: &'static WearStyle, draw: &mut dyn PlayerDraw, ticks: f32) {
    let Some(uv) = draw.sprite(sprite_id(AURA_SPRITE, &AURAS, style)) else {
        return;
    };
    let view = draw.view();
    let feet = (DVec3::from_array(draw.motion().feet) - DVec3::from_array(view.camera)).as_vec3();
    let (right, up) = (Vec3::from_array(view.right), Vec3::from_array(view.up));
    // In first person the eyes are inside the spiral: only what stays below them shows.
    let first_person = draw.pass() == Pass::FirstPerson;
    let top = if first_person { 0.9 } else { 2.25 };
    for (part, particles) in aura_particles(ticks / 20.0, feet, (right, up), top, true) {
        draw.particles(quarter(uv, part), &particles, Light::Full);
    }
}

/// The aura's sprite `uv`'s quarter `(qx, qy)` (each 0 or 0.5).
fn quarter(uv: [f32; 4], (qx, qy): (f32, f32)) -> [f32; 4] {
    let (w, h) = (uv[2] - uv[0], uv[3] - uv[1]);
    [
        uv[0] + w * qx,
        uv[1] + h * qy,
        uv[0] + w * (qx + 0.5),
        uv[1] + h * (qy + 0.5),
    ]
}

/// An aura at `t` (s) around `feet` (relative to the camera), its billboards facing the camera
/// (`right`, `up`), up to `top` over the feet: the motes, the sparkles and the ring, each with
/// the quarter of the sprite it is drawn from. `near`: what comes near the camera fades.
fn aura_particles(
    t: f32,
    feet: Vec3,
    (right, up): (Vec3, Vec3),
    top: f32,
    near: bool,
) -> [((f32, f32), Vec<Particle>); 3] {
    let billboard = |c: Vec3, size: f32, alpha: f32| {
        // Nothing in the camera's face: what comes near it fades away.
        let alpha = if near {
            alpha * ((c.length() - 0.45) / 0.6).clamp(0.0, 1.0)
        } else {
            alpha
        };
        let (r, u) = (right * size, up * size);
        Particle::new(
            [
                (c - r + u).to_array(),
                (c + r + u).to_array(),
                (c + r - u).to_array(),
                (c - r - u).to_array(),
            ],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            alpha,
        )
    };
    // The motes: rising in a spiral from the feet to over the head, fading in and out.
    const MOTES: usize = 22;
    let motes: Vec<Particle> = (0..MOTES)
        .map(|k| {
            let kf = k as f32;
            let p = (t * 0.22 + kf / MOTES as f32 + 0.37 * (kf * 1.7).sin().abs()).fract();
            let angle = kf * 2.399 + t * 1.1 + p * 4.0;
            let radius = 0.62 - 0.22 * p + 0.05 * (t * 2.0 + kf).sin();
            let c = feet
                + Vec3::new(
                    angle.cos() * radius,
                    0.1 + p * (top - 0.1),
                    angle.sin() * radius,
                );
            billboard(c, 0.09 + 0.05 * (1.0 - p), (p * PI).sin().powf(0.8) * 0.9)
        })
        .collect();
    // Sparkles: flaring where they are, here and there around the body.
    let sparkles: Vec<Particle> = (0..7)
        .filter_map(|k| {
            let kf = k as f32;
            let cycle = 1.6 + 0.4 * (kf * 0.9).sin().abs();
            let n = ((t + kf * 0.73) / cycle).floor();
            let life = ((t + kf * 0.73) / cycle).fract();
            let flare = (life * PI).sin().powi(3);
            (flare > 0.02).then(|| {
                let seed = n * 12.9898 + kf * 78.233;
                let h = |o: f32| ((seed + o).sin() * 43758.547).fract().abs();
                let angle = h(1.0) * TAU;
                let c = feet
                    + Vec3::new(
                        angle.cos() * (0.45 + 0.3 * h(2.0)),
                        0.3 + (top - 0.6) * h(3.0),
                        angle.sin() * (0.45 + 0.3 * h(2.0)),
                    );
                billboard(c, 0.2 * flare, flare)
            })
        })
        .collect();
    // The ring at the feet, lying on the ground, turning and breathing.
    let spin = t * 0.6;
    let size = 0.95 + 0.06 * (t * 2.2).sin();
    let (a, b) = (
        Vec3::new(spin.cos(), 0.0, spin.sin()) * size,
        Vec3::new(-spin.sin(), 0.0, spin.cos()) * size,
    );
    let c = feet + Vec3::new(0.0, 0.03, 0.0);
    let ring = Particle::new(
        [
            (c - a - b).to_array(),
            (c + a - b).to_array(),
            (c + a + b).to_array(),
            (c - a + b).to_array(),
        ],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        0.55 + 0.15 * (t * 2.2).sin(),
    );
    [
        ((0.0, 0.0), motes),
        ((0.5, 0.0), sparkles),
        ((0.0, 0.5), vec![ring]),
    ]
}

// ---------------------------------------------------------------------------------------------
// The pictures, 64×64 RGBA, drawn in code.

fn rgb(c: u32) -> [f32; 3] {
    [
        ((c >> 16) & 0xFF) as f32 / 255.0,
        ((c >> 8) & 0xFF) as f32 / 255.0,
        (c & 0xFF) as f32 / 255.0,
    ]
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t.clamp(0.0, 1.0))
}
fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    a.map(|v| v * k)
}
/// A texel's own small variation, -1 to 1.
fn grain(x: usize, y: usize) -> f32 {
    let h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h >> 8) & 0xFFFF) as f32 / 32_768.0 - 1.0
}
fn image(mut texel: impl FnMut(usize, usize) -> ([f32; 3], f32)) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let (c, a) = texel(x, y);
            out.extend(c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
            out.push((a.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    out
}

/// The cape: its outside on the left half (a fall of colour, a trim in its light, an emblem),
/// its inside, darker, on the right.
fn cape_texture(style: &WearStyle) -> Vec<u8> {
    let (main, light) = (rgb(style.main), rgb(style.light));
    image(|x, y| {
        let g = grain(x, y) * 0.05;
        if x >= 32 {
            return (scale(main, 0.42 + g), 1.0);
        }
        let fy = y as f32 / 63.0;
        // Folds down the cloth and a fall from bright to deep.
        let fold = (x as f32 * 0.55).sin() * 0.05;
        let mut c = scale(main, 1.08 - 0.38 * fy + fold + g);
        // The trim: down the sides and along the bottom.
        if !(2..=29).contains(&x) || !(1..=60).contains(&y) {
            c = scale(light, 0.92 + g);
        } else if x == 2 || x == 29 || y == 60 {
            c = scale(main, 0.6);
        }
        // The emblem: a diamond in a diamond, a third of the way down.
        let (dx, dy) = ((x as f32 - 15.5).abs(), (y as f32 - 19.5).abs());
        let d = dx + dy;
        if (7.0..8.6).contains(&d) || d < 3.2 {
            c = mix(c, light, 0.95);
        } else if d < 7.0 {
            c = mix(c, scale(main, 0.7), 0.6);
        }
        (c, 1.0)
    })
}

/// A wing: its membrane over the top 56 rows (the inner part on the left half, the outer on the
/// right), its edge cut in scallops or feathers, its veins; the bones along the bottom rows.
fn wings_texture(style: &WearStyle) -> Vec<u8> {
    let (main, light) = (rgb(style.main), rgb(style.light));
    let feathered = style.id == "angel";
    image(|x, y| {
        if y >= 56 {
            return (
                mix(
                    scale(main, 0.55),
                    light,
                    if y % 4 == 1 { 0.6 } else { 0.25 },
                ),
                1.0,
            );
        }
        let (fx, fy) = (x as f32, y as f32);
        // How far down the membrane reaches here: deep near the body, a point at the tip.
        let reach = 54.0 * (1.0 - (fx / 64.0).powf(1.6)) + 2.0;
        let edge = if feathered {
            reach - 6.0 * ((fx / 64.0 * 9.0 * PI).sin().abs())
        } else {
            reach - 7.0 * ((fx / 64.0 * 4.0 * PI).sin().abs())
        };
        if fy > edge {
            return ([0.0; 3], 0.0);
        }
        let g = grain(x, y) * 0.04;
        let depth = fy / edge.max(1.0);
        let mut c = scale(
            mix(main, light, 0.15 + 0.25 * (1.0 - fx / 64.0)),
            1.0 - 0.3 * depth + g,
        );
        if feathered {
            // Rows of feathers, each a little lighter at its tip.
            let row = (fy + fx * 0.35) % 9.0;
            c = mix(c, light, 0.15 + 0.35 * (row / 9.0));
        } else {
            // Veins, fanning out from the shoulder.
            for k in 0..5 {
                let ang = 0.2 + k as f32 * 0.28;
                let dist = (fx * ang.sin() - fy * ang.cos()).abs();
                if dist < 0.9 && fx + fy > 4.0 {
                    c = mix(c, light, 0.55);
                }
            }
        }
        // The edge glows a little in the light colour.
        if edge - fy < 1.6 {
            c = mix(c, light, 0.7);
        }
        (c, 1.0)
    })
}

/// The aura's sprites, a quarter each: a glowing mote, a sparkle, and a ring.
fn aura_texture(style: &WearStyle) -> Vec<u8> {
    let (main, light) = (rgb(style.main), rgb(style.light));
    let white = [1.0; 3];
    image(|x, y| {
        let (qx, qy) = (x / 32, y / 32);
        let (lx, ly) = ((x % 32) as f32 - 15.5, (y % 32) as f32 - 15.5);
        let r = lx.hypot(ly);
        // Nothing reaches a quarter's edge: no square shows around a sprite.
        let inside = ((15.0 - r) / 3.0).clamp(0.0, 1.0);
        let (c, a) = match (qx, qy) {
            (0, 0) => {
                let core = (-(r * r) / 18.0).exp();
                let glow = (-(r * r) / 90.0).exp();
                (
                    mix(mix(main, light, glow), white, core),
                    (glow * 0.85 + core).min(1.0),
                )
            }
            (1, 0) => {
                let (ax, ay) = (lx.abs() / 15.5, ly.abs() / 15.5);
                let rays =
                    (-ax * 14.0).exp() * (-ay * 2.2).exp() + (-ay * 14.0).exp() * (-ax * 2.2).exp();
                let core = (-(r * r) / 6.0).exp();
                let glow = (-r / 6.0).exp() * 0.4;
                let a = (rays + core + glow).min(1.0);
                (mix(light, white, core + rays * 0.4), a)
            }
            (0, 1) => {
                let band = (-((r - 12.5) / 1.6).powi(2)).exp();
                let haze = (-((r - 11.0) / 4.5).powi(2)).exp() * 0.35;
                (mix(main, light, band), (band + haze).min(1.0))
            }
            _ => ([0.0; 3], 0.0),
        };
        (c, a * inside)
    })
}

// ---------------------------------------------------------------------------------------------
// The Cosmetics page: a wearable alone on its card, and the aura around the preview's player.

/// Full light, packed as the vertices carry it.
const FULL_LIGHT: u32 = 240 | 240 << 8;

/// A cape or wings alone, as the Cosmetics page's cards show them, at `clock` (s): their
/// vertices (blocks), the body's space (x to the body's left, y down, z out of the back) taken by
/// `base`, drawn from the style's sprite `uv`. The cape hangs in the lightest breeze; the wings
/// beat slowly.
pub(super) fn showcase(
    kind: WearKind,
    uv: [f32; 4],
    base: Mat4,
    clock: f32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    use horizon_client::mobs::render_model;
    match kind {
        WearKind::Cape => {
            let mesh = {
                let mut guard = state();
                let s = &mut *guard;
                let ticks = clock * 20.0;
                let cloth = s
                    .card_cloth
                    .get_or_insert_with(|| Cloth::hanging(DVec3::ZERO, ticks));
                if cloth.flutter(ticks, 0.22) || s.card_mesh.is_none() {
                    s.card_mesh = Some(Arc::new(cloth.mesh()));
                }
                s.card_mesh.clone().expect("made")
            };
            let part = super::player_hooks::part_of(&mesh);
            render_model(&part, base, &|p| p.pose, uv, FULL_LIGHT, 0, out);
        }
        WearKind::Wings => {
            let part = super::player_hooks::part_of(wings_mesh());
            let mut poses = horizon_client::mobs::anim::Poses::pooled();
            poses.reset(&part);
            let beat = (clock * 0.7 * TAU).sin();
            wing_poses(beat, 0.22, &mut Posing(&mut poses));
            render_model(&part, base, &|p| poses.pose(p), uv, FULL_LIGHT, 0, out);
        }
        WearKind::Aura => {}
    }
}

/// The game's poses as a mesh's parts are posed.
struct Posing<'a>(&'a mut horizon_client::mobs::anim::Poses);

impl PartPoses for Posing<'_> {
    fn pose(&mut self, name: &str) -> &mut [f32; 9] {
        self.0.get(name)
    }
}

/// An aura at `clock` (s) around `feet`, as a picture shows it: its billboards facing a camera
/// whose right and up are `right` and `up` (in the space of `feet`), unlit (their normals
/// nothing), each its alpha; up to `top` blocks over the feet. From the style's sprite `uv`.
pub(super) fn aura_vertices(
    uv: [f32; 4],
    clock: f32,
    feet: Vec3,
    (right, up): (Vec3, Vec3),
    top: f32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    for (part, particles) in aura_particles(clock, feet, (right, up), top, false) {
        let uv = quarter(uv, part);
        for q in &particles {
            let color = ((q.alpha.clamp(0.0, 1.0) * 255.0) as u32) << 24 | 0x00FF_FFFF;
            for (corner, [u, v]) in q.corners.iter().zip(q.uv) {
                out.push(horizon_client::mesher::Vertex {
                    pos: *corner,
                    uv: [uv[0] + (uv[2] - uv[0]) * u, uv[1] + (uv[3] - uv[1]) * v],
                    layer: 0,
                    color,
                    light: FULL_LIGHT,
                    normal: [0, 0, 0, 0],
                    glint_uv: [0.0, 0.0],
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes() -> Axes {
        // Standing, facing -z: back +z.
        Axes { z: DVec3::Z }
    }

    fn bottom_out(cloth: &Cloth) -> f32 {
        (0..COLS).map(|c| cloth.at[c][ROWS - 1].z).sum::<f32>() / COLS as f32
    }

    #[test]
    fn the_cape_hangs_still_and_whole() {
        let feet = DVec3::new(0.5, 64.0, 0.5);
        let mut cloth = Cloth::hanging(feet, 0.0);
        for k in 1..=200 {
            cloth.advance(feet, axes(), k as f32);
        }
        // Down its length, flat on the back, never in front of it, each spring near its length.
        assert!(
            bottom_out(&cloth).abs() < 0.02,
            "flat: {}",
            bottom_out(&cloth)
        );
        assert!(cloth.at.iter().flatten().all(|p| p.z >= BACK_PLANE - 1e-6));
        assert!((cloth.at[2][ROWS - 1].y - rest(2, ROWS - 1).y).abs() < 0.03);
        for c in 0..COLS {
            for r in 1..ROWS {
                let d = cloth.at[c][r].distance(cloth.at[c][r - 1]);
                assert!((d - SPACING).abs() < 0.02, "spring {c},{r}: {d}");
            }
        }
    }

    #[test]
    fn running_lifts_the_cape_behind_and_backing_up_does_not() {
        let run = |dz: f64| {
            let mut feet = DVec3::new(0.5, 64.0, 0.5);
            let mut cloth = Cloth::hanging(feet, 0.0);
            for k in 1..=60 {
                feet.z += dz;
                cloth.advance(feet, axes(), k as f32);
            }
            bottom_out(&cloth)
        };
        // A sprint forward (-z, away from the back), then backing up.
        let forward = run(-0.28);
        assert!(forward > 0.15, "flies out behind: {forward}");
        assert!(run(0.2) < 0.02, "lies on the back");
    }

    #[test]
    fn falling_lifts_it() {
        let mut feet = DVec3::new(0.5, 90.0, 0.5);
        let mut cloth = Cloth::hanging(feet, 0.0);
        for k in 1..=30 {
            feet.y -= 0.5;
            cloth.advance(feet, axes(), k as f32);
        }
        let middle = cloth.at[2][ROWS - 1];
        assert!(middle.y < rest(2, ROWS - 1).y - 0.1, "lifted: {middle}");
    }

    #[test]
    fn a_teleport_hangs_it_again() {
        let feet = DVec3::new(0.5, 64.0, 0.5);
        let mut cloth = Cloth::hanging(feet, 0.0);
        cloth.at[2][ROWS - 1].z = 0.5;
        cloth.advance(feet + DVec3::new(100.0, 0.0, 0.0), axes(), 1.0);
        assert_eq!(cloth.at[2][ROWS - 1], rest(2, ROWS - 1));
    }

    #[test]
    fn the_cape_mesh_is_whole() {
        let mesh = Cloth::hanging(DVec3::ZERO, 0.0).mesh();
        let rim = 2 * (COLS - 1) + 2 * (ROWS - 1);
        assert_eq!(mesh.quads.len(), (COLS - 1) * (ROWS - 1) * 2 + rim);
        assert!(
            mesh.quads
                .iter()
                .flat_map(|q| q.vertices)
                .flatten()
                .all(f32::is_finite)
        );
        // 10 pixels wide, 16 down, behind the back (z 2).
        let xs: Vec<f32> = mesh
            .quads
            .iter()
            .flat_map(|q| q.vertices)
            .map(|v| v[0])
            .collect();
        let width = xs.iter().cloned().fold(f32::MIN, f32::max)
            - xs.iter().cloned().fold(f32::MAX, f32::min);
        assert!((width - 10.0).abs() < 0.5, "{width}");
        assert!(
            mesh.quads
                .iter()
                .flat_map(|q| q.vertices)
                .all(|v| v[2] > 2.0)
        );
    }

    #[test]
    fn the_pictures_are_whole_and_their_cut_outs_clear() {
        for c in &CAPES {
            assert_eq!(cape_texture(c).len(), 64 * 64 * 4);
        }
        for w in &WINGS {
            let px = wings_texture(w);
            assert_eq!(px.len(), 64 * 64 * 4);
            // The tip's corner is cut out, the shoulder's is not.
            assert_eq!(px[(50 * 64 + 63) * 4 + 3], 0);
            assert_eq!(px[(2 * 64 + 2) * 4 + 3], 255);
        }
        for a in &AURAS {
            let px = aura_texture(a);
            assert_eq!(px[(16 * 64 + 16) * 4 + 3], 255, "the mote's middle");
            assert_eq!(px[3], 0, "its corner");
        }
    }

    #[test]
    fn the_meshes_have_their_named_parts() {
        fn names(m: &Mesh, out: &mut Vec<String>) {
            out.push(m.name.clone());
            m.children.iter().for_each(|c| names(c, out));
        }
        let mut wings = Vec::new();
        names(wings_mesh(), &mut wings);
        for n in [
            "oteria_wing_l",
            "oteria_wing_r",
            "oteria_wing_l_tip",
            "oteria_wing_r_tip",
        ] {
            assert!(wings.iter().any(|w| w == n), "{n}");
        }
    }
}
