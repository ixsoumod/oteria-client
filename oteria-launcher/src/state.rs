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
    /// The local (offline) accounts: player names. The game is started with the chosen one.
    pub accounts: Vec<String>,
    /// The chosen account, one of `accounts`.
    pub account: Option<String>,
}

/// Why a local account can't be added.
#[derive(Debug, PartialEq, Eq)]
pub enum AccountError {
    /// Not 1 to 16 letters, digits or underscores: vanilla's player name rule.
    Invalid,
    Exists,
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

    /// The chosen account, else the first, else none.
    pub fn current_account(&self) -> Option<&str> {
        self.account
            .as_deref()
            .filter(|a| self.accounts.iter().any(|x| x == a))
            .or_else(|| self.accounts.first().map(String::as_str))
    }

    /// Adds a local account and chooses it.
    pub fn add_account(&mut self, name: &str) -> Result<(), AccountError> {
        let name = name.trim();
        if !crate::player::is_valid(name) {
            return Err(AccountError::Invalid);
        }
        if self.accounts.iter().any(|a| a.eq_ignore_ascii_case(name)) {
            return Err(AccountError::Exists);
        }
        self.accounts.push(name.to_owned());
        self.account = Some(name.to_owned());
        Ok(())
    }

    /// Removes an account, never the last one; the first one left is chosen if it was chosen.
    pub fn remove_account(&mut self, name: &str) {
        if self.accounts.len() > 1 {
            self.accounts.retain(|a| a != name);
            if self.account.as_deref() == Some(name) {
                self.account = self.accounts.first().cloned();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_accounts_follow_the_player_name_rule() {
        let mut s = State::default();
        assert_eq!(s.add_account("Ixsou_mod"), Ok(()));
        assert_eq!(s.add_account("ixsou_MOD"), Err(AccountError::Exists));
        assert_eq!(s.add_account("a b"), Err(AccountError::Invalid));
        assert_eq!(s.add_account(""), Err(AccountError::Invalid));
        assert_eq!(s.add_account("Player12345"), Ok(()));
        assert_eq!(s.current_account(), Some("Player12345"));
        s.remove_account("Player12345");
        assert_eq!(s.current_account(), Some("Ixsou_mod"));
        // The last account stays.
        s.remove_account("Ixsou_mod");
        assert_eq!(s.accounts.len(), 1);
    }
}
