//! `state.json` in the install folder: what is installed and where updates come from.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct State {
    /// The update source: a `manifest.json` or the folder that holds it.
    pub source: Option<String>,
    /// The installed launcher (written by the setup and by self-updates).
    pub launcher_version: Option<String>,
    /// The installed updater.
    pub updater_version: Option<String>,
    /// The installed client, in `clients/<version>`.
    pub client_version: Option<String>,
    /// Prepared resources somewhere else than `resources/` (a developer's own, given by
    /// `OTERIA_RESOURCES` when the client was installed).
    pub resources: Option<PathBuf>,
    /// Close the launcher once the game has started (Settings).
    pub close_on_launch: bool,
}

impl State {
    pub fn load(home: &Path) -> Self {
        std::fs::read_to_string(crate::paths::state_file(home))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Written to a temporary file then renamed, so a crash never leaves half a file.
    pub fn save(&self, home: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(home)?;
        let path = crate::paths::state_file(home);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }

    /// The update source: `OTERIA_UPDATE_SOURCE` wins over the one the setup recorded.
    pub fn source(&self) -> Option<String> {
        std::env::var("OTERIA_UPDATE_SOURCE")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| self.source.clone())
            .or_else(|| Some(crate::paths::DEFAULT_SOURCE.to_owned()))
    }

    /// The resources the game is started with.
    pub fn resources_dir(&self, home: &Path) -> PathBuf {
        self.resources
            .clone()
            .unwrap_or_else(|| crate::paths::resources_dir(home))
    }

    /// The installed client's folder, when it is complete.
    pub fn client_dir(&self, home: &Path) -> Option<PathBuf> {
        let dir = crate::paths::client_dir(home, self.client_version.as_deref()?);
        dir.join(crate::paths::CLIENT_EXE).is_file().then_some(dir)
    }
}
