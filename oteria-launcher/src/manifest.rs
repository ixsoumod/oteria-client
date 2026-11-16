//! The update source's `manifest.json`:
//!
//! ```json
//! {
//!   "launcher": { "version": "1.0.1", "file": "launcher/OteriaLauncher.exe", "sha256": "…" },
//!   "updater":  { "version": "1.0.0", "file": "launcher/updater.exe", "sha256": "…" },
//!   "client":   { "version": "0.2.0", "file": "client/oteria-client-0.2.0.zip", "sha256": "…" },
//!   "resources": { "version": "26.3", "file": "…zip", "sha256": "…" },
//!   "news": [ { "title": "…", "body": "…", "date": "16/11/2026", "tag": "Mise à jour" } ],
//!   "servers": [ { "name": "…", "address": "…", "description": "…" } ]
//! }
//! ```
//!
//! Files are relative to the manifest's folder. `resources` is optional: without it the client's
//! `oteria-prepare.exe` prepares them from the player's own Minecraft.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Package {
    pub version: String,
    pub file: String,
    pub sha256: String,
    /// The file's size in bytes, for the progress bar (read from the file when missing).
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NewsItem {
    pub title: String,
    pub body: String,
    pub date: String,
    pub tag: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PartnerServer {
    pub name: String,
    pub address: String,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub launcher: Package,
    #[serde(default)]
    pub updater: Option<Package>,
    pub client: Package,
    #[serde(default)]
    pub resources: Option<Package>,
    #[serde(default)]
    pub news: Vec<NewsItem>,
    #[serde(default)]
    pub servers: Vec<PartnerServer>,
    /// The manifest's own folder (a path, or a URL ending before `manifest.json`), which
    /// package files are relative to.
    #[serde(skip)]
    pub base: String,
}

/// Where a package's file is.
pub enum Location {
    Path(PathBuf),
    Url(String),
}

pub fn is_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// The HTTP client every download goes through: GitHub's release links redirect to its CDN.
pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(60))
        .user_agent(&format!("OteriaLauncher/{}", crate::LAUNCHER_VERSION))
        .build()
}

impl Manifest {
    /// Reads the source: a `manifest.json` on disk or online (`https://…/manifest.json`), a
    /// folder holding one, or a `file:///` URL of either.
    pub fn load(source: &str) -> Result<Self, String> {
        let (text, base) = if is_url(source) {
            let text = agent()
                .get(source)
                .call()
                .map_err(|e| format!("Source de mise à jour injoignable ({source}) : {e}"))?
                .into_string()
                .map_err(|e| format!("Source de mise à jour illisible ({source}) : {e}"))?;
            let base = source
                .rsplit_once('/')
                .map_or(source, |(dir, _)| dir)
                .to_owned();
            (text, base)
        } else {
            let path = PathBuf::from(source.strip_prefix("file:///").unwrap_or(source));
            let file = if path.is_dir() {
                path.join("manifest.json")
            } else {
                path
            };
            let text = std::fs::read_to_string(&file).map_err(|e| {
                format!("Source de mise à jour illisible ({}) : {e}", file.display())
            })?;
            let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
            (text, base.display().to_string())
        };
        // A manifest saved by Windows PowerShell 5 starts with a byte order mark.
        let mut m: Manifest = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("Manifeste invalide ({source}) : {e}"))?;
        m.base = base;
        Ok(m)
    }

    /// Where a package's file is: its own URL, else relative to the manifest.
    pub fn location(&self, p: &Package) -> Location {
        if is_url(&p.file) {
            Location::Url(p.file.clone())
        } else if is_url(&self.base) {
            Location::Url(format!("{}/{}", self.base.trim_end_matches('/'), p.file))
        } else {
            Location::Path(Path::new(&self.base).join(&p.file))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_files_resolve_against_the_manifest() {
        let mut m: Manifest = serde_json::from_str(
            r#"{"launcher":{"version":"1","file":"OteriaLauncher.exe","sha256":""},
                "client":{"version":"1","file":"https://example.org/c.zip","sha256":""}}"#,
        )
        .unwrap();
        m.base = "https://github.com/o/r/releases/download/beta".into();
        assert!(matches!(m.location(&m.launcher),
            Location::Url(u) if u == "https://github.com/o/r/releases/download/beta/OteriaLauncher.exe"));
        assert!(
            matches!(m.location(&m.client), Location::Url(u) if u == "https://example.org/c.zip")
        );
        m.base = r"C:\src".into();
        assert!(matches!(m.location(&m.launcher),
            Location::Path(p) if p == Path::new(r"C:\src\OteriaLauncher.exe")));
    }
}
