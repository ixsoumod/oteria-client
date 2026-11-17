//! `state.json` in the install folder: what is installed, where updates come from, and the
//! accounts. A Microsoft account's refresh token is not here: it is in Windows' Credential
//! Manager (`microsoft::store_refresh`), so this file holds nothing secret.
use std::path::{Path, PathBuf};

/// A Microsoft account that signed in: its Minecraft profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MsAccount {
    /// The profile UUID, without dashes.
    pub uuid: String,
    pub name: String,
}

/// An account as the launcher lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRef {
    /// What `State::account` holds: the name of a local account, `ms:<uuid>` for a Microsoft one.
    pub id: String,
    pub name: String,
    pub microsoft: bool,
}

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
    /// The local (offline) accounts: player names.
    pub accounts: Vec<String>,
    /// The Microsoft accounts that signed in.
    pub ms_accounts: Vec<MsAccount>,
    /// The chosen account: see [`AccountRef::id`].
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

    /// Every account: the Microsoft ones first.
    pub fn all_accounts(&self) -> Vec<AccountRef> {
        let ms = self.ms_accounts.iter().map(|a| AccountRef {
            id: format!("ms:{}", a.uuid),
            name: a.name.clone(),
            microsoft: true,
        });
        let local = self.accounts.iter().map(|n| AccountRef {
            id: n.clone(),
            name: n.clone(),
            microsoft: false,
        });
        ms.chain(local).collect()
    }

    /// The chosen account, else the first, else none.
    pub fn current(&self) -> Option<AccountRef> {
        let all = self.all_accounts();
        let chosen = self
            .account
            .as_ref()
            .and_then(|id| all.iter().find(|a| &a.id == id));
        chosen.or_else(|| all.first()).cloned()
    }

    /// The Microsoft account with this UUID.
    pub fn ms_account(&self, uuid: &str) -> Option<&MsAccount> {
        self.ms_accounts.iter().find(|a| a.uuid == uuid)
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

    /// Adds a Microsoft account (or updates its name, when it changed) and chooses it.
    pub fn add_ms_account(&mut self, uuid: &str, name: &str) {
        match self.ms_accounts.iter_mut().find(|a| a.uuid == uuid) {
            Some(a) => a.name = name.to_owned(),
            None => self.ms_accounts.push(MsAccount {
                uuid: uuid.to_owned(),
                name: name.to_owned(),
            }),
        }
        self.account = Some(format!("ms:{uuid}"));
    }

    /// Removes an account by its id, never the last one; the first one left is chosen if it was
    /// the chosen one.
    pub fn remove_account(&mut self, id: &str) {
        if self.all_accounts().len() <= 1 {
            return;
        }
        match id.strip_prefix("ms:") {
            Some(uuid) => self.ms_accounts.retain(|a| a.uuid != uuid),
            None => self.accounts.retain(|a| a != id),
        }
        if self.account.as_deref() == Some(id) {
            self.account = self.all_accounts().first().map(|a| a.id.clone());
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
        assert_eq!(s.add_account("ms:abc"), Err(AccountError::Invalid));
        assert_eq!(s.add_account(""), Err(AccountError::Invalid));
        assert_eq!(s.add_account("Player12345"), Ok(()));
        assert_eq!(s.current().unwrap().name, "Player12345");
        s.remove_account("Player12345");
        assert_eq!(s.current().unwrap().name, "Ixsou_mod");
        // The last account stays.
        s.remove_account("Ixsou_mod");
        assert_eq!(s.accounts.len(), 1);
    }

    #[test]
    fn a_microsoft_account_is_chosen_updated_and_removed_by_its_id() {
        let mut s = State::default();
        s.add_account("Local1").unwrap();
        s.add_ms_account("069a79f4", "Alex");
        let cur = s.current().unwrap();
        assert!(cur.microsoft && cur.name == "Alex" && cur.id == "ms:069a79f4");
        // Signing in again keeps one entry, with the name it has now.
        s.add_ms_account("069a79f4", "Alex2");
        assert_eq!(s.ms_accounts.len(), 1);
        assert_eq!(s.current().unwrap().name, "Alex2");
        // Microsoft accounts are listed first.
        assert!(s.all_accounts()[0].microsoft);
        s.remove_account("ms:069a79f4");
        assert!(s.ms_accounts.is_empty());
        assert_eq!(s.current().unwrap().name, "Local1");
    }

    #[test]
    fn state_files_from_before_microsoft_accounts_still_load() {
        let s: State = serde_json::from_str(
            r#"{"launcher_version":"1.0.2","accounts":["Player1"],"account":"Player1"}"#,
        )
        .unwrap();
        assert_eq!(s.current().unwrap().name, "Player1");
        assert!(s.ms_accounts.is_empty());
    }
}
