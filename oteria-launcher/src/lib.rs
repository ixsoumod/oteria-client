//! Oteria Client's distribution chain, shared by its three programs:
//!
//! - `oteria-launcher-setup.exe` installs the launcher and `updater.exe` from the update source;
//! - `OteriaLauncher.exe` (the launcher) asks `updater.exe` what is new on every start,
//!   downloads the client and starts it;
//! - `updater.exe` reads the update source, installs the client and replaces the launcher.
//!
//! The update source is a `manifest.json` (or the folder that holds one) on disk for now: it names
//! the latest launcher, updater and client, each a file beside the manifest with its SHA-256
//! ([`Manifest`]). `tools/oteria/dist_test.ps1` makes one to test with.
//!
//! The install folder ([`paths::home`], `%LOCALAPPDATA%\Oteria`) holds the launcher, the updater,
//! `state.json` ([`State`]), the clients (`clients/<version>`) and the prepared resources. The game
//! folder ([`paths::game_dir`], `%APPDATA%\.oteria`) is the one `oteria.cmd` already plays in.

pub mod install;
pub mod manifest;
pub mod mojang;
pub mod paths;
pub mod player;
pub mod state;
pub mod ui;

pub use manifest::{Manifest, NewsItem, Package, PartnerServer};
pub use state::State;

/// This build's launcher version: `OTERIA_LAUNCHER_VERSION` at build time, else the crate's.
pub const LAUNCHER_VERSION: &str = match option_env!("OTERIA_LAUNCHER_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// The game version Oteria Client plays.
pub const MINECRAFT_VERSION: &str = "26.3";

/// Whether version `a` is newer than `b`: dotted numbers compared one by one (`1.10` > `1.9`), a
/// missing part counting as 0. Anything after a `-` in a part is ignored.
pub fn is_newer(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| {
                let digits: String = p.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .collect()
    }
    let (a, b) = (parts(a), parts(b));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

/// What `updater.exe check` answers: one JSON line the launcher reads.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckReport {
    /// The launcher that asked.
    pub launcher_current: String,
    /// The source's launcher, when it is newer: the launcher updates itself at once.
    pub launcher_update: Option<String>,
    /// The client installed, if any.
    pub client_installed: Option<String>,
    /// The source's client.
    pub client_latest: Option<String>,
    /// Whether the client must be downloaded before Play: none installed, or an older one.
    pub client_update: bool,
    /// Whether the updater replaced itself during this check.
    pub updater_updated: Option<String>,
    pub news: Vec<NewsItem>,
    pub servers: Vec<PartnerServer>,
    /// Why the source could not be read; the launcher still plays an installed client.
    pub error: Option<String>,
}

/// One line of `updater.exe install-client`'s progress, as JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Progress {
    /// A stage begins (`download`, `extract`, `resources`).
    Stage {
        stage: String,
    },
    /// Progress in the current stage; `total` is 0 when it is not known.
    Bytes {
        done: u64,
        total: u64,
    },
    /// A line of what is happening (the resource preparer's steps).
    Note {
        text: String,
    },
    Done {
        version: String,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare_number_by_number() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2", "1.99"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(is_newer("v0.2.0", "0.1.5"));
    }
}
