//! `OteriaLauncher.exe`: Oteria Client's launcher.
//!
//! On every start it asks `updater.exe check` what is new. A newer launcher is installed at once
//! (`updater.exe self-update`, which restarts it); a newer client makes the main button
//! « Télécharger la mise à jour » before it can say « Jouer ». The game is started directly from
//! the installed client, with the game folder, the offline name and the prepared resources.
//!
//! Testing (`tools/oteria/dist_test.ps1`): `OTERIA_HOME`, `OTERIA_GAME_DIR` and
//! `OTERIA_UPDATE_SOURCE` move everything to scratch folders; `OTERIA_LAUNCHER_PAGE` opens a page;
//! `OTERIA_LAUNCHER_AUTO=download,play` presses the main button when it can; `OTERIA_LAUNCHER_SHOT`
//! saves a screenshot once the window has settled (`OTERIA_LAUNCHER_SHOT_AT=downloading` takes it
//! mid-download) and closes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2,
};
use oteria_dist::ui::{self, c};
use oteria_dist::{install, paths, player, CheckReport, NewsItem, PartnerServer, Progress, State};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Oteria Client")
        .with_inner_size([1200.0, 760.0])
        .with_min_inner_size([1000.0, 640.0])
        .with_decorations(false)
        .with_icon(ui::icon_data());
    eframe::run_native(
        "Oteria Client",
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
    News,
    Settings,
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
    hero_tex: TextureHandle,
    page: Page,
    phase: Phase,
    report: CheckReport,
    check_rx: Option<Receiver<CheckReport>>,
    download: Option<Download>,
    child: Option<Child>,
    /// A message under the main button: (text, colour).
    notice: Option<(String, Color32)>,
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
        let logo = ui::texture_from_png(ctx, "logo", ui::LOGO_PNG).expect("logo");
        let page = match std::env::var("OTERIA_LAUNCHER_PAGE").as_deref() {
            Ok("mods") => Page::Mods,
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
        let mut me = Self {
            hero_tex: hero_texture(ctx),
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

    fn poll_download(&mut self) {
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
                self.notice = Some((format!("Oteria {v} est installé. Bon jeu !"), c::GOOD));
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

    /// Starts the game: the installed client, right away.
    fn launch(&mut self, server: Option<&str>) {
        let clicked = Instant::now();
        let Some(dir) = self.state.client_dir(&self.home) else {
            self.phase = Phase::NeedDownload { update: false };
            return;
        };
        let _ = std::fs::create_dir_all(self.game_dir.join("home"));
        let mut cmd = Command::new(dir.join(paths::CLIENT_EXE));
        cmd.current_dir(&dir)
            .arg("--game-dir")
            .arg(&self.game_dir)
            .arg("--name")
            .arg(&self.name)
            .arg("--resources")
            .arg(self.state.resources_dir(&self.home))
            .env("HORIZON_HOME", self.game_dir.join("home"));
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

    fn main_action(&mut self) {
        match self.phase {
            Phase::NeedDownload { .. } => self.start_download(),
            Phase::Ready => self.launch(None),
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
            "running" => matches!(self.phase, Phase::Launching { .. } | Phase::Running),
            _ => {
                !matches!(self.phase, Phase::Checking | Phase::Downloading) && self.auto.is_empty()
            }
        }
    }
}

/// The hero banner's gradient, as a texture so the rounded rectangle can carry it.
fn hero_texture(ctx: &egui::Context) -> TextureHandle {
    let (w, h) = (256usize, 128usize);
    let mut px = Vec::with_capacity(w * h);
    let a = Color32::from_rgb(0x0a, 0x2a, 0x45);
    let b = Color32::from_rgb(0x0b, 0x13, 0x22);
    let glow = Color32::from_rgb(0x15, 0x6f, 0xa6);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let base = ui::lerp(a, b, (u * 0.7 + v * 0.5).min(1.0));
            // A light from the right, behind the player.
            let d = ((u - 0.78).powi(2) * 2.2 + (v - 0.45).powi(2)).sqrt();
            let k = (1.0 - d / 0.55).clamp(0.0, 1.0).powf(1.8) * 0.75;
            px.push(ui::lerp(base, glow, k));
        }
    }
    ctx.load_texture(
        "hero",
        egui::ColorImage::new([w, h], px),
        egui::TextureOptions::LINEAR,
    )
}

const MODS: &[(&str, &str, &str)] = &[
    (
        "Crystal Optimizer",
        "Les cristaux posés et cassés sans attendre le serveur.",
        "PvP",
    ),
    (
        "Anchor Optimizer",
        "Ancres chargées et déclenchées plus vite.",
        "PvP",
    ),
    (
        "Hitboxes",
        "Une couleur par type d'entité, ligne des yeux et regard.",
        "PvP",
    ),
    (
        "Pop Counter",
        "Annonce dans le chat chaque totem pop.",
        "PvP",
    ),
    (
        "Tier Tagger",
        "Le tier PvP des joueurs à côté de leur nom.",
        "PvP",
    ),
    (
        "Cooldowns",
        "Perle, chorus, wind charge : le temps qu'il reste.",
        "HUD",
    ),
    (
        "Compteur de totems",
        "Les totems portés et ceux de l'adversaire.",
        "HUD",
    ),
    (
        "Compteur de potions",
        "Potions de soin et temps depuis la dernière.",
        "HUD",
    ),
    ("Zoom", "Un zoom fluide, réglable à la molette.", "Confort"),
    (
        "Mouse Tweaks",
        "Glisse la souris pour ranger l'inventaire.",
        "Confort",
    ),
    ("Motion Blur", "Un flou de mouvement réglable.", "Visuel"),
    (
        "Super Secret Settings",
        "21 effets d'écran, comme en 1.8.",
        "Visuel",
    ),
    (
        "Couteaux",
        "Couteaux riggés et animations d'inspect.",
        "Cosmétique",
    ),
    (
        "Capes et ailes",
        "Capes en tissu, ailes et auras.",
        "Cosmétique",
    ),
    ("Discord", "Ton serveur sur ton profil Discord.", "Social"),
];

fn tag_colour(tag: &str) -> Color32 {
    match tag.to_lowercase().as_str() {
        t if t.contains("mise") || t.contains("update") => c::ACCENT,
        t if t.contains("pvp") || t.contains("event") || t.contains("évén") => {
            Color32::from_rgb(0xf2, 0x6d, 0x5b)
        }
        t if t.contains("cosm") || t.contains("visuel") => Color32::from_rgb(0xb3, 0x8b, 0xff),
        t if t.contains("hud") => Color32::from_rgb(0x3d, 0xd6, 0x8c),
        t if t.contains("confort") || t.contains("social") => c::WARN,
        _ => c::ACCENT_HI,
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
            // The self-update carries on after its picture; so does a run that still has to play.
            let carry_on = matches!(self.phase, Phase::SelfUpdating { .. })
                || (self.auto.iter().any(|a| a == "play") && self.shot_at != "running");
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
                self.sidebar(
                    ui,
                    Rect::from_min_max(full.min, Pos2::new(full.left() + 84.0, full.bottom())),
                );
                let top = Rect::from_min_max(
                    Pos2::new(full.left() + 84.0, full.top()),
                    Pos2::new(full.right(), full.top() + 56.0),
                );
                self.top_bar(ui, top);
                let body = Rect::from_min_max(Pos2::new(top.left(), top.bottom()), full.max);
                match self.page {
                    Page::Play => self.play_page(ui, body),
                    Page::Mods => self.mods_page(ui, body),
                    Page::News => self.news_page(ui, body),
                    Page::Settings => self.settings_page(ui, body),
                }
                if let Phase::SelfUpdating { version, since, .. } = &self.phase {
                    self_update_overlay(
                        ui,
                        full,
                        version,
                        since.elapsed().as_secs_f32(),
                        &self.logo,
                    );
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
            Duration::from_millis(250)
        });
    }
}

// ----- Painting --------------------------------------------------------------------------------

fn nav_icon(p: &egui::Painter, kind: Page, cn: Pos2, col: Color32) {
    let st = Stroke::new(1.8_f32, col);
    match kind {
        Page::Play => {
            let pts = vec![
                cn + Vec2::new(-6.0, -8.0),
                cn + Vec2::new(8.0, 0.0),
                cn + Vec2::new(-6.0, 8.0),
            ];
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        }
        Page::Mods => {
            for (dx, dy) in [(-5.0, -5.0), (5.0, -5.0), (-5.0, 5.0), (5.0, 5.0)] {
                p.rect_stroke(
                    Rect::from_center_size(cn + Vec2::new(dx, dy), Vec2::splat(7.5)),
                    2.0,
                    st,
                    StrokeKind::Middle,
                );
            }
        }
        Page::News => {
            p.rect_stroke(
                Rect::from_center_size(cn, Vec2::new(18.0, 16.0)),
                3.0,
                st,
                StrokeKind::Middle,
            );
            for dy in [-3.0, 1.0, 5.0] {
                p.line_segment(
                    [cn + Vec2::new(-5.0, dy), cn + Vec2::new(5.0, dy)],
                    Stroke::new(1.5_f32, col),
                );
            }
        }
        Page::Settings => {
            p.circle_stroke(cn, 4.0, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([cn + d * 7.0, cn + d * 10.0], Stroke::new(2.4_f32, col));
            }
            p.circle_stroke(cn, 7.5, Stroke::new(1.6_f32, col));
        }
    }
}

fn self_update_overlay(ui: &mut egui::Ui, full: Rect, version: &str, t: f32, logo: &TextureHandle) {
    let p = ui.painter();
    p.rect_filled(full, 0.0, ui::alpha(c::BG, 0.94));
    let cn = full.center();
    ui::glow(p, cn - Vec2::new(0.0, 40.0), 160.0, c::ACCENT_DEEP, 1.0);
    p.image(
        logo.id(),
        Rect::from_center_size(cn - Vec2::new(0.0, 50.0), Vec2::splat(96.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    ui::text(
        p,
        cn + Vec2::new(0.0, 30.0),
        Align2::CENTER_CENTER,
        "Mise à jour du launcher",
        ui::display(26.0),
        c::TEXT,
    );
    ui::text(
        p,
        cn + Vec2::new(0.0, 62.0),
        Align2::CENTER_CENTER,
        &format!(
            "v{} → v{version} · redémarrage automatique",
            oteria_dist::LAUNCHER_VERSION
        ),
        ui::regular(14.0),
        c::MUTED,
    );
    let bar = Rect::from_center_size(cn + Vec2::new(0.0, 96.0), Vec2::new(260.0, 4.0));
    p.rect_filled(bar, CornerRadius::same(2), c::CARD);
    let k = (t / 1.4).clamp(0.0, 1.0);
    p.rect_filled(
        Rect::from_min_size(bar.min, Vec2::new(bar.width() * k, 4.0)),
        CornerRadius::same(2),
        c::ACCENT,
    );
    ui.ctx().request_repaint();
}

impl Launcher {
    fn sidebar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, c::SIDEBAR);
        p.line_segment(
            [rect.right_top(), rect.right_bottom()],
            Stroke::new(1.0_f32, c::BORDER),
        );
        let logo_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 34.0),
            Vec2::splat(40.0),
        );
        p.image(
            self.logo.id(),
            logo_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        let items = [
            (Page::Play, "Jouer"),
            (Page::Mods, "Mods"),
            (Page::News, "Actus"),
            (Page::Settings, "Réglages"),
        ];
        for (i, (page, label)) in items.into_iter().enumerate() {
            let r = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 110.0 + i as f32 * 70.0),
                Vec2::new(64.0, 58.0),
            );
            let resp = ui.interact(r, ui.id().with(("nav", i)), Sense::click());
            let active = self.page == page;
            if active {
                p.rect_filled(r, CornerRadius::same(12), ui::alpha(c::ACCENT, 0.12));
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left(), r.top() + 14.0),
                        Vec2::new(3.0, 30.0),
                    ),
                    CornerRadius::same(2),
                    c::ACCENT,
                );
            } else if resp.hovered() {
                p.rect_filled(r, CornerRadius::same(12), c::CARD);
            }
            let col = if active {
                c::ACCENT
            } else if resp.hovered() {
                c::TEXT
            } else {
                c::MUTED
            };
            nav_icon(&p, page, r.center() - Vec2::new(0.0, 8.0), col);
            ui::text(
                &p,
                r.center() + Vec2::new(0.0, 16.0),
                Align2::CENTER_CENTER,
                label,
                ui::medium(11.5),
                col,
            );
            if resp.clicked() {
                self.page = page;
            }
        }
        ui::text(
            &p,
            Pos2::new(rect.center().x, rect.bottom() - 22.0),
            Align2::CENTER_CENTER,
            &format!("v{}", oteria_dist::LAUNCHER_VERSION),
            ui::regular(11.0),
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
        ui::text(
            &p,
            Pos2::new(rect.left() + 28.0, rect.center().y),
            Align2::LEFT_CENTER,
            "OTERIA",
            ui::display(18.0),
            c::TEXT,
        );
        ui::text(
            &p,
            Pos2::new(rect.left() + 104.0, rect.center().y + 1.0),
            Align2::LEFT_CENTER,
            "CLIENT",
            ui::semi(12.0),
            c::ACCENT,
        );
        let page = match self.page {
            Page::Play => "Accueil",
            Page::Mods => "Mods",
            Page::News => "Actualités",
            Page::Settings => "Réglages",
        };
        ui::text(
            &p,
            Pos2::new(rect.left() + 170.0, rect.center().y),
            Align2::LEFT_CENTER,
            &format!("/  {page}"),
            ui::regular(13.0),
            c::FAINT,
        );

        // The account chip.
        let chip_w = 200.0;
        let chip = Rect::from_min_size(
            Pos2::new(rect.right() - 46.0 * 3.0 - 16.0 - chip_w, rect.top() + 10.0),
            Vec2::new(chip_w, 36.0),
        );
        let resp = ui.interact(chip, ui.id().with("account"), Sense::click());
        ui::plate(
            &p,
            chip,
            18,
            if resp.hovered() {
                c::CARD_HOVER
            } else {
                c::CARD
            },
            c::BORDER,
        );
        let head = Rect::from_min_size(chip.min + Vec2::new(6.0, 6.0), Vec2::splat(24.0));
        p.rect_filled(head.expand(1.0), CornerRadius::same(5), c::BORDER);
        ui::draw_head(&p, head, self.skin.as_ref());
        ui::text(
            &p,
            Pos2::new(head.right() + 10.0, chip.top() + 11.0),
            Align2::LEFT_CENTER,
            &self.name,
            ui::semi(13.0),
            c::TEXT,
        );
        p.circle_filled(
            Pos2::new(head.right() + 13.0, chip.top() + 25.0),
            3.0,
            c::WARN,
        );
        ui::text(
            &p,
            Pos2::new(head.right() + 20.0, chip.top() + 25.0),
            Align2::LEFT_CENTER,
            "Hors ligne",
            ui::regular(11.0),
            c::MUTED,
        );
        if resp.clicked() {
            self.page = Page::Settings;
        }
        ui::window_controls(ui, rect);
    }

    fn play_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let pad = 24.0;
        let right_w = 284.0;
        let main = Rect::from_min_max(
            body.min + Vec2::new(pad, 8.0),
            Pos2::new(body.right() - right_w - pad * 2.0, body.bottom() - pad),
        );
        let side = Rect::from_min_max(
            Pos2::new(main.right() + pad, main.top()),
            Pos2::new(body.right() - pad, main.bottom()),
        );
        let hero = Rect::from_min_size(
            main.min,
            Vec2::new(main.width(), (main.height() * 0.6).clamp(330.0, 430.0)),
        );
        self.hero(ui, hero);
        let news_top = hero.bottom() + 22.0;
        let p = ui.painter().clone();
        ui::text(
            &p,
            Pos2::new(main.left(), news_top),
            Align2::LEFT_TOP,
            "Actualités",
            ui::display(18.0),
            c::TEXT,
        );
        let all = Rect::from_min_size(
            Pos2::new(main.right() - 80.0, news_top),
            Vec2::new(80.0, 22.0),
        );
        let resp = ui.interact(all, ui.id().with("all-news"), Sense::click());
        ui::text(
            &p,
            all.right_center(),
            Align2::RIGHT_CENTER,
            "Tout voir →",
            ui::medium(12.5),
            if resp.hovered() {
                c::ACCENT_HI
            } else {
                c::ACCENT
            },
        );
        if resp.clicked() {
            self.page = Page::News;
        }
        let cards_top = news_top + 34.0;
        let news = self.news();
        let n = 3usize;
        let gap = 16.0;
        let cw = (main.width() - gap * (n as f32 - 1.0)) / n as f32;
        let ch = (main.bottom() - cards_top).max(120.0);
        for (i, item) in news.iter().take(n).enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(main.left() + i as f32 * (cw + gap), cards_top),
                Vec2::new(cw, ch),
            );
            self.news_card(ui, r, item, i, false);
        }
        self.side_column(ui, side);
    }

    fn news(&self) -> Vec<NewsItem> {
        if !self.report.news.is_empty() {
            return self.report.news.clone();
        }
        vec![NewsItem {
            title: "Bienvenue sur Oteria Client".into(),
            body: "Les actualités arrivent avec la source de mise à jour.".into(),
            date: String::new(),
            tag: "Oteria".into(),
        }]
    }

    fn hero(&mut self, ui: &mut egui::Ui, r: Rect) {
        let p = ui.painter().clone();
        p.add(
            egui::epaint::RectShape::filled(r, CornerRadius::same(18), Color32::WHITE)
                .with_texture(
                    self.hero_tex.id(),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                ),
        );
        let clip = p.with_clip_rect(r.shrink(2.0));
        // The logo's waves, large and faint, behind everything.
        let big = r.height() * 1.25;
        clip.image(
            self.logo.id(),
            Rect::from_center_size(
                Pos2::new(
                    r.right() - r.width() * 0.26,
                    r.center().y + r.height() * 0.08,
                ),
                Vec2::splat(big),
            ),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            ui::alpha(Color32::WHITE, 0.07),
        );
        // Diagonal light streaks.
        for i in 0..5 {
            let x = r.left() + r.width() * (0.35 + i as f32 * 0.09);
            clip.line_segment(
                [
                    Pos2::new(x, r.bottom()),
                    Pos2::new(x + r.height() * 0.6, r.top()),
                ],
                Stroke::new(1.0_f32, ui::alpha(c::ACCENT_HI, 0.05)),
            );
        }
        p.rect_stroke(
            r,
            CornerRadius::same(18),
            Stroke::new(1.0_f32, ui::alpha(c::ACCENT, 0.25)),
            StrokeKind::Inside,
        );

        // The player, standing on the right in the light.
        let ph = r.height() * 0.72;
        let feet = Pos2::new(r.right() - r.width() * 0.2, r.bottom() - r.height() * 0.09);
        ui::glow(
            &clip,
            feet - Vec2::new(0.0, ph * 0.5),
            ph * 0.62,
            c::ACCENT,
            0.9,
        );
        let fig = Rect::from_min_max(
            Pos2::new(feet.x - ph / 4.0, feet.y - ph),
            Pos2::new(feet.x + ph / 4.0, feet.y),
        );
        ui::draw_player(&clip, fig, self.skin.as_ref());
        let name_size = p
            .layout_no_wrap(self.name.clone(), ui::semi(12.0), c::TEXT)
            .size();
        let tag = Rect::from_center_size(
            Pos2::new(feet.x, fig.top() - 22.0),
            name_size + Vec2::new(22.0, 12.0),
        );
        p.rect_filled(tag, CornerRadius::same(6), Color32::from_black_alpha(140));
        ui::text(
            &p,
            tag.center(),
            Align2::CENTER_CENTER,
            &self.name,
            ui::semi(12.0),
            c::TEXT,
        );

        // Words on the left.
        let x = r.left() + 36.0;
        let mut y = r.top() + 34.0;
        let pill = ui::pill(
            &p,
            Pos2::new(x, y),
            &format!("MINECRAFT JAVA {}", oteria_dist::MINECRAFT_VERSION),
            c::ACCENT_HI,
            ui::alpha(c::ACCENT, 0.16),
        );
        y = pill.bottom() + 14.0;
        ui::text(
            &p,
            Pos2::new(x - 2.0, y),
            Align2::LEFT_TOP,
            "OTERIA",
            ui::display(56.0),
            c::TEXT,
        );
        y += 58.0;
        ui::text(
            &p,
            Pos2::new(x, y),
            Align2::LEFT_TOP,
            "CLIENT",
            ui::display(30.0),
            c::ACCENT,
        );
        y += 44.0;
        ui::wrapped(&p, Pos2::new(x, y), "Le client PvP réécrit en Rust : plus d'images par seconde, des mods PvP intégrés, zéro Java.", ui::regular(14.5), c::MUTED, (r.width() * 0.46).min(420.0), 2);

        // The main button and, under it, what it is doing.
        let bw = if matches!(self.phase, Phase::NeedDownload { update: true }) {
            380.0
        } else {
            300.0
        };
        let btn = Rect::from_min_size(
            Pos2::new(x, r.bottom() - 36.0 - 66.0 - 22.0),
            Vec2::new(bw, 66.0),
        );
        self.main_button(ui, btn);
        let folder = Rect::from_min_size(
            Pos2::new(btn.right() + 12.0, btn.top()),
            Vec2::new(66.0, 66.0),
        );
        let fr = ui.interact(folder, ui.id().with("folder"), Sense::click());
        ui::plate(
            &p,
            folder,
            14,
            if fr.hovered() {
                ui::alpha(Color32::WHITE, 0.12)
            } else {
                ui::alpha(Color32::WHITE, 0.06)
            },
            ui::alpha(Color32::WHITE, 0.12),
        );
        folder_icon(
            &p,
            folder.center(),
            if fr.hovered() { c::TEXT } else { c::MUTED },
        );
        if fr.on_hover_text("Ouvrir le dossier du jeu").clicked() {
            let _ = std::fs::create_dir_all(&self.game_dir);
            let _ = Command::new("explorer").arg(&self.game_dir).spawn();
        }
        self.status_line(&p, Pos2::new(x, btn.bottom() + 12.0), bw + 78.0);
    }

    fn main_button(&mut self, ui: &mut egui::Ui, btn: Rect) {
        let (label, sub, enabled) = match &self.phase {
            Phase::Checking => (
                "RECHERCHE…".to_owned(),
                "Recherche de mises à jour".to_owned(),
                false,
            ),
            Phase::SelfUpdating { .. } => {
                ("MISE À JOUR…".into(), "Le launcher redémarre".into(), false)
            }
            Phase::NeedDownload { update: false } => (
                "TÉLÉCHARGER".into(),
                format!(
                    "Oteria {} · première installation",
                    self.report.client_latest.clone().unwrap_or_default()
                ),
                true,
            ),
            Phase::NeedDownload { update: true } => (
                "TÉLÉCHARGER LA MISE À JOUR".into(),
                format!(
                    "Oteria {} → {}",
                    self.report.client_installed.clone().unwrap_or_default(),
                    self.report.client_latest.clone().unwrap_or_default()
                ),
                true,
            ),
            Phase::Downloading => ("TÉLÉCHARGEMENT…".into(), String::new(), false),
            Phase::Ready => (
                "JOUER".into(),
                format!(
                    "Oteria {} · Minecraft {}",
                    self.state.client_version.clone().unwrap_or_default(),
                    oteria_dist::MINECRAFT_VERSION
                ),
                true,
            ),
            Phase::Launching { .. } => ("LANCEMENT…".into(), "Le jeu démarre".into(), false),
            Phase::Running => ("EN JEU".into(), "Oteria Client est ouvert".into(), false),
            Phase::Failed(_) => (
                "RÉESSAYER".into(),
                "Source de mise à jour injoignable".into(),
                true,
            ),
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
        if enabled {
            // Its light on the banner.
            p.rect_filled(
                btn.expand(if hovered { 6.0 } else { 3.0 }),
                CornerRadius::same(18),
                ui::alpha(c::ACCENT, if hovered { 0.22 } else { 0.12 }),
            );
        }
        let (top, bottom) = if !enabled {
            (
                Color32::from_rgb(0x1d, 0x3b, 0x52),
                Color32::from_rgb(0x17, 0x2f, 0x44),
            )
        } else if hovered {
            (Color32::from_rgb(0x6f, 0xd6, 0xf6), c::ACCENT)
        } else {
            (c::ACCENT, c::ACCENT_DEEP)
        };
        let mut mesh = egui::epaint::Mesh::default();
        // Rounded shape with a vertical gradient: a plain rounded rect, then a gradient inset.
        p.rect_filled(btn, CornerRadius::same(14), bottom);
        let inner = btn.shrink2(Vec2::new(0.0, 0.0));
        let steps = 14;
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let y0 = inner.top() + inner.height() * t0 * 0.6;
            let y1 = inner.top() + inner.height() * t1 * 0.6;
            let inset = if i == 0 {
                10.0
            } else if i == 1 {
                4.0
            } else {
                1.0
            };
            let col = ui::lerp(top, bottom, t0);
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(Pos2::new(inner.left() + inset, y0), col);
            mesh.colored_vertex(Pos2::new(inner.right() - inset, y0), col);
            mesh.colored_vertex(
                Pos2::new(inner.right() - 1.0, y1),
                ui::lerp(top, bottom, t1),
            );
            mesh.colored_vertex(Pos2::new(inner.left() + 1.0, y1), ui::lerp(top, bottom, t1));
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
        p.add(egui::Shape::mesh(mesh));
        let fg = if enabled { c::ON_ACCENT } else { c::MUTED };
        let has_sub = !sub.is_empty();
        let ly = if has_sub {
            btn.center().y - 9.0
        } else {
            btn.center().y
        };
        let icon_x = btn.left() + 30.0;
        match &self.phase {
            Phase::Ready => {
                let cn = Pos2::new(icon_x, btn.center().y);
                p.add(egui::Shape::convex_polygon(
                    vec![
                        cn + Vec2::new(-7.0, -10.0),
                        cn + Vec2::new(10.0, 0.0),
                        cn + Vec2::new(-7.0, 10.0),
                    ],
                    fg,
                    Stroke::NONE,
                ));
            }
            Phase::NeedDownload { .. } | Phase::Failed(_) => {
                download_icon(&p, Pos2::new(icon_x, btn.center().y), fg)
            }
            _ => spinner(
                &p,
                Pos2::new(icon_x, btn.center().y),
                fg,
                self.started.elapsed().as_secs_f32(),
            ),
        }
        let size = if label.chars().count() > 14 {
            19.0
        } else {
            24.0
        };
        ui::text(
            &p,
            Pos2::new(btn.left() + 56.0, ly),
            Align2::LEFT_CENTER,
            &label,
            ui::display(size),
            fg,
        );
        if has_sub {
            ui::text(
                &p,
                Pos2::new(btn.left() + 57.0, ly + 21.0),
                Align2::LEFT_CENTER,
                &sub,
                ui::medium(12.0),
                ui::alpha(fg, 0.78),
            );
        }
        if resp.clicked() {
            self.main_action();
        }
    }

    fn status_line(&self, p: &egui::Painter, pos: Pos2, width: f32) {
        if let Some(d) = &self.download {
            let stage = match d.stage.as_str() {
                "download" => "Téléchargement du client",
                "extract" => "Installation",
                "minecraft" => "Minecraft (serveurs de Mojang)",
                "java" => "Java (serveurs de Mojang)",
                "resources" => "Préparation du jeu",
                other => other,
            };
            let bar = Rect::from_min_size(pos + Vec2::new(0.0, 4.0), Vec2::new(width, 6.0));
            p.rect_filled(bar, CornerRadius::same(3), ui::alpha(Color32::WHITE, 0.1));
            let k = d.fraction.clamp(0.0, 1.0);
            p.rect_filled(
                Rect::from_min_size(bar.min, Vec2::new((bar.width() * k).max(6.0), 6.0)),
                CornerRadius::same(3),
                c::ACCENT,
            );
            ui::text(
                p,
                bar.left_bottom() + Vec2::new(0.0, 8.0),
                Align2::LEFT_TOP,
                stage,
                ui::medium(12.5),
                c::TEXT,
            );
            ui::text(
                p,
                bar.right_bottom() + Vec2::new(0.0, 8.0),
                Align2::RIGHT_TOP,
                &format!("{:.0} %", k * 100.0),
                ui::semi(12.5),
                c::ACCENT_HI,
            );
            if !d.note.is_empty() {
                ui::wrapped(
                    p,
                    bar.left_bottom() + Vec2::new(0.0, 26.0),
                    &d.note,
                    ui::regular(11.5),
                    c::MUTED,
                    width,
                    1,
                );
            }
            return;
        }
        let (text, col) = match (&self.notice, &self.phase) {
            (Some((t, col)), _) => (t.clone(), *col),
            (None, Phase::Failed(e)) => (e.clone(), c::BAD),
            (None, Phase::NeedDownload { update: true }) => (
                "Une mise à jour du client est obligatoire avant de jouer.".into(),
                c::WARN,
            ),
            (None, Phase::NeedDownload { update: false }) => (
                "Oteria Client n'est pas encore installé sur ce PC.".into(),
                c::MUTED,
            ),
            (None, Phase::Ready) => ("À jour · prêt à jouer".into(), c::GOOD),
            _ => return,
        };
        p.circle_filled(pos + Vec2::new(4.0, 9.0), 3.5, col);
        ui::wrapped(
            p,
            pos + Vec2::new(14.0, 1.0),
            &text,
            ui::medium(12.5),
            ui::lerp(col, c::TEXT, 0.35),
            width,
            2,
        );
    }

    fn news_card(&self, ui: &mut egui::Ui, r: Rect, item: &NewsItem, index: usize, wide: bool) {
        let resp = ui.interact(r, ui.id().with(("news", index, wide)), Sense::hover());
        let p = ui.painter().clone();
        ui::plate(
            &p,
            r,
            14,
            if resp.hovered() {
                c::CARD_HOVER
            } else {
                c::CARD
            },
            c::BORDER,
        );
        let col = tag_colour(&item.tag);
        let art_h = if wide {
            r.height()
        } else {
            (r.height() * 0.42).clamp(56.0, 120.0)
        };
        let art = if wide {
            Rect::from_min_size(r.min, Vec2::new(200.0, r.height()))
        } else {
            Rect::from_min_size(r.min, Vec2::new(r.width(), art_h))
        };
        let clip = p.with_clip_rect(art.shrink(1.0));
        clip.rect_filled(art, CornerRadius::same(14), ui::lerp(c::CARD, col, 0.22));
        ui::glow(
            &clip,
            art.right_top() - Vec2::new(art.width() * 0.25, -art.height() * 0.2),
            art.width() * 0.6,
            col,
            0.9,
        );
        let s = art.height() * 1.1;
        clip.image(
            self.logo.id(),
            Rect::from_center_size(
                art.center() + Vec2::new(art.width() * 0.25, 0.0),
                Vec2::splat(s),
            ),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            ui::alpha(Color32::WHITE, 0.18),
        );
        if !wide {
            // Square corners at the bottom of the art.
            clip.rect_filled(
                Rect::from_min_max(
                    Pos2::new(art.left(), art.bottom() - 14.0),
                    art.right_bottom(),
                ),
                0.0,
                ui::lerp(c::CARD, col, 0.22),
            );
        }
        ui::pill(
            &p,
            art.min + Vec2::new(14.0, 14.0),
            &item.tag.to_uppercase(),
            Color32::WHITE,
            ui::alpha(col, 0.55),
        );
        let (tx, ty, tw) = if wide {
            (
                art.right() + 20.0,
                r.top() + 18.0,
                r.right() - art.right() - 40.0,
            )
        } else {
            (r.left() + 16.0, art.bottom() + 12.0, r.width() - 32.0)
        };
        let t = ui::wrapped(
            &p,
            Pos2::new(tx, ty),
            &item.title,
            ui::semi(15.0),
            c::TEXT,
            tw,
            2,
        );
        let mut y = t.bottom() + 6.0;
        if !item.date.is_empty() {
            ui::text(
                &p,
                Pos2::new(tx, y),
                Align2::LEFT_TOP,
                &item.date,
                ui::regular(11.5),
                c::FAINT,
            );
            y += 20.0;
        }
        let rows = ((r.bottom() - 12.0 - y) / 18.0).floor().max(0.0) as usize;
        if rows > 0 {
            ui::wrapped(
                &p,
                Pos2::new(tx, y),
                &item.body,
                ui::regular(12.5),
                c::MUTED,
                tw,
                rows,
            );
        }
    }

    fn side_column(&mut self, ui: &mut egui::Ui, side: Rect) {
        let p = ui.painter().clone();
        // Friends.
        let friends = Rect::from_min_size(side.min, Vec2::new(side.width(), 214.0));
        ui::plate(&p, friends, 16, c::PANEL, c::BORDER);
        ui::text(
            &p,
            friends.min + Vec2::new(18.0, 18.0),
            Align2::LEFT_TOP,
            "Amis",
            ui::display(16.0),
            c::TEXT,
        );
        ui::pill(
            &p,
            Pos2::new(friends.right() - 88.0, friends.top() + 16.0),
            "0 EN LIGNE",
            c::MUTED,
            c::CARD,
        );
        // Me.
        let me = Rect::from_min_size(
            friends.min + Vec2::new(12.0, 52.0),
            Vec2::new(friends.width() - 24.0, 46.0),
        );
        p.rect_filled(me, CornerRadius::same(10), c::CARD);
        let head = Rect::from_min_size(me.min + Vec2::new(9.0, 9.0), Vec2::splat(28.0));
        ui::draw_head(&p, head, self.skin.as_ref());
        ui::text(
            &p,
            Pos2::new(head.right() + 10.0, me.top() + 14.0),
            Align2::LEFT_CENTER,
            &self.name,
            ui::semi(13.0),
            c::TEXT,
        );
        ui::text(
            &p,
            Pos2::new(head.right() + 10.0, me.top() + 31.0),
            Align2::LEFT_CENTER,
            "Toi · compte hors ligne",
            ui::regular(11.5),
            c::MUTED,
        );
        let msg = Rect::from_min_size(
            Pos2::new(friends.left() + 18.0, me.bottom() + 16.0),
            Vec2::new(friends.width() - 36.0, 80.0),
        );
        people_icon(&p, msg.min + Vec2::new(14.0, 14.0), c::FAINT);
        ui::wrapped(
            &p,
            msg.min + Vec2::new(38.0, 0.0),
            "La liste d'amis arrive avec la connexion Microsoft.",
            ui::regular(12.5),
            c::MUTED,
            msg.width() - 38.0,
            3,
        );
        ui::pill(
            &p,
            Pos2::new(msg.left() + 38.0, msg.top() + 46.0),
            "BIENTÔT",
            c::ACCENT_HI,
            ui::alpha(c::ACCENT, 0.14),
        );

        // Partner servers.
        let servers = Rect::from_min_max(Pos2::new(side.left(), friends.bottom() + 16.0), side.max);
        ui::plate(&p, servers, 16, c::PANEL, c::BORDER);
        ui::text(
            &p,
            servers.min + Vec2::new(18.0, 18.0),
            Align2::LEFT_TOP,
            "Serveurs partenaires",
            ui::display(16.0),
            c::TEXT,
        );
        let list: Vec<PartnerServer> = self.report.servers.clone();
        if list.is_empty() {
            ui::wrapped(
                &p,
                servers.min + Vec2::new(18.0, 54.0),
                "Aucun serveur partenaire pour le moment.",
                ui::regular(12.5),
                c::MUTED,
                servers.width() - 36.0,
                2,
            );
        }
        let can_join = matches!(self.phase, Phase::Ready);
        for (i, s) in list.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(
                    servers.left() + 12.0,
                    servers.top() + 52.0 + i as f32 * 62.0,
                ),
                Vec2::new(servers.width() - 24.0, 54.0),
            );
            if row.bottom() > servers.bottom() - 8.0 {
                break;
            }
            let resp = ui.interact(row, ui.id().with(("srv", i)), Sense::click());
            p.rect_filled(
                row,
                CornerRadius::same(10),
                if resp.hovered() {
                    c::CARD_HOVER
                } else {
                    c::CARD
                },
            );
            let ic = Rect::from_min_size(row.min + Vec2::new(9.0, 9.0), Vec2::splat(36.0));
            let col = tag_colour(&format!("{i}{}", s.name)).gamma_multiply(1.0);
            let hue = [
                c::ACCENT_DEEP,
                Color32::from_rgb(0x8b, 0x5c, 0xf6),
                Color32::from_rgb(0xe0, 0x5a, 0x47),
                Color32::from_rgb(0x2f, 0xb3, 0x7a),
            ][i % 4];
            let _ = col;
            p.rect_filled(ic, CornerRadius::same(9), hue);
            let initial: String = s
                .name
                .chars()
                .next()
                .map(|ch| ch.to_uppercase().collect())
                .unwrap_or_default();
            ui::text(
                &p,
                ic.center(),
                Align2::CENTER_CENTER,
                &initial,
                ui::display(18.0),
                Color32::WHITE,
            );
            ui::text(
                &p,
                Pos2::new(ic.right() + 11.0, row.top() + 17.0),
                Align2::LEFT_CENTER,
                &s.name,
                ui::semi(13.0),
                c::TEXT,
            );
            ui::wrapped(
                &p,
                Pos2::new(ic.right() + 11.0, row.top() + 27.0),
                &s.description,
                ui::regular(11.0),
                c::MUTED,
                row.width() - 130.0,
                1,
            );
            let join = Rect::from_min_size(
                Pos2::new(row.right() - 72.0, row.center().y - 14.0),
                Vec2::new(62.0, 28.0),
            );
            let jr = ui.interact(
                join,
                ui.id().with(("join", i)),
                if can_join {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            p.rect_filled(
                join,
                CornerRadius::same(8),
                if !can_join {
                    c::BORDER
                } else if jr.hovered() {
                    c::ACCENT_HI
                } else {
                    c::ACCENT
                },
            );
            ui::text(
                &p,
                join.center(),
                Align2::CENTER_CENTER,
                "Jouer",
                ui::semi(12.0),
                if can_join { c::ON_ACCENT } else { c::FAINT },
            );
            if jr.on_hover_text(&s.address).clicked() {
                let addr = s.address.clone();
                self.launch(Some(&addr));
            }
        }
    }

    fn page_title(&self, p: &egui::Painter, body: Rect, title: &str, subtitle: &str) -> f32 {
        ui::text(
            p,
            body.min + Vec2::new(28.0, 14.0),
            Align2::LEFT_TOP,
            title,
            ui::display(28.0),
            c::TEXT,
        );
        ui::text(
            p,
            body.min + Vec2::new(29.0, 52.0),
            Align2::LEFT_TOP,
            subtitle,
            ui::regular(13.5),
            c::MUTED,
        );
        body.top() + 92.0
    }

    fn mods_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(
            &p,
            body,
            "Mods",
            "Intégrés au client, sans rien installer : active-les en jeu avec Maj droit.",
        );
        let area = Rect::from_min_max(
            Pos2::new(body.left() + 28.0, top),
            body.max - Vec2::new(28.0, 20.0),
        );
        let cols = ((area.width() + 16.0) / 290.0).floor().max(1.0) as usize;
        let gap = 16.0;
        let w = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
        let h = 96.0;
        for (i, (name, desc, cat)) in MODS.iter().enumerate() {
            let (col, row) = (i % cols, i / cols);
            let r = Rect::from_min_size(
                Pos2::new(
                    area.left() + col as f32 * (w + gap),
                    area.top() + row as f32 * (h + gap),
                ),
                Vec2::new(w, h),
            );
            if r.bottom() > area.bottom() {
                break;
            }
            let resp = ui.interact(r, ui.id().with(("mod", i)), Sense::hover());
            ui::plate(
                &p,
                r,
                14,
                if resp.hovered() {
                    c::CARD_HOVER
                } else {
                    c::CARD
                },
                c::BORDER,
            );
            let colr = tag_colour(cat);
            let ic = Rect::from_min_size(r.min + Vec2::new(16.0, 16.0), Vec2::splat(40.0));
            p.rect_filled(ic, CornerRadius::same(10), ui::alpha(colr, 0.16));
            ui::text(
                &p,
                ic.center(),
                Align2::CENTER_CENTER,
                &name.chars().next().unwrap().to_string(),
                ui::display(20.0),
                colr,
            );
            ui::text(
                &p,
                Pos2::new(ic.right() + 14.0, r.top() + 18.0),
                Align2::LEFT_TOP,
                name,
                ui::semi(14.5),
                c::TEXT,
            );
            ui::wrapped(
                &p,
                Pos2::new(ic.right() + 14.0, r.top() + 40.0),
                desc,
                ui::regular(12.0),
                c::MUTED,
                r.right() - ic.right() - 30.0,
                2,
            );
            ui::pill(
                &p,
                Pos2::new(
                    r.right() - 16.0 - 8.0 * cat.len() as f32 - 10.0,
                    r.top() + 14.0,
                ),
                &cat.to_uppercase(),
                colr,
                ui::alpha(colr, 0.12),
            );
            // The switch: on by default in the client (shown, not changed here).
            let _ = row;
        }
    }

    fn news_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(
            &p,
            body,
            "Actualités",
            "Les nouveautés d'Oteria Client, depuis la source de mise à jour.",
        );
        let area = Rect::from_min_max(
            Pos2::new(body.left() + 28.0, top),
            body.max - Vec2::new(28.0, 20.0),
        );
        for (i, item) in self.news().iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(area.left(), area.top() + i as f32 * 150.0),
                Vec2::new(area.width().min(900.0), 134.0),
            );
            if r.bottom() > area.bottom() {
                break;
            }
            self.news_card(ui, r, item, i, true);
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = ui.painter().clone();
        let top = self.page_title(
            &p,
            body,
            "Réglages",
            "Le compte, le dossier du jeu et les mises à jour.",
        );
        let x = body.left() + 28.0;
        let w = (body.width() - 56.0).min(820.0);
        let mut y = top;
        let section =
            |p: &egui::Painter, y: &mut f32, title: &str, rows: &[(&str, String)]| -> Rect {
                let h = 46.0 + rows.len() as f32 * 34.0;
                let r = Rect::from_min_size(Pos2::new(x, *y), Vec2::new(w, h));
                ui::plate(p, r, 14, c::PANEL, c::BORDER);
                ui::text(
                    p,
                    r.min + Vec2::new(18.0, 14.0),
                    Align2::LEFT_TOP,
                    title,
                    ui::semi(14.5),
                    c::TEXT,
                );
                for (i, (k, v)) in rows.iter().enumerate() {
                    let ry = r.top() + 50.0 + i as f32 * 34.0;
                    ui::text(
                        p,
                        Pos2::new(r.left() + 18.0, ry),
                        Align2::LEFT_TOP,
                        k,
                        ui::regular(13.0),
                        c::MUTED,
                    );
                    ui::wrapped(
                        p,
                        Pos2::new(r.left() + 220.0, ry),
                        v,
                        ui::medium(13.0),
                        c::TEXT,
                        w - 240.0 - 150.0,
                        1,
                    );
                }
                *y = r.bottom() + 14.0;
                r
            };
        section(
            &p,
            &mut y,
            "Compte",
            &[
                ("Pseudo", self.name.clone()),
                (
                    "Type",
                    "Hors ligne · la connexion Microsoft arrive bientôt".into(),
                ),
            ],
        );
        let game = section(
            &p,
            &mut y,
            "Jeu",
            &[
                ("Dossier du jeu", self.game_dir.display().to_string()),
                (
                    "Client installé",
                    self.state
                        .client_version
                        .clone()
                        .map_or("aucun".into(), |v| format!("Oteria {v}")),
                ),
            ],
        );
        if small_button(
            ui,
            &p,
            Rect::from_min_size(
                Pos2::new(game.right() - 132.0, game.top() + 44.0),
                Vec2::new(116.0, 30.0),
            ),
            "Ouvrir",
            "open-game",
        ) {
            let _ = std::fs::create_dir_all(&self.game_dir);
            let _ = Command::new("explorer").arg(&self.game_dir).spawn();
        }
        let launcher = section(
            &p,
            &mut y,
            "Launcher",
            &[("Au lancement du jeu", String::new())],
        );
        let check = Rect::from_min_size(
            Pos2::new(launcher.left() + 220.0, launcher.top() + 46.0),
            Vec2::new(360.0, 24.0),
        );
        let cr = ui.interact(check, ui.id().with("close-on-launch"), Sense::click());
        let on = self.state.close_on_launch;
        let sw = Rect::from_min_size(check.min, Vec2::new(38.0, 22.0));
        p.rect_filled(
            sw,
            CornerRadius::same(11),
            if on { c::ACCENT } else { c::BORDER },
        );
        p.circle_filled(
            Pos2::new(
                if on {
                    sw.right() - 11.0
                } else {
                    sw.left() + 11.0
                },
                sw.center().y,
            ),
            8.0,
            Color32::WHITE,
        );
        ui::text(
            &p,
            Pos2::new(sw.right() + 12.0, sw.center().y),
            Align2::LEFT_CENTER,
            "Fermer le launcher",
            ui::medium(13.0),
            c::TEXT,
        );
        if cr.clicked() {
            self.state = State::load(&self.home);
            self.state.close_on_launch = !on;
            let _ = self.state.save(&self.home);
        }
        let upd = section(
            &p,
            &mut y,
            "Mises à jour",
            &[
                ("Launcher", format!("v{}", oteria_dist::LAUNCHER_VERSION)),
                (
                    "Dernier client",
                    self.report
                        .client_latest
                        .clone()
                        .map_or("—".into(), |v| format!("Oteria {v}")),
                ),
                (
                    "Source",
                    self.state.source().unwrap_or_else(|| "aucune".into()),
                ),
            ],
        );
        let busy = matches!(self.phase, Phase::Checking | Phase::Downloading);
        if small_button(
            ui,
            &p,
            Rect::from_min_size(
                Pos2::new(upd.right() - 132.0, upd.top() + 44.0),
                Vec2::new(116.0, 30.0),
            ),
            if busy { "…" } else { "Vérifier" },
            "check",
        ) && !busy
        {
            self.notice = None;
            self.start_check();
        }
        ui::text(&p, Pos2::new(x, y + 6.0), Align2::LEFT_TOP, "Oteria Client est un fork d'Horizon (MIT). Ni produit officiel Minecraft, ni approuvé par Mojang ou Microsoft.", ui::regular(11.5), c::FAINT);
    }
}

fn small_button(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, label: &str, id: &str) -> bool {
    let resp = ui.interact(r, ui.id().with(("btn", id)), Sense::click());
    ui::plate(
        p,
        r,
        9,
        if resp.hovered() {
            c::CARD_HOVER
        } else {
            c::CARD
        },
        c::BORDER,
    );
    ui::text(
        p,
        r.center(),
        Align2::CENTER_CENTER,
        label,
        ui::semi(12.5),
        c::TEXT,
    );
    resp.clicked()
}

fn download_icon(p: &egui::Painter, cn: Pos2, col: Color32) {
    let st = Stroke::new(2.6_f32, col);
    p.line_segment([cn + Vec2::new(0.0, -10.0), cn + Vec2::new(0.0, 4.0)], st);
    p.line_segment([cn + Vec2::new(-6.0, -2.0), cn + Vec2::new(0.0, 4.5)], st);
    p.line_segment([cn + Vec2::new(6.0, -2.0), cn + Vec2::new(0.0, 4.5)], st);
    p.line_segment([cn + Vec2::new(-9.0, 10.0), cn + Vec2::new(9.0, 10.0)], st);
}

fn folder_icon(p: &egui::Painter, cn: Pos2, col: Color32) {
    let st = Stroke::new(1.8_f32, col);
    let body = Rect::from_center_size(cn + Vec2::new(0.0, 2.0), Vec2::new(24.0, 17.0));
    p.rect_stroke(body, 3.0, st, StrokeKind::Middle);
    p.line_segment(
        [
            body.left_top() + Vec2::new(1.0, 0.0),
            body.left_top() + Vec2::new(4.0, -4.0),
        ],
        st,
    );
    p.line_segment(
        [
            body.left_top() + Vec2::new(4.0, -4.0),
            body.left_top() + Vec2::new(10.0, -4.0),
        ],
        st,
    );
    p.line_segment(
        [
            body.left_top() + Vec2::new(10.0, -4.0),
            body.left_top() + Vec2::new(12.0, 0.0),
        ],
        st,
    );
}

fn people_icon(p: &egui::Painter, cn: Pos2, col: Color32) {
    let st = Stroke::new(1.6_f32, col);
    p.circle_stroke(cn + Vec2::new(0.0, -5.0), 4.5, st);
    p.line_segment([cn + Vec2::new(-8.0, 9.0), cn + Vec2::new(-5.0, 2.0)], st);
    p.line_segment([cn + Vec2::new(-5.0, 2.0), cn + Vec2::new(5.0, 2.0)], st);
    p.line_segment([cn + Vec2::new(5.0, 2.0), cn + Vec2::new(8.0, 9.0)], st);
}

fn spinner(p: &egui::Painter, cn: Pos2, col: Color32, t: f32) {
    let n = 10;
    for i in 0..n {
        let a = t * 5.0 + i as f32 * std::f32::consts::TAU / n as f32;
        let k = i as f32 / n as f32;
        p.circle_filled(
            cn + Vec2::new(a.cos(), a.sin()) * 9.0,
            1.2 + 1.3 * k,
            ui::alpha(col, 0.25 + 0.75 * k),
        );
    }
}

#[allow(dead_code)]
fn _paths(_: &Path) {}
