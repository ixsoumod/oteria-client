//! `OteriaLauncher.exe`: Oteria Client's launcher.
//!
//! On every start it asks `updater.exe check` what is new. A newer launcher is installed at once
//! (`updater.exe self-update`, which restarts it); a newer client makes the main button
//! « Mettre à jour » before it can say « Jouer ». The game is started directly from the
//! installed client, with no console window, and can be stopped from the same button.
//!
//! Testing (`tools/oteria/dist_test.ps1`): `OTERIA_HOME`, `OTERIA_GAME_DIR` and
//! `OTERIA_UPDATE_SOURCE` move everything to scratch folders; `OTERIA_LAUNCHER_PAGE`
//! (`mods|market|news|settings|accounts`) opens a page, `OTERIA_LAUNCHER_MENU=1` the account menu;
//! `OTERIA_LAUNCHER_AUTO=download,play,stop` presses the main button when it can;
//! `OTERIA_LAUNCHER_SHOT` saves a screenshot once the window has settled
//! (`OTERIA_LAUNCHER_SHOT_AT=downloading|running|self-update` picks the moment) and closes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{
    self, epaint::RectShape, text::LayoutJob, Align2, Color32, CornerRadius, Margin, Pos2, Rect,
    Sense, Stroke, StrokeKind, TextFormat, TextureHandle, Vec2,
};
use oteria_dist::microsoft::{self, DeviceCode, Poll, Session, SignedIn};
use oteria_dist::state::AccountRef;
use oteria_dist::ui::{self, c, Icons};
use oteria_dist::{install, paths, player, update_popup, CheckReport, NewsItem, Progress, State};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The icon column: an icon's bubble with the same small gap on each side.
const SIDE_W: f32 = NAV_GAP * 2.0 + NAV_TILE;
const NAV_TILE: f32 = 36.0;
const NAV_GAP: f32 = 12.0;
/// The content panel's plain dark colour, and the darker frame around it (the top bar, the icon
/// column, and a thin edge at the right and the bottom).
const WINDOW_BG: Color32 = Color32::from_rgb(0x14, 0x19, 0x21);
const SIDE_BG: Color32 = Color32::from_rgb(0x0c, 0x0f, 0x14);
/// Rows and cards, a step lighter than the plain background so they still stand out on it.
const PANEL: Color32 = Color32::from_rgb(0x1c, 0x23, 0x2e);
/// Buttons and chips on those rows, and their hover: lighter again.
const RAISED: Color32 = Color32::from_rgb(0x26, 0x2f, 0x3c);
const HOVER: Color32 = Color32::from_rgb(0x2f, 0x3a, 0x49);
/// The dark top bar: low, its logo, name, account chip and window buttons all `CHIP_H` tall.
const TOP_H: f32 = 44.0;
/// The height of everything in the top bar: the window buttons' bubble.
const CHIP_H: f32 = ui::WINDOW_ICON + 16.0;
/// The account chip's right edge, from the window's: just left of the three window buttons.
const CHIP_RIGHT: f32 = 26.0 + 36.0 * 2.0 + CHIP_H / 2.0 + 10.0;
/// The dark frame's thin right and bottom edges.
const FRAME: f32 = 8.0;
/// Inside the content panel: the gap between its rounded edge and what it holds.
const INSET: f32 = 20.0;
/// The corner radius of every surface.
const R: u8 = 8;
const BACKGROUND: &[u8] = include_bytes!("../../assets/background.jpg");
const FULL_UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

/// The banner's looping video: 0:02-0:20 of the clip recorded 2026-11-18, cropped of its black
/// strip and encoded as raw H.264 at 30 fps by `tools/oteria/make_menu_loop.ps1`.
const MENU_VIDEO: &[u8] = include_bytes!("../../assets/menu-loop.h264");
const MENU_VIDEO_FPS: f32 = 30.0;

fn main() -> eframe::Result {
    let home = paths::home();
    // Before any window: a newer launcher opens the small update window instead of this one.
    if let Ok(p) = std::env::var("OTERIA_UPDATE_POPUP_DEMO") {
        return update_popup::run(update_popup::Options {
            version: std::env::var("OTERIA_UPDATE_POPUP_VERSION").unwrap_or_else(|_| "1.0.6".into()),
            updater: updater_path(&home),
            home,
            demo: Some(p.parse().unwrap_or(6.0)),
            shot: std::env::var_os("OTERIA_LAUNCHER_SHOT").map(PathBuf::from),
        });
    }
    let no_self_update = std::env::args().any(|a| a == "--no-self-update");
    let report = run_check(&updater_path(&home));
    if let Some(version) = report.launcher_update.clone() {
        if !no_self_update && is_installed(&home) {
            return update_popup::run(update_popup::Options {
                updater: updater_copy(&home),
                version,
                home,
                demo: None,
                shot: std::env::var_os("OTERIA_LAUNCHER_SHOT").map(PathBuf::from),
            });
        }
    }
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
        Box::new(move |cc| Ok(Box::new(Launcher::new(&cc.egui_ctx, report, no_self_update)))),
    )
}

/// `updater.exe`: beside this launcher, else in the install folder.
fn updater_path(home: &Path) -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(paths::UPDATER_EXE)));
    beside
        .filter(|p| p.is_file())
        .unwrap_or_else(|| home.join(paths::UPDATER_EXE))
}

/// Whether this launcher is the installed one (a developer's build never replaces itself).
fn is_installed(home: &Path) -> bool {
    let me = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok());
    let there = home.join(paths::LAUNCHER_EXE).canonicalize().ok();
    me.is_some() && me == there
}

/// The updater that replaces this launcher runs from a copy: it may replace itself too.
fn updater_copy(home: &Path) -> PathBuf {
    let src = updater_path(home);
    let copy = home.join("updater.run.exe");
    if std::fs::copy(&src, &copy).is_ok() {
        copy
    } else {
        src
    }
}

/// Asks `updater.exe check` what is new (it also updates the updater itself).
fn run_check(updater: &Path) -> CheckReport {
    install::quiet_command(updater)
        .args(["check", "--launcher-version", oteria_dist::LAUNCHER_VERSION])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("updater.exe introuvable ({}) : {e}", updater.display()))
        .and_then(|o| {
            serde_json::from_slice::<CheckReport>(&o.stdout)
                .map_err(|e| format!("Réponse de l'updater illisible : {e}"))
        })
        .unwrap_or_else(|e| CheckReport {
            error: Some(e),
            ..Default::default()
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Play,
    Mods,
    Market,
    Settings,
    News,
    Accounts,
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
    /// Whether this replaces an older client.
    update: bool,
    stage: String,
    fraction: f32,
    done: u64,
    total: u64,
    note: String,
}

impl Download {
    /// How far the whole installation is, from 0 to 1: each stage takes its share of the bar.
    fn overall(&self) -> f32 {
        let (lo, hi) = match self.stage.as_str() {
            "download" => (0.0, 0.15),
            "extract" => (0.15, 0.2),
            "minecraft" => (0.2, 0.55),
            "java" => (0.55, 0.65),
            _ => (0.65, 1.0),
        };
        lo + (hi - lo) * self.fraction.clamp(0.0, 1.0)
    }
}

/// Which kind of account the "add an account" page is about.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Microsoft,
    Local,
}

/// Where a Microsoft sign-in is.
enum MsFlow {
    Idle,
    /// Asking Microsoft for a code.
    Starting,
    /// The player has to type `code` at `uri`.
    Waiting {
        code: String,
        uri: String,
        expires_at: u64,
    },
    /// Signed in: the account's name.
    Done(String),
    Failed(String),
}

/// What the sign-in thread reports.
enum MsEvent {
    Code(DeviceCode),
    Done(Result<SignedIn, String>),
}

struct Launcher {
    home: PathBuf,
    game_dir: PathBuf,
    state: State,
    /// The chosen account's name.
    name: String,
    /// The chosen account.
    current: Option<AccountRef>,
    /// A Minecraft session for the chosen Microsoft account, renewed in the background.
    session: Option<Session>,
    session_rx: Option<Receiver<Result<SignedIn, String>>>,
    /// Play was pressed and waits for the session: the server to join, if any.
    pending_launch: Option<Option<String>>,
    add_kind: Kind,
    ms_flow: MsFlow,
    ms_rx: Option<Receiver<MsEvent>>,
    ms_cancel: Arc<AtomicBool>,
    /// The skin chosen in Cosmetics, overriding the account's for this client only.
    skin: Option<ui::Skin>,
    /// The chosen Microsoft account's own skin, kept up to date as it signs in or renews.
    account_skin: Option<ui::Skin>,
    logo: TextureHandle,
    icons: Icons,
    background: TextureHandle,
    /// The video behind the Jouer button, decoded in the background.
    menu_video: oteria_dist::video::VideoLoop,
    /// The Jouer button's light, from its top left corner to its bottom right.
    button_sheen: TextureHandle,
    /// Darkens the bottom of the banner, under the buttons.
    hero_shade: TextureHandle,
    /// Darkens the bottom of a news picture, under its title.
    card_shade: TextureHandle,
    page: Page,
    phase: Phase,
    report: CheckReport,
    check_rx: Option<Receiver<CheckReport>>,
    download: Option<Download>,
    child: Option<Child>,
    /// A message on the banner: (text, colour).
    notice: Option<(String, Color32)>,
    total_ram: Option<u64>,
    menu_open: bool,
    account_input: String,
    account_error: Option<String>,
    auto: Vec<String>,
    shot: Option<PathBuf>,
    shot_at: String,
    settled_frames: u32,
    shot_sent: bool,
    started: Instant,
    /// Started again after its update window failed: never tries to update itself this run.
    no_self_update: bool,
}

impl Launcher {
    fn new(ctx: &egui::Context, report: CheckReport, no_self_update: bool) -> Self {
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
        // No account until the player signs in (Microsoft) or adds a local one: Play waits.
        let state = State::load(&home);
        let current = state.current();
        let name = current
            .as_ref()
            .map_or("Player".to_owned(), |a| a.name.clone());
        let skin = player::worn_skin(&game_dir).and_then(|p| ui::Skin::load(&p));
        let logo = ui::svg_texture(ctx, "logo", ui::LOGO_SVG, 192, false)
            .or_else(|| ui::texture_from_png(ctx, "logo", ui::LOGO_PNG))
            .expect("logo");
        let page = match std::env::var("OTERIA_LAUNCHER_PAGE").as_deref() {
            Ok("mods") => Page::Mods,
            Ok("market") => Page::Market,
            Ok("news") => Page::News,
            Ok("settings") => Page::Settings,
            Ok("accounts") => Page::Accounts,
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
        let mut me = Self {
            icons: Icons::load(ctx),
            background: ui::texture_from_png(ctx, "background", BACKGROUND).expect("background"),
            hero_shade: ui::vramp_texture(ctx, "hero-shade", 0.0, 0.7),
            card_shade: ui::vramp_texture(ctx, "card-shade", 0.1, 0.9),
            home,
            game_dir,
            state,
            name,
            current,
            session: None,
            session_rx: None,
            pending_launch: None,
            add_kind: if microsoft::client_id().is_some() {
                Kind::Microsoft
            } else {
                Kind::Local
            },
            ms_flow: MsFlow::Idle,
            ms_rx: None,
            ms_cancel: Arc::new(AtomicBool::new(false)),
            skin,
            account_skin: None,
            menu_video: oteria_dist::video::VideoLoop::start(MENU_VIDEO, MENU_VIDEO_FPS),
            button_sheen: sheen_texture(ctx),
            logo,
            page,
            phase: Phase::Checking,
            report: CheckReport::default(),
            check_rx: None,
            download: None,
            child: None,
            notice: None,
            total_ram: memory::total(),
            menu_open: std::env::var_os("OTERIA_LAUNCHER_MENU").is_some(),
            account_input: std::env::var("OTERIA_LAUNCHER_INPUT").unwrap_or_default(),
            account_error: None,
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
            no_self_update,
        };
        if updated {
            me.notice = Some((
                format!("Launcher mis à jour : v{}", oteria_dist::LAUNCHER_VERSION),
                c::GOOD,
            ));
        }
        me.log(&format!("démarrage v{}", oteria_dist::LAUNCHER_VERSION));
        // `main` already asked the updater, before opening any window.
        me.on_report(report);
        me.prefetch_session();
        if std::env::var_os("OTERIA_LAUNCHER_MS").is_some() {
            me.add_kind = Kind::Microsoft;
            me.start_sign_in();
        }
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

    /// Changes `state.json` from what is on disk (the updater writes it too), and keeps the result.
    fn update_state(&mut self, change: impl FnOnce(&mut State)) {
        let mut state = State::load(&self.home);
        change(&mut state);
        let _ = state.save(&self.home);
        let before = self.current.as_ref().map(|a| a.id.clone());
        self.current = state.current();
        self.name = self
            .current
            .as_ref()
            .map_or("Player".to_owned(), |a| a.name.clone());
        self.state = state;
        if self.current.as_ref().map(|a| a.id.clone()) != before {
            self.session = None;
            self.account_skin = None;
            self.prefetch_session();
        }
    }

    /// The skin shown in the launcher: the Cosmetics override if there is one, else the chosen
    /// Microsoft account's own skin.
    fn shown_skin(&self) -> Option<&ui::Skin> {
        self.skin.as_ref().or(self.account_skin.as_ref())
    }

    /// The chosen Microsoft account's UUID.
    fn ms_uuid(&self) -> Option<String> {
        let a = self.current.as_ref()?;
        a.microsoft
            .then(|| a.id.trim_start_matches("ms:").to_owned())
    }

    /// Gets a Minecraft session for the chosen Microsoft account in the background, so Play
    /// finds it ready.
    fn prefetch_session(&mut self) {
        let Some(uuid) = self.ms_uuid() else { return };
        if self.session_rx.is_some()
            || self
                .session
                .as_ref()
                .is_some_and(|s| s.uuid == uuid && s.fresh())
        {
            return;
        }
        let Some(client_id) = microsoft::client_id() else {
            return;
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(microsoft::session_for(&client_id, &uuid));
        });
        self.session_rx = Some(rx);
    }

    fn updater(&self) -> PathBuf {
        updater_path(&self.home)
    }

    fn installed(&self) -> bool {
        is_installed(&self.home)
    }

    fn start_check(&mut self) {
        self.phase = Phase::Checking;
        let (tx, rx) = channel();
        let updater = self.updater();
        std::thread::spawn(move || {
            let _ = tx.send(run_check(&updater));
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
            if self.no_self_update {
                self.notice = Some((
                    format!("Launcher v{v} disponible, mais sa mise à jour a échoué (voir launcher.log)"),
                    c::WARN,
                ));
            } else if self.installed() {
                self.phase = Phase::SelfUpdating {
                    version: v,
                    since: Instant::now(),
                    handed_over: false,
                };
                return;
            } else {
                self.notice = Some((
                    format!("Launcher v{v} disponible (cette copie n'est pas installée : pas de mise à jour automatique)"),
                    c::WARN,
                ));
            }
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
        let update = matches!(self.phase, Phase::NeedDownload { update: true });
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
            update,
            stage: "download".into(),
            fraction: 0.0,
            done: 0,
            total: 0,
            note: String::new(),
        });
        self.notice = None;
        self.phase = Phase::Downloading;
    }

    fn poll_download(&mut self) {
        let Some(d) = &mut self.download else { return };
        let mut finished = None;
        while let Ok(p) = d.rx.try_recv() {
            match p {
                Progress::Stage { stage } => {
                    d.stage = stage;
                    d.fraction = 0.0;
                    d.done = 0;
                    d.total = 0;
                    d.note.clear();
                }
                Progress::Bytes { done, total } => {
                    d.done = done;
                    d.total = total;
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

    /// Play: starts the game right away for a local account; for a Microsoft one, with its
    /// session, which is usually already there.
    fn launch(&mut self, server: Option<&str>) {
        if self.current.is_none() {
            self.page = Page::Accounts;
            return;
        }
        if self.state.client_dir(&self.home).is_none() {
            self.phase = Phase::NeedDownload { update: false };
            return;
        }
        let Some(uuid) = self.ms_uuid() else {
            self.start_game(server, None);
            return;
        };
        if let Some(session) = self.session.clone().filter(|s| s.uuid == uuid && s.fresh()) {
            self.start_game(server, Some(session));
            return;
        }
        // The session is not there yet (or too old): wait for it, then start.
        self.pending_launch = Some(server.map(str::to_owned));
        self.phase = Phase::Launching {
            since: Instant::now(),
        };
        self.notice = Some(("Connexion à Microsoft…".into(), Color32::WHITE));
        self.prefetch_session();
        if self.session_rx.is_none() {
            // No way to ask (no client ID): say so.
            self.pending_launch = None;
            self.phase = Phase::Ready;
            self.notice = Some((
                "Cette version du launcher n'a pas de connexion Microsoft.".into(),
                c::BAD,
            ));
        }
    }

    /// Starts the installed client, with no console window; with a Microsoft session it goes
    /// through the game's standard input, never the command line.
    fn start_game(&mut self, server: Option<&str>, session: Option<Session>) {
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
            .arg("--resources")
            .arg(self.state.resources_dir(&self.home))
            .env("HORIZON_HOME", self.game_dir.join("home"))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if session.is_some() {
            cmd.arg("--session-stdin").stdin(Stdio::piped());
        } else {
            cmd.arg("--name").arg(&self.name).stdin(Stdio::null());
        }
        if let Some(s) = server {
            cmd.arg("--server").arg(s);
        }
        if let Ok(extra) = std::env::var("OTERIA_GAME_ARGS") {
            cmd.args(extra.split_whitespace());
        }
        match cmd.spawn() {
            Ok(mut child) => {
                if let (Some(s), Some(mut stdin)) = (&session, child.stdin.take()) {
                    use std::io::Write as _;
                    let _ = stdin.write_all(s.game_json().as_bytes());
                }
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
        if self.pending_launch.take().is_some() {
            self.notice = Some(("Lancement annulé.".into(), Color32::WHITE));
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            self.log(&format!("jeu arrêté (pid {})", child.id()));
            self.notice = Some(("Le jeu a été arrêté.".into(), Color32::WHITE));
        }
        self.phase = Phase::Ready;
    }

    fn main_action(&mut self) {
        let busy = matches!(self.phase, Phase::Launching { .. } | Phase::Running);
        if self.current.is_none() && !busy {
            return;
        }
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
        self.poll_download();
        self.poll_session();
        self.poll_sign_in();
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
            Phase::Launching { since }
                if self.child.is_some() && since.elapsed() > Duration::from_millis(2500) =>
            {
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

    /// The session fetched in the background arrived: keep it, and start the game if Play waits.
    fn poll_session(&mut self) {
        let Some(rx) = &self.session_rx else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.session_rx = None;
        match result {
            Ok(signed) => {
                self.log(&format!(
                    "session Microsoft prête pour {}",
                    signed.session.name
                ));
                self.account_skin = signed.skin.as_deref().and_then(ui::Skin::from_bytes);
                self.session = Some(signed.session.clone());
                if let Some(server) = self.pending_launch.take() {
                    self.start_game(server.as_deref(), Some(signed.session));
                }
            }
            Err(e) => {
                self.log(&format!("session Microsoft refusée : {e}"));
                if self.pending_launch.take().is_some() {
                    self.phase = Phase::Ready;
                }
                self.notice = Some((e, c::BAD));
            }
        }
    }

    /// Starts a Microsoft sign-in: a code to type at microsoft.com/link, then the account.
    /// The code page in a private Edge window: no Microsoft account is signed in there, so the
    /// player signs in with their own, never the one their browser remembers (microsoft.com/link
    /// takes it without asking). Without Edge, the default browser.
    fn open_sign_in_page(uri: &str) {
        let edge = ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|d| std::path::Path::new(&d).join(r"Microsoft\Edge\Application\msedge.exe"))
            .find(|p| p.is_file());
        let opened = edge.is_some_and(|edge| {
            std::process::Command::new(edge)
                .args(["--inprivate", uri])
                .spawn()
                .is_ok()
        });
        if !opened {
            let _ = install::quiet_command("cmd")
                .args(["/c", "start", "", uri])
                .spawn();
        }
    }

    fn start_sign_in(&mut self) {
        let Some(client_id) = microsoft::client_id() else {
            self.ms_flow =
                MsFlow::Failed("Cette version du launcher n'a pas de connexion Microsoft.".into());
            return;
        };
        self.ms_cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.ms_cancel = cancel.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<SignedIn, String> {
                let code = microsoft::device_code(&client_id)?;
                let _ = tx.send(MsEvent::Code(code.clone()));
                let mut wait = code.interval;
                let tokens = loop {
                    let until = Instant::now() + Duration::from_secs(wait);
                    while Instant::now() < until {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("Connexion annulée.".into());
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    if microsoft::now() > code.expires_at {
                        return Err("Le code a expiré. Recommence.".into());
                    }
                    match microsoft::poll(&client_id, &code)? {
                        Poll::Pending => {}
                        Poll::SlowDown => wait += 5,
                        Poll::Done(tokens) => break tokens,
                    }
                };
                // An account is added only if it owns the game.
                let signed = microsoft::sign_in_with(tokens, true)?;
                microsoft::store_refresh(&signed.session.uuid, &signed.refresh)?;
                Ok(signed)
            })();
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send(MsEvent::Done(result));
            }
        });
        self.ms_rx = Some(rx);
        self.ms_flow = MsFlow::Starting;
    }

    fn cancel_sign_in(&mut self) {
        self.ms_cancel.store(true, Ordering::Relaxed);
        self.ms_rx = None;
        self.ms_flow = MsFlow::Idle;
    }

    fn poll_sign_in(&mut self) {
        let Some(rx) = &self.ms_rx else { return };
        while let Ok(event) = rx.try_recv() {
            match event {
                MsEvent::Code(code) => {
                    // The page opens by itself: the player asked for this sign-in.
                    if std::env::var_os("OTERIA_NO_BROWSER").is_none() {
                        Self::open_sign_in_page(&code.verification_uri);
                    }
                    self.ms_flow = MsFlow::Waiting {
                        code: code.user_code.clone(),
                        uri: code.verification_uri.clone(),
                        expires_at: code.expires_at,
                    };
                }
                MsEvent::Done(Ok(signed)) => {
                    self.log(&format!(
                        "compte Microsoft ajouté : {}",
                        signed.session.name
                    ));
                    let (uuid, name) = (signed.session.uuid.clone(), signed.session.name.clone());
                    self.session = Some(signed.session);
                    self.update_state(|s| s.add_ms_account(&uuid, &name));
                    // After `update_state`, which clears it when the account changes.
                    self.account_skin = signed.skin.as_deref().and_then(ui::Skin::from_bytes);
                    self.ms_flow = MsFlow::Done(name);
                    self.ms_rx = None;
                    return;
                }
                MsEvent::Done(Err(e)) => {
                    self.log(&format!("connexion Microsoft échouée : {e}"));
                    self.ms_flow = MsFlow::Failed(e);
                    self.ms_rx = None;
                    return;
                }
            }
        }
    }

    fn updater_for_self_update(&self) -> PathBuf {
        updater_copy(&self.home)
    }

    fn settled(&self) -> bool {
        match self.shot_at.as_str() {
            "downloading" => self
                .download
                .as_ref()
                .is_some_and(|d| d.stage == "download" && d.fraction > 0.35),
            "self-update" => matches!(self.phase, Phase::SelfUpdating { .. }),
            "ms-code" => matches!(self.ms_flow, MsFlow::Waiting { .. }),
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

fn mib(bytes: u64) -> String {
    format!("{:.0}", bytes as f64 / (1u64 << 20) as f64)
}

fn kind_label(a: &AccountRef) -> &'static str {
    if a.microsoft {
        "Compte Microsoft"
    } else {
        "Compte local"
    }
}

fn open_folder(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = Command::new("explorer").arg(dir).spawn();
}

/// A dark translucent fill for buttons over a picture.
fn over_picture(hovered: bool) -> Color32 {
    Color32::from_rgba_unmultiplied(8, 12, 18, if hovered { 235 } else { 190 })
}

impl eframe::App for Launcher {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        WINDOW_BG.to_normalized_gamma_f32()
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
            .frame(egui::Frame::NONE.fill(SIDE_BG))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                // The content panel, framed by the dark bars, its corners rounded.
                let content = Rect::from_min_max(
                    Pos2::new(full.left() + SIDE_W, full.top() + TOP_H),
                    full.max - Vec2::splat(FRAME),
                );
                ui.painter()
                    .rect_filled(content, CornerRadius::same(12), WINDOW_BG);
                let body = content.shrink(INSET);
                match self.page {
                    Page::Play => self.play_page(ui, body),
                    Page::Mods => self.mods_page(ui, body),
                    Page::Market => self.market_page(ui, body),
                    Page::Settings => self.settings_page(ui, body),
                    Page::News => self.news_page(ui, body),
                    Page::Accounts => self.accounts_page(ui, body),
                }
                self.sidebar(
                    ui,
                    Rect::from_min_max(
                        Pos2::new(full.left(), full.top() + TOP_H),
                        Pos2::new(full.left() + SIDE_W, full.bottom()),
                    ),
                );
                self.top_bar(ui, full);
                if self.menu_open {
                    self.account_menu(ui, full);
                }
                self.modal(ui, full);
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

/// Draws `tex` (`aspect` wide for 1 high) covering `r`, the part starting at `u0` (0 to 1) of
/// what is left over when it is cropped sideways.
fn cover(p: &egui::Painter, r: Rect, radius: u8, tex: &TextureHandle, u0: f32, tint: Color32) {
    let aspect = r.width() / r.height();
    let uv = if aspect < tex.aspect_ratio() {
        let w = aspect / tex.aspect_ratio();
        let x = (1.0 - w) * u0;
        Rect::from_min_max(Pos2::new(x, 0.0), Pos2::new(x + w, 1.0))
    } else {
        let h = tex.aspect_ratio() / aspect;
        let y = (1.0 - h) * 0.4;
        Rect::from_min_max(Pos2::new(0.0, y), Pos2::new(1.0, y + h))
    };
    p.add(RectShape::filled(r, CornerRadius::same(radius), tint).with_texture(tex.id(), uv));
}

fn shade(p: &egui::Painter, r: Rect, radius: u8, tex: &TextureHandle) {
    p.add(
        RectShape::filled(r, CornerRadius::same(radius), Color32::WHITE)
            .with_texture(tex.id(), FULL_UV),
    );
}

fn surface(p: &egui::Painter, r: Rect, fill: Color32) {
    p.rect_filled(r, CornerRadius::same(R), fill);
}

/// What a button looks like.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Quiet,
    Accent,
}

/// A flat button: an optional icon and a label. Returns whether it was clicked.
fn button(
    ui: &mut egui::Ui,
    icons: &Icons,
    r: Rect,
    icon: Option<&str>,
    label: &str,
    id: &str,
    look: Look,
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
    let hovered = enabled && resp.hovered();
    let p = ui.painter();
    let (fill, fg) = match (look, enabled) {
        (_, false) => (PANEL, c::FAINT),
        (Look::Accent, true) => (
            if hovered { c::ACCENT_HOVER } else { c::ACCENT },
            Color32::WHITE,
        ),
        (Look::Quiet, true) => (if hovered { HOVER } else { RAISED }, c::TEXT),
    };
    surface(p, r, fill);
    let g = p.layout_no_wrap(label.to_owned(), ui::semi(13.5), fg);
    let icon_w = if icon.is_some() { 26.0 } else { 0.0 };
    let x0 = r.center().x - (g.size().x + icon_w) / 2.0;
    if let Some(name) = icon {
        icons.draw(p, name, Pos2::new(x0 + 8.0, r.center().y), 16.0, fg);
    }
    p.galley(
        Pos2::new(x0 + icon_w, r.center().y - g.size().y / 2.0),
        g,
        fg,
    );
    enabled && resp.clicked()
}

/// A square button with just an icon, over the video. Returns whether it was clicked.
fn icon_button(ui: &mut egui::Ui, icons: &Icons, r: Rect, icon: &str, id: &str) -> bool {
    let resp = ui.interact(r, ui.id().with(("icon-button", id)), Sense::click());
    let p = ui.painter();
    surface(p, r, over_picture(resp.hovered()));
    icons.draw_fit(p, icon, r.center(), 17.0, Color32::WHITE);
    resp.clicked()
}

/// The Jouer button's light: white at its top left, a little darker at its bottom right, so a
/// tint of the button's colour gives it its gradient.
fn sheen_texture(ctx: &egui::Context) -> TextureHandle {
    let (w, h) = (128usize, 32usize);
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let t = (x as f32 / (w - 1) as f32) * 0.7 + (y as f32 / (h - 1) as f32) * 0.3;
            let v = (255.0 * (1.0 - 0.22 * t)).round() as u8;
            pixels.push(Color32::from_gray(v));
        }
    }
    ctx.load_texture(
        "button-sheen",
        egui::ColorImage {
            size: [w, h],
            source_size: Vec2::new(w as f32, h as f32),
            pixels,
        },
        egui::TextureOptions::LINEAR,
    )
}

/// A switch. Returns whether it was clicked.
fn toggle(ui: &mut egui::Ui, r: Rect, on: bool, id: &str) -> bool {
    let resp = ui.interact(r, ui.id().with(("toggle", id)), Sense::click());
    let p = ui.painter();
    let fill = if on {
        c::ACCENT
    } else {
        Color32::from_rgb(0x2b, 0x36, 0x46)
    };
    p.rect_filled(r, CornerRadius::same(11), fill);
    let x = if on {
        r.right() - 12.0
    } else {
        r.left() + 12.0
    };
    p.circle_filled(Pos2::new(x, r.center().y), 8.0, Color32::WHITE);
    resp.clicked()
}

/// A small caption in spaced capitals.
fn caption(p: &egui::Painter, pos: Pos2, s: &str) -> Rect {
    ui::spaced(p, pos, s, ui::semi(11.5), c::MUTED, 1.2)
}

/// "OTERIA" and "LAUNCHER" in one line, left-aligned on `pos` (the line's middle).
fn wordmark(p: &egui::Painter, pos: Pos2, size: f32) {
    let mut job = LayoutJob::default();
    job.append(
        "OTERIA",
        0.0,
        TextFormat {
            font_id: ui::semi(size),
            color: c::TEXT,
            ..Default::default()
        },
    );
    job.append(
        "LAUNCHER",
        size * 0.4,
        TextFormat {
            font_id: ui::regular(size),
            color: c::MUTED,
            ..Default::default()
        },
    );
    let g = p.layout_job(job);
    let at = Pos2::new(pos.x, pos.y - g.size().y / 2.0);
    p.galley(at, g, c::TEXT);
}

impl Launcher {
    fn top_bar(&mut self, ui: &mut egui::Ui, full: Rect) {
        let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), TOP_H));
        // Dragging the bar moves the window; a double click maximises it.
        let drag = ui.interact(bar, ui.id().with("titlebar"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let m = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!m));
        }
        let p = ui.painter().clone();
        p.image(
            self.logo.id(),
            Rect::from_center_size(
                Pos2::new(bar.left() + SIDE_W / 2.0, bar.center().y),
                Vec2::splat(CHIP_H),
            ),
            FULL_UV,
            Color32::WHITE,
        );
        wordmark(&p, Pos2::new(bar.left() + SIDE_W + 2.0, bar.center().y), 17.0);

        // The account: its face, its name, and the menu; with none yet, a plus and "Connexion",
        // which opens the page to add one. As tall as the window buttons' bubble.
        let signed_in = self.current.is_some();
        let label = if signed_in {
            self.name.clone()
        } else {
            "Connexion".to_owned()
        };
        let font = ui::semi(13.0);
        let text_w = p.layout_no_wrap(label.clone(), font.clone(), c::TEXT).size().x;
        let pad = 6.0;
        let lead = CHIP_H - pad * 2.0;
        let w = pad + lead + 8.0 + text_w + if signed_in { 30.0 } else { 12.0 };
        let chip = Rect::from_min_size(
            Pos2::new(bar.right() - CHIP_RIGHT - w, bar.center().y - CHIP_H / 2.0),
            Vec2::new(w, CHIP_H),
        );
        let resp = ui.interact(chip, ui.id().with("account-chip"), Sense::click());
        p.rect_filled(
            chip,
            CornerRadius::same((CHIP_H * 0.28).round() as u8),
            if resp.hovered() || self.menu_open {
                HOVER
            } else {
                RAISED
            },
        );
        let lead_rect = Rect::from_min_size(
            Pos2::new(chip.left() + pad, chip.center().y - lead / 2.0),
            Vec2::splat(lead),
        );
        if signed_in {
            ui::draw_head(&p, lead_rect, self.shown_skin());
            self.icons.draw(
                &p,
                "chevron-down",
                Pos2::new(chip.right() - 15.0, chip.center().y),
                10.0,
                c::MUTED,
            );
        } else {
            self.icons
                .draw(&p, "plus-lg", lead_rect.center(), 13.0, c::TEXT);
        }
        ui::text(
            &p,
            Pos2::new(lead_rect.right() + 8.0, chip.center().y),
            Align2::LEFT_CENTER,
            &label,
            font,
            c::TEXT,
        );
        if resp.clicked() {
            if signed_in {
                self.menu_open = !self.menu_open;
            } else {
                self.page = Page::Accounts;
                self.menu_open = false;
            }
        }
        ui::window_controls(ui, bar, &self.icons);
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        // Each entry: just an icon, all of the same drawn size, and a bubble behind it when it is
        // hovered or open. Paramètres sits alone at the bottom left, as far from the bottom as
        // from the left.
        const ICON: f32 = 20.0;
        const TILE: Vec2 = Vec2::splat(NAV_TILE);
        let margin = NAV_GAP;
        let items = [
            (Page::Play, "play-fill"),
            (Page::Mods, "puzzle-fill"),
            (Page::Market, "cart-fill"),
            (Page::Settings, "gear-fill"),
        ];
        let last = items.len() - 1;
        for (i, (page, icon)) in items.into_iter().enumerate() {
            let min = if i == last {
                Pos2::new(rect.left() + margin, rect.bottom() - margin - TILE.y)
            } else {
                Pos2::new(rect.left() + margin, rect.top() + margin + i as f32 * (TILE.y + 10.0))
            };
            let r = Rect::from_min_size(min, TILE);
            let resp = ui.interact(r, ui.id().with(("nav", i)), Sense::click());
            let active = self.page == page
                || (page == Page::Play && self.page == Page::News)
                || (page == Page::Settings && self.page == Page::Accounts);
            let center = r.center();
            if resp.hovered() || active {
                ui::bubble(&p, center, NAV_TILE, if resp.hovered() { 1.0 } else { 0.7 });
            }
            let col = if active || resp.hovered() {
                Color32::WHITE
            } else {
                c::MUTED
            };
            self.icons.draw_fit(&p, icon, center, ICON, col);
            if resp.clicked() {
                self.page = page;
                self.menu_open = false;
            }
        }
    }

    fn play_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let gap = 22.0;
        // The banner spans about three quarters of the window, the partner servers the rest.
        let window_w = ui.ctx().content_rect().width();
        let main_w = (window_w * 0.75).min(body.width() - 200.0 - gap);
        let main = Rect::from_min_size(body.min, Vec2::new(main_w, body.height()));
        let side = Rect::from_min_max(Pos2::new(main.right() + gap, body.top()), body.max);
        let hero = Rect::from_min_size(main.min, Vec2::new(main.width(), main.height() * 0.48));
        self.hero(ui, hero);

        let p = ui.painter().clone();
        let top = hero.bottom() + 22.0;
        let news_w = main.width() * 0.64;
        ui::text(
            &p,
            Pos2::new(main.left(), top),
            Align2::LEFT_TOP,
            "ACTUALITÉS",
            ui::semi(15.0),
            c::TEXT,
        );
        let cards = Rect::from_min_max(
            Pos2::new(main.left(), top + 32.0),
            Pos2::new(main.left() + news_w, main.bottom()),
        );
        self.news_cards(ui, cards);
        let fx = main.left() + news_w + 30.0;
        ui::text(
            &p,
            Pos2::new(fx, top),
            Align2::LEFT_TOP,
            "AMIS",
            ui::semi(15.0),
            c::TEXT,
        );
        ui::wrapped(
            &p,
            Pos2::new(fx, top + 34.0),
            "Aucun ami pour l'instant. Connecte un compte Microsoft pour en ajouter.",
            ui::regular(13.5),
            c::MUTED,
            main.right() - fx,
            4,
        );
        self.partner_servers(ui, side);
    }

    fn hero(&mut self, ui: &mut egui::Ui, r: Rect) {
        let p = ui.painter().clone();
        match self.menu_video.texture(ui.ctx()) {
            Some(tex) => cover(&p, r, R, tex, 0.5, Color32::WHITE),
            None => cover(&p, r, R, &self.background, 0.5, Color32::WHITE),
        }
        shade(&p, r, R, &self.hero_shade);

        // Jouer, in the middle of the video.
        let btn = Rect::from_center_size(r.center(), Vec2::new(300.0, 66.0));
        self.main_button(ui, btn);

        // The game folder and its screenshots: two icons, one over the other, bottom left.
        let size = 40.0;
        let shots = Rect::from_min_size(
            Pos2::new(r.left() + 16.0, r.bottom() - 16.0 - size),
            Vec2::splat(size),
        );
        let folder = shots.translate(Vec2::new(0.0, -size - 8.0));
        if icon_button(ui, &self.icons, folder, "folder-fill", "game-folder") {
            open_folder(&self.game_dir);
        }
        if icon_button(ui, &self.icons, shots, "file-earmark-image-fill", "screenshots") {
            open_folder(&self.game_dir.join("screenshots"));
        }
        let notice_x = shots.right() + 14.0;

        // What just happened, in the corner.
        let text = match (&self.notice, &self.phase) {
            (Some((t, col)), _) => Some((t.clone(), *col)),
            (None, Phase::Failed(e)) => Some((e.clone(), c::BAD)),
            (None, Phase::NeedDownload { update: true }) => {
                Some(("Une mise à jour est nécessaire pour jouer.".into(), c::WARN))
            }
            (None, Phase::NeedDownload { update: false }) => Some((
                "Le jeu n'est pas encore installé sur ce PC.".into(),
                Color32::WHITE,
            )),
            _ => None,
        };
        if let Some((t, col)) = text {
            ui::wrapped(
                &p,
                Pos2::new(notice_x, r.bottom() - 40.0),
                &t,
                ui::medium(13.0),
                ui::lerp(col, Color32::WHITE, 0.35),
                r.width() * 0.3,
                2,
            );
        }
    }

    fn main_button(&mut self, ui: &mut egui::Ui, btn: Rect) {
        let latest = self.report.client_latest.clone().unwrap_or_default();
        let installed = self.state.client_version.clone().unwrap_or_default();
        let game = format!(
            "Oteria {installed} · Minecraft {}",
            oteria_dist::MINECRAFT_VERSION
        );
        let (label, sub, enabled, danger) = match &self.phase {
            Phase::Checking => ("RECHERCHE…", "Mises à jour".to_owned(), false, false),
            Phase::SelfUpdating { .. } => {
                ("MISE À JOUR…", "Le launcher redémarre".into(), false, false)
            }
            Phase::NeedDownload { update: false } => {
                ("TÉLÉCHARGER", format!("Oteria {latest}"), true, false)
            }
            Phase::NeedDownload { update: true } => (
                "METTRE À JOUR",
                format!("{installed} → {latest}"),
                true,
                false,
            ),
            Phase::Downloading => ("INSTALLATION…", String::new(), false, false),
            Phase::Ready => ("JOUER", game, true, false),
            Phase::Launching { .. } if self.child.is_none() => {
                ("ANNULER", "Connexion à Microsoft…".to_owned(), true, true)
            }
            Phase::Launching { .. } | Phase::Running => {
                ("ARRÊTER LE JEU", "Le jeu est ouvert".into(), true, true)
            }
            Phase::Failed(_) => ("RÉESSAYER", "Source injoignable".into(), true, false),
        };
        // No account yet: nothing to download or play until there is one.
        let locked = self.current.is_none()
            && matches!(
                self.phase,
                Phase::NeedDownload { .. } | Phase::Ready | Phase::Failed(_)
            );
        let (label, sub, enabled, danger) = if locked {
            (
                "CONNECTEZ-VOUS",
                "Compte local ou Microsoft requis".to_owned(),
                false,
                true,
            )
        } else {
            (label, sub, enabled, danger)
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
        // Green, or red to stop or when an account is missing, or a muted green while busy.
        let (base, edge, glow) = match (enabled, danger) {
            (_, true) => (
                Color32::from_rgb(0xe0, 0x4b, 0x4b),
                Color32::from_rgb(0xf2, 0x7a, 0x7a),
                Color32::from_rgb(0xe0, 0x4b, 0x4b),
            ),
            (true, false) => (
                Color32::from_rgb(0x22, 0xc5, 0x5e),
                Color32::from_rgb(0x5e, 0xe6, 0x8f),
                Color32::from_rgb(0x2e, 0xe0, 0x6e),
            ),
            (false, false) => (
                Color32::from_rgb(0x3f, 0x7a, 0x55),
                Color32::from_rgb(0x55, 0x8f, 0x6a),
                Color32::TRANSPARENT,
            ),
        };
        let radius = 9u8;
        // On hover, a soft light around it.
        if hovered {
            p.add(
                egui::epaint::Shadow {
                    offset: [0, 0],
                    blur: 34,
                    spread: 5,
                    color: ui::alpha(glow, 0.85),
                }
                .as_shape(btn, CornerRadius::same(radius)),
            );
        }
        let base = if hovered {
            ui::lerp(base, Color32::WHITE, 0.08)
        } else {
            base
        };
        p.add(
            RectShape::filled(btn, CornerRadius::same(radius), base)
                .with_texture(self.button_sheen.id(), FULL_UV),
        );
        p.rect_stroke(
            btn,
            CornerRadius::same(radius),
            Stroke::new(1.0_f32, ui::alpha(edge, 0.55)),
            StrokeKind::Inside,
        );
        let fg = if enabled || locked {
            Color32::WHITE
        } else {
            Color32::from_gray(215)
        };
        let two = !sub.is_empty();
        ui::text(
            &p,
            Pos2::new(btn.center().x, btn.center().y - if two { 10.0 } else { 0.0 }),
            Align2::CENTER_CENTER,
            label,
            ui::semi(if locked { 19.0 } else { 22.0 }),
            fg,
        );
        if two {
            // The second line: Oteria's logo, then what will start.
            let g = p.layout_no_wrap(sub.clone(), ui::medium(12.5), ui::alpha(fg, 0.9));
            let logo = 15.0;
            let total = logo + 6.0 + g.size().x;
            let x0 = btn.center().x - total / 2.0;
            let y = btn.center().y + 14.0;
            p.image(
                self.logo.id(),
                Rect::from_center_size(Pos2::new(x0 + logo / 2.0, y), Vec2::splat(logo)),
                FULL_UV,
                Color32::WHITE,
            );
            p.galley(
                Pos2::new(x0 + logo + 6.0, y - g.size().y / 2.0),
                g,
                fg,
            );
        }
        if enabled && resp.clicked() {
            self.main_action();
        }
    }

    fn news_cards(&mut self, ui: &mut egui::Ui, area: Rect) {
        let p = ui.painter().clone();
        let news = self.news();
        if news.is_empty() {
            ui::text(
                &p,
                area.min,
                Align2::LEFT_TOP,
                "Pas d'actualité pour le moment.",
                ui::regular(13.5),
                c::MUTED,
            );
            return;
        }
        let gap = 16.0;
        let w = (area.width() - gap) / 2.0;
        for (i, item) in news.iter().take(2).enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(area.left() + i as f32 * (w + gap), area.top()),
                Vec2::new(w, area.height()),
            );
            let resp = ui.interact(r, ui.id().with(("news", i)), Sense::click());
            cover(
                &p,
                r,
                R,
                &self.background,
                i as f32,
                Color32::from_gray(if resp.hovered() { 255 } else { 235 }),
            );
            shade(&p, r, R, &self.card_shade);
            let mut job = LayoutJob::simple(
                item.title.clone(),
                ui::semi(16.0),
                Color32::WHITE,
                r.width() - 36.0,
            );
            job.wrap.max_rows = 2;
            let g = p.layout_job(job);
            let date = format!("{}  ·  {}", item.date, item.tag);
            let y = r.bottom() - 18.0 - 16.0 - 6.0 - g.size().y;
            p.galley(Pos2::new(r.left() + 18.0, y), g, Color32::WHITE);
            ui::text(
                &p,
                Pos2::new(r.left() + 18.0, r.bottom() - 18.0),
                Align2::LEFT_BOTTOM,
                &date,
                ui::regular(12.0),
                Color32::from_gray(200),
            );
            if resp.clicked() {
                self.page = Page::News;
            }
        }
    }

    fn partner_servers(&mut self, ui: &mut egui::Ui, side: Rect) {
        let p = ui.painter().clone();
        // The title with a line on each side.
        let y = side.top() + 12.0;
        let g = p.layout_no_wrap("SERVEURS PARTENAIRES".into(), ui::semi(11.5), c::MUTED);
        let tx = side.center().x - g.size().x / 2.0;
        p.line_segment(
            [
                Pos2::new(side.left(), y + 7.0),
                Pos2::new(tx - 12.0, y + 7.0),
            ],
            Stroke::new(1.0_f32, c::BORDER),
        );
        p.line_segment(
            [
                Pos2::new(tx + g.size().x + 12.0, y + 7.0),
                Pos2::new(side.right(), y + 7.0),
            ],
            Stroke::new(1.0_f32, c::BORDER),
        );
        p.galley(Pos2::new(tx, y), g, c::MUTED);
        let servers = self.report.servers.clone();
        if servers.is_empty() {
            ui::text(
                &p,
                Pos2::new(side.center().x, y + 44.0),
                Align2::CENTER_CENTER,
                "Aucun serveur pour le moment.",
                ui::regular(13.5),
                c::MUTED,
            );
            return;
        }
        let can_join = matches!(self.phase, Phase::Ready);
        for (i, s) in servers.iter().take(7).enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(side.left(), y + 34.0 + i as f32 * 76.0),
                Vec2::new(side.width(), 68.0),
            );
            surface(&p, row, PANEL);
            let ic = Rect::from_min_size(row.min + Vec2::new(14.0, 14.0), Vec2::splat(40.0));
            surface(&p, ic, RAISED);
            self.icons
                .draw(&p, "hdd-network", ic.center(), 20.0, c::MUTED);
            ui::text(
                &p,
                Pos2::new(ic.right() + 14.0, row.top() + 16.0),
                Align2::LEFT_TOP,
                &s.name,
                ui::semi(15.0),
                c::TEXT,
            );
            ui::wrapped(
                &p,
                Pos2::new(ic.right() + 14.0, row.top() + 38.0),
                &s.address,
                ui::regular(12.5),
                c::MUTED,
                row.width() - 170.0,
                1,
            );
            let join = Rect::from_min_size(
                Pos2::new(row.right() - 96.0, row.center().y - 17.0),
                Vec2::new(82.0, 34.0),
            );
            if button(
                ui,
                &self.icons,
                join,
                None,
                "Rejoindre",
                &format!("join-{i}"),
                Look::Quiet,
                can_join,
            ) {
                let addr = s.address.clone();
                self.launch(Some(&addr));
            }
        }
    }

    fn page_title(&self, p: &egui::Painter, body: Rect, title: &str) -> f32 {
        ui::text(
            p,
            body.min + Vec2::new(4.0, 6.0),
            Align2::LEFT_TOP,
            title,
            ui::semi(26.0),
            c::TEXT,
        );
        body.top() + 62.0
    }

    fn mods_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(&p, body, "Mods");
        let dir = self.game_dir.join("mods");
        let open = Rect::from_min_size(
            Pos2::new(body.right() - 230.0, body.top() + 4.0),
            Vec2::new(230.0, 40.0),
        );
        if button(
            ui,
            &self.icons,
            open,
            Some("folder2-open"),
            "Ouvrir le dossier des mods",
            "open-mods",
            Look::Quiet,
            true,
        ) {
            open_folder(&dir);
        }
        let mut files: Vec<(String, u64)> = std::fs::read_dir(&dir)
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
        if files.is_empty() {
            ui::text(
                &p,
                Pos2::new(body.left() + 4.0, top + 4.0),
                Align2::LEFT_TOP,
                "Aucun mod installé.",
                ui::regular(14.0),
                c::MUTED,
            );
            return;
        }
        for (i, (name, size)) in files.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(body.left(), top + i as f32 * 64.0),
                Vec2::new(body.width().min(900.0), 56.0),
            );
            if row.bottom() > body.bottom() {
                break;
            }
            surface(&p, row, PANEL);
            self.icons.draw(
                &p,
                "puzzle-fill",
                Pos2::new(row.left() + 30.0, row.center().y),
                20.0,
                c::MUTED,
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 58.0, row.center().y),
                Align2::LEFT_CENTER,
                name,
                ui::medium(14.5),
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
        let top = self.page_title(&p, body, "Marketplace");
        ui::text(
            &p,
            Pos2::new(body.left() + 4.0, top + 4.0),
            Align2::LEFT_TOP,
            "La marketplace arrive bientôt.",
            ui::regular(14.0),
            c::MUTED,
        );
    }

    fn news_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(&p, body, "Actualités");
        let news = self.news();
        if news.is_empty() {
            ui::text(
                &p,
                Pos2::new(body.left() + 4.0, top + 4.0),
                Align2::LEFT_TOP,
                "Pas d'actualité pour le moment.",
                ui::regular(14.0),
                c::MUTED,
            );
        }
        for (i, item) in news.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(body.left(), top + i as f32 * 118.0),
                Vec2::new(body.width().min(900.0), 106.0),
            );
            if row.bottom() > body.bottom() {
                break;
            }
            surface(&p, row, PANEL);
            ui::text(
                &p,
                row.min + Vec2::new(22.0, 18.0),
                Align2::LEFT_TOP,
                &item.title,
                ui::semi(16.5),
                c::TEXT,
            );
            ui::text(
                &p,
                Pos2::new(row.right() - 22.0, row.top() + 20.0),
                Align2::RIGHT_TOP,
                &format!("{}  ·  {}", item.date, item.tag),
                ui::regular(12.5),
                c::FAINT,
            );
            ui::wrapped(
                &p,
                row.min + Vec2::new(22.0, 50.0),
                &item.body,
                ui::regular(13.5),
                c::MUTED,
                row.width() - 44.0,
                2,
            );
        }
    }

    // ----- Accounts ---------------------------------------------------------------------------

    /// The account menu under the chip: the accounts, then manage and add.
    fn account_menu(&mut self, ui: &mut egui::Ui, full: Rect) {
        // Anything outside the menu closes it.
        let eater = ui.interact(full, ui.id().with("menu-eater"), Sense::click());
        if eater.clicked() {
            self.menu_open = false;
            return;
        }
        let p = ui.painter().clone();
        let accounts = self.state.all_accounts();
        let current = self.current.as_ref().map(|a| a.id.clone());
        let w = 290.0;
        let h = 52.0 + accounts.len() as f32 * 54.0 + 12.0 + 2.0 * 46.0 + 8.0;
        // Right-aligned under the chip.
        let r = Rect::from_min_size(
            Pos2::new(
                full.right() - CHIP_RIGHT - w,
                full.top() + TOP_H + 2.0,
            ),
            Vec2::new(w, h),
        );
        p.rect(
            r,
            CornerRadius::same(10),
            PANEL,
            Stroke::new(1.0_f32, c::BORDER),
            StrokeKind::Inside,
        );
        caption(&p, r.min + Vec2::new(20.0, 22.0), "VOS COMPTES");
        let mut y = r.top() + 48.0;
        let mut picked = None;
        for (i, account) in accounts.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(r.left() + 6.0, y), Vec2::new(w - 12.0, 50.0));
            let resp = ui.interact(row, ui.id().with(("menu-account", i)), Sense::click());
            if resp.hovered() {
                p.rect_filled(row, CornerRadius::same(6), RAISED);
            }
            ui::draw_head(
                &p,
                Rect::from_min_size(row.min + Vec2::new(10.0, 9.0), Vec2::splat(32.0)),
                self.shown_skin(),
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 54.0, row.top() + 18.0),
                Align2::LEFT_CENTER,
                &account.name,
                ui::semi(14.5),
                c::TEXT,
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 54.0, row.top() + 36.0),
                Align2::LEFT_CENTER,
                kind_label(account),
                ui::regular(12.0),
                c::MUTED,
            );
            if Some(&account.id) == current.as_ref() {
                self.icons.draw(
                    &p,
                    "check-lg",
                    Pos2::new(row.right() - 22.0, row.center().y),
                    18.0,
                    Color32::WHITE,
                );
            }
            if resp.clicked() {
                picked = Some(account.id.clone());
            }
            y += 54.0;
        }
        y += 4.0;
        p.line_segment(
            [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
            Stroke::new(1.0_f32, c::BORDER),
        );
        y += 8.0;
        let mut go = None;
        for (i, (icon, label)) in [
            ("people-fill", "Gérer les comptes"),
            ("person-plus", "Ajouter un compte"),
        ]
        .into_iter()
        .enumerate()
        {
            let row = Rect::from_min_size(Pos2::new(r.left() + 6.0, y), Vec2::new(w - 12.0, 44.0));
            let resp = ui.interact(row, ui.id().with(("menu-action", i)), Sense::click());
            if resp.hovered() {
                p.rect_filled(row, CornerRadius::same(6), RAISED);
            }
            self.icons.draw(
                &p,
                icon,
                Pos2::new(row.left() + 24.0, row.center().y),
                18.0,
                c::TEXT,
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 50.0, row.center().y),
                Align2::LEFT_CENTER,
                label,
                ui::medium(14.0),
                c::TEXT,
            );
            if resp.clicked() {
                go = Some(i);
            }
            y += 46.0;
        }
        if let Some(id) = picked {
            self.update_state(|s| s.account = Some(id));
            self.menu_open = false;
        }
        if go.is_some() {
            self.page = Page::Accounts;
            self.menu_open = false;
        }
    }

    fn accounts_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(&p, body, "Ajouter un compte");
        let left_w = (body.width() * 0.46).min(520.0);

        // The kinds of account: Microsoft (the real one) or a local name.
        let tile = 150.0;
        let has_microsoft = microsoft::client_id().is_some();
        let kinds = [
            ("windows", "Compte Microsoft", Kind::Microsoft),
            ("person-fill", "Compte local", Kind::Local),
        ];
        for (i, (icon, label, kind)) in kinds.into_iter().enumerate() {
            let t = Rect::from_min_size(
                Pos2::new(body.left() + i as f32 * (tile + 18.0), top + 8.0),
                Vec2::splat(tile),
            );
            let resp = ui.interact(t, ui.id().with(("kind", i)), Sense::click());
            let on = self.add_kind == kind;
            surface(&p, t, if resp.hovered() { HOVER } else { RAISED });
            self.icons.draw(
                &p,
                icon,
                t.center(),
                54.0,
                if on { Color32::WHITE } else { c::MUTED },
            );
            if on {
                p.rect_stroke(
                    t,
                    CornerRadius::same(R),
                    Stroke::new(2.0_f32, c::ACCENT),
                    StrokeKind::Inside,
                );
                let badge =
                    Rect::from_min_size(t.right_top() + Vec2::new(-34.0, 10.0), Vec2::splat(24.0));
                p.rect_filled(badge, CornerRadius::same(5), c::ACCENT);
                self.icons
                    .draw(&p, "check-lg", badge.center(), 15.0, Color32::WHITE);
            }
            ui::text(
                &p,
                Pos2::new(t.left(), t.bottom() + 10.0),
                Align2::LEFT_TOP,
                label,
                ui::semi(14.5),
                if on { c::TEXT } else { c::MUTED },
            );
            if kind == Kind::Microsoft && !has_microsoft {
                ui::text(
                    &p,
                    Pos2::new(t.left(), t.bottom() + 30.0),
                    Align2::LEFT_TOP,
                    "Indisponible ici",
                    ui::regular(12.5),
                    c::FAINT,
                );
            }
            if resp.clicked() {
                self.add_kind = kind;
            }
        }

        // The accounts already there.
        let list_top = top + 8.0 + tile + 78.0;
        caption(&p, Pos2::new(body.left() + 4.0, list_top), "COMPTES");
        let accounts = self.state.all_accounts();
        let current = self.current.as_ref().map(|a| a.id.clone());
        let mut picked = None;
        let mut removed: Option<AccountRef> = None;
        for (i, account) in accounts.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(body.left(), list_top + 26.0 + i as f32 * 58.0),
                Vec2::new(left_w, 52.0),
            );
            if row.bottom() > body.bottom() {
                break;
            }
            let resp = ui.interact(row, ui.id().with(("account-row", i)), Sense::click());
            surface(&p, row, if resp.hovered() { HOVER } else { PANEL });
            ui::draw_head(
                &p,
                Rect::from_min_size(row.min + Vec2::new(12.0, 10.0), Vec2::splat(32.0)),
                self.shown_skin(),
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 58.0, row.top() + 19.0),
                Align2::LEFT_CENTER,
                &account.name,
                ui::semi(14.5),
                c::TEXT,
            );
            ui::text(
                &p,
                Pos2::new(row.left() + 58.0, row.top() + 37.0),
                Align2::LEFT_CENTER,
                kind_label(account),
                ui::regular(12.0),
                c::MUTED,
            );
            if accounts.len() > 1 {
                let bin = Rect::from_center_size(
                    Pos2::new(row.right() - 24.0, row.center().y),
                    Vec2::splat(32.0),
                );
                let b = ui.interact(bin, ui.id().with(("account-remove", i)), Sense::click());
                self.icons.draw(
                    &p,
                    "trash3",
                    bin.center(),
                    16.0,
                    if b.hovered() { c::BAD_HOVER } else { c::FAINT },
                );
                if b.clicked() {
                    removed = Some(account.clone());
                }
            }
            if Some(&account.id) == current.as_ref() {
                let x = row.right() - if accounts.len() > 1 { 62.0 } else { 26.0 };
                self.icons.draw(
                    &p,
                    "check-lg",
                    Pos2::new(x, row.center().y),
                    18.0,
                    Color32::WHITE,
                );
            }
            if resp.clicked() && removed.is_none() {
                picked = Some(account.id.clone());
            }
        }
        if let Some(account) = removed {
            if account.microsoft {
                microsoft::forget(account.id.trim_start_matches("ms:"));
            }
            self.update_state(|s| s.remove_account(&account.id));
        } else if let Some(id) = picked {
            self.update_state(|s| s.account = Some(id));
        }

        // The form on the right.
        let divider_x = body.left() + left_w + 28.0;
        p.line_segment(
            [
                Pos2::new(divider_x, body.top() + 8.0),
                Pos2::new(divider_x, body.bottom()),
            ],
            Stroke::new(1.0_f32, c::BORDER),
        );
        let fx = divider_x + 36.0;
        let fw = body.right() - fx;
        match self.add_kind {
            Kind::Local => self.local_form(ui, &p, body, fx, fw),
            Kind::Microsoft => self.microsoft_form(ui, &p, body, fx, fw),
        }
    }

    /// The form for a local account: a name.
    fn local_form(&mut self, ui: &mut egui::Ui, p: &egui::Painter, body: Rect, fx: f32, fw: f32) {
        caption(p, Pos2::new(fx, body.top() + 14.0), "COMPTE LOCAL");
        let row = Rect::from_min_size(Pos2::new(fx, body.top() + 40.0), Vec2::new(fw, 66.0));
        surface(p, row, PANEL);
        self.icons.draw(
            p,
            "shuffle",
            Pos2::new(row.left() + 26.0, row.center().y),
            20.0,
            c::TEXT,
        );
        ui::text(
            p,
            Pos2::new(row.left() + 54.0, row.top() + 22.0),
            Align2::LEFT_CENTER,
            "Nom aléatoire",
            ui::semi(14.5),
            c::TEXT,
        );
        ui::text(
            p,
            Pos2::new(row.left() + 54.0, row.top() + 43.0),
            Align2::LEFT_CENTER,
            "Choisis un nom pour ton compte local.",
            ui::regular(12.5),
            c::MUTED,
        );
        let gen = Rect::from_min_size(
            Pos2::new(row.right() - 118.0, row.center().y - 19.0),
            Vec2::new(104.0, 38.0),
        );
        if button(
            ui,
            &self.icons,
            gen,
            Some("shuffle"),
            "Générer",
            "generate",
            Look::Quiet,
            true,
        ) {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64);
            self.account_input = player::generate(nanos / 1000);
            self.account_error = None;
        }
        caption(p, Pos2::new(fx, body.top() + 134.0), "NOM D'UTILISATEUR");
        let field = Rect::from_min_size(Pos2::new(fx, body.top() + 160.0), Vec2::new(fw, 48.0));
        surface(p, field, PANEL);
        let inner = Rect::from_center_size(field.center(), Vec2::new(field.width() - 28.0, 26.0));
        let edit = ui.put(
            inner,
            egui::TextEdit::singleline(&mut self.account_input)
                .frame(false)
                .margin(Margin::ZERO)
                .font(ui::regular(14.5))
                .char_limit(16)
                .hint_text("Entre le nom d'utilisateur")
                .desired_width(inner.width()),
        );
        if edit.changed() {
            self.account_error = None;
        }
        if let Some(e) = &self.account_error {
            ui::text(
                p,
                Pos2::new(fx, field.bottom() + 12.0),
                Align2::LEFT_TOP,
                e,
                ui::medium(13.0),
                c::BAD_HOVER,
            );
        }
        let ok = player::is_valid(self.account_input.trim());
        let add = Rect::from_min_size(Pos2::new(fx, body.bottom() - 52.0), Vec2::new(fw, 52.0));
        if button(
            ui,
            &self.icons,
            add,
            Some("person-plus"),
            "Ajouter un compte local",
            "add-account",
            Look::Accent,
            ok,
        ) {
            let name = self.account_input.trim().to_owned();
            let mut result = Ok(());
            self.update_state(|s| result = s.add_account(&name));
            match result {
                Ok(()) => {
                    self.account_input.clear();
                    self.log(&format!("compte local ajouté : {name}"));
                }
                Err(oteria_dist::AccountError::Exists) => {
                    self.account_error = Some("Ce compte existe déjà.".into());
                }
                Err(oteria_dist::AccountError::Invalid) => {
                    self.account_error = Some("1 à 16 lettres, chiffres ou _.".into());
                }
            }
        }
    }

    /// The form for a Microsoft account: a code to type at microsoft.com/link.
    fn microsoft_form(
        &mut self,
        ui: &mut egui::Ui,
        p: &egui::Painter,
        body: Rect,
        fx: f32,
        fw: f32,
    ) {
        caption(p, Pos2::new(fx, body.top() + 14.0), "COMPTE MICROSOFT");
        let has_id = microsoft::client_id().is_some();
        let row = Rect::from_min_size(Pos2::new(fx, body.top() + 40.0), Vec2::new(fw, 66.0));
        surface(p, row, PANEL);
        self.icons.draw(
            p,
            "windows",
            Pos2::new(row.left() + 26.0, row.center().y),
            20.0,
            c::TEXT,
        );
        ui::text(
            p,
            Pos2::new(row.left() + 54.0, row.top() + 22.0),
            Align2::LEFT_CENTER,
            "Connexion Microsoft",
            ui::semi(14.5),
            c::TEXT,
        );
        let line = if has_id {
            "Joue en ligne avec ton skin et rejoins les serveurs."
        } else {
            "Cette version du launcher n'a pas de connexion Microsoft."
        };
        ui::text(
            p,
            Pos2::new(row.left() + 54.0, row.top() + 43.0),
            Align2::LEFT_CENTER,
            line,
            ui::regular(12.5),
            c::MUTED,
        );
        let waiting = matches!(self.ms_flow, MsFlow::Starting | MsFlow::Waiting { .. });
        let go = Rect::from_min_size(
            Pos2::new(row.right() - 168.0, row.center().y - 19.0),
            Vec2::new(154.0, 38.0),
        );
        if button(
            ui,
            &self.icons,
            go,
            None,
            if waiting { "…" } else { "Se connecter" },
            "ms-sign-in",
            Look::Accent,
            has_id && !waiting,
        ) {
            self.start_sign_in();
        }

        let y = row.bottom() + 24.0;
        match &self.ms_flow {
            MsFlow::Idle => {
                ui::wrapped(
                    p,
                    Pos2::new(fx, y),
                    "Une page Microsoft s'ouvre : tu y tapes un code, ton mot de passe ne passe jamais par Oteria. Le compte doit posséder Minecraft: Java Edition.",
                    ui::regular(13.5),
                    c::MUTED,
                    fw,
                    4,
                );
            }
            MsFlow::Starting => {
                ui::text(
                    p,
                    Pos2::new(fx, y),
                    Align2::LEFT_TOP,
                    "Demande d'un code à Microsoft…",
                    ui::regular(13.5),
                    c::MUTED,
                );
            }
            MsFlow::Waiting {
                code,
                uri,
                expires_at,
            } => {
                let (code, uri, left) = (
                    code.clone(),
                    uri.clone(),
                    expires_at.saturating_sub(microsoft::now()),
                );
                let card = Rect::from_min_size(Pos2::new(fx, y), Vec2::new(fw, 196.0));
                surface(p, card, PANEL);
                ui::text(
                    p,
                    Pos2::new(card.center().x, card.top() + 28.0),
                    Align2::CENTER_CENTER,
                    "Fenêtre privée : tape ce code, puis connecte-toi avec ton compte Minecraft",
                    ui::regular(13.5),
                    c::MUTED,
                );
                let width = p
                    .layout_no_wrap(code.clone(), ui::semi(40.0), Color32::WHITE)
                    .size()
                    .x
                    + 4.0 * (code.len() as f32 - 1.0);
                ui::spaced(
                    p,
                    Pos2::new(card.center().x - width / 2.0, card.top() + 54.0),
                    &code,
                    ui::semi(40.0),
                    Color32::WHITE,
                    4.0,
                );
                let b1 = Rect::from_min_size(
                    Pos2::new(card.left() + 24.0, card.bottom() - 70.0),
                    Vec2::new((card.width() - 60.0) / 2.0, 42.0),
                );
                let b2 = Rect::from_min_size(Pos2::new(b1.right() + 12.0, b1.top()), b1.size());
                if button(
                    ui,
                    &self.icons,
                    b1,
                    Some("box-arrow-up-right"),
                    "Ouvrir la page",
                    "ms-open",
                    Look::Quiet,
                    true,
                ) {
                    Self::open_sign_in_page(&uri);
                }
                if button(
                    ui,
                    &self.icons,
                    b2,
                    Some("clipboard"),
                    "Copier le code",
                    "ms-copy",
                    Look::Quiet,
                    true,
                ) {
                    ui.ctx().copy_text(code.clone());
                }
                ui::text(
                    p,
                    Pos2::new(card.center().x, card.bottom() - 16.0),
                    Align2::CENTER_CENTER,
                    &format!(
                        "En attente de ta connexion… ({} min {:02} s)",
                        left / 60,
                        left % 60
                    ),
                    ui::regular(12.5),
                    c::FAINT,
                );
                let cancel = Rect::from_min_size(
                    Pos2::new(fx, card.bottom() + 16.0),
                    Vec2::new(150.0, 40.0),
                );
                if button(
                    ui,
                    &self.icons,
                    cancel,
                    None,
                    "Annuler",
                    "ms-cancel",
                    Look::Quiet,
                    true,
                ) {
                    self.cancel_sign_in();
                }
            }
            MsFlow::Done(name) => {
                ui::wrapped(
                    p,
                    Pos2::new(fx, y),
                    &format!("Connecté : {name}. Ce compte est maintenant celui du jeu."),
                    ui::medium(14.0),
                    c::GOOD,
                    fw,
                    3,
                );
            }
            MsFlow::Failed(e) => {
                ui::wrapped(
                    p,
                    Pos2::new(fx, y),
                    e,
                    ui::medium(14.0),
                    c::BAD_HOVER,
                    fw,
                    4,
                );
            }
        }
        let _ = body;
    }

    // ----- Settings ---------------------------------------------------------------------------

    fn settings_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(&p, body, "Paramètres");
        let area = Rect::from_min_max(Pos2::new(body.left(), top), body.max);
        let game_ram = self.child.as_ref().and_then(|c| memory::of_process(c.id()));
        let busy = matches!(self.phase, Phase::Checking | Phase::Downloading);
        let close_on_launch = self.state.close_on_launch;
        let mut actions: Vec<&'static str> = Vec::new();
        let resources = self.state.resources_dir(&self.home);
        let source = self.state.source().unwrap_or_default();
        let client = self
            .state
            .client_version
            .clone()
            .map_or("aucun".into(), |v| format!("Oteria {v}"));

        egui::ScrollArea::vertical()
            .max_height(area.height())
            .show_viewport(&mut ui.new_child(egui::UiBuilder::new().max_rect(area)), |ui, _| {
                let width = area.width() - 16.0;
                let row_h = 72.0;
                let sections: [(&str, usize); 4] = [("GÉNÉRAL", 2), ("INSTALLATION", 3), ("MÉMOIRE", 2), ("MISES À JOUR", 2)];
                let total: f32 = sections.iter().map(|(_, n)| 34.0 + *n as f32 * (row_h + 10.0) + 14.0).sum();
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, total), Sense::hover());
                let p = ui.painter().clone();
                let mut y = rect.top();
                let row = |y: &mut f32, icon: &str, title: &str, desc: &str| -> Rect {
                    let r = Rect::from_min_size(Pos2::new(rect.left(), *y), Vec2::new(rect.width(), row_h));
                    surface(&p, r, PANEL);
                    self.icons.draw(&p, icon, Pos2::new(r.left() + 36.0, r.center().y), 22.0, c::MUTED);
                    ui::text(&p, Pos2::new(r.left() + 70.0, r.top() + 24.0), Align2::LEFT_CENTER, title, ui::semi(14.5), c::TEXT);
                    ui::wrapped(&p, Pos2::new(r.left() + 70.0, r.top() + 42.0), desc, ui::regular(12.5), c::MUTED, r.width() - 260.0, 1);
                    *y += row_h + 10.0;
                    r
                };
                caption(&p, Pos2::new(rect.left() + 2.0, y), "GÉNÉRAL");
                y += 28.0;
                let r = row(&mut y, "controller", "Fermer le launcher au lancement du jeu", "Le launcher se ferme quand le jeu démarre. Rouvre-le avec le raccourci Oteria.");
                if toggle(ui, Rect::from_center_size(Pos2::new(r.right() - 50.0, r.center().y), Vec2::new(46.0, 24.0)), close_on_launch, "close-on-launch") {
                    actions.push("toggle-close");
                }
                let r = row(&mut y, "people-fill", "Compte", &match &self.current {
                    Some(a) if a.microsoft => format!("{} · compte Microsoft", a.name),
                    Some(a) => format!("{} · compte local", a.name),
                    None => "Aucun compte : ajoutes-en un pour jouer".to_owned(),
                });
                if button(ui, &self.icons, Rect::from_center_size(Pos2::new(r.right() - 82.0, r.center().y), Vec2::new(130.0, 36.0)), None, "Gérer", "manage-accounts", Look::Quiet, true) {
                    actions.push("accounts");
                }
                y += 14.0;

                caption(&p, Pos2::new(rect.left() + 2.0, y), "INSTALLATION");
                y += 28.0;
                for (icon, title, desc, id) in [
                    ("hdd", "Dossier du launcher", self.home.display().to_string(), "open-home"),
                    ("folder2-open", "Dossier du jeu", self.game_dir.display().to_string(), "open-game"),
                    ("box-seam", "Ressources du jeu", resources.display().to_string(), "open-res"),
                ] {
                    let r = row(&mut y, icon, title, &desc);
                    if button(ui, &self.icons, Rect::from_center_size(Pos2::new(r.right() - 82.0, r.center().y), Vec2::new(130.0, 36.0)), None, "Ouvrir", id, Look::Quiet, true) {
                        actions.push(id);
                    }
                }
                y += 14.0;

                caption(&p, Pos2::new(rect.left() + 2.0, y), "MÉMOIRE");
                y += 28.0;
                let r = row(&mut y, "cpu", "Mémoire du PC", "Oteria est écrit en Rust : il ne réserve rien à l'avance, il prend ce qu'il lui faut.");
                ui::text(&p, Pos2::new(r.right() - 26.0, r.center().y), Align2::RIGHT_CENTER, &self.total_ram.map_or("inconnue".into(), gib), ui::semi(14.5), c::TEXT);
                let r = row(&mut y, "cpu", "Mémoire utilisée par le jeu", "Ce que le jeu occupe en ce moment.");
                ui::text(&p, Pos2::new(r.right() - 26.0, r.center().y), Align2::RIGHT_CENTER, &game_ram.map_or("le jeu n'est pas lancé".into(), gib), ui::semi(14.5), c::TEXT);
                y += 14.0;

                caption(&p, Pos2::new(rect.left() + 2.0, y), "MISES À JOUR");
                y += 28.0;
                let r = row(&mut y, "arrow-repeat", &format!("Launcher v{}", oteria_dist::LAUNCHER_VERSION), &format!("Source : {source}"));
                if button(ui, &self.icons, Rect::from_center_size(Pos2::new(r.right() - 105.0, r.center().y), Vec2::new(176.0, 36.0)), None, if busy { "…" } else { "Rechercher" }, "check", Look::Quiet, !busy) {
                    actions.push("check");
                }
                row(&mut y, "download", &client, &format!("Minecraft {}", oteria_dist::MINECRAFT_VERSION));
            });
        for a in actions {
            match a {
                "open-home" => open_folder(&self.home),
                "open-game" => open_folder(&self.game_dir),
                "open-res" => open_folder(&resources),
                "accounts" => self.page = Page::Accounts,
                "check" => {
                    self.notice = None;
                    self.start_check();
                }
                "toggle-close" => self.update_state(|s| s.close_on_launch = !close_on_launch),
                _ => {}
            }
        }
    }

    // ----- The installation and update cards ---------------------------------------------------

    fn modal(&mut self, ui: &mut egui::Ui, full: Rect) {
        if let Phase::SelfUpdating { version, since, .. } = &self.phase {
            let k = (since.elapsed().as_secs_f32() / 1.4).clamp(0.0, 1.0);
            let sub = format!("v{} → v{version}", oteria_dist::LAUNCHER_VERSION);
            self.progress_card(
                ui,
                full,
                Card {
                    subtitle: "MISE À JOUR",
                    tag: "LAUNCHER",
                    icon: "arrow-repeat",
                    title: "Mise à jour".into(),
                    sub: "Le launcher se met à jour puis redémarre tout seul.".into(),
                    percent: k,
                    left: sub,
                    right: String::new(),
                    footer: "Rien à faire : la fenêtre se rouvre dans un instant.",
                },
            );
            ui.ctx().request_repaint();
            return;
        }
        let Some(d) = &self.download else { return };
        let (title, doing) = match d.stage.as_str() {
            "download" => ("Téléchargement", "Téléchargement du client Oteria…"),
            "extract" => ("Installation", "Installation du client…"),
            "minecraft" => ("Minecraft", "Téléchargement de Minecraft chez Mojang…"),
            "java" => ("Java", "Téléchargement de Java chez Mojang…"),
            _ => ("Préparation", "Préparation des ressources du jeu…"),
        };
        let sub = "Cela ne prend que quelques minutes.";
        let counted = matches!(d.stage.as_str(), "download" | "extract") && d.total > 0;
        let right = if counted {
            format!("{} / {} Mo", mib(d.done), mib(d.total))
        } else {
            String::new()
        };
        let left = if d.note.is_empty() {
            doing.to_owned()
        } else {
            d.note.clone()
        };
        let card = Card {
            subtitle: "INSTALLATION DU JEU",
            tag: if d.update { "MISE À JOUR" } else { "PREMIÈRE INSTALLATION" },
            icon: "download",
            title: title.to_owned(),
            sub: sub.to_owned(),
            percent: d.overall(),
            left,
            right,
            footer: "Cette installation n'a lieu qu'une fois. Les prochains lancements seront instantanés.",
        };
        self.progress_card(ui, full, card);
    }

    fn progress_card(&self, ui: &mut egui::Ui, full: Rect, card: Card) {
        let p = ui.painter().clone();
        // Dim the page and keep clicks from reaching it.
        ui.interact(full, ui.id().with("modal-eater"), Sense::click_and_drag());
        p.rect_filled(full, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 165));
        let r = Rect::from_center_size(full.center(), Vec2::new(540.0, 300.0));
        p.rect(
            r,
            CornerRadius::same(10),
            PANEL,
            Stroke::new(1.0_f32, c::BORDER),
            StrokeKind::Inside,
        );

        // The header: the logo, the name, and what this is.
        p.image(
            self.logo.id(),
            Rect::from_min_size(r.min + Vec2::new(22.0, 16.0), Vec2::splat(34.0)),
            FULL_UV,
            Color32::WHITE,
        );
        ui::text(
            &p,
            r.min + Vec2::new(68.0, 19.0),
            Align2::LEFT_TOP,
            "Oteria",
            ui::semi(16.0),
            c::TEXT,
        );
        ui::spaced(
            &p,
            r.min + Vec2::new(68.0, 40.0),
            card.subtitle,
            ui::semi(10.0),
            c::FAINT,
            1.0,
        );
        let g = p.layout_no_wrap(card.tag.into(), ui::semi(10.0), c::MUTED);
        let tag = Rect::from_min_size(
            Pos2::new(r.right() - 22.0 - g.size().x - 24.0, r.top() + 23.0),
            g.size() + Vec2::new(24.0, 12.0),
        );
        p.rect_filled(tag, CornerRadius::same(4), RAISED);
        p.galley(tag.min + Vec2::new(12.0, 6.0), g, c::MUTED);
        p.line_segment(
            [
                Pos2::new(r.left(), r.top() + 68.0),
                Pos2::new(r.right(), r.top() + 68.0),
            ],
            Stroke::new(1.0_f32, c::BORDER),
        );

        // The body.
        let icon_tile = Rect::from_min_size(r.min + Vec2::new(26.0, 92.0), Vec2::splat(52.0));
        surface(&p, icon_tile, RAISED);
        self.icons
            .draw(&p, card.icon, icon_tile.center(), 22.0, c::TEXT);
        ui::text(
            &p,
            r.min + Vec2::new(98.0, 92.0),
            Align2::LEFT_TOP,
            &card.title,
            ui::semi(22.0),
            Color32::WHITE,
        );
        ui::text(
            &p,
            r.min + Vec2::new(98.0, 124.0),
            Align2::LEFT_TOP,
            &card.sub,
            ui::regular(13.5),
            c::MUTED,
        );
        ui::text(
            &p,
            Pos2::new(r.right() - 26.0, r.top() + 96.0),
            Align2::RIGHT_TOP,
            &format!("{:.0}%", card.percent * 100.0),
            ui::semi(16.0),
            Color32::WHITE,
        );
        ui::wrapped(
            &p,
            Pos2::new(r.left() + 26.0, r.top() + 168.0),
            &card.left,
            ui::semi(12.5),
            c::TEXT,
            r.width() - 190.0,
            1,
        );
        ui::text(
            &p,
            Pos2::new(r.right() - 26.0, r.top() + 168.0),
            Align2::RIGHT_TOP,
            &card.right,
            ui::regular(12.5),
            c::MUTED,
        );
        let bar = Rect::from_min_size(
            Pos2::new(r.left() + 26.0, r.top() + 194.0),
            Vec2::new(r.width() - 52.0, 6.0),
        );
        p.rect_filled(bar, CornerRadius::same(3), RAISED);
        p.rect_filled(
            Rect::from_min_size(
                bar.min,
                Vec2::new(bar.width() * card.percent.clamp(0.0, 1.0), 6.0),
            ),
            CornerRadius::same(3),
            c::ACCENT_HOVER,
        );

        // The footer.
        p.line_segment(
            [
                Pos2::new(r.left() + 26.0, r.top() + 232.0),
                Pos2::new(r.right() - 26.0, r.top() + 232.0),
            ],
            Stroke::new(1.0_f32, c::BORDER),
        );
        self.icons.draw(
            &p,
            "box-seam",
            Pos2::new(r.left() + 38.0, r.top() + 262.0),
            16.0,
            c::MUTED,
        );
        ui::wrapped(
            &p,
            Pos2::new(r.left() + 58.0, r.top() + 247.0),
            card.footer,
            ui::regular(12.5),
            c::MUTED,
            r.width() - 90.0,
            2,
        );
    }
}

/// What the installation and update card says.
struct Card {
    subtitle: &'static str,
    tag: &'static str,
    icon: &'static str,
    title: String,
    sub: String,
    percent: f32,
    left: String,
    right: String,
    footer: &'static str,
}
