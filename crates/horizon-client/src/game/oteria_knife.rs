//! Oteria's butterfly knife (a cosmetic, `docs/oteria/ROADMAP.md`): with it on (the Cosmetics
//! page), every sword in the player's main hand is drawn in first person as Oteria's balisong
//! (`branding/cosmetics/make_balisong.py`), in the player's own arm, which shows as it holds it.
//!
//! The knife moves as a real balisong does: the hand holds one handle (the safe one); the blade
//! turns on that handle's pin, half a turn from shut to open; the other handle (the bite one)
//! turns on the blade's other pin, half a turn from lying on the blade to lying by the held
//! handle. Y plays an inspect made of real tricks: a basic close (the bite handle swings over
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

/// The model and its texture's sprite: a knife converted on this computer
/// (`oteria_art::KNIFE_PRIVATE_FILE`, never in git) when there is one, else Oteria's own.
fn model() -> Option<&'static (Part, &'static str)> {
    static MODEL: OnceLock<Option<(Part, &'static str)>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            if let Some(part) = private_model() {
                return Some((part, horizon_client::oteria_art::KNIFE_PRIVATE));
            }
            let bytes = include_bytes!("../../assets/horizon/cosmetics/oteria_balisong.json");
            match horizon_cosmetics::Item::parse(bytes) {
                Ok(item) => {
                    // The file's attachment (its place on a player's arm) is not the hand's.
                    let mut root = convert(&item.root);
                    root.pose = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
                    Some((root, horizon_client::oteria_art::KNIFE))
                }
                Err(e) => {
                    log::warn!("Oteria: the butterfly knife's model doesn't read: {e}");
                    None
                }
            }
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

/// How long the inspect lasts (s), and what it plays when: (time, sound event, volume, pitch).
const INSPECT_S: f32 = 3.0;
const SWISH: &str = "minecraft:entity.player.attack.sweep";
const CLICK: &str = "minecraft:block.tripwire.click_on";
const SOUNDS: [(f32, &str, f32, f32); 13] = [
    (0.26, SWISH, 0.15, 1.9),
    (0.46, CLICK, 0.6, 1.8),
    (0.70, "minecraft:block.iron_trapdoor.close", 0.3, 2.0),
    (0.92, SWISH, 0.16, 2.0),
    (1.11, CLICK, 0.6, 1.6),
    (1.18, SWISH, 0.2, 1.5),
    (1.70, "minecraft:block.chain.step", 0.55, 1.7),
    (1.78, SWISH, 0.15, 2.0),
    (1.96, CLICK, 0.7, 1.9),
    (2.06, SWISH, 0.15, 1.8),
    (2.26, "minecraft:block.iron_trapdoor.close", 0.3, 2.0),
    (2.38, SWISH, 0.16, 2.0),
    (2.60, CLICK, 0.7, 1.8),
];

#[derive(Default)]
struct Inspect {
    started: Option<Instant>,
    /// The sounds already played.
    played: usize,
}
thread_local! {
    static INSPECT: RefCell<Inspect> = RefCell::new(Inspect::default());
}

/// `t` from 0 to 1, slow at both ends.
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// How far `t` is through `from..to` (0 to 1, eased).
fn span(t: f32, from: f32, to: f32) -> f32 {
    smooth((t - from) / (to - from))
}
/// A part thrown round its pin through `from..to`: it speeds up as a swinging weight does, hits
/// its stop before the end and bounces off it a little.
fn swing(t: f32, from: f32, to: f32) -> f32 {
    let u = ((t - from) / (to - from)).clamp(0.0, 1.0);
    if u < 0.8 {
        (u / 0.8).powi(2)
    } else {
        1.0 - 0.06 * ((u - 0.8) / 0.2 * PI).sin()
    }
}

/// The knife at `t` seconds into the inspect. `blade` and `bite` are how open each joint is (1:
/// as held open, 0: folded): the blade on the held handle's pin, the bite handle on the blade's.
/// `spin` turns the whole knife about its pins' axis (radians; the aerial), `toss` lifts it out
/// of the hand (blocks), `wrist` tips it with the flicks (radians), and `up` is how far the
/// hand has brought it up to be looked at (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pose {
    pub blade: f32,
    pub bite: f32,
    pub spin: f32,
    pub toss: f32,
    pub wrist: f32,
    pub up: f32,
}
impl Default for Pose {
    fn default() -> Self {
        Self { blade: 1.0, bite: 1.0, spin: 0.0, toss: 0.0, wrist: 0.0, up: 0.0 }
    }
}
pub(super) fn inspect_pose(t: f32) -> Pose {
    let up = span(t, 0.0, 0.25) * (1.0 - span(t, 2.7, INSPECT_S));
    // Basic close: the bite handle swings over onto the blade, then both fold into the hand.
    // Flick out: the blade, in the bite handle, swings back out. Aerial (below). Flick open: the
    // bite handle comes round to the hand. Then a fast close and open, the joints overlapping.
    let bite = 1.0 - swing(t, 0.25, 0.50) + swing(t, 1.75, 2.00) - swing(t, 2.05, 2.22)
        + swing(t, 2.45, 2.65);
    let blade = 1.0 - swing(t, 0.50, 0.74) + swing(t, 0.90, 1.15) - swing(t, 2.12, 2.30)
        + swing(t, 2.36, 2.56);
    // The aerial: thrown up turning once about its pins, caught as it comes down.
    let air = ((t - 1.18) / (1.70 - 1.18)).clamp(0.0, 1.0);
    let spin = -2.0 * PI * air;
    let toss = 0.22 * 4.0 * air * (1.0 - air);
    // The wrist cocks back before each flick and follows through.
    let flick = |at: f32| (span(t, at - 0.10, at) - span(t, at, at + 0.22)) * 0.35;
    let wrist = flick(0.25) - flick(0.50) + flick(0.90) + flick(1.18) - flick(1.75) + flick(2.05)
        - flick(2.36);
    Pose { blade, bite, spin, toss, wrist, up }
}

/// How the knife sits against the fist's palm side, in the arm's own space (model pixels from the arm part's
/// pivot, y toward the hand; degrees; the knife's size): at rest, and brought up for the
/// inspect. `private/knife_pose.txt` (14 numbers, the two sets) overrides it while tuning.
const GRIP_REST: [f32; 7] = [-1.0, 11.0, -2.2, 0.0, 10.0, -45.0, 0.45];
const GRIP_SHOWN: [f32; 7] = [-1.0, 11.0, -2.2, 0.0, 0.0, -30.0, 0.5];
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

    /// The inspect key, in play with a sword shown as the knife. True when taken.
    pub(super) fn knife_key_press(&mut self, name: &str) -> bool {
        if self.screen != Screen::Playing || name != self.settings.extras.pvp.skins.inspect_key {
            return false;
        }
        let held = self.inventory.get(self.selected).and_then(|s| s.as_ref()).map(|s| s.item.clone());
        if !held.is_some_and(|item| self.knife_shown(&item)) {
            return false;
        }
        INSPECT.with(|i| *i.borrow_mut() = Inspect { started: Some(Instant::now()), played: 0 });
        true
    }

    /// Once a frame: the inspect's sounds as it reaches them; it stops when the knife goes.
    pub(super) fn knife_frame(&mut self) {
        let held = self.inventory.get(self.selected).and_then(|s| s.as_ref()).map(|s| s.item.clone());
        let shown = held.is_some_and(|item| self.knife_shown(&item));
        let due: Vec<(&str, f32, f32)> = INSPECT.with(|i| {
            let mut i = i.borrow_mut();
            let Some(start) = i.started else {
                return Vec::new();
            };
            let t = start.elapsed().as_secs_f32();
            if !shown || t >= INSPECT_S {
                *i = Inspect::default();
                return Vec::new();
            }
            let mut due = Vec::new();
            while i.played < SOUNDS.len() && SOUNDS[i.played].0 <= t {
                let (_, event, volume, pitch) = SOUNDS[i.played];
                due.push((event, volume, pitch));
                i.played += 1;
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
        let pose = INSPECT
            .with(|i| i.borrow().started.map(|s| inspect_pose(s.elapsed().as_secs_f32())))
            .unwrap_or_default();
        let side = if left { -1.0 } else { 1.0 };
        // The hand comes up and in for the inspect.
        let lift = Mat4::from_translation(Vec3::new(-0.20 * pose.up * side, 0.16 * pose.up, 0.0));
        let arm = to_world * lift * horizon_client::first_person::player_arm_pose(left, inverse_height, attack);
        if !self.own_invisible() {
            self.first_person_arm(left, arm, light, out);
        }
        let start = out.len();
        let Some((part, sprite)) = model() else {
            return start;
        };
        let Some(r) = self.gui.atlas.sprites.get(*sprite).copied() else {
            return start;
        };
        // The arm part's own space, as `first_person_arm` poses it: its pivot, turned outwards.
        let slim = self.player_model.as_ref().is_some_and(|m| m.slim);
        let pivot = self.res.player_arm(slim, left).as_ref().map_or([-5.0 * side, 2.0, 0.0], |a| {
            [a.pose[0], a.pose[1], a.pose[2]]
        });
        let hand = arm
            * Mat4::from_translation(Vec3::from(pivot) / 16.0)
            * Mat4::from_rotation_z(0.1 * side);
        let (rest, shown) = grips();
        let g: [f32; 7] = std::array::from_fn(|k| rest[k] + (shown[k] - rest[k]) * pose.up);
        // The knife's model: pixels at twice the detail, the blade toward -x, the pins along z,
        // the tang between them at the origin; the fist closes on the safe handle's middle.
        let pin = |name: &str| {
            part.children.iter().find(|p| p.name == name).map_or(Vec3::ZERO, |p| Vec3::new(p.pose[0], p.pose[1], p.pose[2]))
        };
        let (safe_pin, bite_pin) = (pin("handle_safe"), pin("handle_latch"));
        let grip = safe_pin + Vec3::new(10.0, 0.0, 0.0);
        let base = hand
            // The aerial: out of the hand, on along the arm.
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
            * Mat4::from_scale(Vec3::splat(g[6]))
            * Mat4::from_translation(grip / 16.0)
            * Mat4::from_rotation_z(pose.spin)
            * Mat4::from_translation(-grip / 16.0);
        // The joints: the blade turns on the held handle's pin; the bite handle turns on its own
        // pin, which the blade carries round.
        let blade_turn = PI * (1.0 - pose.blade);
        let bite_turn = -PI * (1.0 - pose.bite);
        let about = |centre: Vec3, angle: f32, point: Vec3| {
            centre + Mat4::from_rotation_z(angle).transform_vector3(point - centre)
        };
        let pose_of = |p: &Part| {
            let mut q = p.pose;
            match p.name.as_str() {
                "blade" => {
                    let at = about(safe_pin, blade_turn, Vec3::new(q[0], q[1], q[2]));
                    (q[0], q[1], q[5]) = (at.x, at.y, blade_turn);
                }
                "handle_latch" => {
                    let at = about(safe_pin, blade_turn, bite_pin);
                    (q[0], q[1], q[5]) = (at.x, at.y, blade_turn + bite_turn);
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
        let (part, _) = model().expect("the knife's model reads");
        let names: Vec<&str> = part.children.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["blade", "handle_safe", "handle_latch"]);
    }

    #[test]
    fn the_inspect_is_made_of_a_balisongs_own_moves() {
        // It starts and ends open, in the hand, at rest.
        assert_eq!(inspect_pose(0.0), Pose::default());
        let end = inspect_pose(INSPECT_S);
        assert!((end.blade - 1.0).abs() < 1e-4 && (end.bite - 1.0).abs() < 1e-4);
        assert!(end.up.abs() < 1e-4 && end.toss.abs() < 1e-4 && end.wrist.abs() < 1e-4);
        assert!((end.spin / (2.0 * PI)).fract().abs() < 1e-4, "a whole turn in the air");
        // The joints never pass their stops (but for the small bounce off them).
        for k in 0..=300 {
            let p = inspect_pose(k as f32 * 0.01);
            assert!((-0.07..=1.07).contains(&p.blade) && (-0.07..=1.07).contains(&p.bite), "{k}");
            // The blade only moves while it is inside the bite handle, as a real one's must: the
            // bite handle can't lie by the held handle with the blade folded in.
            assert!(p.bite <= p.blade + 0.75, "{k}: {p:?}");
        }
        // Shut after the basic close; a straight stick in the air (blade out, in the bite handle).
        let shut = inspect_pose(0.82);
        assert!(shut.blade.abs() < 1e-3 && shut.bite.abs() < 1e-3);
        let air = inspect_pose(1.45);
        assert!((air.blade - 1.0).abs() < 1e-3 && air.bite.abs() < 1e-3 && air.toss > 0.2);
        assert!(SOUNDS.windows(2).all(|w| w[0].0 <= w[1].0), "the sounds in order");
    }
}
