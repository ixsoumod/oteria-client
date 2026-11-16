//! Oteria's skins (the Cosmetics page's right side): the skin this player wears in Oteria, and
//! every skin they have had on this computer.
//!
//! - **A player's name**: their skin, from the Minecraft services, as anyone's head is fetched.
//! - **A file**: a PNG dropped on the window while the Cosmetics page is open, or put in the
//!   `oteria-skins/import` folder (Import opens it).
//! - **The history**: each skin is kept in `oteria-skins` (its PNG, named by its content), newest
//!   first in `history.txt`; picking one wears it again.
//!
//! Worn here, the skin replaces the account's for this client only (`skins_override`). "On my
//! account" puts it on the signed-in Microsoft account too, so that everyone sees it.

use super::Game;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A skin of the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkinEntry {
    /// Its content's hash, in hex: its file's name.
    pub id: String,
    /// Where it came from: a player's name, or the file's.
    pub name: String,
    pub slim: bool,
}

/// What a background job found.
enum Done {
    /// A skin to keep (and wear): its name, its PNG, slim.
    Skin(String, Vec<u8>, bool),
    /// It is on the account now.
    Uploaded,
    /// A language key saying what went wrong.
    Failed(&'static str),
}

/// What the page says after an action: a language key (an error, or done), and when.
#[derive(Clone, Debug)]
pub(super) struct Status {
    pub key: &'static str,
    pub ok: bool,
    pub at: Instant,
}

#[derive(Default)]
struct Store {
    loaded: bool,
    entries: Vec<SkinEntry>,
    /// Each skin's 64x64 RGBA, as the atlas takes it.
    images: std::collections::HashMap<String, Vec<u8>>,
    job: Option<mpsc::Receiver<Done>>,
    status: Option<Status>,
    /// The import folder's files already taken, and when it was last looked at.
    imported: HashSet<PathBuf>,
    looked: Option<Instant>,
}

thread_local! {
    static STORE: std::cell::RefCell<Store> = std::cell::RefCell::new(Store::default());
}

/// The most skins the history keeps.
const KEEP: usize = 40;

/// The folder the skins are kept in, beside the game's other files.
pub(super) fn skins_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join("oteria-skins")
}
/// The folder whose PNGs are taken into the history.
pub(super) fn import_dir() -> PathBuf {
    skins_dir().join("import")
}

/// The skin in `png` as the atlas takes it (64x64 RGBA), or `None` when it isn't a skin; and
/// whether its arms look slim.
fn decode(png: &[u8]) -> Option<(Vec<u8>, bool)> {
    let (w, h, rgba) = horizon_client::hud::decode_bytes(png).ok()?;
    let slim = w == 64 && h == 64 && looks_slim(&rgba);
    Some((
        horizon_client::skins::process_legacy_skin(w, h, &rgba)?,
        slim,
    ))
}
/// Whether a 64x64 skin, as drawn (before the base layer is made opaque), has slim arms: the
/// strip a wide right arm has and a slim one leaves empty.
fn looks_slim(rgba: &[u8]) -> bool {
    let alpha = |x: usize, y: usize| rgba.get((y * 64 + x) * 4 + 3).copied().unwrap_or(255);
    (20..32).all(|y| alpha(54, y) == 0 && alpha(55, y) == 0)
}
/// A content's id: its FNV-1a hash in hex.
fn content_id(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

impl Store {
    fn load(&mut self, dir: &Path) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let text = std::fs::read_to_string(dir.join("history.txt")).unwrap_or_default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let (Some(id), Some(name), Some(slim)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if !id.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }
            let Some((image, _)) = std::fs::read(dir.join(format!("{id}.png")))
                .ok()
                .and_then(|b| decode(&b))
            else {
                continue;
            };
            self.images.insert(id.to_owned(), image);
            self.entries.push(SkinEntry {
                id: id.to_owned(),
                name: name.to_owned(),
                slim: slim == "slim",
            });
        }
    }
    fn save(&self, dir: &Path) {
        let text: String = self
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{}\t{}\t{}\n",
                    e.id,
                    e.name.replace(['\t', '\n'], " "),
                    if e.slim { "slim" } else { "classic" }
                )
            })
            .collect();
        if let Err(e) = std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::write(dir.join("history.txt"), text))
        {
            log::warn!("Oteria skins: history not saved: {e}");
        }
    }
    /// Keeps `png` as a skin named `name` (first in the history); its id, or why not.
    fn add(
        &mut self,
        dir: &Path,
        name: &str,
        png: &[u8],
        slim: Option<bool>,
    ) -> Result<String, &'static str> {
        // The history read first, so the new skin goes in front of it, not before it is read.
        self.load(dir);
        let (image, seen_slim) = decode(png).ok_or("oteria.skin.error.not_a_skin")?;
        let id = content_id(png);
        let slim = slim.unwrap_or(seen_slim);
        std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::write(dir.join(format!("{id}.png")), png))
            .map_err(|_| "oteria.skin.error.not_saved")?;
        self.entries.retain(|e| e.id != id);
        self.entries.insert(
            0,
            SkinEntry {
                id: id.clone(),
                name: name.to_owned(),
                slim,
            },
        );
        for old in self.entries.split_off(KEEP.min(self.entries.len())) {
            self.images.remove(&old.id);
            let _ = std::fs::remove_file(dir.join(format!("{}.png", old.id)));
        }
        self.images.insert(id.clone(), image);
        self.save(dir);
        Ok(id)
    }
}

impl Game<'_> {
    /// The history, newest first, the skin worn (its id, empty for the account's), whether a
    /// job runs, and what the page should say.
    pub(super) fn skins_view(&self) -> (Vec<SkinEntry>, String, bool, Option<Status>) {
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            s.load(&skins_dir());
            let status = s
                .status
                .clone()
                .filter(|st| st.at.elapsed() < Duration::from_secs(6));
            (
                s.entries.clone(),
                self.settings.extras.pvp.skins.player.clone(),
                s.job.is_some(),
                status,
            )
        })
    }

    /// The atlas sprite of skin `id` (stored on first use), and whether it is slim.
    pub(super) fn skin_sprite(&mut self, id: &str) -> Option<(String, bool)> {
        let (image, slim) = STORE.with(|s| {
            let mut s = s.borrow_mut();
            s.load(&skins_dir());
            let slim = s.entries.iter().find(|e| e.id == id)?.slim;
            Some((s.images.get(id)?.clone(), slim))
        })?;
        let location = format!("horizon/oteria_skin/{id}");
        if !self.skin_slots.contains(&location) || !self.gui.atlas.sprites.contains_key(&location) {
            self.store_skin(location.clone(), image);
        }
        Some((location, slim))
    }

    /// The skin worn in Oteria, as the player's own (`update_remote_skins`): its sprite and model.
    pub(super) fn skins_override(&mut self) -> Option<(String, bool)> {
        let id = self.settings.extras.pvp.skins.player.clone();
        if id.is_empty() {
            return None;
        }
        self.skin_sprite(&id)
    }

    /// Once a frame: a job's result, and the import folder's new PNGs.
    pub(super) fn skins_frame(&mut self) {
        let dir = skins_dir();
        let done = STORE.with(|s| {
            let mut s = s.borrow_mut();
            let done = s.job.as_ref().and_then(|j| j.try_recv().ok());
            if done.is_some() {
                s.job = None;
            }
            done
        });
        match done {
            Some(Done::Skin(name, png, slim)) => self.skin_keep(&name, &png, Some(slim)),
            Some(Done::Uploaded) => self.skin_status("oteria.skin.uploaded", true),
            Some(Done::Failed(key)) => self.skin_status(key, false),
            None => {}
        }
        // The import folder, every two seconds while the menu is open.
        let look = STORE.with(|s| {
            let mut s = s.borrow_mut();
            let due = s
                .looked
                .is_none_or(|t| t.elapsed() > Duration::from_secs(2));
            if due {
                s.looked = Some(Instant::now());
            }
            due
        });
        if !look {
            return;
        }
        let Ok(files) = std::fs::read_dir(dir.join("import")) else {
            return;
        };
        for path in files.flatten().map(|f| f.path()) {
            let png = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"));
            let new = STORE.with(|s| s.borrow_mut().imported.insert(path.clone()));
            if png && new {
                self.skin_import(&path);
            }
        }
    }

    /// Wears `id` in Oteria (empty: the account's skin again).
    pub(super) fn skin_wear(&mut self, id: &str) {
        self.settings.extras.pvp.skins.player = id.to_owned();
        self.settings.save(&Self::options_path());
    }

    /// The skin of the player named `name`, fetched in the background, then kept and worn.
    pub(super) fn skin_from_name(&mut self, name: &str) {
        let name = name.trim().to_owned();
        let valid = (1..=16).contains(&name.len())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return self.skin_status("oteria.skin.error.bad_name", false);
        }
        self.skin_job(move || match horizon_client::skins::skin_by_name(&name) {
            Ok((png, slim)) => Done::Skin(name, png, slim),
            Err(key) => Done::Failed(key),
        });
    }

    /// A skin file (dropped on the window, or in the import folder), kept and worn.
    pub(super) fn skin_import(&mut self, path: &Path) {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::read(path) {
            Ok(png) => self.skin_keep(&name, &png, None),
            Err(_) => self.skin_status("oteria.skin.error.not_a_skin", false),
        }
    }

    /// The import folder, made and opened in the file explorer.
    pub(super) fn skin_open_import(&mut self) {
        let dir = import_dir();
        let _ = std::fs::create_dir_all(&dir);
        if open::that_detached(&dir).is_err() {
            self.skin_status("oteria.skin.error.no_folder", false);
        }
    }

    /// The skin worn put on the signed-in account (in the background).
    pub(super) fn skin_upload(&mut self) {
        let Some(session) = self.auth() else {
            return self.skin_status("oteria.skin.error.signed_out", false);
        };
        let id = self.settings.extras.pvp.skins.player.clone();
        let entry = STORE.with(|s| s.borrow().entries.iter().find(|e| e.id == id).cloned());
        let Some(entry) = entry else {
            return self.skin_status("oteria.skin.error.nothing_to_send", false);
        };
        let Ok(png) = std::fs::read(skins_dir().join(format!("{}.png", entry.id))) else {
            return self.skin_status("oteria.skin.error.not_a_skin", false);
        };
        self.skin_job(move || {
            match horizon_client::auth::upload_skin(&session, &png, entry.slim) {
                Ok(()) => Done::Uploaded,
                Err(key) => Done::Failed(key),
            }
        });
    }

    /// Whether the account can take a skin (signed in with Microsoft).
    pub(super) fn skin_can_upload(&self) -> bool {
        self.auth().is_some()
    }

    fn skin_keep(&mut self, name: &str, png: &[u8], slim: Option<bool>) {
        let kept = STORE.with(|s| s.borrow_mut().add(&skins_dir(), name, png, slim));
        match kept {
            Ok(id) => {
                self.skin_wear(&id);
                self.skin_status("oteria.skin.worn", true);
            }
            Err(key) => self.skin_status(key, false),
        }
    }

    fn skin_job(&mut self, job: impl FnOnce() -> Done + Send + 'static) {
        let busy = STORE.with(|s| s.borrow().job.is_some());
        if busy {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("oteria skin".into())
            .spawn(move || {
                let _ = tx.send(job());
            })
            .ok();
        STORE.with(|s| s.borrow_mut().job = Some(rx));
    }

    fn skin_status(&mut self, key: &'static str, ok: bool) {
        STORE.with(|s| {
            s.borrow_mut().status = Some(Status {
                key,
                ok,
                at: Instant::now(),
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64x64 skin PNG: wide arms, or slim (the strip past a slim arm left empty).
    fn skin_png(slim: bool) -> Vec<u8> {
        let mut rgba = vec![200u8; 64 * 64 * 4];
        if slim {
            for y in 16..32 {
                for x in 54..56 {
                    rgba[(y * 64 + x) * 4 + 3] = 0;
                }
            }
        }
        let mut png = Vec::new();
        let mut e = png::Encoder::new(&mut png, 64, 64);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&rgba).unwrap();
        png
    }

    /// Asks the real services (network): `cargo test -p horizon-client --lib skin_of_a_name --
    /// --ignored`.
    #[test]
    #[ignore]
    fn skin_of_a_name() {
        let (png, _slim) = horizon_client::skins::skin_by_name("jeb_").expect("jeb_'s skin");
        assert!(decode(&png).is_some(), "a skin");
        assert_eq!(
            horizon_client::skins::skin_by_name("zz_no_such_player_zz").err(),
            Some("oteria.skin.error.no_player")
        );
    }

    #[test]
    fn skins_are_kept_newest_first_once_each_and_read_back() {
        let dir = std::env::temp_dir().join(format!("oteria-skins-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Store {
            loaded: true,
            ..Store::default()
        };
        let wide = s.add(&dir, "Wide", &skin_png(false), None).unwrap();
        let slim = s.add(&dir, "Slim", &skin_png(true), None).unwrap();
        assert_ne!(wide, slim);
        // Again: to the front, not twice.
        s.add(&dir, "Wide again", &skin_png(false), None).unwrap();
        assert_eq!(
            s.entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["Wide again", "Slim"]
        );
        assert!(s.entries[1].slim && !s.entries[0].slim, "slim arms seen");
        assert_eq!(
            s.add(&dir, "x", b"not a png", None),
            Err("oteria.skin.error.not_a_skin")
        );
        let mut back = Store::default();
        back.load(&dir);
        assert_eq!(back.entries, s.entries);
        assert_eq!(back.images.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
