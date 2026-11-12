//! Boost, the rocket of Oteria's menu: a performance mode. Turned on, it saves the video
//! settings as they are (`options.txt`'s and `horizon-graphics.txt`'s, in `oteria-boost/` next
//! to them, so a restart while on still knows them) and puts on the Performance preset with
//! smooth lighting off. Turned off, it puts back exactly what was saved. Nothing here reaches the
//! server.

use horizon_client::graphics::{self, Graphics, Preset};
use horizon_client::settings::Settings;
use std::fs;
use std::path::Path;

/// The folder next to `options.txt` that holds what Boost replaced: on while it exists.
pub(super) const DIR: &str = "oteria-boost";
const OPTIONS: &str = "options.txt";
const GRAPHICS: &str = "graphics.txt";

/// Whether Boost is on (what it replaced is saved).
pub(super) fn is_on(options_dir: &Path) -> bool {
    options_dir.join(DIR).join(OPTIONS).exists()
}

/// Boost on: the video settings saved, then the fast ones put on. False when they could not be
/// saved (then nothing changes: they could not come back).
pub(super) fn turn_on(s: &mut Settings, options_dir: &Path) -> bool {
    let dir = options_dir.join(DIR);
    if !is_on(options_dir) {
        let saved = fs::create_dir_all(&dir).is_ok()
            && fs::write(dir.join(GRAPHICS), s.horizon.serialize()).is_ok()
            // Last: its being there says Boost is on.
            && fs::write(dir.join(OPTIONS), s.serialize()).is_ok();
        if !saved {
            log::warn!("Oteria: Boost could not save the video settings, so it stays off");
            let _ = fs::remove_dir_all(&dir);
            return false;
        }
    }
    graphics::apply_preset(Preset::Performance, s);
    s.smooth_lighting = false;
    true
}

/// Boost off: the video settings it saved put back, and forgotten.
pub(super) fn turn_off(s: &mut Settings, options_dir: &Path) {
    let dir = options_dir.join(DIR);
    if let Ok(text) = fs::read_to_string(dir.join(OPTIONS)) {
        restore(s, &Settings::parse(&text));
    }
    if let Ok(text) = fs::read_to_string(dir.join(GRAPHICS)) {
        s.horizon = Graphics::parse(&text);
    }
    let _ = fs::remove_dir_all(&dir);
}

/// `options.txt`'s video settings from `old`; the rest (keys, sounds…) stays as it is now.
fn restore(s: &mut Settings, old: &Settings) {
    s.render_distance = old.render_distance;
    s.simulation_distance = old.simulation_distance;
    s.clouds = old.clouds.clone();
    s.particles = old.particles;
    s.entity_distance = old.entity_distance;
    s.entity_shadows = old.entity_shadows;
    s.biome_blend = old.biome_blend;
    s.mipmap_levels = old.mipmap_levels;
    s.weather_radius = old.weather_radius;
    s.cutout_leaves = old.cutout_leaves;
    s.smooth_lighting = old.smooth_lighting;
    s.graphics_preset = old.graphics_preset.clone();
    s.max_fps = old.max_fps;
    s.vsync = old.vsync;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boost_puts_back_the_video_settings_it_replaced_and_keeps_the_rest() {
        let dir = std::env::temp_dir().join(format!("oteria-boost-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::default();
        s.render_distance = 20;
        s.particles = 0;
        s.smooth_lighting = true;
        s.horizon.preset = Preset::Ultra;
        s.horizon.fog = false;
        let before = (s.render_distance, s.particles, s.smooth_lighting, s.horizon.clone());

        assert!(turn_on(&mut s, &dir));
        assert!(is_on(&dir));
        assert_eq!((s.render_distance, s.particles, s.smooth_lighting), (8, 2, false));
        assert_eq!(s.horizon.preset, Preset::Performance);
        // Turned on again (a second click, a restart): what it saved stays the first settings.
        assert!(turn_on(&mut s, &dir));
        // Meanwhile something else changes: it stays.
        s.master_volume = 0.25;

        turn_off(&mut s, &dir);
        assert!(!is_on(&dir));
        assert_eq!((s.render_distance, s.particles, s.smooth_lighting, s.horizon.clone()), before);
        assert_eq!(s.master_volume, 0.25);
        let _ = fs::remove_dir_all(dir);
    }
}
