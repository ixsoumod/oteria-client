//! Where things are installed. Every folder can be moved with an environment variable, so tests
//! run in scratch folders and never touch a player's install.
use std::path::{Path, PathBuf};

pub const LAUNCHER_EXE: &str = "OteriaLauncher.exe";
pub const UPDATER_EXE: &str = "updater.exe";
/// The next launcher, downloaded before `updater.exe self-update` swaps it in.
pub const LAUNCHER_NEW_EXE: &str = "OteriaLauncher.new.exe";
/// The game and its resource preparer, as the client package names them
/// (`tools/oteria/publish_beta.ps1` renames Horizon's `horizon-client.exe`).
pub const CLIENT_EXE: &str = "Oteria.exe";
pub const PREPARE_EXE: &str = "oteria-prepare.exe";

/// Where updates come from when nothing else says: the `beta` release of Oteria's GitHub
/// repository, which `tools/oteria/publish_beta.ps1` keeps up to date. `OTERIA_DEFAULT_SOURCE`
/// at build time replaces it.
pub const DEFAULT_SOURCE: &str = match option_env!("OTERIA_DEFAULT_SOURCE") {
    Some(s) => s,
    None => "https://github.com/ixsoumod/oteria-client/releases/download/beta/manifest.json",
};

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The install folder: `OTERIA_HOME`, else `%LOCALAPPDATA%\Oteria`.
pub fn home() -> PathBuf {
    env_dir("OTERIA_HOME").unwrap_or_else(|| {
        env_dir("LOCALAPPDATA")
            .unwrap_or_else(std::env::temp_dir)
            .join("Oteria")
    })
}

/// The game folder (saves, options, skins): `OTERIA_GAME_DIR`, else `%APPDATA%\.oteria`, the
/// folder `oteria.cmd` plays in.
pub fn game_dir() -> PathBuf {
    env_dir("OTERIA_GAME_DIR").unwrap_or_else(|| {
        env_dir("APPDATA")
            .unwrap_or_else(std::env::temp_dir)
            .join(".oteria")
    })
}

pub fn state_file(home: &Path) -> PathBuf {
    home.join("state.json")
}
pub fn clients_dir(home: &Path) -> PathBuf {
    home.join("clients")
}
pub fn client_dir(home: &Path, version: &str) -> PathBuf {
    clients_dir(home).join(version)
}
/// The resources prepared from the player's own Minecraft (or given by the source).
pub fn resources_dir(home: &Path) -> PathBuf {
    home.join("resources")
}
/// Whether a folder holds prepared resources the game accepts.
pub fn resources_ready(dir: &Path) -> bool {
    dir.join("horizon-resources.json").is_file()
}
