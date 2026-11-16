//! Fetching packages from the source (copied in chunks, checked against their SHA-256),
//! unpacking the client and preparing its resources.
use crate::manifest::Location;
use crate::{paths, Manifest, Package, Progress, State};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `CREATE_NO_WINDOW`: helpers started by a windowed program open no console.
#[cfg(windows)]
pub const NO_WINDOW: u32 = 0x0800_0000;

/// A command that opens no console window on Windows.
pub fn quiet_command(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(NO_WINDOW);
    }
    cmd
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// The SHA-256 of a file, in lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(hex(&h.finalize()));
        }
        h.update(&buf[..n]);
    }
}

/// Copies a package from the source to `dest`, calling `progress(done, total)` as it goes, and
/// checks its SHA-256: a file that doesn't match is deleted and refused.
pub fn fetch(
    manifest: &Manifest,
    pkg: &Package,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    let (mut from, length): (Box<dyn Read>, Option<u64>) = match manifest.location(pkg) {
        Location::Path(src) => {
            let f = std::fs::File::open(&src).map_err(|e| {
                format!(
                    "Fichier introuvable dans la source ({}) : {e}",
                    src.display()
                )
            })?;
            let len = f.metadata().ok().map(|m| m.len());
            (Box::new(f), len)
        }
        Location::Url(url) => {
            let resp = crate::manifest::agent()
                .get(&url)
                .call()
                .map_err(|e| format!("Téléchargement impossible ({url}) : {e}"))?;
            let len = resp.header("Content-Length").and_then(|v| v.parse().ok());
            (Box::new(resp.into_reader()), len)
        }
    };
    let total = pkg.size.or(length).unwrap_or(0);
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = dest.with_extension("part");
    let mut to = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    let mut done = 0u64;
    progress(0, total);
    // Tests slow a local copy down to see its progress bar (milliseconds per megabyte).
    let slow = std::env::var("OTERIA_TEST_SLOW_FETCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());
    loop {
        if let Some(ms) = slow {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        let n = from.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        to.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done, total.max(done));
    }
    drop(to);
    let got = hex(&h.finalize());
    if !got.eq_ignore_ascii_case(pkg.sha256.trim()) {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "{} est corrompu : SHA-256 {got}, attendu {}",
            pkg.file, pkg.sha256
        ));
    }
    let _ = std::fs::remove_file(dest);
    std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))
}

/// Unpacks a ZIP into `dir`, refusing entries that would land outside it.
pub fn unzip(archive: &Path, dir: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Archive invalide : {e}"))?;
    let total: u64 = (0..zip.len())
        .filter_map(|i| zip.by_index_raw(i).ok().map(|f| f.size()))
        .sum();
    let mut done = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!(
                "Chemin refusé dans l'archive : {}",
                entry.name().unwrap_or_default()
            ));
        };
        let out = dir.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        done += std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
        progress(done, total);
    }
    Ok(())
}

/// The folder of an unpacked client that holds the game: the archive's root, or the one folder
/// it is wrapped in (`oteria-client/`).
fn client_root(dir: &Path) -> Option<PathBuf> {
    if dir.join(paths::CLIENT_EXE).is_file() {
        return Some(dir.to_path_buf());
    }
    let mut subdirs = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir());
    let only = subdirs.next()?;
    subdirs
        .next()
        .is_none()
        .then(|| only.path())
        .filter(|p| p.join(paths::CLIENT_EXE).is_file())
}

/// Installs the source's client in `clients/<version>`, then makes sure resources are ready.
/// Each step is reported through `emit`. Older clients are removed when nothing holds them.
pub fn install_client(home: &Path, emit: &mut dyn FnMut(Progress)) -> Result<String, String> {
    let mut state = State::load(home);
    let source = state
        .source()
        .ok_or("Aucune source de mise à jour : relance oteria-launcher-setup.exe")?;
    let manifest = Manifest::load(&source)?;
    let pkg = &manifest.client;
    let version = pkg.version.clone();
    let target = paths::client_dir(home, &version);

    if !target.join(paths::CLIENT_EXE).is_file() {
        emit(Progress::Stage {
            stage: "download".into(),
        });
        let cache = home.join("downloads");
        let archive = cache.join(format!("client-{version}.zip"));
        fetch(&manifest, pkg, &archive, &mut |done, total| {
            emit(Progress::Bytes { done, total })
        })?;

        emit(Progress::Stage {
            stage: "extract".into(),
        });
        let staging = paths::clients_dir(home).join(format!("{version}.partial"));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        unzip(&archive, &staging, &mut |done, total| {
            emit(Progress::Bytes { done, total })
        })?;
        let root = client_root(&staging).ok_or("L'archive du client ne contient pas Oteria.exe")?;
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&root, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_file(&archive);
        // The first start of a new 100 MB program waits for the antivirus to scan it (1.7 s
        // measured): pay that here, with a run that only prints and exits, not after Play.
        let _ = quiet_command(target.join(paths::CLIENT_EXE))
            .arg("--launch-api")
            .current_dir(&target)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    emit(Progress::Stage {
        stage: "resources".into(),
    });
    ensure_resources(home, &mut state, &manifest, &target, emit)?;

    state.client_version = Some(version.clone());
    state.save(home).map_err(|e| e.to_string())?;

    // Older clients go; one still running (locked) stays until the next update.
    if let Ok(entries) = std::fs::read_dir(paths::clients_dir(home)) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy() != version.as_str() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    emit(Progress::Done {
        version: version.clone(),
    });
    Ok(version)
}

/// Prepared resources, in this order: those already used, `OTERIA_RESOURCES` (a developer's own
/// prepared folder, used where it is), the source's `resources` package, else prepared by the
/// client's `oteria-prepare.exe` from the player's own Minecraft: Java Edition.
fn ensure_resources(
    home: &Path,
    state: &mut State,
    manifest: &Manifest,
    client: &Path,
    emit: &mut dyn FnMut(Progress),
) -> Result<(), String> {
    if let Some(dir) = std::env::var_os("OTERIA_RESOURCES").map(PathBuf::from) {
        if paths::resources_ready(&dir) {
            state.resources = Some(dir);
            emit(Progress::Bytes { done: 1, total: 1 });
            return Ok(());
        }
    }
    if paths::resources_ready(&state.resources_dir(home)) {
        emit(Progress::Bytes { done: 1, total: 1 });
        return Ok(());
    }
    state.resources = None;
    let out = paths::resources_dir(home);
    if let Some(pkg) = &manifest.resources {
        let archive = home.join("downloads").join("resources.zip");
        fetch(manifest, pkg, &archive, &mut |d, t| {
            emit(Progress::Bytes { done: d, total: t })
        })?;
        unzip(&archive, &out, &mut |d, t| {
            emit(Progress::Bytes { done: d, total: t })
        })?;
        let _ = std::fs::remove_file(&archive);
        return if paths::resources_ready(&out) {
            Ok(())
        } else {
            Err("Le paquet de ressources est incomplet".into())
        };
    }
    // Minecraft itself and the Java it runs on, from Mojang (mojang.rs).
    let minecraft = home.join("minecraft");
    let pins = crate::mojang::Pins::read(client)?;
    emit(Progress::Stage {
        stage: "minecraft".into(),
    });
    let server_jar = crate::mojang::download(&pins, &minecraft, &mut Report(emit))?;
    emit(Progress::Stage {
        stage: "java".into(),
    });
    let java = crate::mojang::java_runtime(&minecraft, &pins.version, &mut Report(emit))?;
    emit(Progress::Stage {
        stage: "resources".into(),
    });
    emit(Progress::Note {
        text: "Préparation des ressources depuis ton Minecraft: Java Edition…".into(),
    });
    let mut child = quiet_command(client.join(paths::PREPARE_EXE))
        .args([
            "--prepare",
            "--complete",
            "--progress",
            "json",
            "--server-jar",
        ])
        .arg(&server_jar)
        .arg("--minecraft-dir")
        .arg(&minecraft)
        .arg("--java-home")
        .arg(&java)
        .arg("--root")
        .arg(client)
        .arg("--output")
        .arg(&out)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("oteria-prepare n'a pas démarré : {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let mut last_error = None;
    for line in std::io::BufRead::lines(std::io::BufReader::new(stdout)).map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(f) = v.get("fraction").and_then(|f| f.as_f64()) {
            emit(Progress::Bytes {
                done: (f * 1000.0) as u64,
                total: 1000,
            });
        } else if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
            emit(Progress::Note { text: m.to_owned() });
        } else if let Some(m) = v.get("error").and_then(|m| m.as_str()) {
            last_error = Some(m.to_owned());
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() && paths::resources_ready(&out) {
        // The sounds are now in the prepared resources too (`--complete`): Mojang's copy goes
        // (about 400 MB), downloaded again only if a later client prepares anew.
        let _ = std::fs::remove_dir_all(minecraft.join("assets").join("objects"));
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| {
            "La préparation des ressources a échoué : il faut Minecraft: Java Edition 26.3, lancé une fois depuis le launcher officiel.".into()
        }))
    }
}

/// Waits until process `pid` has ended (at most `timeout`).
pub fn wait_for_exit(pid: u32, timeout: std::time::Duration) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
        };
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if h.is_null() {
            return true; // already gone
        }
        let r = WaitForSingleObject(h, timeout.as_millis().min(u32::MAX as u128) as u32);
        CloseHandle(h);
        r == WAIT_OBJECT_0
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, timeout);
        std::thread::sleep(std::time::Duration::from_millis(500));
        true
    }
}

/// Replaces `target` (which may be a running program: Windows lets it be renamed, not
/// overwritten) with `new`. The old one is left as `<name>.old.exe` and removed next time.
pub fn replace_exe(new: &Path, target: &Path) -> Result<(), String> {
    let old = target.with_extension("old.exe");
    let _ = std::fs::remove_file(&old);
    if target.exists() {
        std::fs::rename(target, &old).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    if let Err(e) = std::fs::rename(new, target) {
        let _ = std::fs::rename(&old, target);
        return Err(format!("{}: {e}", target.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oteria-dist-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_package_that_does_not_match_its_checksum_is_refused() {
        let dir = scratch("fetch");
        std::fs::write(dir.join("a.bin"), b"oteria").unwrap();
        let mut m: Manifest = serde_json::from_str(
            r#"{"launcher":{"version":"1","file":"a.bin","sha256":"00"},
                "client":{"version":"1","file":"a.bin","sha256":"00"}}"#,
        )
        .unwrap();
        m.base = dir.display().to_string();
        let out = dir.join("out/a.bin");
        assert!(fetch(&m, &m.launcher, &out, &mut |_, _| {}).is_err());
        assert!(!out.exists() && !out.with_extension("part").exists());
        m.launcher.sha256 = sha256_hex(b"oteria");
        let mut last = (0, 0);
        fetch(&m, &m.launcher, &out, &mut |d, t| last = (d, t)).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"oteria");
        assert_eq!(last, (6, 6));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_running_program_is_replaced_by_renaming_it() {
        let dir = scratch("replace");
        let (t, n) = (dir.join("x.exe"), dir.join("x.new.exe"));
        std::fs::write(&t, b"v1").unwrap();
        std::fs::write(&n, b"v2").unwrap();
        replace_exe(&n, &t).unwrap();
        assert_eq!(std::fs::read(&t).unwrap(), b"v2");
        assert_eq!(std::fs::read(t.with_extension("old.exe")).unwrap(), b"v1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The Mojang download's report, as progress lines.
struct Report<'a>(&'a mut dyn FnMut(Progress));

impl crate::mojang::Report for Report<'_> {
    fn note(&mut self, text: String) {
        (self.0)(Progress::Note { text });
    }
    fn files(&mut self, done: usize, total: usize) {
        (self.0)(Progress::Bytes {
            done: done as u64,
            total: total as u64,
        });
    }
}
