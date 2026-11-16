//! Oteria's barrel roll (the Barrel Roll tile, off by default), in the spirit of the Fabric mod
//! "Do a Barrel Roll", written anew: flying with an elytra, the view turns as a plane does. The
//! mouse's up and down pitch the nose about the wings and its left and right roll the plane about
//! the nose (Switch roll and yaw swaps the mouse's left and right for vanilla's turn); Left and
//! Right (A and D) are the rudder. Rolled, the plane banks: it turns towards the lower wing, as a
//! plane in a turn does. Loops and barrel rolls go all the way round.
//!
//! Only the view changes: the game still flies by the look vector (yaw and pitch, which the
//! server receives as always), taken from the plane's nose; the roll is the camera's alone.

use super::Game;
use glam::Vec3;
use horizon_client::actions::Action;
use std::cell::RefCell;

/// The plane while flying: its nose and its top, unit vectors, at right angles.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Plane {
    forward: Vec3,
    up: Vec3,
}

#[derive(Default)]
struct State {
    plane: Option<Plane>,
    /// The roll the camera shows, degrees; it eases back to 0 once the flight ends.
    roll: f32,
    roll_old: f32,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The look vector of `yaw` and `pitch` (degrees), as `Entity.calculateViewVector`.
fn look(yaw: f32, pitch: f32) -> Vec3 {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    Vec3::new(-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos())
}

impl Plane {
    /// Level, facing where the player looks.
    fn new(yaw: f32, pitch: f32) -> Self {
        Self {
            forward: look(yaw, pitch),
            up: look(yaw, pitch - 90.0),
        }
    }
    fn right(&self) -> Vec3 {
        self.forward.cross(self.up).normalize_or_zero()
    }
    /// Turns the plane by `angle` degrees about `axis`, then squares it up again.
    fn turn(&mut self, axis: Vec3, angle: f32) {
        if angle == 0.0 || axis == Vec3::ZERO {
            return;
        }
        let q = glam::Quat::from_axis_angle(axis.normalize(), angle.to_radians());
        let forward = (q * self.forward).normalize_or_zero();
        let up = (q * self.up).normalize_or_zero();
        // Kept at right angles, so rounding errors don't add up turn after turn.
        let right = forward.cross(up).normalize_or_zero();
        if forward != Vec3::ZERO && right != Vec3::ZERO {
            self.forward = forward;
            self.up = right.cross(forward).normalize_or_zero();
        }
    }
    /// Nose up (positive) or down, about the wings.
    fn pitch(&mut self, degrees: f32) {
        self.turn(self.right(), degrees);
    }
    /// Right wing down (positive) or up, about the nose.
    fn roll(&mut self, degrees: f32) {
        self.turn(self.forward, degrees);
    }
    /// Nose right (positive) or left, about the plane's top.
    fn yaw(&mut self, degrees: f32) {
        self.turn(self.up, -degrees);
    }
    /// The game's yaw and pitch for the nose, and the camera's roll: how far the plane's top is
    /// turned from the top of a level view along the same look.
    fn angles(&self) -> (f32, f32, f32) {
        let f = self.forward;
        let pitch = (-f.y).clamp(-1.0, 1.0).asin().to_degrees();
        // Straight up or down the yaw is the plane's top's (its belly's going down).
        let flat = if f.x.hypot(f.z) > 1e-4 {
            f
        } else {
            -self.up * f.y.signum()
        };
        let yaw = (-flat.x).atan2(flat.z).to_degrees();
        let level_up = look(yaw, pitch - 90.0);
        let roll = level_up
            .cross(self.up)
            .dot(f)
            .atan2(level_up.dot(self.up))
            .to_degrees();
        (yaw, pitch, roll)
    }
}

/// How fast a rolled plane turns, degrees a second at 90 degrees of roll and full banking.
const BANKING: f32 = 45.0;
/// The rudder's speed, degrees a second.
const RUDDER: f32 = 40.0;
/// How fast righting brings a roll of less than 90 degrees back level, degrees a second.
const RIGHTING: f32 = 60.0;

impl Game<'_> {
    /// Whether the barrel roll flies the view now: on, gliding, in play.
    fn barrel_active(&self) -> bool {
        self.settings.extras.pvp.barrel.enabled
            && self.screen == super::Screen::Playing
            && !self.studio.flying()
            && self.player.as_ref().is_some_and(|p| p.fall_flying)
    }

    /// A look from the mouse (`Entity.turn` units) while gliding: the plane pitches and rolls.
    /// False when the barrel roll is off, so the look turns as always.
    pub(super) fn barrel_turn(&mut self, x: f64, y: f64) -> bool {
        if !self.barrel_active() {
            return false;
        }
        let b = self.settings.extras.pvp.barrel;
        let k = 0.15 * f32::from(b.sensitivity) / 100.0;
        let (dx, dy) = (x as f32 * k, y as f32 * k);
        let (yaw, pitch) = (self.yaw, self.pitch);
        let (yaw, pitch) = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let plane = s.plane.get_or_insert_with(|| Plane::new(yaw, pitch));
            // Mouse down looks down, as always, unless inverted.
            plane.pitch(if b.invert_pitch { dy } else { -dy });
            if b.switch_roll_yaw {
                plane.yaw(dx);
            } else {
                plane.roll(dx);
            }
            let (yaw, pitch, roll) = plane.angles();
            s.roll = roll;
            (yaw, pitch)
        });
        self.barrel_set_look(yaw, pitch);
        true
    }

    /// Once a frame (`dt` seconds): banking, the rudder and righting while gliding; the roll
    /// easing back level once the flight ends.
    pub(super) fn barrel_frame(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let active = self.barrel_active();
        let b = self.settings.extras.pvp.barrel;
        let rudder =
            (self.keys.held(Action::Right) as i32 - self.keys.held(Action::Left) as i32) as f32;
        let (yaw, pitch) = (self.yaw, self.pitch);
        let look = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.roll_old = s.roll;
            if !active {
                s.plane = None;
                // Level again, quickly.
                s.roll *= (1.0 - dt * 10.0).max(0.0);
                if s.roll.abs() < 0.05 {
                    s.roll = 0.0;
                }
                return None;
            }
            let plane = s.plane.get_or_insert_with(|| Plane::new(yaw, pitch));
            // The plane may have been turned by something else (a teleport, the server): it
            // follows the game's look, keeping its roll.
            let (y0, p0, _) = plane.angles();
            if (wrap(y0 - yaw)).abs() > 0.5 || (p0 - pitch).abs() > 0.5 {
                let (_, _, roll) = plane.angles();
                *plane = Plane::new(yaw, pitch);
                plane.roll(roll);
            }
            let (_, _, roll) = plane.angles();
            if b.banking {
                // Rolled, the nose turns towards the lower wing, about the world's vertical.
                let turn = roll.to_radians().sin() * BANKING * dt;
                plane.turn(Vec3::NEG_Y, turn);
            }
            if rudder != 0.0 {
                plane.yaw(rudder * RUDDER * dt);
            }
            if b.righting && roll.abs() < 90.0 {
                let step = (RIGHTING * dt).min(roll.abs());
                plane.roll(-roll.signum() * step);
            }
            let (yaw, pitch, roll) = plane.angles();
            s.roll = roll;
            Some((yaw, pitch))
        });
        if let Some((yaw, pitch)) = look {
            self.barrel_set_look(yaw, pitch);
        }
    }

    /// The look set to `yaw` and `pitch` as a turn would, the old values moved along.
    fn barrel_set_look(&mut self, yaw: f32, pitch: f32) {
        // The yaw goes on from where it is, never jumping a full turn.
        let yaw = self.yaw + wrap(yaw - self.yaw);
        let (dyaw, dpitch) = (yaw - self.yaw, pitch - self.pitch);
        self.rope_rot_old.0 += dyaw;
        self.rope_rot_old.1 += dpitch;
        self.yaw = yaw;
        self.pitch = pitch.clamp(-90.0, 90.0);
        if let Some(m) = self.player_model.as_mut() {
            m.previous_head_pitch = m.head_pitch;
            m.head_pitch = self.pitch;
        }
    }

    /// The camera's roll for this frame, as a matrix over the view (`bob * spin * view`).
    pub(super) fn barrel_matrix(&self, partial: f32) -> glam::Mat4 {
        let roll = STATE.with(|s| {
            let s = s.borrow();
            s.roll_old + wrap(s.roll - s.roll_old) * partial.clamp(0.0, 1.0)
        });
        if roll == 0.0 {
            glam::Mat4::IDENTITY
        } else {
            glam::Mat4::from_rotation_z(roll.to_radians())
        }
    }
}

/// `angle` (degrees) brought within -180..180.
fn wrap(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        wrap(a - b).abs() < 0.01
    }

    #[test]
    fn a_level_plane_reads_as_the_look_it_started_from() {
        for (yaw, pitch) in [(0.0, 0.0), (90.0, -30.0), (-135.0, 60.0)] {
            let (y, p, r) = Plane::new(yaw, pitch).angles();
            assert!(
                close(y, yaw) && close(p, pitch) && close(r, 0.0),
                "{yaw} {pitch}: {y} {p} {r}"
            );
        }
    }

    #[test]
    fn rolling_turns_only_the_camera_and_pitching_after_it_turns_the_nose_sideways() {
        let mut plane = Plane::new(0.0, 0.0);
        plane.roll(90.0);
        let (yaw, pitch, roll) = plane.angles();
        assert!(
            close(yaw, 0.0) && close(pitch, 0.0) && close(roll, 90.0),
            "{yaw} {pitch} {roll}"
        );
        // Rolled right and pulled up: the nose goes right.
        plane.pitch(30.0);
        let (yaw, pitch, _) = plane.angles();
        assert!(close(yaw, 30.0), "the nose went to {yaw}");
        assert!(pitch.abs() < 0.01);
    }

    #[test]
    fn a_loop_comes_back_round() {
        let mut plane = Plane::new(20.0, 0.0);
        for _ in 0..36 {
            plane.pitch(10.0);
        }
        let (yaw, pitch, roll) = plane.angles();
        assert!(
            close(yaw, 20.0) && close(pitch, 0.0) && close(roll, 0.0),
            "{yaw} {pitch} {roll}"
        );
        // Over the top, upside down: the look is the other way, the camera rolled over.
        let mut plane = Plane::new(0.0, 0.0);
        for _ in 0..12 {
            plane.pitch(10.0);
        }
        let (yaw, _, roll) = plane.angles();
        assert!(
            close(yaw, 180.0) && close(roll.abs(), 180.0),
            "{yaw} {roll}"
        );
    }
}
