//! The player's own download of Minecraft: Java Edition from Mojang's servers, as Lunar, Feather
//! or Horizon's launcher get it: the version file, the client jar, the libraries, the assets
//! (sounds, languages), the server jar the preparer reads, and Mojang's Java runtime. Each file is
//! checked against the SHA-1 Mojang lists (the version file against `research-lock.json`'s pin).
//! Nothing of Mojang's is part of Oteria's packages.
//!
//! Ported from Horizon's launcher (`launcher/src/mojang.rs`, MIT). Files go into
//! `<home>/minecraft` in the official launcher's layout, where `oteria-prepare --minecraft-dir`
//! reads them and finds the Java runtime. A file the player's own `.minecraft` already holds
//! whole is copied instead of downloaded. A download stopped half way keeps the files it finished.
//!
//! The game is meant to be owned: once the launcher signs in with Microsoft, this runs only for
//! an account that owns it, as Horizon's does.

use serde_json::Value;
use sha1::Digest as _;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Duration;

const OBJECTS: &str = "https://resources.download.minecraft.net";
/// The index of Mojang's Java runtimes, the one the official launcher reads.
const JAVA_RUNTIMES: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const JOBS: usize = 8;

/// What `research-lock.json` pins.
pub struct Pins {
    pub version: String,
    pub manifest_sha1: String,
    pub client_sha1: String,
    pub server_sha1: String,
}

impl Pins {
    pub fn read(client: &Path) -> Result<Self, String> {
        let lock: Value = fs::read_to_string(client.join("research-lock.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .ok_or("research-lock.json manque dans le client")?;
        let pin = |k: &str| {
            lock["minecraft"][k]
                .as_str()
                .map(str::to_owned)
                .ok_or(format!("research-lock.json : {k} manque"))
        };
        Ok(Self {
            version: pin("version")?,
            manifest_sha1: pin("manifest_sha1")?,
            client_sha1: pin("client_sha1")?,
            server_sha1: pin("server_sha1")?,
        })
    }
}

/// One file to get, with the size Mojang lists for it when it lists one.
struct File {
    url: String,
    sha1: String,
    size: Option<u64>,
    path: PathBuf,
    /// The same file in the player's own `.minecraft`, copied when it is whole.
    local: Option<PathBuf>,
}

/// What the download reports: a line, and files done of how many.
pub trait Report {
    fn note(&mut self, text: String);
    fn files(&mut self, done: usize, total: usize);
}

/// The official launcher's folder, whose files are reused when they are whole.
fn official_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(".minecraft"))
}

/// Downloads the pinned version into `dir`: the game, its libraries, its assets and the server
/// jar. Returns the server jar's path.
pub fn download(pins: &Pins, dir: &Path, report: &mut dyn Report) -> Result<PathBuf, String> {
    let v = &pins.version;
    let official = official_dir();
    let theirs = |rel: &Path| official.as_ref().map(|o| o.join(rel));
    let versions = dir.join("versions").join(v);
    let manifest_path = versions.join(format!("{v}.json"));
    // Mojang's own version file, the one research-lock.json pins (a launcher's rewrite is refused
    // by the preparer).
    if sha1_file(&manifest_path).as_deref() != Some(pins.manifest_sha1.as_str()) {
        report.note(format!("Téléchargement de Minecraft {v} chez Mojang…"));
        let url = format!(
            "https://piston-meta.mojang.com/v1/packages/{}/{v}.json",
            pins.manifest_sha1
        );
        let body = fetch_checked(&url, &pins.manifest_sha1, None)?;
        write_file(&manifest_path, &body)?;
    }
    let manifest = read_json(&manifest_path)?;
    let client = &manifest["downloads"]["client"];
    let server = &manifest["downloads"]["server"];
    let jar = PathBuf::from("versions").join(v).join(format!("{v}.jar"));
    let server_jar = dir.join(format!("minecraft-server-{v}.jar"));
    let mut game = vec![
        File {
            url: text(client, "url")?,
            sha1: pins.client_sha1.clone(),
            size: client["size"].as_u64(),
            path: dir.join(&jar),
            local: theirs(&jar),
        },
        File {
            url: text(server, "url")?,
            sha1: pins.server_sha1.clone(),
            size: server["size"].as_u64(),
            path: server_jar.clone(),
            local: None,
        },
    ];
    let libraries = manifest["libraries"]
        .as_array()
        .ok_or("La version de Mojang n'a pas de bibliothèques")?;
    for library in libraries.iter().filter(|l| allowed(l, "windows")) {
        let native = native_classifier(library, "windows")
            .map(|key| &library["downloads"]["classifiers"][key.as_str()]);
        for artifact in [Some(&library["downloads"]["artifact"]), native]
            .into_iter()
            .flatten()
            .filter(|a| !a.is_null())
        {
            let rel = PathBuf::from("libraries").join(relative(&text(artifact, "path")?)?);
            game.push(File {
                url: text(artifact, "url")?,
                sha1: text(artifact, "sha1")?,
                size: artifact["size"].as_u64(),
                path: dir.join(&rel),
                local: theirs(&rel),
            });
        }
    }
    let logging = &manifest["logging"]["client"]["file"];
    if !logging.is_null() {
        let rel = PathBuf::from("assets/log_configs").join(relative(&text(logging, "id")?)?);
        game.push(File {
            url: text(logging, "url")?,
            sha1: text(logging, "sha1")?,
            size: logging["size"].as_u64(),
            path: dir.join(&rel),
            local: theirs(&rel),
        });
    }
    report.note(format!(
        "Minecraft {v} : le jeu et {} bibliothèques",
        game.len() - 2
    ));
    fetch_each(&game, report, true)?;

    let index = &manifest["assetIndex"];
    let index_rel =
        PathBuf::from("assets/indexes").join(relative(&format!("{}.json", text(index, "id")?))?);
    let index_file = File {
        url: text(index, "url")?,
        sha1: text(index, "sha1")?,
        size: index["size"].as_u64(),
        path: dir.join(&index_rel),
        local: theirs(&index_rel),
    };
    fetch_each(std::slice::from_ref(&index_file), report, true)?;
    let objects = objects(&read_json(&index_file.path)?, dir, official.as_deref())?;
    report.note(format!(
        "Sons et langues de Minecraft ({} fichiers)",
        objects.len()
    ));
    fetch_each(&objects, report, false)?;
    Ok(server_jar)
}

/// Mojang's Java runtime for the version (`javaVersion.component`), into `<dir>/runtime` as the
/// official launcher lays it out, where the preparer finds it. Returns its home.
pub fn java_runtime(dir: &Path, version: &str, report: &mut dyn Report) -> Result<PathBuf, String> {
    let manifest = read_json(
        &dir.join("versions")
            .join(version)
            .join(format!("{version}.json")),
    )?;
    let component = manifest["javaVersion"]["component"]
        .as_str()
        .ok_or("La version de Mojang ne dit pas quel Java il faut")?
        .to_owned();
    let platform = match std::env::consts::ARCH {
        "aarch64" => "windows-arm64",
        "x86" => "windows-x86",
        _ => "windows-x64",
    };
    let index: Value = serde_json::from_slice(&fetch_bytes(JAVA_RUNTIMES)?)
        .map_err(|e| format!("Index Java de Mojang : {e}"))?;
    let entry = &index[platform][component.as_str()][0];
    if entry.is_null() {
        return Err(format!("Mojang n'a pas de {component} pour {platform}"));
    }
    let m = &entry["manifest"];
    let files: Value =
        serde_json::from_slice(&fetch_checked(&text(m, "url")?, &text(m, "sha1")?, None)?)
            .map_err(|e| format!("Java de Mojang : {e}"))?;
    let files = files["files"]
        .as_object()
        .ok_or("Le Java de Mojang ne liste aucun fichier")?;
    let rel_home = PathBuf::from("runtime")
        .join(&component)
        .join(platform)
        .join(&component);
    let home = dir.join(&rel_home);
    let official = official_dir();
    let mut downloads = Vec::new();
    for (name, file) in files {
        if file["type"].as_str() == Some("file") {
            let raw = &file["downloads"]["raw"];
            let rel = rel_home.join(relative(name)?);
            downloads.push(File {
                url: text(raw, "url")?,
                sha1: text(raw, "sha1")?,
                size: raw["size"].as_u64(),
                path: dir.join(&rel),
                local: official.as_ref().map(|o| o.join(&rel)),
            });
        }
    }
    let name = entry["version"]["name"]
        .as_str()
        .unwrap_or(&component)
        .to_owned();
    report.note(format!(
        "Java {name} de Mojang ({} fichiers)",
        downloads.len()
    ));
    fetch_each(&downloads, report, true)?;
    Ok(home)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn text(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("La version de Mojang n'a pas de {key}"))
}

/// A path from Mojang's files, refused if it could leave the folder.
fn relative(path: &str) -> Result<PathBuf, String> {
    let safe = !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    safe.then(|| PathBuf::from(path))
        .ok_or_else(|| format!("Chemin refusé : {path:?}"))
}

/// Whether a library's rules let this system use it: the last rule that matches decides.
fn allowed(library: &Value, os: &str) -> bool {
    let Some(rules) = library["rules"].as_array() else {
        return true;
    };
    rules
        .iter()
        .rfind(|rule| rule["os"]["name"].as_str().is_none_or(|name| name == os))
        .is_some_and(|rule| rule["action"] == "allow")
}

fn native_classifier(library: &Value, os: &str) -> Option<String> {
    let arch = if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    };
    library["natives"][os]
        .as_str()
        .map(|key| key.replace("${arch}", arch))
}

/// The asset index's objects, once each.
fn objects(index: &Value, dir: &Path, official: Option<&Path>) -> Result<Vec<File>, String> {
    let objects = index["objects"]
        .as_object()
        .ok_or("L'index des sons est vide")?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for o in objects.values() {
        let Some(hash) = o["hash"].as_str() else {
            continue;
        };
        if !seen.insert(hash) {
            continue;
        }
        if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Empreinte invalide {hash:?} dans l'index des sons"));
        }
        let rel = PathBuf::from("assets/objects").join(&hash[..2]).join(hash);
        out.push(File {
            url: format!("{OBJECTS}/{}/{hash}", &hash[..2]),
            sha1: hash.to_owned(),
            size: o["size"].as_u64(),
            path: dir.join(&rel),
            local: official.map(|o| o.join(&rel)),
        });
    }
    Ok(out)
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    sha1::Sha1::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn sha1_file(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| sha1_hex(&b))
}

fn write_file(path: &Path, body: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let part = path.with_extension("part");
    fs::write(&part, body).map_err(|e| format!("{}: {e}", part.display()))?;
    let _ = fs::remove_file(path);
    fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = crate::manifest::agent()
        .get(url)
        .call()
        .map_err(|e| format!("{url} : {e}"))?;
    let mut body = Vec::new();
    resp.into_reader()
        .take(1 << 30)
        .read_to_end(&mut body)
        .map_err(|e| format!("{url} : {e}"))?;
    Ok(body)
}

/// A file from Mojang, refused unless it has the size and SHA-1 Mojang lists.
fn fetch_checked(url: &str, sha1: &str, size: Option<u64>) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    // A connection that fails now and then is tried again.
    for _ in 0..3 {
        match fetch_bytes(url) {
            Ok(body) => {
                if size.is_some_and(|s| s != body.len() as u64) {
                    last = format!("{url} : taille inattendue");
                } else if sha1_hex(&body) != sha1 {
                    last = format!("{url} : ce n'est pas le fichier de Mojang attendu");
                } else {
                    return Ok(body);
                }
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// One file: kept when whole, copied from `.minecraft` when whole there, else downloaded.
fn get(file: &File) -> Result<(), String> {
    if sha1_file(&file.path).as_deref() == Some(file.sha1.as_str()) {
        return Ok(());
    }
    if let Some(local) = &file.local {
        if let Ok(body) = fs::read(local) {
            if sha1_hex(&body) == file.sha1 {
                return write_file(&file.path, &body);
            }
        }
    }
    let body = fetch_checked(&file.url, &file.sha1, file.size)?;
    write_file(&file.path, &body)
}

/// Gets the files, `JOBS` at a time, reporting every 100 ms. With `stop_on_error` false a file
/// that fails does not stop the others (a missing sound leaves the game partly silent).
fn fetch_each(files: &[File], report: &mut dyn Report, stop_on_error: bool) -> Result<(), String> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let errors = AtomicUsize::new(0);
    let failed = Mutex::new(None::<String>);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..JOBS.clamp(1, files.len().max(1)))
            .map(|_| {
                scope.spawn(|| {
                    while let Some(file) = files.get(next.fetch_add(1, Relaxed)) {
                        if stop_on_error && failed.lock().unwrap().is_some() {
                            break;
                        }
                        if let Err(e) = get(file) {
                            errors.fetch_add(1, Relaxed);
                            failed.lock().unwrap().get_or_insert(e);
                        }
                        done.fetch_add(1, Relaxed);
                    }
                })
            })
            .collect();
        while !workers.iter().all(|w| w.is_finished()) {
            report.files(done.load(Relaxed), files.len());
            std::thread::sleep(Duration::from_millis(100));
        }
        report.files(done.load(Relaxed), files.len());
    });
    match failed.into_inner().unwrap() {
        Some(e) if !stop_on_error => {
            report.note(format!(
                "{} fichiers sur {} n'ont pas pu être téléchargés : {e}",
                errors.load(Relaxed),
                files.len()
            ));
            Ok(())
        }
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_from_mojang_stay_inside() {
        assert!(relative("com/mojang/x.jar").is_ok());
        assert!(relative("../x").is_err());
        assert!(relative("/etc/x").is_err());
        assert!(relative("a\\b").is_err());
    }

    #[test]
    fn the_last_matching_rule_decides() {
        let lib: Value = serde_json::from_str(
            r#"{"rules":[{"action":"allow"},{"action":"disallow","os":{"name":"windows"}}]}"#,
        )
        .unwrap();
        assert!(!allowed(&lib, "windows"));
        assert!(allowed(&lib, "linux"));
    }

    #[test]
    fn a_whole_file_in_minecraft_is_copied_not_downloaded() {
        let dir = std::env::temp_dir().join(format!("oteria-mojang-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("theirs.jar"), b"jar").unwrap();
        let file = File {
            // Never reached: the copy is whole.
            url: "http://127.0.0.1:9/none".into(),
            sha1: sha1_hex(b"jar"),
            size: Some(3),
            path: dir.join("ours/x.jar"),
            local: Some(dir.join("theirs.jar")),
        };
        get(&file).unwrap();
        assert_eq!(fs::read(dir.join("ours/x.jar")).unwrap(), b"jar");
        let _ = fs::remove_dir_all(&dir);
    }
}
