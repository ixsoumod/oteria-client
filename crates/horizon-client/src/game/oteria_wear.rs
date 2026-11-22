//! Oteria's own wearables (the Cosmetics page's Capes, Hats, Pets and Auras), drawn on the
//! window's own player through the player hooks (`horizon_hooks::Hooks::player_layers`), seen
//! from outside (third person, the inventory, the Cosmetics page); none in first person:
//!
//! - **A cape** that is cloth, simulated as Cloth-Cloaks (pheological, CC0) does: a grid of
//!   points held at its top corners, springs between them, gravity, a push out behind as the
//!   player runs and a lift as they fall, and the back it can't pass through. Its texture is a
//!   Minecraft cape's (64x32, its frames stacked when it is animated), so the elytra worn instead
//!   of it takes the same one (`perspective::player_elytra_geometry`). A Mojang cape on the
//!   player (Migrator...) becomes Oteria's cape.
//! - **A hat** on the head and **a pet** on the left shoulder (as a parrot sits): Java block
//!   models (Blockbench, as Cosmetica serves them) made into meshes once. A pet breathes and
//!   looks about; over a helmet a hat grows to sit on it.
//! - **An aura**: motes rising in a spiral around the player, sparkles, and a ring at the feet,
//!   glowing; drawn in code.
//!
//! Only this window's player wears them; nothing is sent anywhere.

use super::oteria_cosmetics::WearKind;
use glam::{DVec3, Mat4, Quat, Vec3};
use horizon_client::oteria_art::{self, AURAS, CAPES, HATS, PETS, WearStyle, wear_style};
use horizon_hooks::players::{
    Bone, Frame, Kind, Light, Look, Mesh, Particle, Pass, PlayerDraw, PlayerModel, Quad,
    SpriteStore,
};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

/// The hooks Oteria's wearables draw through (`player_hooks::register`).
pub(super) struct Wear;
pub(super) static HOOKS: Wear = Wear;

/// What is worn, and what moves between frames.
#[derive(Default)]
struct State {
    cape: Option<&'static WearStyle>,
    hat: Option<&'static WearStyle>,
    pet: Option<&'static WearStyle>,
    aura: Option<&'static WearStyle>,
    /// Each textured wearable's whole texture in the GUI atlas (u0, v0, u1, v1), its frames
    /// stacked top to bottom.
    sheets: HashMap<&'static str, [f32; 4]>,
    cloth: Option<Cloth>,
    /// The cloth's mesh as it last stepped.
    cape_mesh: Option<Arc<Mesh>>,
    /// The wearable tried on in the Cosmetics page's preview (its pictures only), and the last
    /// aura tried, whose sprite stays stored.
    trying: Option<(WearKind, &'static WearStyle)>,
    tried_aura: Option<&'static WearStyle>,
    /// The cloth in the pictures (the preview, the inventory): fluttering as in a breeze.
    still_cloth: Option<Cloth>,
    still_mesh: Option<Arc<Mesh>>,
    /// The cloth the Cosmetics page's cards show, alone, in the lightest breeze.
    card_cloth: Option<Cloth>,
    card_mesh: Option<Arc<Mesh>>,
    /// The Cosmetics page shows: every aura's sprite is kept.
    showing: bool,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);
fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The clock animated textures and pets go by (s).
fn clock() -> f32 {
    static START: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);
    START.elapsed().as_secs_f32()
}

/// The textured wearables and their sprites' names, made once.
fn textured() -> &'static [(&'static WearStyle, String)] {
    static ALL: LazyLock<Vec<(&'static WearStyle, String)>> = LazyLock::new(|| {
        CAPES
            .iter()
            .chain(HATS.iter())
            .chain(PETS.iter())
            .filter(|s| oteria_art::wear_available(s))
            .map(|s| (s, oteria_art::wear_sprite(s.id)))
            .collect()
    });
    &ALL
}

/// What the player wears now, from their settings (each frame): `mojang_cape` when the
/// account's cape is on them (it becomes Oteria's), `sheet` where a sprite is in the atlas.
pub(super) fn sync(
    skins: &horizon_client::pvp::Skins,
    mojang_cape: bool,
    sheet: &dyn Fn(&str) -> Option<[f32; 4]>,
) {
    let mut s = state();
    let there = |all: &'static [WearStyle], id: &str| {
        wear_style(all, id).filter(|w| oteria_art::wear_available(w))
    };
    s.cape = there(&CAPES, &skins.cape).or_else(|| mojang_cape.then_some(&CAPES[0]));
    s.hat = there(&HATS, &skins.hat);
    s.pet = there(&PETS, &skins.pet);
    s.aura = wear_style(&AURAS, &skins.aura);
    if s.cape.is_none() {
        s.cloth = None;
        s.cape_mesh = None;
    }
    // The atlas is made again with the resource packs: where each texture is, each frame.
    for (style, sprite) in textured() {
        match sheet(sprite) {
            Some(r) => s.sheets.insert(style.id, r),
            None => s.sheets.remove(style.id),
        };
    }
}

/// The Cosmetics page's preview tries `trying` on (a cape, hat, pet or aura, by its id) over
/// what is worn, in the pictures only; None: what is worn.
pub(super) fn try_on(trying: Option<(WearKind, &str)>) {
    let mut s = state();
    s.trying = trying.and_then(|(kind, id)| Some((kind, wear_style(kind.styles(), id)?)));
    if let Some((WearKind::Aura, style)) = s.trying {
        s.tried_aura = Some(style);
    }
}

/// The Cosmetics page shows (each frame it does): every aura's picture is kept in the atlas,
/// for its card.
pub(super) fn showing(on: bool) {
    state().showing = on;
}

/// The aura sprites' ids: a kind and the style's place in its list.
const SPRITE: u64 = 0x07E7_1A00_0000_0000;
const AURA_SPRITE: u64 = 3;
pub(super) fn aura_sprite(style: &WearStyle) -> u64 {
    let i = AURAS.iter().position(|s| s.id == style.id).unwrap_or(0) as u64;
    SPRITE | AURA_SPRITE << 8 | i
}

/// `sheet`'s frame of `style` at `at` (s).
fn frame_uv(sheet: [f32; 4], style: &WearStyle, at: f32) -> [f32; 4] {
    let n = style.frames.max(1);
    let f = (at / oteria_art::FRAME_SECONDS) as u32 % n;
    let h = (sheet[3] - sheet[1]) / n as f32;
    [
        sheet[0],
        sheet[1] + h * f as f32,
        sheet[2],
        sheet[1] + h * (f + 1) as f32,
    ]
}

/// A textured wearable's texture now (its frame), for the Cosmetics page.
pub(super) fn uv_now(style: &WearStyle) -> Option<[f32; 4]> {
    let sheet = *state().sheets.get(style.id)?;
    Some(frame_uv(sheet, style, clock()))
}

/// The window's player wears an Oteria cape (or a Mojang one that became Oteria's): the
/// vanilla cape is not drawn.
pub(super) fn own_cape_worn() -> bool {
    state().cape.is_some()
}

/// The window's player's cape now as their elytra takes it: its frame's place in the atlas (a
/// cape's 64x32, as a vanilla cape's sprite is: `store_skin`). While the Cosmetics page's
/// preview draws (`try_on`), the cape tried on.
pub(super) fn own_elytra_uv() -> Option<[f32; 4]> {
    let s = state();
    let cape = match s.trying {
        Some((WearKind::Cape, tried)) => tried,
        _ => s.cape?,
    };
    Some(frame_uv(*s.sheets.get(cape.id)?, cape, clock()))
}

impl horizon_hooks::Hooks for Wear {
    fn frame(&self, _frame: &Frame, sprites: &mut dyn SpriteStore) {
        let (aura, tried, showing) = {
            let s = state();
            (s.aura, s.tried_aura, s.showing)
        };
        let mut put = |a: &WearStyle| {
            let id = aura_sprite(a);
            if !sprites.has(id) {
                sprites.put(id, aura_texture(a));
            }
        };
        aura.into_iter().chain(tried).for_each(&mut put);
        if showing {
            AURAS.iter().for_each(put);
        }
    }

    fn player_layers(&self, model: &PlayerModel, draw: &mut dyn PlayerDraw) {
        if model.kind != Kind::Own || model.invisible {
            return;
        }
        let mut s = state();
        let ticks = draw.ticks();
        // In a picture, what the preview tries on in its place.
        let picture = draw.pass() != Pass::World;
        let trying = s.trying.filter(|_| picture);
        let worn = |kind: WearKind, worn: Option<&'static WearStyle>| match trying {
            Some((k, style)) if k == kind => Some(style),
            _ => worn,
        };
        let (cape, hat, pet) = (
            worn(WearKind::Cape, s.cape),
            worn(WearKind::Hat, s.hat),
            worn(WearKind::Pet, s.pet),
        );
        // Under an elytra no cape hangs, as vanilla's: the elytra wears its texture.
        if let Some(style) = cape.filter(|_| !model.gear.elytra) {
            draw_cape(&mut s, style, draw, ticks);
        }
        if let Some(style) = hat {
            draw_model(&s, WearKind::Hat, style, model.gear.head, draw);
        }
        if let Some(style) = pet {
            draw_model(&s, WearKind::Pet, style, model.gear.chest, draw);
        }
        if let (Some(style), Pass::World) = (s.aura, draw.pass()) {
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

    /// The cloth's mesh, on a Minecraft cape's texture (64x32): its outside ((1,1)-(11,17)), its
    /// inside ((12,1)-(22,17)), and its four edges (the side, (0,1)-(1,17)), each corner a side
    /// of the cloth.
    fn mesh(&self) -> Mesh {
        let (at, normal) = self.surface();
        let side =
            |c: usize, r: usize, s: f32| at[c][r] + normal[c][r] * (HALF_THICKNESS * 16.0 * s);
        let v = |r: usize| (1.0 + 16.0 * r as f32 / (ROWS - 1) as f32) / 32.0;
        let t = |c: usize| c as f32 / (COLS - 1) as f32;
        let corner = |p: Vec3, u: f32, v: f32| [p.x, p.y, p.z, u, v];
        let mut quads = Vec::with_capacity((COLS - 1) * (ROWS - 1) * 2 + 2 * (COLS + ROWS));
        for c in 0..COLS - 1 {
            for r in 0..ROWS - 1 {
                let n = (normal[c][r] + normal[c + 1][r] + normal[c + 1][r + 1] + normal[c][r + 1])
                    .normalize_or_zero();
                let cells = [(c, r), (c + 1, r), (c + 1, r + 1), (c, r + 1)];
                // Outside: seen from behind, its texture runs the other way.
                quads.push(Quad::new(
                    n.to_array(),
                    cells.map(|(c, r)| {
                        corner(side(c, r, 1.0), (1.0 + 10.0 * (1.0 - t(c))) / 64.0, v(r))
                    }),
                ));
                quads.push(Quad::new(
                    (-n).to_array(),
                    cells.map(|(c, r)| corner(side(c, r, -1.0), (12.0 + 10.0 * t(c)) / 64.0, v(r))),
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
                    corner(side(ac, ar, -1.0), 1.0 / 64.0, va),
                    corner(side(bc, br, -1.0), 1.0 / 64.0, vb),
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
    let Some(&sheet) = s.sheets.get(style.id) else {
        return;
    };
    let uv = frame_uv(sheet, style, clock());
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
        let now = clock() * 20.0;
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
// Hats and pets: Java block models.

/// The part a hat's or pet's mesh is, posed by name.
fn part_name(kind: WearKind) -> &'static str {
    match kind {
        WearKind::Hat => "oteria_hat",
        _ => "oteria_pet",
    }
}

/// Where a model's part turns, on its bone (pixels, y down): a hat about the head's middle, a
/// pet on top of the left shoulder.
fn pivot(kind: WearKind) -> Vec3 {
    match kind {
        WearKind::Hat => Vec3::new(0.0, -4.0, 0.0),
        _ => Vec3::new(6.0, 0.0, 0.0),
    }
}

/// A hat's or pet's mesh, made once from its model (None: no model, or not one that reads).
fn model_mesh(kind: WearKind, style: &'static WearStyle) -> Option<Arc<Mesh>> {
    static MESHES: LazyLock<Mutex<HashMap<&'static str, Option<Arc<Mesh>>>>> =
        LazyLock::new(Default::default);
    let mut meshes = MESHES.lock().unwrap_or_else(PoisonError::into_inner);
    meshes
        .entry(style.id)
        .or_insert_with(|| {
            let json = oteria_art::wear_bytes(style.id, "json")?;
            let quads = block_model(&json, kind)?;
            let p = pivot(kind);
            Some(Arc::new(Mesh::new(
                part_name(kind).to_owned(),
                [p.x, p.y, p.z, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                quads,
                Vec::new(),
            )))
        })
        .clone()
}

/// A Java block model's faces (`elements`: boxes `from` `to` in pixels, y up, its front to the
/// north, each turned by its `rotation`, each face's `uv` in sixteenths of the texture, turned by
/// its own `rotation`), set on the bone about `pivot(kind)`. As Cosmetica places them: a hat's
/// head is the box (4, -8, 4) to (12, 0, 12); a pet sits at (8, 0, 8) on the shoulder. The bone's
/// x runs to the body's left and its y down: the model's x and y turn round.
fn block_model(json: &[u8], kind: WearKind) -> Option<Vec<Quad>> {
    let model: serde_json::Value = serde_json::from_slice(json).ok()?;
    let lift = match kind {
        WearKind::Hat => -8.0,
        _ => 0.0,
    };
    let p = pivot(kind);
    let place = |v: Vec3| Vec3::new(8.0 - v.x, lift - v.y, v.z - 8.0) - Vec3::new(0.0, p.y, 0.0);
    let turn_normal = |n: Vec3| Vec3::new(-n.x, -n.y, n.z);
    let three = |v: &serde_json::Value| -> Option<Vec3> {
        let a = v.as_array()?;
        Some(Vec3::new(
            a.first()?.as_f64()? as f32,
            a.get(1)?.as_f64()? as f32,
            a.get(2)?.as_f64()? as f32,
        ))
    };
    let mut quads = Vec::new();
    for e in model.get("elements")?.as_array()? {
        let (Some(from), Some(to)) = (three(&e["from"]), three(&e["to"])) else {
            continue;
        };
        // The box's turn: about an axis through `origin`, rescaled to keep its span if asked.
        let turn = e.get("rotation").and_then(|r| {
            let origin = three(&r["origin"]).unwrap_or(Vec3::splat(8.0));
            let angle = (r["angle"].as_f64()? as f32).to_radians();
            let (q, stretch) = match r["axis"].as_str()? {
                "x" => (Quat::from_rotation_x(angle), Vec3::new(1.0, 0.0, 0.0)),
                "y" => (Quat::from_rotation_y(angle), Vec3::new(0.0, 1.0, 0.0)),
                _ => (Quat::from_rotation_z(angle), Vec3::new(0.0, 0.0, 1.0)),
            };
            let rescale = r["rescale"].as_bool().unwrap_or(false);
            let k = if rescale { 1.0 / angle.cos() } else { 1.0 };
            let scale = stretch + (Vec3::ONE - stretch) * k;
            Some((origin, q, scale))
        });
        let at = |v: Vec3| match turn {
            Some((o, q, scale)) => place(o + q * ((v - o) * scale)),
            None => place(v),
        };
        let normal = |n: Vec3| turn_normal(turn.map_or(n, |(_, q, _)| q * n));
        let (a, b) = (from, to);
        let Some(faces) = e.get("faces").and_then(|f| f.as_object()) else {
            continue;
        };
        for (dir, face) in faces {
            // Its corners as seen from outside: top left, top right, bottom right, bottom left.
            let (corners, n) = match dir.as_str() {
                "north" => (
                    [[b.x, b.y, a.z], [a.x, b.y, a.z], [a.x, a.y, a.z], [b.x, a.y, a.z]],
                    Vec3::NEG_Z,
                ),
                "south" => (
                    [[a.x, b.y, b.z], [b.x, b.y, b.z], [b.x, a.y, b.z], [a.x, a.y, b.z]],
                    Vec3::Z,
                ),
                "east" => (
                    [[b.x, b.y, b.z], [b.x, b.y, a.z], [b.x, a.y, a.z], [b.x, a.y, b.z]],
                    Vec3::X,
                ),
                "west" => (
                    [[a.x, b.y, a.z], [a.x, b.y, b.z], [a.x, a.y, b.z], [a.x, a.y, a.z]],
                    Vec3::NEG_X,
                ),
                "up" => (
                    [[a.x, b.y, a.z], [b.x, b.y, a.z], [b.x, b.y, b.z], [a.x, b.y, b.z]],
                    Vec3::Y,
                ),
                "down" => (
                    [[a.x, a.y, b.z], [b.x, a.y, b.z], [b.x, a.y, a.z], [a.x, a.y, a.z]],
                    Vec3::NEG_Y,
                ),
                _ => continue,
            };
            let uv: Vec<f32> = face["uv"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_f64()).map(|v| v as f32 / 16.0).collect())
                .unwrap_or_default();
            let [u0, v0, u1, v1] = uv[..] else {
                continue;
            };
            let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
            // A face's texture turned clockwise a quarter at a time (`BlockFaceUV`).
            let quarter = (face["rotation"].as_f64().unwrap_or(0.0) / 90.0).round() as usize % 4;
            let vertices: [[f32; 5]; 4] = std::array::from_fn(|i| {
                let p = at(Vec3::from_array(corners[i]));
                let [u, v] = uvs[(i + 4 - quarter) % 4];
                [p.x, p.y, p.z, u, v]
            });
            quads.push(Quad::new(normal(n).to_array(), vertices));
        }
    }
    (!quads.is_empty()).then_some(quads)
}

/// A hat's or pet's part posed at `t` (s): a pet breathes and looks about, lifted over a
/// chestplate (`covered`); a hat grows over a helmet to sit on it.
fn pose_model(kind: WearKind, covered: bool, t: f32, pose: &mut [f32; 9]) {
    match kind {
        WearKind::Hat => {
            if covered {
                pose[6..9].copy_from_slice(&[1.25; 3]);
            }
        }
        _ => {
            let breath = (t * 2.4).sin();
            pose[1] -= 0.25 + 0.25 * breath + if covered { 1.0 } else { 0.0 };
            pose[4] = (t * 0.45).sin() * 0.22 + (t * 1.3).sin() * 0.04;
            pose[3] = (t * 0.7).sin() * 0.04;
            pose[7] = 1.0 + 0.025 * breath;
        }
    }
}

fn draw_model(
    s: &State,
    kind: WearKind,
    style: &'static WearStyle,
    covered: bool,
    draw: &mut dyn PlayerDraw,
) {
    let Some(&sheet) = s.sheets.get(style.id) else {
        return;
    };
    let Some(mesh) = model_mesh(kind, style) else {
        return;
    };
    let bone = if kind == WearKind::Hat { Bone::Head } else { Bone::Body };
    let Some(on) = draw.bone(bone) else {
        return;
    };
    let t = clock();
    let name = part_name(kind);
    draw.draw(
        &mesh,
        on,
        Look::new(frame_uv(sheet, style, t)),
        &mut |p| pose_model(kind, covered, t, p.pose(name)),
    );
}

// ---------------------------------------------------------------------------------------------
// The aura.

fn draw_aura(style: &'static WearStyle, draw: &mut dyn PlayerDraw, ticks: f32) {
    let Some(uv) = draw.sprite(aura_sprite(style)) else {
        return;
    };
    let view = draw.view();
    let feet = (DVec3::from_array(draw.motion().feet) - DVec3::from_array(view.camera)).as_vec3();
    let (right, up) = (Vec3::from_array(view.right), Vec3::from_array(view.up));
    for (part, particles) in aura_particles(ticks / 20.0, feet, (right, up), 2.25, true) {
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
/// A texel's own small variation, -1 to 1.
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

/// A cape, hat or pet alone, as the Cosmetics page's cards show it, at `clock` (s): its
/// vertices (blocks), the body's space (x to the body's left, y down, z out of the back) taken
/// by `base`, drawn from its texture `uv` (the frame now). The cape hangs in the lightest
/// breeze; a pet breathes and looks about.
pub(super) fn showcase(
    kind: WearKind,
    style: &'static WearStyle,
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
        WearKind::Hat | WearKind::Pet => {
            let Some(mesh) = model_mesh(kind, style) else {
                return;
            };
            let part = super::player_hooks::part_of(&mesh);
            let mut poses = horizon_client::mobs::anim::Poses::pooled();
            poses.reset(&part);
            pose_model(kind, false, clock, poses.get(part_name(kind)));
            render_model(&part, base, &|p| poses.pose(p), uv, FULL_LIGHT, 0, out);
        }
        WearKind::Aura => {}
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
    fn the_aura_pictures_are_whole_and_their_cut_outs_clear() {
        for a in &AURAS {
            let px = aura_texture(a);
            assert_eq!(px.len(), 64 * 64 * 4);
            assert_eq!(px[(16 * 64 + 16) * 4 + 3], 255, "the mote's middle");
            assert_eq!(px[3], 0, "its corner");
        }
    }

    #[test]
    fn a_frame_is_its_slice_of_the_sheet() {
        let style = &CAPES[1];
        let sheet = [0.0, 0.0, 1.0, 1.0];
        let n = style.frames as f32;
        assert_eq!(frame_uv(sheet, style, 0.0), [0.0, 0.0, 1.0, 1.0 / n]);
        let third = frame_uv(sheet, style, oteria_art::FRAME_SECONDS * 2.5);
        assert!((third[1] - 2.0 / n).abs() < 1e-6 && (third[3] - 3.0 / n).abs() < 1e-6);
        // Round again after the last.
        let again = frame_uv(sheet, style, oteria_art::FRAME_SECONDS * (n + 0.5));
        assert_eq!(again[1], 0.0);
    }

    #[test]
    fn a_block_model_sits_on_the_head_and_the_shoulder() {
        // A 2x2x2 box at the top of a hat's head (y 0 is its top), and a pet's at (8, 0, 8).
        let json = br##"{"elements": [{"from": [7, 0, 7], "to": [9, 2, 9],
            "faces": {"north": {"uv": [0, 0, 2, 2], "texture": "#1"},
                      "up": {"uv": [0, 0, 2, 2], "texture": "#1", "rotation": 90}}}]}"##;
        let hat = block_model(json, WearKind::Hat).unwrap();
        assert_eq!(hat.len(), 2);
        let ys: Vec<f32> = hat.iter().flat_map(|q| q.vertices).map(|v| v[1]).collect();
        // On the bone about the head's middle (y -4): from the top of the head (-8) up two.
        assert!(ys.iter().all(|&y| (-6.0..=-4.0).contains(&y)), "{ys:?}");
        // Its north face looks to the front (-z).
        assert_eq!(hat[0].normal, [0.0, 0.0, -1.0]);
        assert!(hat[0].vertices.iter().all(|v| v[2] == -1.0));
        // Its uv in the texture's share, and a turned face's turned.
        assert!(hat[0].vertices.iter().all(|v| (0.0..=0.125).contains(&v[3])));
        assert_eq!([hat[1].vertices[0][3], hat[1].vertices[0][4]], [0.0, 0.125]);
        let pet = block_model(json, WearKind::Pet).unwrap();
        let ys: Vec<f32> = pet.iter().flat_map(|q| q.vertices).map(|v| v[1]).collect();
        assert!(ys.iter().all(|&y| (-2.0..=0.0).contains(&y)), "{ys:?}");
        assert!(block_model(b"{}", WearKind::Pet).is_none());
    }

    #[test]
    fn every_model_there_reads() {
        for (kind, all) in [(WearKind::Hat, &HATS[..]), (WearKind::Pet, &PETS[..])] {
            for style in all.iter().filter(|s| oteria_art::wear_available(s)) {
                let json = oteria_art::wear_bytes(style.id, "json").unwrap();
                let quads = block_model(&json, kind).unwrap_or_default();
                assert!(quads.len() > 6, "{}", style.id);
                assert!(quads.iter().flat_map(|q| q.vertices).flatten().all(|v| v.is_finite()));
            }
        }
    }
}
