//! The Cosmetics page's 3D: each card's cosmetic alone, as its real model (a knife turning a
//! little on its length, a cape in the lightest breeze, a hat, a pet breathing, an aura going
//! round), and
//! the preview's popup: the player turning in what they wear and what is tried on, the aura
//! around them, or a knife in first person in both arms (`oteria_knife_rig::draw_knife_preview`).
//!
//! A card's model is seen with nothing behind it, framed once (from a few moments of its
//! movement) so it fills its card and keeps its size as it moves.

use super::Game;
use super::oteria_cosmetics::{CardModel, Item, WearKind};
use glam::{Mat4, Vec2, Vec3};
use horizon_client::hud::HudBatch;
use horizon_client::mesher::Vertex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::PI;

/// How a card's model is set in its card: its view's centre and half size (blocks), and for a
/// knife the turn that lays it out along the card's diagonal, flat side out, about its middle.
#[derive(Clone, Copy)]
struct Framing {
    centre: Vec3,
    half: Vec2,
    lay: Mat4,
}

thread_local! {
    static FRAMINGS: RefCell<HashMap<String, Framing>> = RefCell::new(HashMap::new());
}

/// How far a knife turns either way on its length (radians), and how fast (radians a second).
const KNIFE_SWAY: f32 = 0.45;
const KNIFE_PACE: f32 = 0.9;
/// The share of its card a model fills.
const FILL: f32 = 0.84;

impl Game<'_> {
    /// Card `card`'s model at `clock` (s), in its rectangle, cut to its clip.
    pub(super) fn draw_cosmetic_card(&self, batch: &mut HudBatch, card: &CardModel, clock: f32) {
        let key = card.item.id();
        let framing = FRAMINGS.with_borrow(|f| f.get(&key).copied());
        let framing = match framing {
            Some(f) => f,
            None => {
                let Some(f) = self.frame_card(card.item) else {
                    return;
                };
                FRAMINGS.with_borrow_mut(|all| all.insert(key, f));
                f
            }
        };
        let Some(verts) = self.card_view(card.item, &framing, clock) else {
            return;
        };
        let [x, y, w, h] = card.rect;
        let s = (w / 2.0 / framing.half.x.max(1e-3))
            .min(h / 2.0 / framing.half.y.max(1e-3))
            * FILL
            * (1.0 + 0.05 * card.hover);
        let gui = Mat4::from_translation(Vec3::new(x + w / 2.0, y + h / 2.0, 0.0))
            * Mat4::from_scale(Vec3::new(s, -s, s))
            * Mat4::from_translation(-framing.centre);
        let n = verts.len();
        let quads = super::perspective::player_preview_quads_no_cull(
            verts,
            gui,
            None,
            &self.gui.atlas.sprites,
            0..n,
        );
        super::perspective::emit_preview(batch, quads, Vec::new(), card.clip);
    }

    /// `item`'s model in its card's view (x right, y up, z toward the eye; blocks) at `clock`.
    fn card_view(&self, item: Item, framing: &Framing, clock: f32) -> Option<Vec<Vertex>> {
        match item {
            Item::Knife(knife) => {
                let mut verts = self.knife_alone(knife)?;
                let sway = (clock * KNIFE_PACE).sin() * KNIFE_SWAY;
                let m = Mat4::from_axis_angle(diagonal(), sway) * framing.lay;
                for v in &mut verts {
                    v.pos = m.transform_point3(Vec3::from_array(v.pos)).to_array();
                }
                Some(verts)
            }
            Item::Wear(WearKind::Aura, style) => {
                let uv = self.hook_sprite(super::oteria_wear::aura_sprite(style))?;
                let view = Mat4::from_rotation_x(AURA_PITCH);
                let up = view.inverse().transform_vector3(Vec3::Y);
                let mut verts = Vec::new();
                super::oteria_wear::aura_vertices(
                    uv,
                    clock,
                    Vec3::ZERO,
                    (Vec3::X, up),
                    1.9,
                    &mut verts,
                );
                for v in &mut verts {
                    v.pos = view.transform_point3(Vec3::from_array(v.pos)).to_array();
                }
                Some(verts)
            }
            Item::Wear(kind, style) => {
                let uv = super::oteria_wear::uv_now(style)?;
                // A cape seen from behind, a hat or pet from the front; turned a little, from a
                // little above.
                let yaw = if kind == WearKind::Cape { -0.5 } else { PI + 0.55 };
                // A hat from higher up: a halo lies flat.
                let pitch = match kind {
                    WearKind::Cape => 0.18,
                    WearKind::Hat => 0.6,
                    _ => 0.32,
                };
                let base = Mat4::from_rotation_x(pitch)
                    * Mat4::from_rotation_y(yaw)
                    * Mat4::from_rotation_z(PI);
                let mut verts = Vec::new();
                super::oteria_wear::showcase(kind, style, uv, base, clock, &mut verts);
                (!verts.is_empty()).then_some(verts)
            }
        }
    }

    /// How `item` is set in its card: from its model at a few moments of its movement.
    fn frame_card(&self, item: Item) -> Option<Framing> {
        let mut framing = Framing {
            centre: Vec3::ZERO,
            half: Vec2::ONE,
            lay: Mat4::IDENTITY,
        };
        let moments: &[f32] = match item {
            Item::Knife(knife) => {
                framing.lay = lay_out(&self.knife_alone(knife)?);
                // The sway's middle and both its ends.
                &[
                    0.0,
                    PI / 2.0 / KNIFE_PACE,
                    3.0 * PI / 2.0 / KNIFE_PACE,
                ]
            }
            // The aura round a turn; a pet as it looks about.
            Item::Wear(WearKind::Aura, _) => &[0.0, 1.3, 2.6, 3.9, 5.2],
            Item::Wear(WearKind::Pet, _) => &[0.0, 1.75, 3.5, 5.25, 7.0],
            // The cape hangs; a hat sits.
            Item::Wear(WearKind::Cape | WearKind::Hat, _) => &[-1.0],
        };
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for &t in moments {
            for v in self.card_view(item, &framing, t)? {
                // A mote faded out takes no room.
                if v.normal == [0; 4] && v.color >> 24 < 24 {
                    continue;
                }
                let p = Vec3::from_array(v.pos);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        if lo.x > hi.x {
            return None;
        }
        framing.centre = (lo + hi) / 2.0;
        framing.half = ((hi - lo) / 2.0).truncate().max(Vec2::splat(0.05));
        Some(framing)
    }

    /// The Cosmetics page's popup preview of the player in `well` (GUI pixels), turned `turn`
    /// degrees, wearing `trying` over what they wear, the aura (tried on or worn) around them.
    pub(super) fn draw_cosmetic_player(
        &self,
        batch: &mut HudBatch,
        well: [f32; 4],
        turn: f32,
        trying: Option<(WearKind, &'static str)>,
        aura: Option<&'static horizon_client::oteria_art::WearStyle>,
        skin: Option<(String, bool)>,
        clock: f32,
    ) {
        let [x, y, w, h] = well;
        let size = (h * 0.36).min(w * 0.5).max(8.0);
        let aura =
            aura.and_then(|a| self.hook_sprite(super::oteria_wear::aura_sprite(a)));
        let extra = move |gui: Mat4, out: &mut Vec<Vertex>| {
            let Some(uv) = aura else {
                return;
            };
            // The picture's camera, in the player's space.
            let back = gui.inverse();
            let right = back.transform_vector3(Vec3::X).normalize_or_zero();
            let up = back.transform_vector3(Vec3::NEG_Y).normalize_or_zero();
            super::oteria_wear::aura_vertices(uv, clock, Vec3::ZERO, (right, up), 2.2, out);
        };
        super::oteria_wear::try_on(trying);
        self.draw_player_preview_with(
            batch,
            [x, y, x + w, y + h],
            size,
            0.0625,
            Some(turn),
            skin,
            Some(&extra),
        );
        super::oteria_wear::try_on(None);
    }
}

/// How far the aura's card looks down on it (radians).
const AURA_PITCH: f32 = 0.42;

/// The card's diagonal a knife lies along: up to the right.
fn diagonal() -> Vec3 {
    Vec3::new(0.766, 0.643, 0.0)
}

/// The turn that lays a model out along the card's diagonal, its flat side to the eye, about
/// its middle: its longest spread along the diagonal, its next across it.
fn lay_out(verts: &[Vertex]) -> Mat4 {
    let n = verts.len().max(1) as f32;
    let mean = verts.iter().map(|v| Vec3::from_array(v.pos)).sum::<Vec3>() / n;
    // The spread's matrix, and its two main directions (power iteration).
    let mut c = glam::Mat3::ZERO;
    for v in verts {
        let d = Vec3::from_array(v.pos) - mean;
        c += glam::Mat3::from_cols(d * d.x, d * d.y, d * d.z);
    }
    let main = |c: &glam::Mat3, seed: Vec3| {
        let mut e = seed.normalize();
        for _ in 0..48 {
            e = (*c * e).normalize_or(e);
        }
        e
    };
    let e1 = main(&c, Vec3::new(0.3, 0.5, 0.8));
    let l1 = e1.dot(c * e1);
    let c2 = c - glam::Mat3::from_cols(e1 * e1.x, e1 * e1.y, e1 * e1.z) * l1;
    let mut e2 = main(&c2, e1.any_orthonormal_vector());
    e2 = (e2 - e1 * e1.dot(e2)).normalize_or(e1.any_orthonormal_vector());
    let e3 = e1.cross(e2);
    let d = diagonal();
    let across = Vec3::Z.cross(d);
    let from = glam::Mat3::from_cols(e1, e2, e3).transpose();
    let to = glam::Mat3::from_cols(d, across, Vec3::Z);
    Mat4::from_mat3(to * from) * Mat4::from_translation(-mean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32, z: f32) -> Vertex {
        Vertex {
            pos: [x, y, z],
            uv: [0.0; 2],
            layer: 0,
            color: !0,
            light: 0,
            normal: [0, 0, 127, 0],
            glint_uv: [0.0; 2],
        }
    }

    #[test]
    fn a_long_thing_is_laid_along_the_diagonal_flat_side_out() {
        // A blade along z (pointing away), wide in y, thin in x.
        let mut verts = Vec::new();
        for z in 0..20 {
            for y in 0..4 {
                for x in 0..2 {
                    verts.push(at(5.0 + x as f32 * 0.05, 1.0 + y as f32 * 0.2, -(z as f32)));
                }
            }
        }
        let m = lay_out(&verts);
        let a = m.transform_point3(Vec3::from_array(verts[0].pos));
        let b = m.transform_point3(Vec3::from_array(verts[verts.len() - 1].pos));
        let along = (b - a).normalize();
        assert!(along.dot(diagonal()).abs() > 0.99, "{along}");
        // Its thin side toward the eye: the depth spread is the smallest.
        let zs: Vec<f32> = verts
            .iter()
            .map(|v| m.transform_point3(Vec3::from_array(v.pos)).z)
            .collect();
        let spread = zs.iter().cloned().fold(f32::MIN, f32::max) - zs.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread < 0.2, "{spread}");
    }
}
