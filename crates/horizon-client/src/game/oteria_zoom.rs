//! Oteria's zoom, as OptiFine's (the Zoom tile of the Mods page): while its key is held
//! (`key.oteria.zoom`, unbound until the player sets it in Controls), the view is magnified
//! `pvp.zoom.level` times, and the wheel zooms further in or back out instead of turning the
//! hotbar. Smooth, it eases in and out as the spyglass does (the field of view modifier); else
//! it narrows at once. Mouse turning slows by as much, so aiming stays steady. Only the picture
//! changes: nothing is sent.

use super::{Game, Screen};
use std::cell::Cell;

/// The magnification the wheel can reach, either way.
const RANGE: (f32, f32) = (1.5, 50.0);
/// How much one wheel notch multiplies (or divides) the magnification.
const STEP: f32 = 1.25;

thread_local! {
    /// The magnification while the key is held (the wheel's), `None` when it is not.
    static FACTOR: Cell<Option<f32>> = const { Cell::new(None) };
}

impl Game<'_> {
    /// Whether the zoom key is held now, a key or a mouse button.
    fn oteria_zoom_key_down(&self) -> bool {
        let name = self.settings.keys.get("key.oteria.zoom");
        if name == "key.keyboard.unknown" {
            return false;
        }
        horizon_client::keybinds::key_code(name).is_some_and(|k| self.physical_keys.contains(&k))
            || self
                .physical_buttons
                .iter()
                .any(|b| horizon_client::keybinds::mouse_name(*b) == name)
    }

    /// Whether the zoom is on, its key held, in play.
    fn oteria_zooming(&self) -> bool {
        self.settings.extras.pvp.zoom.enabled
            && self.screen == Screen::Playing
            && self.player.is_some()
            && self.oteria_zoom_key_down()
    }

    /// The zoom in effect: (magnification, smooth, turning scale), or `None` when the key is
    /// not held. A press starts at the chosen level; a release forgets the wheel's.
    pub(super) fn oteria_zoom_now(&self) -> Option<(f32, bool, f64)> {
        if !self.oteria_zooming() {
            FACTOR.with(|f| f.set(None));
            return None;
        }
        let level = f32::from(self.settings.extras.pvp.zoom.level);
        let factor = FACTOR.with(|f| {
            let v = f.get().unwrap_or(level);
            f.set(Some(v));
            v
        });
        Some((
            factor,
            self.settings.extras.pvp.zoom.smooth,
            f64::from(1.0 / factor),
        ))
    }

    /// The wheel while zooming: up zooms in, down back out. True when it was taken, so the
    /// hotbar doesn't turn.
    pub(super) fn oteria_zoom_wheel(&mut self, d: f32) -> bool {
        if d == 0.0 || self.oteria_zoom_now().is_none() {
            return false;
        }
        FACTOR.with(|f| {
            let now = f.get().unwrap_or(1.0);
            let next = now * STEP.powf(d.signum() * d.abs().min(4.0));
            f.set(Some(next.clamp(RANGE.0, RANGE.1)));
        });
        true
    }
}
