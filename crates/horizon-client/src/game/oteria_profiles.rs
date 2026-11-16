//! Oteria's profiles (the mods page's PROFILES tab): named sets of the mods' settings, as
//! Lunar's. A profile is a copy of `oteria-extras.txt` (every mod's switch and settings, the HUD
//! layout, the crosshair) in `oteria-profiles/<name>.txt` next to `options.txt`; the one in use is
//! named in `oteria-profiles/active`. Switching saves what is on into the profile in use first,
//! so nothing set since is lost. Nothing here reaches the server.

use std::fs;
use std::path::{Path, PathBuf};

/// The folder next to `options.txt`, and the file naming the profile in use.
pub(super) const DIR: &str = "oteria-profiles";
const ACTIVE: &str = "active";
/// The longest name, in characters.
pub(super) const NAME_MAX: usize = 24;

pub(super) struct Profiles {
    dir: PathBuf,
    /// The profiles' names, in the order shown (by name, ignoring case).
    pub(super) names: Vec<String>,
    /// The one in use (always one of `names` once loaded).
    pub(super) active: usize,
}

/// `name` as a profile's name: its letters, digits, spaces, `-` and `_`, trimmed and cut to
/// `NAME_MAX`. None when nothing is left, or for a name Windows keeps for devices.
pub(super) fn clean(name: &str) -> Option<String> {
    let kept: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(NAME_MAX)
        .collect();
    let kept = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = kept.to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str())
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.ends_with(|c: char| c.is_ascii_digit()));
    (!kept.is_empty() && !reserved).then_some(kept)
}

impl Profiles {
    /// The profiles in `options_dir`; the first time, one named `first` made of `current` (the
    /// settings on now), in use.
    pub(super) fn load(options_dir: &Path, first: &str, current: &str) -> Self {
        let dir = options_dir.join(DIR);
        let mut names: Vec<String> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                (path.extension()? == "txt").then(|| path.file_stem()?.to_str().map(str::to_owned))?
            })
            .filter(|n| clean(n).as_deref() == Some(n.as_str()))
            .collect();
        let mut me = Self {
            dir,
            names: Vec::new(),
            active: 0,
        };
        if names.is_empty() {
            let first = clean(first).unwrap_or_else(|| "Default".into());
            me.write(&first, current);
            names.push(first);
        }
        me.names = names;
        me.sort();
        let active = fs::read_to_string(me.dir.join(ACTIVE)).unwrap_or_default();
        me.active = me.index(active.trim()).unwrap_or(0);
        me.save_active();
        me
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.txt"))
    }
    fn write(&self, name: &str, text: &str) {
        let _ = fs::create_dir_all(&self.dir);
        if let Err(e) = fs::write(self.path(name), text) {
            log::warn!("Oteria: the profile {name} is not saved: {e}");
        }
    }
    fn save_active(&self) {
        let _ = fs::create_dir_all(&self.dir);
        let _ = fs::write(self.dir.join(ACTIVE), self.active_name());
    }
    fn sort(&mut self) {
        let active = self.names.get(self.active).cloned();
        self.names.sort_by_key(|n| n.to_lowercase());
        self.active = active.and_then(|a| self.index(&a)).unwrap_or(0);
    }
    fn index(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n.eq_ignore_ascii_case(name))
    }
    pub(super) fn active_name(&self) -> &str {
        &self.names[self.active]
    }
    /// `base`, or `base 2`, `base 3`… : the first no profile has.
    fn free(&self, base: &str) -> String {
        let base: String = base.chars().take(NAME_MAX - 3).collect();
        (1..)
            .map(|k| if k == 1 { base.clone() } else { format!("{base} {k}") })
            .find(|n| self.index(n).is_none())
            .expect("a free name")
    }

    /// Saves `current` (the settings on now) into the profile in use.
    pub(super) fn store(&self, current: &str) {
        self.write(self.active_name(), current);
    }

    /// A new profile named after `base`, made of `current`, and in use (the one left keeps
    /// `current` too). Its index.
    pub(super) fn create(&mut self, base: &str, current: &str) -> usize {
        self.store(current);
        let name = self.free(base);
        self.write(&name, current);
        self.names.push(name.clone());
        self.active = self.names.len() - 1;
        self.sort();
        self.save_active();
        self.index(&name).unwrap_or(0)
    }

    /// Profile `i` in use, after `current` is saved into the one left: its settings' text.
    pub(super) fn switch(&mut self, i: usize, current: &str) -> Option<String> {
        if i >= self.names.len() {
            return None;
        }
        self.store(current);
        self.active = i;
        self.save_active();
        fs::read_to_string(self.path(&self.names[i])).ok()
    }

    /// A copy of profile `i` (with `current` saved first, for the one in use), not in use.
    pub(super) fn duplicate(&mut self, i: usize, current: &str) -> Option<usize> {
        let name = self.names.get(i)?.clone();
        self.store(current);
        let text = fs::read_to_string(self.path(&name)).ok()?;
        let copy = self.free(&name);
        self.write(&copy, &text);
        self.names.push(copy.clone());
        self.sort();
        self.index(&copy)
    }

    /// Profile `i` renamed `to` (cleaned). False when the name is taken or empty.
    pub(super) fn rename(&mut self, i: usize, to: &str) -> bool {
        let (Some(from), Some(to)) = (self.names.get(i).cloned(), clean(to)) else {
            return false;
        };
        if from == to {
            return true;
        }
        // Only its case changing is the same profile; any other taken name is not free.
        if self.index(&to).is_some_and(|k| k != i) {
            return false;
        }
        if fs::rename(self.path(&from), self.path(&to)).is_err() {
            return false;
        }
        self.names[i] = to;
        self.sort();
        self.save_active();
        true
    }

    /// Profile `i` deleted, never the last one. When it was in use, the first left is: its
    /// settings' text, to put on.
    pub(super) fn delete(&mut self, i: usize) -> Result<Option<String>, ()> {
        if self.names.len() < 2 || i >= self.names.len() {
            return Err(());
        }
        let was_active = i == self.active;
        let _ = fs::remove_file(self.path(&self.names[i]));
        self.names.remove(i);
        if was_active {
            self.active = 0;
        } else if i < self.active {
            self.active -= 1;
        }
        self.save_active();
        Ok(if was_active {
            fs::read_to_string(self.path(self.active_name())).ok()
        } else {
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oteria-profiles-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_keep_only_what_a_file_can_hold() {
        assert_eq!(clean("  My   PvP / setup!  ").as_deref(), Some("My PvP setup"));
        assert_eq!(clean("Défaut").as_deref(), Some("Défaut"));
        assert_eq!(clean("..\\..").as_deref(), None);
        assert_eq!(clean("con").as_deref(), None);
        assert_eq!(clean("COM1").as_deref(), None);
        assert_eq!(clean("Combo").as_deref(), Some("Combo"));
        assert_eq!(clean(&"a".repeat(40)).unwrap().chars().count(), NAME_MAX);
    }

    #[test]
    fn profiles_keep_each_their_settings_across_switches() {
        let dir = temp();
        let mut p = Profiles::load(&dir, "Default", "fps:true\n");
        assert_eq!(p.names, ["Default"]);
        // A new profile starts from what is on, and is in use.
        let pvp = p.create("Profile", "fps:true\n");
        assert_eq!(p.active, pvp);
        // Changed in it, then back to the first: what was set is saved, the first's comes back.
        assert_eq!(p.switch(p.index("Default").unwrap(), "fps:false\n").as_deref(), Some("fps:true\n"));
        assert_eq!(fs::read_to_string(dir.join(DIR).join("Profile.txt")).unwrap(), "fps:false\n");
        // Loaded again, the one in use is remembered.
        let again = Profiles::load(&dir, "Default", "x");
        assert_eq!(again.active_name(), "Default");
        assert_eq!(again.names, ["Default", "Profile"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rename_duplicate_and_delete() {
        let dir = temp();
        let mut p = Profiles::load(&dir, "Default", "a\n");
        let copy = p.duplicate(0, "b\n").unwrap();
        assert_eq!(p.names[copy], "Default 2");
        assert_eq!(fs::read_to_string(dir.join(DIR).join("Default 2.txt")).unwrap(), "b\n");
        assert_eq!(p.active_name(), "Default", "a copy is not put on");
        // A taken name is refused; a free one moves the file, and keeps the one in use.
        assert!(!p.rename(copy, "default"));
        assert!(p.rename(p.active, "Bedwars"));
        assert_eq!(p.active_name(), "Bedwars");
        assert!(dir.join(DIR).join("Bedwars.txt").exists());
        // Deleting the one in use puts on the one left; the last one stays.
        let i = p.active;
        assert_eq!(p.delete(i), Ok(Some("b\n".into())));
        assert_eq!(p.names, ["Default 2"]);
        assert_eq!(p.delete(0), Err(()));
        let _ = fs::remove_dir_all(dir);
    }
}
