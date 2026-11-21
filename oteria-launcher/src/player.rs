//! The player shown in the launcher: the offline name the game keeps in `oteria-player.txt`
//! (`horizon_client::offline_name`, the same file and the same `PlayerXXXXX` form), and the skin
//! the player wears in Oteria (`oteria-skins/`, newest first in `history.txt`).
use std::path::{Path, PathBuf};

const FILE: &str = "oteria-player.txt";
/// Its name before Oteria renamed the game's files (the game renames it on its next start).
const OLD_FILE: &str = "horizon-player.txt";
const KEY: &str = "name:";

/// Vanilla's player name rule (`StringUtil.isValidPlayerName`): 1 to 16 of `[A-Za-z0-9_]`.
pub fn is_valid(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// `Player` and five digits, as the game makes them.
pub fn generate(seed: u64) -> String {
    format!("Player{:05}", seed % 100_000)
}

/// The name kept in the game folder, created there on first use as the game would.
pub fn load_or_create(game_dir: &Path) -> String {
    let path = game_dir.join(FILE);
    let text = std::fs::read_to_string(&path)
        .or_else(|_| std::fs::read_to_string(game_dir.join(OLD_FILE)));
    if let Some(name) = text.ok().and_then(|t| {
        t.lines()
            .find_map(|l| l.trim_end_matches('\r').strip_prefix(KEY))
            .filter(|n| is_valid(n))
            .map(str::to_owned)
    }) {
        return name;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let mut x = nanos ^ u64::from(std::process::id()).rotate_left(32);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    let name = generate(x ^ (x >> 33));
    let _ = std::fs::create_dir_all(game_dir);
    let _ = std::fs::write(&path, format!("{KEY}{name}\n"));
    name
}

/// The skin the player wears in Oteria, as the game reads it: the `playerSkin` line of
/// `oteria-extras.txt` (the id of a PNG in `oteria-skins`, the Cosmetics page's choice). `None`
/// when none is worn there: the account's own skin shows. (The history's first skin is only the
/// last one added, not necessarily the one worn.)
pub fn worn_skin(game_dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(game_dir.join(EXTRAS)).ok()?;
    let id = text
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("playerSkin:"))?
        .trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let png = game_dir.join("oteria-skins").join(format!("{id}.png"));
    png.is_file().then_some(png)
}

/// The game's own settings, where it keeps the worn skin.
pub const EXTRAS: &str = "oteria-extras.txt";

/// What says the worn skin may have changed: the settings' last write.
pub fn worn_skin_stamp(game_dir: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(game_dir.join(EXTRAS)).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_name_is_kept_and_read_back() {
        let dir = std::env::temp_dir().join(format!("oteria-player-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = load_or_create(&dir);
        assert!(first.starts_with("Player") && first.len() == 11 && is_valid(&first));
        assert_eq!(load_or_create(&dir), first);
        std::fs::write(dir.join(FILE), "name:Ixsou_mod\n").unwrap();
        assert_eq!(load_or_create(&dir), "Ixsou_mod");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_worn_skin_is_the_games_choice_not_the_newest() {
        let dir = std::env::temp_dir().join(format!("oteria-worn-{}", std::process::id()));
        let skins = dir.join("oteria-skins");
        std::fs::create_dir_all(&skins).unwrap();
        for id in ["aa11", "bb22"] {
            std::fs::write(skins.join(format!("{id}.png")), b"png").unwrap();
        }
        // The history's first skin is the newest, not the one worn.
        std::fs::write(skins.join("history.txt"), "bb22	new	classic
aa11	old	classic
").unwrap();
        assert_eq!(worn_skin(&dir), None);
        std::fs::write(dir.join(EXTRAS), "skinKnife:true
playerSkin:aa11
").unwrap();
        assert_eq!(worn_skin(&dir), Some(skins.join("aa11.png")));
        std::fs::write(dir.join(EXTRAS), "playerSkin:
").unwrap();
        assert_eq!(worn_skin(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
