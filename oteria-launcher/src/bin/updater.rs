//! `updater.exe`: what the launcher asks on every start, and what replaces it.
//!
//! ```text
//! updater check [--launcher-version X]   one JSON line: what is new (oteria_dist::CheckReport)
//! updater install-client                 installs the source's client; JSON progress lines
//! updater self-update --wait-pid PID     replaces the launcher once PID has ended, then restarts it
//! ```
//!
//! The source is `state.json`'s (written by oteria-launcher-setup.exe), or `OTERIA_UPDATE_SOURCE`; the
//! install folder is `OTERIA_HOME`, else `%LOCALAPPDATA%\Oteria`.
use oteria_dist::{install, is_newer, paths, CheckReport, Manifest, Progress, State};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let home = paths::home();
    let code = match args.first().map(String::as_str) {
        Some("check") => {
            let current = flag("--launcher-version")
                .or_else(|| State::load(&home).launcher_version)
                .unwrap_or_else(|| "0".into());
            println!(
                "{}",
                serde_json::to_string(&check(&home, &current)).unwrap()
            );
            0
        }
        Some("install-client") => {
            let mut out = std::io::stdout().lock();
            let mut emit = |p: Progress| {
                let _ = writeln!(out, "{}", serde_json::to_string(&p).unwrap());
                let _ = out.flush();
            };
            match install::install_client(&home, &mut emit) {
                Ok(_) => 0,
                Err(message) => {
                    emit(Progress::Error { message });
                    1
                }
            }
        }
        Some("self-update") => {
            let pid = flag("--wait-pid").and_then(|p| p.parse().ok());
            match self_update(&home, pid, !args.iter().any(|a| a == "--no-restart")) {
                Ok(v) => {
                    log(&home, &format!("launcher mis à jour vers {v}"));
                    0
                }
                Err(e) => {
                    log(&home, &format!("échec de la mise à jour du launcher : {e}"));
                    // The launcher comes back anyway, still the old one.
                    if pid.is_some() {
                        let _ = start_launcher(&home);
                    }
                    1
                }
            }
        }
        Some("--version") => {
            println!("updater {}", oteria_dist::LAUNCHER_VERSION);
            0
        }
        _ => {
            eprintln!(
                "updater {}\n\n  updater check [--launcher-version X]\n  updater install-client\n  updater self-update --wait-pid PID [--no-restart]",
                oteria_dist::LAUNCHER_VERSION
            );
            2
        }
    };
    std::process::exit(code);
}

/// What the source has that is newer than what is installed.
fn check(home: &Path, launcher_current: &str) -> CheckReport {
    let mut state = State::load(home);
    let mut report = CheckReport {
        launcher_current: launcher_current.to_owned(),
        client_installed: state.client_dir(home).and(state.client_version.clone()),
        ..CheckReport::default()
    };
    let Some(source) = state.source() else {
        report.error = Some("Aucune source de mise à jour configurée".into());
        report.client_update = report.client_installed.is_none();
        return report;
    };
    let manifest = match Manifest::load(&source) {
        Ok(m) => m,
        Err(e) => {
            report.error = Some(e);
            report.client_update = report.client_installed.is_none();
            return report;
        }
    };
    if is_newer(&manifest.launcher.version, launcher_current) {
        report.launcher_update = Some(manifest.launcher.version.clone());
    }
    report.client_latest = Some(manifest.client.version.clone());
    report.client_update = match &report.client_installed {
        None => true,
        Some(v) => is_newer(&manifest.client.version, v),
    };
    report.news = manifest.news.clone();
    report.servers = manifest.servers.clone();
    report.updater_updated = update_myself(home, &mut state, &manifest);
    report
}

/// The updater replaces itself when the source has a newer one, only when it runs from the
/// install folder (never a developer's build).
fn update_myself(home: &Path, state: &mut State, manifest: &Manifest) -> Option<String> {
    let pkg = manifest.updater.as_ref()?;
    let me = std::env::current_exe().ok()?;
    let installed = home.join(paths::UPDATER_EXE);
    if !same_file(&me, &installed) {
        return None;
    }
    let current = state
        .updater_version
        .clone()
        .unwrap_or_else(|| oteria_dist::LAUNCHER_VERSION.into());
    if !is_newer(&pkg.version, &current) {
        return None;
    }
    let new = home.join("updater.new.exe");
    install::fetch(manifest, pkg, &new, &mut |_, _| {}).ok()?;
    install::replace_exe(&new, &installed).ok()?;
    state.updater_version = Some(pkg.version.clone());
    state.save(home).ok()?;
    log(home, &format!("updater mis à jour vers {}", pkg.version));
    Some(pkg.version.clone())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Fetches the source's launcher, waits for the running one to end, swaps them, and starts it.
fn self_update(home: &Path, pid: Option<u32>, restart: bool) -> Result<String, String> {
    let mut state = State::load(home);
    let source = state.source().ok_or("aucune source")?;
    let manifest = Manifest::load(&source)?;
    let pkg = manifest.launcher.clone();
    let new = home.join("OteriaLauncher.new.exe");
    install::fetch(&manifest, &pkg, &new, &mut |_, _| {})?;
    if let Some(pid) = pid {
        if !install::wait_for_exit(pid, Duration::from_secs(20)) {
            return Err(format!("le launcher (pid {pid}) ne s'est pas fermé"));
        }
    }
    install::replace_exe(&new, &home.join(paths::LAUNCHER_EXE))?;
    state.launcher_version = Some(pkg.version.clone());
    state.save(home).map_err(|e| e.to_string())?;
    if restart {
        start_launcher(home)?;
    }
    Ok(pkg.version)
}

fn start_launcher(home: &Path) -> Result<(), String> {
    std::process::Command::new(home.join(paths::LAUNCHER_EXE))
        .arg("--updated")
        .current_dir(home)
        .spawn()
        .map(drop)
        .map_err(|e| format!("le launcher n'a pas redémarré : {e}"))
}

/// `updater.log` in the install folder: self-updates run with no window to tell.
fn log(home: &Path, line: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("updater.log"))
    {
        let _ = writeln!(f, "[{secs}] {line}");
    }
}
