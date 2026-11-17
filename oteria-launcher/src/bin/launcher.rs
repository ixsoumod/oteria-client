//! `OteriaLauncher.exe`: Oteria Client's launcher.
//!
//! On every start it asks `updater.exe check` what is new. A newer launcher is installed at once
//! (`updater.exe self-update`, which restarts it); a newer client makes the main button
//! « Télécharger la mise à jour » before it can say « Jouer ». The game is started directly from
//! the installed client, with no console window, and can be stopped from the same button.
//!
//! Testing (`tools/oteria/dist_test.ps1`): `OTERIA_HOME`, `OTERIA_GAME_DIR` and
//! `OTERIA_UPDATE_SOURCE` move everything to scratch folders; `OTERIA_LAUNCHER_PAGE` opens a page;
//! `OTERIA_LAUNCHER_AUTO=download,play,stop` presses the main button when it can;
//! `OTERIA_LAUNCHER_SHOT` saves a screenshot once the window has settled
//! (`OTERIA_LAUNCHER_SHOT_AT=downloading|running|self-update` picks the moment) and closes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{
    self, epaint::RectShape, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, Vec2,
};
use oteria_dist::ui::{self, c, Icons};
use oteria_dist::{install, paths, player, CheckReport, NewsItem, Progress, State};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Oteria Launcher")
        .with_inner_size([1280.0, 780.0])
        .with_min_inner_size([1120.0, 700.0])
        .with_decorations(false)
        .with_icon(ui::icon_data());
    eframe::run_native(
        "Oteria Launcher",
        eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(Launcher::new(&cc.egui_ctx)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Play,
    Mods,
    Market,
    Settings,
    /// Every news item ("Tout voir" on the Play page).
    News,
}

enum Phase {
    /// Asking the updater.
    Checking,
    /// A newer launcher: the window says so, then hands over to the updater.
    /// `handed_over` once `updater.exe self-update` runs: started once, never again.
    SelfUpdating {
        version: String,
        since: Instant,
        handed_over: bool,
    },
    /// No client installed (`update` false), or an older one: Play waits for the download.
    NeedDownload {
        update: bool,
    },
    Downloading,
    Ready,
    Launching {
        since: Instant,
    },
    Running,
    /// Nothing to play and the source can't be read.
    Failed(String),
}

struct Download {
    rx: Receiver<Progress>,
    stage: String,
    fraction: f32,
    note: String,
}

struct Launcher {
    home: PathBuf,
    game_dir: PathBuf,
    state: State,
    name: String,
    skin: Option<ui::Skin>,
    logo: TextureHandle,
    icons: Icons,
    /// The four sides of the title-screen panorama from the player's prepared resources; empty
    /// before the client is installed.
    panorama: Vec<TextureHandle>,
    /// A transparent-to-dark ramp laid over the banner so its text stays readable.
    scrim: TextureHandle,
    page: Page,
    phase: Phase,
    report: CheckReport,
    check_rx: Option<Receiver<CheckReport>>,
    download: Option<Download>,
    child: Option<Child>,
    /// A message under the main button: (text, colour).
    notice: Option<(String, Color32)>,
    total_ram: Option<u64>,
    auto: Vec<String>,
    shot: Option<PathBuf>,
    shot_at: String,
    settled_frames: u32,
    shot_sent: bool,
    started: Instant,
}

impl Launcher {
    fn new(ctx: &egui::Context) -> Self {
        ui::install_fonts(ctx);
        let home = paths::home();
        let game_dir = paths::game_dir();
        for leftover in [
            "OteriaLauncher.old.exe",
            "updater.run.exe",
            "updater.old.exe",
        ] {
            let _ = std::fs::remove_file(home.join(leftover));
        }
        let state = State::load(&home);
        let name = player::load_or_create(&game_dir);
        let skin = player::worn_skin(&game_dir).and_then(|p| ui::Skin::load(&p));
        let logo = ui::svg_texture(ctx, "logo", ui::LOGO_SVG, 192, false)
            .or_else(|| ui::texture_from_png(ctx, "logo", ui::LOGO_PNG))
            .expect("logo");
        let page = match std::env::var("OTERIA_LAUNCHER_PAGE").as_deref() {
            Ok("mods") => Page::Mods,
            Ok("market") => Page::Market,
            Ok("news") => Page::News,
            Ok("settings") => Page::Settings,
            _ => Page::Play,
        };
        let env_list = |k: &str| {
            std::env::var(k)
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let updated = std::env::args().any(|a| a == "--updated");
        let panorama = load_panorama(ctx, &state.resources_dir(&home));
        let mut me = Self {
            icons: Icons::load(ctx),
            scrim: scrim_texture(ctx),
            panorama,
            home,
            game_dir,
            state,
            name,
            skin,
            logo,
            page,
            phase: Phase::Checking,
            report: CheckReport::default(),
            check_rx: None,
            download: None,
            child: None,
            notice: None,
            total_ram: memory::total(),
            auto: env_list("OTERIA_LAUNCHER_AUTO"),
            // A launcher restarted by its update takes its own picture (`…_AFTER_UPDATE`).
            shot: std::env::var_os(if updated {
                "OTERIA_LAUNCHER_SHOT_AFTER_UPDATE"
            } else {
                "OTERIA_LAUNCHER_SHOT"
            })
            .map(PathBuf::from),
            shot_at: if updated {
                String::new()
            } else {
                std::env::var("OTERIA_LAUNCHER_SHOT_AT").unwrap_or_default()
            },
            settled_frames: 0,
            shot_sent: false,
            started: Instant::now(),
        };
        if updated {
            me.notice = Some((
                format!("Launcher mis à jour : v{}", oteria_dist::LAUNCHER_VERSION),
                c::GOOD,
            ));
        }
        me.log(&format!("démarrage v{}", oteria_dist::LAUNCHER_VERSION));
        me.start_check();
        me
    }

    fn log(&self, line: &str) {
        let _ = std::fs::create_dir_all(&self.home);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.home.join("launcher.log"))
        {
            let ms = self.started.elapsed().as_millis();
            let _ = writeln!(f, "[+{ms} ms] {line}");
        }
    }

    /// `updater.exe`: beside this launcher, else in the install folder.
    fn updater(&self) -> PathBuf {
        let beside = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(paths::UPDATER_EXE)));
        beside
            .filter(|p| p.is_file())
            .unwrap_or_else(|| self.home.join(paths::UPDATER_EXE))
    }

    /// Whether this launcher is the installed one (a developer's build never replaces itself).
    fn installed(&self) -> bool {
        let me = std::env::current_exe()
            .ok()
            .and_then(|p| p.canonicalize().ok());
        let there = self.home.join(paths::LAUNCHER_EXE).canonicalize().ok();
        me.is_some() && me == there
    }

    fn start_check(&mut self) {
        self.phase = Phase::Checking;
        let (tx, rx) = channel();
        let updater = self.updater();
        std::thread::spawn(move || {
            let report = install::quiet_command(&updater)
                .args(["check", "--launcher-version", oteria_dist::LAUNCHER_VERSION])
                .stderr(Stdio::null())
                .output()
                .map_err(|e| format!("updater.exe introuvable ({}) : {e}", updater.display()))
                .and_then(|o| {
                    serde_json::from_slice::<CheckReport>(&o.stdout)
                        .map_err(|e| format!("Réponse de l'updater illisible : {e}"))
                });
            let _ = tx.send(report.unwrap_or_else(|e| CheckReport {
                error: Some(e),
                ..Default::default()
            }));
        });
        self.check_rx = Some(rx);
    }

    fn on_report(&mut self, report: CheckReport) {
        self.log(&format!(
            "vérification : launcher {:?}, client {:?} -> {:?} (maj {}), erreur {:?}",
            report.launcher_update,
            report.client_installed,
            report.client_latest,
            report.client_update,
            report.error
        ));
        self.state = State::load(&self.home);
        let installed = self.state.client_dir(&self.home).is_some();
        self.report = report;
        if let Some(v) = self.report.launcher_update.clone() {
            if self.installed() {
                self.phase = Phase::SelfUpdating {
                    version: v,
                    since: Instant::now(),
                    handed_over: false,
                };
                return;
            }
            self.notice = Some((
                format!("Launcher v{v} disponible (cette copie n'est pas installée : pas de mise à jour automatique)"),
                c::WARN,
            ));
        }
        self.phase = if let Some(e) = self.report.error.clone() {
            if installed {
                self.notice = Some((format!("Hors ligne : {e}"), c::WARN));
                Phase::Ready
            } else {
                Phase::Failed(e)
            }
        } else if self.report.client_update {
            Phase::NeedDownload { update: installed }
        } else {
            Phase::Ready
        };
    }

    fn start_download(&mut self) {
        self.log("téléchargement du client");
        let (tx, rx) = channel();
        let updater = self.updater();
        std::thread::spawn(move || {
            let child = install::quiet_command(&updater)
                .arg("install-client")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Progress::Error {
                        message: format!("updater.exe : {e}"),
                    });
                    return;
                }
            };
            let out = child.stdout.take().unwrap();
            let mut ended = false;
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(p) = serde_json::from_str::<Progress>(&line) {
                    ended |= matches!(p, Progress::Done { .. } | Progress::Error { .. });
                    let _ = tx.send(p);
                }
            }
            let _ = child.wait();
            if !ended {
                let _ = tx.send(Progress::Error {
                    message: "L'updater s'est arrêté pendant l'installation".into(),
                });
            }
        });
        self.download = Some(Download {
            rx,
            stage: "download".into(),
            fraction: 0.0,
            note: String::new(),
        });
        self.notice = None;
        self.phase = Phase::Downloading;
    }

    fn poll_download(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.download else { return };
        let mut finished = None;
        while let Ok(p) = d.rx.try_recv() {
            match p {
                Progress::Stage { stage } => {
                    d.stage = stage;
                    d.fraction = 0.0;
                }
                Progress::Bytes { done, total } => {
                    d.fraction = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                }
                Progress::Note { text } => d.note = text,
                Progress::Done { version } => finished = Some(Ok(version)),
                Progress::Error { message } => finished = Some(Err(message)),
            }
        }
        match finished {
            Some(Ok(v)) => {
                self.log(&format!("client {v} installé"));
                self.download = None;
                self.state = State::load(&self.home);
                self.report.client_installed = Some(v.clone());
                self.report.client_update = false;
                self.notice = Some((format!("Oteria {v} est installé."), c::GOOD));
                self.phase = Phase::Ready;
                if self.panorama.is_empty() {
                    self.panorama = load_panorama(ctx, &self.state.resources_dir(&self.home));
                }
            }
            Some(Err(e)) => {
                self.log(&format!("échec de l'installation : {e}"));
                self.download = None;
                let installed = self.state.client_dir(&self.home).is_some();
                self.notice = Some((e, c::BAD));
                self.phase = Phase::NeedDownload { update: installed };
            }
            None => {}
        }
    }

    /// Starts the installed client right away, with no console window.
    fn launch(&mut self, server: Option<&str>) {
        let clicked = Instant::now();
        let Some(dir) = self.state.client_dir(&self.home) else {
            self.phase = Phase::NeedDownload { update: false };
            return;
        };
        let _ = std::fs::create_dir_all(self.game_dir.join("home"));
        let mut cmd = Command::new(dir.join(paths::CLIENT_EXE));
        // `DETACHED_PROCESS`: the game (a console program) gets no console at all, so no window
        // of logs opens beside it; its logs are in the game folder's `home/logs`.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0000_0008);
        }
        cmd.current_dir(&dir)
            .arg("--game-dir")
            .arg(&self.game_dir)
            .arg("--name")
            .arg(&self.name)
            .arg("--resources")
            .arg(self.state.resources_dir(&self.home))
            .env("HORIZON_HOME", self.game_dir.join("home"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(s) = server {
            cmd.arg("--server").arg(s);
        }
        if let Ok(extra) = std::env::var("OTERIA_GAME_ARGS") {
            cmd.args(extra.split_whitespace());
        }
        match cmd.spawn() {
            Ok(child) => {
                self.log(&format!(
                    "jeu lancé (pid {}) en {} ms après le clic",
                    child.id(),
                    clicked.elapsed().as_millis()
                ));
                self.child = Some(child);
                self.phase = Phase::Launching {
                    since: Instant::now(),
                };
                self.notice = None;
            }
            Err(e) => {
                self.log(&format!("le jeu n'a pas démarré : {e}"));
                self.notice = Some((format!("Le jeu n'a pas démarré : {e}"), c::BAD));
            }
        }
    }

    /// Stops the game this launcher started.
    fn stop_game(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            self.log(&format!("jeu arrêté (pid {})", child.id()));
            self.notice = Some(("Le jeu a été arrêté.".into(), c::MUTED));
        }
        self.phase = Phase::Ready;
    }

    fn main_action(&mut self) {
        match self.phase {
            Phase::NeedDownload { .. } => self.start_download(),
            Phase::Ready => self.launch(None),
            Phase::Launching { .. } | Phase::Running => self.stop_game(),
            Phase::Failed(_) => self.start_check(),
            _ => {}
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.check_rx {
            if let Ok(r) = rx.try_recv() {
                self.check_rx = None;
                self.on_report(r);
            }
        }
        self.poll_download(ctx);
        match &self.phase {
            Phase::SelfUpdating {
                since,
                handed_over: false,
                ..
            } if since.elapsed() > Duration::from_millis(1400) => {
                let spawned = install::quiet_command(self.updater_for_self_update())
                    .args(["self-update", "--wait-pid", &std::process::id().to_string()])
                    .spawn();
                self.log(&format!(
                    "auto-mise à jour confiée à l'updater : {:?}",
                    spawned.as_ref().map(|c| c.id())
                ));
                if spawned.is_ok() {
                    if let Phase::SelfUpdating { handed_over, .. } = &mut self.phase {
                        *handed_over = true;
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.notice = Some((
                        "La mise à jour du launcher n'a pas pu démarrer".into(),
                        c::BAD,
                    ));
                    self.phase = Phase::Ready;
                }
            }
            Phase::Launching { since } if since.elapsed() > Duration::from_millis(2500) => {
                self.phase = Phase::Running;
                if self.state.close_on_launch {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            _ => {}
        }
        if matches!(self.phase, Phase::Launching { .. } | Phase::Running) {
            if let Some(child) = &mut self.child {
                if let Ok(Some(status)) = child.try_wait() {
                    self.log(&format!("jeu fermé : {status}"));
                    self.child = None;
                    self.phase = Phase::Ready;
                }
            }
        }
        // Scripted presses (tests).
        if let Some(next) = self.auto.first().cloned() {
            let pressed = match (next.as_str(), &self.phase) {
                ("download", Phase::NeedDownload { .. }) => true,
                ("play", Phase::Ready) => true,
                ("stop", Phase::Running) => true,
                _ => false,
            };
            if pressed {
                self.auto.remove(0);
                self.log(&format!("appui automatique : {next}"));
                self.main_action();
            }
        }
    }

    /// The updater that replaces this launcher runs from a copy: it may replace itself too.
    fn updater_for_self_update(&self) -> PathBuf {
        let src = self.updater();
        let copy = self.home.join("updater.run.exe");
        if std::fs::copy(&src, &copy).is_ok() {
            copy
        } else {
            src
        }
    }

    fn settled(&self) -> bool {
        match self.shot_at.as_str() {
            "downloading" => self
                .download
                .as_ref()
                .is_some_and(|d| d.stage == "download" && d.fraction > 0.35),
            "self-update" => matches!(self.phase, Phase::SelfUpdating { .. }),
            "running" => matches!(self.phase, Phase::Running),
            _ => {
                !matches!(self.phase, Phase::Checking | Phase::Downloading) && self.auto.is_empty()
            }
        }
    }

    fn news(&self) -> Vec<NewsItem> {
        self.report.news.clone()
    }
}

/// The title-screen panorama's four sides from the prepared resources: the player's own copy of
/// Minecraft's art, never shipped with Oteria. Empty when they are not there yet.
fn load_panorama(ctx: &egui::Context, resources: &Path) -> Vec<TextureHandle> {
    let dir = resources.join("assets/minecraft/textures/gui/title/background");
    (0..4)
        .map_while(|i| {
            let face = image::open(dir.join(format!("panorama_{i}.png")))
                .ok()?
                .to_rgba8();
            Some(ctx.load_texture(
                format!("panorama-{i}"),
                egui::ColorImage::from_rgba_unmultiplied(
                    [face.width() as usize, face.height() as usize],
                    face.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            ))
        })
        .collect()
}

fn scrim_texture(ctx: &egui::Context) -> TextureHandle {
    let w = 256;
    let px = (0..w)
        .map(|x| {
            let t = x as f32 / (w - 1) as f32;
            // Dark on the left, where the text is, clear past the middle.
            let a = (1.0 - t / 0.7).clamp(0.0, 1.0).powf(1.3) * 0.88 + 0.12;
            Color32::from_rgba_unmultiplied(6, 11, 20, (a * 255.0) as u8)
        })
        .collect();
    ctx.load_texture(
        "scrim",
        egui::ColorImage::new([w, 1], px),
        egui::TextureOptions::LINEAR,
    )
}

/// The computer's memory and the game's, for the Settings page.
mod memory {
    #[cfg(windows)]
    pub fn total() -> Option<u64> {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then_some(m.ullTotalPhys)
    }

    /// The memory process `pid` holds (its working set).
    #[cfg(windows)]
    pub fn of_process(pid: u32) -> Option<u64> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::ProcessStatus::{
            K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
        };
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let ok = K32GetProcessMemoryInfo(h, &mut c, c.cb) != 0;
            CloseHandle(h);
            ok.then_some(c.WorkingSetSize as u64)
        }
    }

    #[cfg(not(windows))]
    pub fn total() -> Option<u64> {
        None
    }
    #[cfg(not(windows))]
    pub fn of_process(_: u32) -> Option<u64> {
        None
    }
}

fn gib(bytes: u64) -> String {
    format!("{:.1} Go", bytes as f64 / (1u64 << 30) as f64).replace('.', ",")
}

fn open_folder(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = Command::new("explorer").arg(dir).spawn();
}

fn tag_colour(tag: &str) -> Color32 {
    let t = tag.to_lowercase();
    if t.contains("pvp") {
        Color32::from_rgb(0xe0, 0x5a, 0x47)
    } else if t.contains("cosm") {
        Color32::from_rgb(0xd9, 0x9a, 0x2b)
    } else if t.contains("mise") {
        c::GOOD
    } else {
        c::CYAN
    }
}

impl eframe::App for Launcher {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        c::BG.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
        // Screenshots for the tests.
        let shots: Vec<_> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
                .collect()
        });
        if let (Some(img), Some(path)) = (shots.first(), &self.shot) {
            let r = ui::save_screenshot(img, path);
            self.log(&format!("capture {} : {r:?}", path.display()));
            // The self-update carries on after its picture; so does a run that still has to press.
            let carry_on = matches!(self.phase, Phase::SelfUpdating { .. })
                || (!self.auto.is_empty() && self.shot_at != "running");
            if !carry_on {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            self.shot = None;
        }
        if self.shot.is_some() && !self.shot_sent {
            if self.settled() {
                self.settled_frames += 1;
                if self.settled_frames > 12 {
                    self.shot_sent = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            } else {
                self.settled_frames = 0;
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(c::BG))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let side_w = 112.0;
                self.sidebar(
                    ui,
                    Rect::from_min_max(full.min, Pos2::new(full.left() + side_w, full.bottom())),
                );
                let top = Rect::from_min_max(
                    Pos2::new(full.left() + side_w, full.top()),
                    Pos2::new(full.right(), full.top() + 80.0),
                );
                self.top_bar(ui, top);
                let body = Rect::from_min_max(
                    Pos2::new(top.left() + 28.0, top.bottom()),
                    full.max - Vec2::new(24.0, 24.0),
                );
                match self.page {
                    Page::Play => self.play_page(ui, body),
                    Page::Mods => self.mods_page(ui, body),
                    Page::Market => self.market_page(ui, body),
                    Page::Settings => self.settings_page(ui, body),
                    Page::News => self.news_page(ui, body),
                }
                if let Phase::SelfUpdating { version, since, .. } = &self.phase {
                    let (v, t) = (version.clone(), since.elapsed().as_secs_f32());
                    self.self_update_overlay(ui, full, &v, t);
                }
            });
        let busy = matches!(
            self.phase,
            Phase::Checking
                | Phase::Downloading
                | Phase::SelfUpdating { .. }
                | Phase::Launching { .. }
        );
        ctx.request_repaint_after(if busy || self.shot.is_some() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(500)
        });
    }
}

// ----- Pieces ----------------------------------------------------------------------------------

/// A flat button: an optional icon and a label. Returns whether it was clicked.
fn button(
    ui: &mut egui::Ui,
    icons: &Icons,
    r: Rect,
    icon: Option<&str>,
    label: &str,
    id: &str,
    enabled: bool,
) -> bool {
    let resp = ui.interact(
        r,
        ui.id().with(("button", id)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    let fill = if !enabled {
        c::PANEL
    } else if resp.hovered() {
        c::HOVER
    } else {
        c::RAISED
    };
    ui::plate(p, r, 4, fill, c::BORDER);
    let col = if enabled { c::TEXT } else { c::FAINT };
    let g = p.layout_no_wrap(label.to_owned(), ui::semi(13.0), col);
    let icon_w = if icon.is_some() { 24.0 } else { 0.0 };
    let x0 = r.center().x - (g.size().x + icon_w) / 2.0;
    if let Some(name) = icon {
        icons.draw(p, name, Pos2::new(x0 + 8.0, r.center().y), 17.0, col);
    }
    p.galley(
        Pos2::new(x0 + icon_w, r.center().y - g.size().y / 2.0),
        g,
        col,
    );
    enabled && resp.clicked()
}

/// A section title in small spaced capitals, with an icon.
fn section_title(p: &egui::Painter, icons: &Icons, pos: Pos2, icon: &str, title: &str) {
    icons.draw(p, icon, pos + Vec2::new(10.0, 10.0), 20.0, c::CYAN);
    ui::spaced(
        p,
        pos + Vec2::new(28.0, 1.0),
        title,
        ui::semi(14.0),
        c::TEXT,
        1.2,
    );
}

/// Draws `tex` (`tex_aspect` wide for 1 high) covering `r` with rounded corners, the part
/// starting at `u0` (0 to 1) of what is left over.
fn cover(
    p: &egui::Painter,
    r: Rect,
    radius: u8,
    tex: &TextureHandle,
    tex_aspect: f32,
    u0: f32,
    tint: Color32,
) {
    let aspect = r.width() / r.height();
    let uv = if aspect < tex_aspect {
        let w = aspect / tex_aspect;
        let x = (1.0 - w) * u0;
        Rect::from_min_max(Pos2::new(x, 0.0), Pos2::new(x + w, 1.0))
    } else {
        let h = tex_aspect / aspect;
        let y = (1.0 - h) * 0.45;
        Rect::from_min_max(Pos2::new(0.0, y), Pos2::new(1.0, y + h))
    };
    p.add(RectShape::filled(r, CornerRadius::same(radius), tint).with_texture(tex.id(), uv));
}

impl Launcher {
    fn sidebar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, c::SIDEBAR);
        p.line_segment(
            [rect.right_top(), rect.right_bottom()],
            Stroke::new(1.0_f32, c::BORDER),
        );
        p.image(
            self.logo.id(),
            Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 42.0),
                Vec2::splat(52.0),
            ),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        let items = [
            (Page::Play, "device-gamepad-2", "JOUER"),
            (Page::Mods, "puzzle", "MODS"),
            (Page::Market, "building-store", "MARKETPLACE"),
            (Page::Settings, "settings", "PARAMÈTRES"),
        ];
        for (i, (page, icon, label)) in items.into_iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + 8.0, rect.top() + 96.0 + i as f32 * 92.0),
                Vec2::new(rect.width() - 16.0, 84.0),
            );
            let resp = ui.interact(r, ui.id().with(("nav", i)), Sense::click());
            let active = self.page == page || (page == Page::Play && self.page == Page::News);
            if active {
                ui::plate(&p, r, 6, c::SELECTED, ui::alpha(c::ACCENT, 0.55));
            } else if resp.hovered() {
                p.rect_filled(r, CornerRadius::same(6), c::RAISED);
            }
            let col = if active || resp.hovered() {
                c::TEXT
            } else {
                c::MUTED
            };
            self.icons.draw(
                &p,
                icon,
                r.center() - Vec2::new(0.0, 12.0),
                30.0,
                if active { c::CYAN } else { col },
            );
            ui::text(
                &p,
                r.center() + Vec2::new(0.0, 20.0),
                Align2::CENTER_CENTER,
                label,
                ui::semi(11.0),
                col,
            );
            if resp.clicked() {
                self.page = page;
            }
        }
        ui::text(
            &p,
            Pos2::new(rect.center().x, rect.bottom() - 20.0),
            Align2::CENTER_CENTER,
            &format!("v{}", oteria_dist::LAUNCHER_VERSION),
            ui::medium(12.0),
            c::FAINT,
        );
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        // Dragging the bar moves the window; a double click maximises it.
        let drag = ui.interact(rect, ui.id().with("titlebar"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let m = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!m));
        }
        let p = ui.painter().clone();
        let x = rect.left() + 28.0;
        ui::text(
            &p,
            Pos2::new(x, rect.top() + 14.0),
            Align2::LEFT_TOP,
            "OTERIA",
            ui::pixel(32.0),
            c::TEXT,
        );
        ui::spaced(
            &p,
            Pos2::new(x + 2.0, rect.top() + 50.0),
            "LAUNCHER",
            ui::semi(11.0),
            c::MUTED,
            3.4,
        );

        // The account.
        let w = 220.0;
        let acc = Rect::from_min_size(
            Pos2::new(rect.right() - 46.0 * 3.0 - 16.0 - w, rect.top() + 18.0),
            Vec2::new(w, 44.0),
        );
        let resp = ui.interact(acc, ui.id().with("account"), Sense::click());
        ui::plate(
            &p,
            acc,
            6,
            if resp.hovered() { c::HOVER } else { c::RAISED },
            c::BORDER,
        );
        let head = Rect::from_min_size(acc.min + Vec2::new(8.0, 8.0), Vec2::splat(28.0));
        ui::draw_head(&p, head, self.skin.as_ref());
        ui::text(
            &p,
            Pos2::new(head.right() + 12.0, acc.center().y),
            Align2::LEFT_CENTER,
            &self.name,
            ui::semi(14.0),
            c::TEXT,
        );
        self.icons.draw(
            &p,
            "chevron-right",
            Pos2::new(acc.right() - 18.0, acc.center().y),
            16.0,
            c::MUTED,
        );
        if resp.clicked() {
            self.page = Page::Settings;
        }
        ui::window_controls(ui, rect, &self.icons);
    }

    fn play_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let side_w = 340.0;
        let gap = 24.0;
        let main = Rect::from_min_max(
            body.min + Vec2::new(0.0, 4.0),
            Pos2::new(body.right() - side_w - gap, body.bottom()),
        );
        let side = Rect::from_min_max(Pos2::new(main.right() + gap, main.top()), body.max);
        let hero_h = (main.height() * 0.6).clamp(330.0, 460.0);
        let hero = Rect::from_min_size(main.min, Vec2::new(main.width(), hero_h));
        self.hero(ui, hero);

        let p = ui.painter().clone();
        let news_top = hero.bottom() + 24.0;
        section_title(
            &p,
            &self.icons,
            Pos2::new(main.left(), news_top),
            "news",
            "DERNIÈRES ACTUALITÉS",
        );
        let all = Rect::from_min_size(
            Pos2::new(main.right() - 96.0, news_top),
            Vec2::new(96.0, 22.0),
        );
        let resp = ui.interact(all, ui.id().with("all-news"), Sense::click());
        let col = if resp.hovered() { c::TEXT } else { c::CYAN };
        ui::text(
            &p,
            Pos2::new(all.right() - 18.0, all.center().y),
            Align2::RIGHT_CENTER,
            "Tout voir",
            ui::medium(13.0),
            col,
        );
        self.icons.draw(
            &p,
            "chevron-right",
            Pos2::new(all.right() - 7.0, all.center().y),
            15.0,
            col,
        );
        if resp.clicked() {
            self.page = Page::News;
        }
        let cards_top = news_top + 36.0;
        let news = self.news();
        let h = (main.bottom() - cards_top).max(120.0);
        if news.is_empty() {
            let r = Rect::from_min_max(Pos2::new(main.left(), cards_top), main.max);
            ui::plate(&p, r, 6, c::PANEL, c::BORDER);
            ui::text(
                &p,
                r.center(),
                Align2::CENTER_CENTER,
                "Pas d'actualité pour le moment.",
                ui::regular(13.0),
                c::MUTED,
            );
        }
        let gap = 18.0;
        let w = (main.width() - gap) / 2.0;
        for (i, item) in news.iter().take(2).enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(main.left() + i as f32 * (w + gap), cards_top),
                Vec2::new(w, h),
            );
            self.news_card(ui, r, item, i);
        }
        self.side_column(ui, side);
    }

    fn hero(&mut self, ui: &mut egui::Ui, r: Rect) {
        let p = ui.painter().clone();
        match self.panorama.get(3) {
            Some(tex) => {
                cover(&p, r, 8, tex, 1.0, 0.5, Color32::WHITE);
                p.add(
                    RectShape::filled(r, CornerRadius::same(8), Color32::WHITE).with_texture(
                        self.scrim.id(),
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    ),
                );
            }
            None => {
                p.rect_filled(
                    r,
                    CornerRadius::same(8),
                    Color32::from_rgb(0x0f, 0x22, 0x38),
                );
            }
        }
        p.rect_stroke(
            r,
            CornerRadius::same(8),
            Stroke::new(1.0_f32, c::BORDER),
            StrokeKind::Inside,
        );

        // The player on the right, with a name tag as the game draws one.
        let ph = r.height() * 0.7;
        let feet = Pos2::new(r.right() - r.width() * 0.19, r.bottom() - r.height() * 0.08);
        let fig = Rect::from_min_max(
            Pos2::new(feet.x - ph / 4.0, feet.y - ph),
            Pos2::new(feet.x + ph / 4.0, feet.y),
        );
        ui::draw_player(&p.with_clip_rect(r), fig, self.skin.as_ref());
        let g = p.layout_no_wrap(self.name.clone(), ui::semi(13.0), Color32::WHITE);
        let tag = Rect::from_center_size(
            Pos2::new(feet.x, fig.top() - 20.0),
            g.size() + Vec2::new(14.0, 6.0),
        );
        p.rect_filled(tag, 0.0, Color32::from_black_alpha(110));
        p.galley(tag.min + Vec2::new(7.0, 3.0), g, Color32::WHITE);

        // The title, with the hard shadow of the game's own titles.
        let x = r.left() + 40.0;
        let ty = r.top() + 36.0;
        ui::text(
            &p,
            Pos2::new(x + 4.0, ty + 4.0),
            Align2::LEFT_TOP,
            "OTERIA",
            ui::pixel(78.0),
            Color32::from_black_alpha(150),
        );
        ui::text(
            &p,
            Pos2::new(x, ty),
            Align2::LEFT_TOP,
            "OTERIA",
            ui::pixel(78.0),
            Color32::WHITE,
        );
        ui::spaced(
            &p,
            Pos2::new(x + 4.0, ty + 94.0),
            &format!(
                "CLIENT PVP  ·  MINECRAFT JAVA {}",
                oteria_dist::MINECRAFT_VERSION
            ),
            ui::semi(13.0),
            Color32::from_rgb(0xc9, 0xd6, 0xe6),
            2.0,
        );

        let btn = Rect::from_min_size(
            Pos2::new(x, r.bottom() - 32.0 - 44.0 - 62.0 - 66.0),
            Vec2::new(366.0, 66.0),
        );
        self.main_button(ui, btn);
        self.status_line(&p, Pos2::new(x, btn.bottom() + 12.0), btn.width());

        let row_y = btn.bottom() + 62.0;
        let b1 = Rect::from_min_size(Pos2::new(x, row_y), Vec2::new(178.0, 44.0));
        let b2 = Rect::from_min_size(Pos2::new(b1.right() + 10.0, row_y), Vec2::new(178.0, 44.0));
        if button(
            ui,
            &self.icons,
            b1,
            Some("folder"),
            "Dossier du jeu",
            "game-folder",
            true,
        ) {
            open_folder(&self.game_dir);
        }
        if button(
            ui,
            &self.icons,
            b2,
            Some("external-link"),
            "Captures d'écran",
            "screenshots",
            true,
        ) {
            open_folder(&self.game_dir.join("screenshots"));
        }
    }

    fn main_button(&mut self, ui: &mut egui::Ui, btn: Rect) {
        let (label, icon, enabled, danger) = match &self.phase {
            Phase::Checking => ("RECHERCHE…", "refresh", false, false),
            Phase::SelfUpdating { .. } => ("MISE À JOUR…", "refresh", false, false),
            Phase::NeedDownload { update: false } => ("TÉLÉCHARGER", "download", true, false),
            Phase::NeedDownload { update: true } => ("METTRE À JOUR", "download", true, false),
            Phase::Downloading => ("TÉLÉCHARGEMENT…", "download", false, false),
            Phase::Ready => ("JOUER", "player-play-filled", true, false),
            Phase::Launching { .. } | Phase::Running => {
                ("ARRÊTER LE JEU", "player-stop-filled", true, true)
            }
            Phase::Failed(_) => ("RÉESSAYER", "refresh", true, false),
        };
        let resp = ui.interact(
            btn,
            ui.id().with("main"),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let p = ui.painter().clone();
        let hovered = enabled && resp.hovered();
        let (fill, edge) = match (enabled, danger, hovered) {
            (false, _, _) => (
                Color32::from_rgb(0x1f, 0x35, 0x50),
                Color32::from_rgb(0x17, 0x28, 0x3e),
            ),
            (true, true, false) => (c::BAD, c::BAD_EDGE),
            (true, true, true) => (c::BAD_HOVER, c::BAD_EDGE),
            (true, false, false) => (c::ACCENT, c::ACCENT_EDGE),
            (true, false, true) => (c::ACCENT_HOVER, c::ACCENT_EDGE),
        };
        // A darker edge at the bottom, as the game's own buttons have.
        p.rect_filled(btn, CornerRadius::same(6), edge);
        let face = Rect::from_min_max(btn.min, btn.max - Vec2::new(0.0, 4.0));
        p.rect_filled(face, CornerRadius::same(6), fill);
        let fg = if enabled { Color32::WHITE } else { c::MUTED };
        let g = p.layout_no_wrap(label.to_owned(), ui::semi(21.0), fg);
        let total = 30.0 + g.size().x;
        let x0 = face.center().x - total / 2.0;
        self.icons
            .draw(&p, icon, Pos2::new(x0 + 11.0, face.center().y), 24.0, fg);
        p.galley(
            Pos2::new(x0 + 32.0, face.center().y - g.size().y / 2.0),
            g,
            fg,
        );
        if enabled && resp.clicked() {
            self.main_action();
        }
    }

    /// What the main button is doing, beside it: the download's progress, or a short message.
    fn status_line(&self, p: &egui::Painter, pos: Pos2, width: f32) {
        let version = match (
            &self.phase,
            &self.report.client_latest,
            &self.state.client_version,
        ) {
            (Phase::NeedDownload { update: true }, Some(new), Some(old)) => {
                format!("Mise à jour obligatoire : {old} → {new}")
            }
            (Phase::NeedDownload { update: false }, Some(new), _) => {
                format!("Oteria {new} · première installation")
            }
            (_, _, Some(v)) => format!("Oteria {v} · Minecraft {}", oteria_dist::MINECRAFT_VERSION),
            _ => String::new(),
        };
        if let Some(d) = &self.download {
            let stage = match d.stage.as_str() {
                "download" => "Téléchargement du client",
                "extract" => "Installation",
                "minecraft" => "Minecraft (serveurs de Mojang)",
                "java" => "Java (serveurs de Mojang)",
                "resources" => "Préparation du jeu",
                other => other,
            };
            ui::text(
                p,
                pos,
                Align2::LEFT_TOP,
                stage,
                ui::semi(13.0),
                Color32::WHITE,
            );
            let k = d.fraction.clamp(0.0, 1.0);
            ui::text(
                p,
                pos + Vec2::new(width, 0.0),
                Align2::RIGHT_TOP,
                &format!("{:.0} %", k * 100.0),
                ui::semi(13.0),
                c::CYAN,
            );
            let bar = Rect::from_min_size(pos + Vec2::new(0.0, 24.0), Vec2::new(width, 6.0));
            p.rect_filled(bar, CornerRadius::same(2), Color32::from_white_alpha(28));
            p.rect_filled(
                Rect::from_min_size(bar.min, Vec2::new(bar.width() * k, 6.0)),
                CornerRadius::same(2),
                c::ACCENT_HOVER,
            );
            if !d.note.is_empty() {
                ui::wrapped(
                    p,
                    bar.left_bottom() + Vec2::new(0.0, 8.0),
                    &d.note,
                    ui::regular(12.0),
                    Color32::from_rgb(0xb5, 0xc3, 0xd6),
                    width,
                    1,
                );
            }
            return;
        }
        ui::text(
            p,
            pos,
            Align2::LEFT_TOP,
            &version,
            ui::semi(13.0),
            Color32::WHITE,
        );
        let (text, col) = match (&self.notice, &self.phase) {
            (Some((t, col)), _) => (t.clone(), *col),
            (None, Phase::Failed(e)) => (e.clone(), c::BAD),
            (None, Phase::NeedDownload { update: true }) => {
                ("Télécharge la mise à jour pour jouer.".into(), c::WARN)
            }
            (None, Phase::NeedDownload { update: false }) => (
                "Le jeu n'est pas encore installé sur ce PC.".into(),
                Color32::from_rgb(0xb5, 0xc3, 0xd6),
            ),
            (None, Phase::Ready) => ("Prêt à jouer.".into(), c::GOOD),
            (None, Phase::Launching { .. }) => (
                "Le jeu démarre…".into(),
                Color32::from_rgb(0xb5, 0xc3, 0xd6),
            ),
            (None, Phase::Running) => ("Le jeu est ouvert.".into(), c::GOOD),
            (None, Phase::Checking) => (
                "Recherche de mises à jour…".into(),
                Color32::from_rgb(0xb5, 0xc3, 0xd6),
            ),
            _ => return,
        };
        let y = pos.y + 24.0;
        p.rect_filled(
            Rect::from_min_size(Pos2::new(pos.x, y + 5.0), Vec2::splat(7.0)),
            0.0,
            col,
        );
        ui::wrapped(
            p,
            Pos2::new(pos.x + 15.0, y),
            &text,
            ui::medium(13.0),
            ui::lerp(col, Color32::WHITE, 0.4),
            width - 15.0,
            2,
        );
    }

    fn news_card(&self, ui: &mut egui::Ui, r: Rect, item: &NewsItem, index: usize) {
        let resp = ui.interact(r, ui.id().with(("news", index)), Sense::hover());
        let p = ui.painter().clone();
        ui::plate(
            &p,
            r,
            6,
            if resp.hovered() { c::RAISED } else { c::PANEL },
            c::BORDER,
        );
        // The player's own panorama as the picture; text alone before the game is installed.
        let img_h = if self.panorama.is_empty() {
            0.0
        } else {
            (r.height() - 92.0).max(0.0)
        };
        if img_h > 40.0 {
            let img = Rect::from_min_size(
                r.min + Vec2::new(1.0, 1.0),
                Vec2::new(r.width() - 2.0, img_h),
            );
            match self.panorama.get(index) {
                Some(tex) => cover(&p, img, 5, tex, 1.0, 0.5, Color32::from_gray(225)),
                None => {
                    p.rect_filled(
                        img,
                        CornerRadius::same(5),
                        Color32::from_rgb(0x13, 0x26, 0x3e),
                    );
                }
            }
            // Square corners where the image meets the text.
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(img.left(), img.bottom() - 6.0),
                    img.right_bottom(),
                ),
                0.0,
                c::PANEL,
            );
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(img.left(), img.bottom() - 6.0),
                    Pos2::new(img.right(), img.bottom() - 5.0),
                ),
                0.0,
                c::BORDER,
            );
        }
        let x = r.left() + 18.0;
        let y = r.top() + img_h.max(0.0) + 10.0;
        let col = tag_colour(&item.tag);
        p.rect_filled(
            Rect::from_min_size(Pos2::new(x, y + 6.0), Vec2::splat(9.0)),
            0.0,
            col,
        );
        ui::wrapped(
            &p,
            Pos2::new(x + 18.0, y),
            &item.title,
            ui::semi(16.0),
            c::TEXT,
            r.width() - 140.0,
            1,
        );
        ui::wrapped(
            &p,
            Pos2::new(x, y + 28.0),
            &item.body,
            ui::regular(13.0),
            c::MUTED,
            r.width() - 64.0,
            if img_h > 0.0 { 2 } else { 5 },
        );
        ui::text(
            &p,
            Pos2::new(r.right() - 18.0, y + 3.0),
            Align2::RIGHT_TOP,
            &item.date,
            ui::regular(12.0),
            c::FAINT,
        );
    }

    fn side_column(&mut self, ui: &mut egui::Ui, side: Rect) {
        let p = ui.painter().clone();
        // The account.
        let acc = Rect::from_min_size(side.min, Vec2::new(side.width(), 88.0));
        let resp = ui.interact(acc, ui.id().with("account-card"), Sense::click());
        let fill = if resp.hovered() {
            Color32::from_rgb(0x15, 0x5a, 0x90)
        } else {
            Color32::from_rgb(0x12, 0x4d, 0x7c)
        };
        ui::plate(&p, acc, 6, fill, ui::alpha(c::ACCENT, 0.6));
        let head = Rect::from_min_size(acc.min + Vec2::new(20.0, 20.0), Vec2::splat(48.0));
        ui::draw_head(&p, head, self.skin.as_ref());
        ui::text(
            &p,
            Pos2::new(head.right() + 16.0, acc.top() + 26.0),
            Align2::LEFT_TOP,
            &self.name,
            ui::semi(18.0),
            Color32::WHITE,
        );
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(head.right() + 16.0, acc.top() + 55.0),
                Vec2::splat(8.0),
            ),
            0.0,
            c::WARN,
        );
        ui::text(
            &p,
            Pos2::new(head.right() + 30.0, acc.top() + 59.0),
            Align2::LEFT_CENTER,
            "Compte hors ligne",
            ui::regular(13.0),
            Color32::from_rgb(0xc9, 0xd6, 0xe6),
        );
        self.icons.draw(
            &p,
            "chevron-right",
            Pos2::new(acc.right() - 22.0, acc.center().y),
            20.0,
            Color32::WHITE,
        );
        if resp.clicked() {
            self.page = Page::Settings;
        }

        // Partner servers.
        let mut y = acc.bottom() + 24.0;
        section_title(
            &p,
            &self.icons,
            Pos2::new(side.left(), y),
            "server",
            "SERVEURS PARTENAIRES",
        );
        y += 34.0;
        let servers = self.report.servers.clone();
        let list_h = if servers.is_empty() {
            64.0
        } else {
            16.0 + servers.len().min(4) as f32 * 62.0
        };
        let list = Rect::from_min_size(Pos2::new(side.left(), y), Vec2::new(side.width(), list_h));
        ui::plate(&p, list, 6, c::PANEL, c::BORDER);
        if servers.is_empty() {
            ui::text(
                &p,
                list.left_center() + Vec2::new(18.0, 0.0),
                Align2::LEFT_CENTER,
                "Aucun serveur partenaire pour l'instant.",
                ui::regular(13.0),
                c::MUTED,
            );
        }
        let can_join = matches!(self.phase, Phase::Ready);
        for (i, s) in servers.iter().take(4).enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(list.left() + 8.0, list.top() + 8.0 + i as f32 * 62.0),
                Vec2::new(list.width() - 16.0, 58.0),
            );
            let ic = Rect::from_min_size(row.min + Vec2::new(8.0, 9.0), Vec2::splat(40.0));
            p.rect_filled(ic, CornerRadius::same(4), c::RAISED);
            self.icons.draw(&p, "server", ic.center(), 22.0, c::CYAN);
            ui::text(
                &p,
                Pos2::new(ic.right() + 12.0, row.top() + 12.0),
                Align2::LEFT_TOP,
                &s.name,
                ui::semi(14.0),
                c::TEXT,
            );
            ui::wrapped(
                &p,
                Pos2::new(ic.right() + 12.0, row.top() + 32.0),
                &s.address,
                ui::regular(12.0),
                c::MUTED,
                row.width() - 160.0,
                1,
            );
            let join = Rect::from_min_size(
                Pos2::new(row.right() - 88.0, row.center().y - 16.0),
                Vec2::new(80.0, 32.0),
            );
            if button(
                ui,
                &self.icons,
                join,
                None,
                "Rejoindre",
                &format!("join-{i}"),
                can_join,
            ) {
                let addr = s.address.clone();
                self.launch(Some(&addr));
            }
        }

        // Friends: they come with Microsoft accounts.
        y = list.bottom() + 24.0;
        section_title(&p, &self.icons, Pos2::new(side.left(), y), "users", "AMIS");
        y += 34.0;
        let friends = Rect::from_min_size(Pos2::new(side.left(), y), Vec2::new(side.width(), 84.0));
        ui::plate(&p, friends, 6, c::PANEL, c::BORDER);
        ui::wrapped(&p, friends.min + Vec2::new(18.0, 18.0), "Connecte un compte Microsoft pour voir tes amis et les rejoindre. La connexion arrive bientôt dans Oteria.", ui::regular(13.0), c::MUTED, friends.width() - 36.0, 4);
    }

    fn self_update_overlay(&self, ui: &mut egui::Ui, full: Rect, version: &str, t: f32) {
        let p = ui.painter();
        p.rect_filled(full, 0.0, c::BG);
        let cn = full.center();
        p.image(
            self.logo.id(),
            Rect::from_center_size(cn - Vec2::new(0.0, 70.0), Vec2::splat(88.0)),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        ui::text(
            p,
            cn + Vec2::new(0.0, 4.0),
            Align2::CENTER_CENTER,
            "Mise à jour du launcher",
            ui::semi(22.0),
            c::TEXT,
        );
        ui::text(
            p,
            cn + Vec2::new(0.0, 36.0),
            Align2::CENTER_CENTER,
            &format!(
                "v{} → v{version}, le launcher redémarre tout seul",
                oteria_dist::LAUNCHER_VERSION
            ),
            ui::regular(14.0),
            c::MUTED,
        );
        let bar = Rect::from_center_size(cn + Vec2::new(0.0, 72.0), Vec2::new(280.0, 6.0));
        p.rect_filled(bar, CornerRadius::same(2), c::RAISED);
        let k = (t / 1.4).clamp(0.0, 1.0);
        p.rect_filled(
            Rect::from_min_size(bar.min, Vec2::new(bar.width() * k, 6.0)),
            CornerRadius::same(2),
            c::ACCENT,
        );
        ui.ctx().request_repaint();
    }

    /// A page's title and its line of explanation; returns where the page's content starts.
    fn page_head(&self, p: &egui::Painter, body: Rect, title: &str, subtitle: &str) -> f32 {
        ui::text(
            p,
            body.min + Vec2::new(0.0, 8.0),
            Align2::LEFT_TOP,
            title,
            ui::semi(26.0),
            c::TEXT,
        );
        ui::text(
            p,
            body.min + Vec2::new(0.0, 46.0),
            Align2::LEFT_TOP,
            subtitle,
            ui::regular(14.0),
            c::MUTED,
        );
        body.top() + 86.0
    }

    /// A page with nothing in it yet.
    fn empty_state(&self, p: &egui::Painter, area: Rect, icon: &str, title: &str, line: &str) {
        ui::plate(p, area, 6, c::PANEL, c::BORDER);
        let cn = area.center() - Vec2::new(0.0, 30.0);
        self.icons.draw(p, icon, cn, 52.0, c::FAINT);
        ui::text(
            p,
            cn + Vec2::new(0.0, 50.0),
            Align2::CENTER_CENTER,
            title,
            ui::semi(17.0),
            c::TEXT,
        );
        ui::text(
            p,
            cn + Vec2::new(0.0, 76.0),
            Align2::CENTER_CENTER,
            line,
            ui::regular(13.5),
            c::MUTED,
        );
    }

    fn mods_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_head(&p, body, "Mods", "Les mods installés pour Oteria Client.");
        let mods_dir = self.game_dir.join("mods");
        let open = Rect::from_min_size(
            Pos2::new(body.right() - 210.0, body.top() + 14.0),
            Vec2::new(210.0, 40.0),
        );
        if button(
            ui,
            &self.icons,
            open,
            Some("folder"),
            "Ouvrir le dossier des mods",
            "open-mods",
            true,
        ) {
            open_folder(&mods_dir);
        }
        let mut files: Vec<(String, u64)> = std::fs::read_dir(&mods_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    e.metadata().map_or(0, |m| m.len()),
                )
            })
            .collect();
        files.sort();
        let area = Rect::from_min_max(Pos2::new(body.left(), top), body.max);
        if files.is_empty() {
            self.empty_state(
                &p,
                area,
                "puzzle",
                "Aucun mod installé",
                "Les mods pour Oteria arrivent bientôt : ils apparaîtront ici.",
            );
            return;
        }
        ui::plate(&p, area, 6, c::PANEL, c::BORDER);
        for (i, (name, size)) in files.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(area.left(), area.top() + i as f32 * 52.0),
                Vec2::new(area.width(), 52.0),
            );
            if row.bottom() > area.bottom() {
                break;
            }
            if i > 0 {
                p.line_segment(
                    [
                        row.left_top() + Vec2::new(16.0, 0.0),
                        row.right_top() - Vec2::new(16.0, 0.0),
                    ],
                    Stroke::new(1.0_f32, c::BORDER),
                );
            }
            self.icons.draw(
                &p,
                "puzzle",
                Pos2::new(row.left() + 32.0, row.center().y),
                22.0,
                c::CYAN,
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 58.0, row.center().y),
                Align2::LEFT_CENTER,
                name,
                ui::medium(14.0),
                c::TEXT,
            );
            ui::text(
                &p,
                Pos2::new(row.right() - 20.0, row.center().y),
                Align2::RIGHT_CENTER,
                &format!("{:.1} Mo", *size as f64 / 1e6),
                ui::regular(13.0),
                c::MUTED,
            );
        }
    }

    fn market_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_head(
            &p,
            body,
            "Marketplace",
            "Cosmétiques, packs et mods pour Oteria.",
        );
        let area = Rect::from_min_max(Pos2::new(body.left(), top), body.max);
        self.empty_state(
            &p,
            area,
            "building-store",
            "La marketplace n'est pas encore ouverte",
            "Elle arrivera avec les comptes Microsoft.",
        );
    }

    fn news_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_head(&p, body, "Actualités", "Les nouveautés d'Oteria Client.");
        let area = Rect::from_min_max(Pos2::new(body.left(), top), body.max);
        let news = self.news();
        if news.is_empty() {
            self.empty_state(
                &p,
                area,
                "news",
                "Pas d'actualité pour le moment",
                "Les nouveautés apparaîtront ici.",
            );
            return;
        }
        for (i, item) in news.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(area.left(), area.top() + i as f32 * 112.0),
                Vec2::new(area.width().min(900.0), 100.0),
            );
            if r.bottom() > area.bottom() {
                break;
            }
            ui::plate(&p, r, 6, c::PANEL, c::BORDER);
            let col = tag_colour(&item.tag);
            p.rect_filled(
                Rect::from_min_size(r.min + Vec2::new(20.0, 24.0), Vec2::splat(9.0)),
                0.0,
                col,
            );
            ui::text(
                &p,
                r.min + Vec2::new(38.0, 18.0),
                Align2::LEFT_TOP,
                &item.title,
                ui::semi(16.0),
                c::TEXT,
            );
            ui::text(
                &p,
                Pos2::new(r.right() - 20.0, r.top() + 20.0),
                Align2::RIGHT_TOP,
                &format!("{} · {}", item.tag, item.date),
                ui::regular(12.0),
                c::FAINT,
            );
            ui::wrapped(
                &p,
                r.min + Vec2::new(20.0, 48.0),
                &item.body,
                ui::regular(13.5),
                c::MUTED,
                r.width() - 40.0,
                2,
            );
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_head(
            &p,
            body,
            "Paramètres",
            "L'installation, la mémoire, le launcher et le compte.",
        );
        let area = Rect::from_min_max(
            Pos2::new(body.left(), top),
            Pos2::new(body.left() + body.width().min(880.0), body.bottom()),
        );
        let game_ram = self.child.as_ref().and_then(|c| memory::of_process(c.id()));
        let resources = self.state.resources_dir(&self.home);
        let source = self.state.source().unwrap_or_default();
        let client = self
            .state
            .client_version
            .clone()
            .map_or("aucun".into(), |v| format!("Oteria {v}"));
        let busy = matches!(self.phase, Phase::Checking | Phase::Downloading);
        let mut close_on_launch = self.state.close_on_launch;
        let mut actions: Vec<&'static str> = Vec::new();

        egui::ScrollArea::vertical()
            .max_height(area.height())
            .show_viewport(&mut ui.new_child(egui::UiBuilder::new().max_rect(area)), |ui, _| {
                // Each row: label, value, optional action (id, label).
                type Row<'a> = (&'a str, String, Option<(&'static str, &'a str)>);
                let sections: Vec<(&str, &str, Vec<Row>)> = vec![
                    ("folder", "INSTALLATION", vec![
                        ("Launcher", self.home.display().to_string(), Some(("open-home", "Ouvrir"))),
                        ("Dossier du jeu", self.game_dir.display().to_string(), Some(("open-game", "Ouvrir"))),
                        ("Ressources du jeu", resources.display().to_string(), Some(("open-res", "Ouvrir"))),
                        ("Client installé", client.clone(), None),
                    ]),
                    ("cpu", "MÉMOIRE", vec![
                        ("Mémoire du PC", self.total_ram.map_or("inconnue".into(), gib), None),
                        ("Utilisée par le jeu", game_ram.map_or("le jeu n'est pas lancé".into(), gib), None),
                    ]),
                    ("refresh", "LAUNCHER", vec![
                        ("Version", format!("v{}", oteria_dist::LAUNCHER_VERSION), Some(("check", if busy { "…" } else { "Rechercher une mise à jour" }))),
                        ("Source des mises à jour", source.clone(), None),
                        ("Au lancement du jeu", String::new(), None),
                    ]),
                    ("user", "COMPTE", vec![
                        ("Pseudo", self.name.clone(), None),
                        ("Type", "Hors ligne (la connexion Microsoft arrive bientôt)".into(), None),
                    ]),
                ];
                let row_h = 46.0;
                let total: f32 = sections.iter().map(|(_, _, rows)| 44.0 + rows.len() as f32 * row_h + 22.0).sum::<f32>() + 60.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(area.width() - 22.0, total), Sense::hover());
                let p = ui.painter().clone();
                let mut y = rect.top();
                for (icon, title, rows) in &sections {
                    section_title(&p, &self.icons, Pos2::new(rect.left(), y), icon, title);
                    y += 36.0;
                    let panel = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), rows.len() as f32 * row_h));
                    ui::plate(&p, panel, 6, c::PANEL, c::BORDER);
                    for (i, (label, value, action)) in rows.iter().enumerate() {
                        let row = Rect::from_min_size(Pos2::new(panel.left(), panel.top() + i as f32 * row_h), Vec2::new(panel.width(), row_h));
                        if i > 0 {
                            p.line_segment([row.left_top() + Vec2::new(16.0, 0.0), row.right_top() - Vec2::new(16.0, 0.0)], Stroke::new(1.0_f32, c::BORDER));
                        }
                        ui::text(&p, Pos2::new(row.left() + 18.0, row.center().y), Align2::LEFT_CENTER, label, ui::regular(13.5), c::MUTED);
                        let action_w = action.map_or(0.0, |(_, l)| if l.len() > 10 { 220.0 } else { 90.0 });
                        ui::wrapped(&p, Pos2::new(row.left() + 220.0, row.center().y - 9.0), value, ui::medium(13.5), c::TEXT, row.width() - 250.0 - action_w, 1);
                        if *label == "Au lancement du jeu" {
                            let check = Rect::from_min_size(Pos2::new(row.left() + 220.0, row.center().y - 10.0), Vec2::new(260.0, 20.0));
                            let resp = ui.interact(check, ui.id().with("close-on-launch"), Sense::click());
                            let bx = Rect::from_min_size(check.min, Vec2::splat(20.0));
                            ui::plate(&p, bx, 4, if close_on_launch { c::ACCENT } else { c::RAISED }, if close_on_launch { c::ACCENT } else { c::BORDER });
                            if close_on_launch {
                                p.line_segment([bx.min + Vec2::new(5.0, 10.0), bx.min + Vec2::new(9.0, 14.0)], Stroke::new(2.0_f32, Color32::WHITE));
                                p.line_segment([bx.min + Vec2::new(9.0, 14.0), bx.min + Vec2::new(15.5, 6.5)], Stroke::new(2.0_f32, Color32::WHITE));
                            }
                            ui::text(&p, Pos2::new(bx.right() + 10.0, bx.center().y), Align2::LEFT_CENTER, "Fermer le launcher", ui::medium(13.5), c::TEXT);
                            if resp.clicked() {
                                close_on_launch = !close_on_launch;
                                actions.push("toggle-close");
                            }
                        }
                        if let Some((id, label)) = action {
                            let b = Rect::from_min_size(Pos2::new(row.right() - 12.0 - action_w, row.center().y - 16.0), Vec2::new(action_w, 32.0));
                            if button(ui, &self.icons, b, None, label, id, *id != "check" || !busy) {
                                actions.push(id);
                            }
                        }
                    }
                    y = panel.bottom() + 26.0;
                }
                ui::wrapped(
                    &p,
                    Pos2::new(rect.left(), y),
                    "Oteria est écrit en Rust : il ne réserve pas de mémoire à l'avance comme Java et prend ce qu'il lui faut. Oteria Client est un fork d'Horizon (MIT). Ce n'est pas un produit officiel Minecraft, ni approuvé par Mojang ou Microsoft.",
                    ui::regular(12.5),
                    c::FAINT,
                    rect.width(),
                    3,
                );
            });
        for a in actions {
            match a {
                "open-home" => open_folder(&self.home),
                "open-game" => open_folder(&self.game_dir),
                "open-res" => open_folder(&resources),
                "check" => {
                    self.notice = None;
                    self.start_check();
                }
                "toggle-close" => {
                    self.state = State::load(&self.home);
                    self.state.close_on_launch = close_on_launch;
                    let _ = self.state.save(&self.home);
                }
                _ => {}
            }
        }
    }
}
