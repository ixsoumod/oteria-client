//! Oteria's butterfly knife (a cosmetic, `docs/oteria/ROADMAP.md`): with it on (the Cosmetics
//! page), every sword in the player's main hand is drawn in first person as Oteria's balisong
//! (`branding/cosmetics/make_balisong.py`), held as a sword is. Y plays its inspect: the knife comes
//! up, each handle flips about its pin, it twirls, snaps shut with the latch and opens again, to
//! metal clicks made of the game's own sounds. Only the picture changes: the sword is the sword
//! for the game and the server, and nothing is sent.

use super::{Game, Screen};
use horizon_client::mesher::Vertex;
use horizon_client::mobs::{Part, Polygon};
use std::cell::RefCell;
use std::f32::consts::PI;
use std::sync::OnceLock;
use std::time::Instant;

/// The model, as the item file has it (the hand renderer's own parts).
fn model() -> Option<&'static Part> {
    static MODEL: OnceLock<Option<Part>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let bytes = include_bytes!("../../assets/horizon/cosmetics/oteria_balisong.json");
            match horizon_cosmetics::Item::parse(bytes) {
                Ok(item) => {
                    // The file's attachment (its place on a player's arm) is not the hand's.
                    let mut root = convert(&item.root);
                    root.pose = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
                    Some(root)
                }
                Err(e) => {
                    log::warn!("Oteria: the butterfly knife's model doesn't read: {e}");
                    None
                }
            }
        })
        .as_ref()
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
const INSPECT_S: f32 = 2.5;
const SOUNDS: [(f32, &str, f32, f32); 9] = [
    (0.30, "minecraft:entity.player.attack.sweep", 0.18, 1.9),
    (0.58, "minecraft:block.tripwire.click_on", 0.6, 1.8),
    (0.62, "minecraft:entity.player.attack.sweep", 0.18, 2.0),
    (0.88, "minecraft:block.tripwire.click_on", 0.6, 1.6),
    (1.10, "minecraft:block.chain.step", 0.5, 1.8),
    (1.58, "minecraft:block.iron_trapdoor.close", 0.3, 2.0),
    (1.60, "minecraft:block.tripwire.click_on", 0.7, 1.4),
    (2.10, "minecraft:entity.player.attack.sweep", 0.15, 1.7),
    (2.14, "minecraft:block.tripwire.click_on", 0.6, 1.9),
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

/// The knife at `t` seconds into the inspect: where the hand takes it (view space: x right, y up,
/// z toward the eye; blocks), how it turns in the hand (radians about its own axes: along the
/// blade, across, about the pins) and the two handles' turns about their pins (radians; 0 open,
/// half a turn shut).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Pose {
    pub lift: [f32; 3],
    pub turn: [f32; 3],
    pub safe: f32,
    pub latch: f32,
}
pub(super) fn inspect_pose(t: f32) -> Pose {
    // Up and in toward the middle of the view, and back down at the end.
    let up = span(t, 0.0, 0.3) * (1.0 - span(t, 2.15, 2.5));
    let toss = (span(t, 1.62, 1.72) - span(t, 1.72, 1.85)) * 0.05;
    let lift = [-0.26 * up, 0.16 * up + toss, 0.06 * up];
    // Shown flat toward the eye, a whole twirl along the blade, then back.
    let turn = [
        span(t, 0.9, 1.3) * 2.0 * PI,
        -0.5 * up,
        0.35 * up,
    ];
    // Each handle flips a whole turn, they both shut, then open again.
    let safe = span(t, 0.3, 0.6) * 2.0 * PI + span(t, 1.3, 1.6) * PI + span(t, 1.85, 2.15) * PI;
    let latch = -(span(t, 0.6, 0.9) * 2.0 * PI + span(t, 1.3, 1.6) * PI + span(t, 1.85, 2.15) * PI);
    Pose { lift, turn, safe, latch }
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

    /// The knife in the hand at `m` (the held item's pose, `item_pose`), as a sword's model would
    /// sit there, posed by the inspect when one plays.
    pub(super) fn draw_knife(&self, m: glam::Mat4, left: bool, light: u32, out: &mut Vec<Vertex>) {
        let Some(part) = model() else {
            return;
        };
        let Some(r) = self.gui.atlas.sprites.get(horizon_client::oteria_art::KNIFE).copied() else {
            return;
        };
        let pose = INSPECT
            .with(|i| i.borrow().started.map(|s| inspect_pose(s.elapsed().as_secs_f32())))
            .unwrap_or_default();
        use glam::{EulerRot, Mat4, Quat, Vec3};
        let side = if left { -1.0 } else { 1.0 };
        // The inspect's lift, in the hand's view space (mirrored for the left hand).
        let lift = Mat4::from_translation(Vec3::new(pose.lift[0] * side, pose.lift[1], pose.lift[2]));
        // A sword's handheld display (`item/handheld`'s firstperson_righthand), then its model
        // centred on the block: the knife lies as the sword's picture does, blade up and right.
        let display = Mat4::from_translation(Vec3::new(1.13 / 16.0 * side, 3.2 / 16.0, 1.13 / 16.0))
            * Mat4::from_quat(Quat::from_euler(
                EulerRot::XYZ,
                0.0,
                (-90.0f32 * side).to_radians(),
                (25.0f32 * side).to_radians(),
            ))
            * Mat4::from_scale(Vec3::splat(0.68));
        // Our model: the tang at the origin, the blade toward -x, y down, at twice the detail. It
        // lies on the sword picture's diagonal (blade up and right), its pins where the sword's
        // guard is, the block's middle at the origin (as an item model is drawn).
        let lay = Mat4::from_translation(Vec3::new(1.0 / 16.0, 1.0 / 16.0, 0.0))
            * Mat4::from_rotation_z((-135.0f32).to_radians())
            * Mat4::from_scale(Vec3::new(0.4, -0.4, 0.4))
            * Mat4::from_quat(Quat::from_euler(EulerRot::XYZ, pose.turn[0], pose.turn[1], pose.turn[2]));
        let base = m * lift * display * lay;
        let pose_of = |p: &Part| {
            let mut q = p.pose;
            match p.name.as_str() {
                "handle_safe" => q[5] = pose.safe,
                "handle_latch" => q[5] = pose.latch,
                _ => {}
            }
            q
        };
        horizon_client::mobs::render_model(part, base, &pose_of, [r.u0, r.v0, r.u1, r.v1], light, 0, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_reads_and_has_its_moving_parts() {
        let part = model().expect("the knife's model reads");
        let names: Vec<&str> = part.children.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["blade", "handle_safe", "handle_latch"]);
    }

    #[test]
    fn the_inspect_starts_and_ends_open_at_rest() {
        let start = inspect_pose(0.0);
        assert_eq!(start, Pose::default());
        let end = inspect_pose(INSPECT_S);
        // Whole turns: open again, in the hand where it was.
        for a in [end.safe, end.latch, end.turn[0]] {
            assert!((a / (2.0 * PI)).fract().abs() < 1e-4, "{a}");
        }
        assert!(end.lift.iter().all(|v| v.abs() < 1e-4));
        // Shut in the middle: both handles half a turn from open.
        let shut = inspect_pose(1.7);
        assert!(((shut.safe / PI) % 2.0 - 1.0).abs() < 1e-3, "{}", shut.safe);
        assert!(SOUNDS.windows(2).all(|w| w[0].0 <= w[1].0), "the sounds in order");
    }
}
