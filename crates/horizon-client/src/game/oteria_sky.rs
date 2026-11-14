//! Oteria's sky mode (`pvp::Sky`, the Sky mode tile): one of the sky packs in the player's
//! resourcepacks folder (OptiFine's custom sky format, `optifine/sky` or `mcpatcher/sky`) is put
//! on top of their resource packs while it is on, and the engine's custom sky
//! (`horizon_client::custom_sky`) draws it: its skybox faces at full size, smoothly filtered,
//! fading in and out with the time of day and turning with the sky.
//!
//! The packs stay the player's own files: Oteria ships none and only changes which are on, as
//! the resource pack screen does. Turned off, the pack it put on goes; sky packs the player put
//! on themselves stay.

use super::Game;
use super::packs::packs_dir;
use horizon_client::packs::{self, Pack, Source};
use std::path::Path;

/// Whether a pack has a custom sky (OptiFine's or MCPatcher's folder).
fn has_sky(source: &Source) -> bool {
    const DIRS: [&str; 2] = ["assets/minecraft/optifine/sky/", "assets/minecraft/mcpatcher/sky/"];
    match source {
        Source::Dir(dir) => DIRS.iter().any(|d| dir.join(d).is_dir()),
        Source::Zip(path) => zip_has(path, &DIRS),
    }
}
fn zip_has(path: &Path, dirs: &[&str]) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    archive.file_names().any(|name| name.is_ok_and(|name| dirs.iter().any(|d| name.starts_with(d))))
}

impl Game<'_> {
    /// The player's sky packs, as `pvp::SKY_PACKS` lists them for the Sky option.
    fn sky_packs(&self) -> Vec<Pack> {
        let all = packs::discover(&self.vanilla_root(), &packs_dir());
        let found: Vec<Pack> = all.into_iter().filter(|p| p.id.starts_with("file/") && has_sky(&p.source)).collect();
        let listed = found
            .iter()
            .map(|p| {
                let title = p.title.plain();
                let title = title.trim_end_matches(".zip").to_owned();
                (p.id.clone(), title)
            })
            .collect();
        if let Ok(mut list) = horizon_client::pvp::SKY_PACKS.lock() {
            *list = listed;
        }
        found
    }

    /// Looks for the sky packs again (the menu opening), so the Sky option lists them.
    pub(super) fn sky_packs_refresh(&self) {
        self.sky_packs();
    }

    /// After the settings change (`apply_settings`): when the sky mode's switch or pack is not
    /// what it was, the packs follow. Not on the first call (the game starting: the selection
    /// saved is already the right one), nor when only other settings changed, so a sky pack the
    /// player turns on or off on the pack screen is left as they set it.
    pub(super) fn sky_mode_changed(&mut self) {
        thread_local! {
            static SEEN: std::cell::RefCell<Option<horizon_client::pvp::Sky>> = const { std::cell::RefCell::new(None) };
        }
        let now = self.settings.extras.pvp.sky.clone();
        let before = SEEN.with(|s| s.borrow_mut().replace(now.clone()));
        if before.is_some_and(|b| b != now) {
            self.sky_mode_sync();
            let synced = self.settings.extras.pvp.sky.clone();
            SEEN.with(|s| *s.borrow_mut() = Some(synced));
        }
    }

    /// Puts the chosen sky pack on top of the resource packs while the sky mode is on, and takes
    /// it off when it's off; reloads the resources when that changes the selection.
    pub(super) fn sky_mode_sync(&mut self) {
        let found = self.sky_packs();
        let sky = &mut self.settings.extras.pvp.sky;
        // The chosen pack, or the first one when none is (or it has gone).
        let chosen = found.iter().find(|p| p.id == sky.pack).or(found.first()).map(|p| p.id.clone());
        if let Some(id) = &chosen {
            sky.pack = id.clone();
        }
        let mut selection = self.settings.resource_packs.clone();
        if sky.enabled {
            // The other sky packs off: one sky at a time, the chosen one on top.
            let others: Vec<&str> = found.iter().map(|p| p.id.as_str()).collect();
            selection.retain(|id| !others.contains(&id.as_str()));
            if let Some(id) = chosen {
                selection.push(id);
            }
        } else if !sky.pack.is_empty() {
            selection.retain(|id| *id != sky.pack);
        }
        if selection != self.settings.resource_packs {
            log::info!("Oteria: the sky mode's packs are now {selection:?}");
            self.settings.resource_packs = selection;
            self.settings.save(&Self::options_path());
            self.reload_packs();
        }
    }
}
