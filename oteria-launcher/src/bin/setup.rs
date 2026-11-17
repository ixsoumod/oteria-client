//! `oteria-launcher-setup.exe`: installs Oteria Client's launcher and `updater.exe` from the update source
//! into `%LOCALAPPDATA%\Oteria`, adds the Desktop and Start menu shortcuts, then starts the
//! launcher (which downloads the client itself).
//!
//! The source: `--source PATH`, else `OTERIA_UPDATE_SOURCE`, else a `manifest.json` beside the
//! setup (or in `oteria-updates\` beside it), else Oteria's GitHub `beta` release. `--silent` installs without asking.
//! Tests: `OTERIA_HOME` moves the install, `OTERIA_NO_SHORTCUTS=1` skips the shortcuts,
//! `OTERIA_SETUP_SHOT=path` (with `OTERIA_SETUP_SHOT_AT=done`) saves a screenshot and closes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use oteria_dist::ui::{self, c};
use oteria_dist::{install, paths, Manifest, State};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

enum Msg {
    Step(String, f32),
    Done(String),
    Failed(String),
}

enum Phase {
    Welcome,
    Installing { step: String, fraction: f32 },
    Done { version: String },
    Failed(String),
}

struct Setup {
    source: Option<String>,
    home: PathBuf,
    phase: Phase,
    rx: Option<Receiver<Msg>>,
    logo: egui::TextureHandle,
    icons: ui::Icons,
    silent: bool,
    shot: Option<PathBuf>,
    shot_at: String,
    frames: u32,
    shot_sent: bool,
}

fn find_source() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--source") {
        return args.get(i + 1).cloned();
    }
    if let Ok(s) = std::env::var("OTERIA_UPDATE_SOURCE") {
        if !s.is_empty() {
            return Some(s);
        }
    }
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    [
        dir.join("manifest.json"),
        dir.join("oteria-updates").join("manifest.json"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .map(|p| p.display().to_string())
    // Else Oteria's GitHub `beta` release, what a friend's setup always uses.
    .or_else(|| Some(paths::DEFAULT_SOURCE.to_owned()))
}

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Installation d'Oteria Client")
        .with_inner_size([620.0, 420.0])
        .with_resizable(false)
        .with_decorations(false)
        .with_icon(ui::icon_data());
    eframe::run_native(
        "OteriaSetup",
        eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        },
        Box::new(|cc| {
            ui::install_fonts(&cc.egui_ctx);
            let source = find_source().map(|s| {
                if oteria_dist::manifest::is_url(&s) {
                    s
                } else {
                    std::path::absolute(&s).map_or(s.clone(), |p| p.display().to_string())
                }
            });
            let mut me = Setup {
                source,
                home: paths::home(),
                phase: Phase::Welcome,
                rx: None,
                logo: ui::svg_texture(&cc.egui_ctx, "logo", ui::LOGO_SVG, 192, false).unwrap(),
                icons: ui::Icons::load(&cc.egui_ctx),
                silent: std::env::args().any(|a| a == "--silent"),
                shot: std::env::var_os("OTERIA_SETUP_SHOT").map(PathBuf::from),
                shot_at: std::env::var("OTERIA_SETUP_SHOT_AT").unwrap_or_default(),
                frames: 0,
                shot_sent: false,
            };
            if me.silent {
                me.start();
            }
            Ok(Box::new(me))
        }),
    )
}

impl Setup {
    fn start(&mut self) {
        let Some(source) = self.source.clone() else {
            self.phase = Phase::Failed(
                "Aucune source de mise à jour : place manifest.json à côté d'oteria-launcher-setup.exe ou lance-le avec --source.".into(),
            );
            return;
        };
        let home = self.home.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = run_install(&home, &source, &mut |s, f| {
                let _ = tx.send(Msg::Step(s.to_owned(), f));
            });
            let _ = tx.send(match r {
                Ok(v) => Msg::Done(v),
                Err(e) => Msg::Failed(e),
            });
        });
        self.rx = Some(rx);
        self.phase = Phase::Installing {
            step: "Connexion à la source…".into(),
            fraction: 0.0,
        };
    }

    fn start_launcher(&self) {
        let _ = std::process::Command::new(self.home.join(paths::LAUNCHER_EXE))
            .current_dir(&self.home)
            .spawn();
    }
}

/// Copies the launcher and the updater from the source, records the source, adds shortcuts.
fn run_install(
    home: &Path,
    source: &str,
    step: &mut dyn FnMut(&str, f32),
) -> Result<String, String> {
    let manifest = Manifest::load(source)?;
    std::fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
    let launcher_new = home.join("OteriaLauncher.new.exe");
    install::fetch(&manifest, &manifest.launcher, &launcher_new, &mut |d, t| {
        step(
            "Téléchargement du launcher",
            if t == 0 {
                0.0
            } else {
                0.75 * d as f32 / t as f32
            },
        )
    })?;
    install::replace_exe(&launcher_new, &home.join(paths::LAUNCHER_EXE))?;
    let mut state = State::load(home);
    if let Some(u) = &manifest.updater {
        let new = home.join("updater.new.exe");
        install::fetch(&manifest, u, &new, &mut |d, t| {
            step(
                "Téléchargement de l'updater",
                0.75 + if t == 0 {
                    0.0
                } else {
                    0.2 * d as f32 / t as f32
                },
            )
        })?;
        install::replace_exe(&new, &home.join(paths::UPDATER_EXE))?;
        state.updater_version = Some(u.version.clone());
    } else {
        return Err("Le manifeste ne contient pas d'updater".into());
    }
    step("Raccourcis", 0.96);
    state.source = Some(source.to_owned());
    state.launcher_version = Some(manifest.launcher.version.clone());
    state.save(home).map_err(|e| e.to_string())?;
    if std::env::var_os("OTERIA_NO_SHORTCUTS").is_none() {
        shortcuts(home);
    }
    step("Terminé", 1.0);
    Ok(manifest.launcher.version)
}

/// Desktop and Start menu shortcuts, written by the shell (WScript.Shell) so Explorer reads them.
fn shortcuts(home: &Path) {
    let exe = home.join(paths::LAUNCHER_EXE);
    let script = format!(
        "$s = New-Object -ComObject WScript.Shell; \
         foreach ($d in @([Environment]::GetFolderPath('Desktop'), (Join-Path $env:APPDATA 'Microsoft\\Windows\\Start Menu\\Programs'))) {{ \
           $l = $s.CreateShortcut((Join-Path $d 'Oteria Client.lnk')); $l.TargetPath = '{0}'; $l.WorkingDirectory = '{1}'; $l.IconLocation = '{0},0'; $l.Save() }}",
        exe.display().to_string().replace('\'', "''"),
        home.display().to_string().replace('\'', "''"),
    );
    let _ = install::quiet_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status();
}

impl eframe::App for Setup {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        c::BG.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(rx) = &self.rx {
            while let Ok(m) = rx.try_recv() {
                self.phase = match m {
                    Msg::Step(step, fraction) => Phase::Installing { step, fraction },
                    Msg::Done(version) => Phase::Done { version },
                    Msg::Failed(e) => Phase::Failed(e),
                };
            }
        }
        if self.silent {
            if let Phase::Done { .. } = self.phase {
                if self.shot.is_none() {
                    self.start_launcher();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        // Test screenshots.
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let (Some(img), Some(path)) = (shot, &self.shot) {
            let _ = ui::save_screenshot(&img, path);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let ready = match self.shot_at.as_str() {
            "done" => matches!(self.phase, Phase::Done { .. }),
            _ => true,
        };
        if self.shot.is_some() && !self.shot_sent && ready {
            self.frames += 1;
            if self.frames > 10 {
                self.shot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(c::BG))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let icons = &self.icons;
                let p = ui.painter().clone();
                let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), 40.0));
                let drag = ui.interact(bar, ui.id().with("bar"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui::text(&p, Pos2::new(full.left() + 18.0, bar.center().y), Align2::LEFT_CENTER, "Installation d'Oteria Client", ui::medium(12.5), c::MUTED);
                ui::window_controls(ui, bar, &icons);
                p.image(
                    self.logo.id(),
                    Rect::from_center_size(Pos2::new(full.center().x, full.top() + 112.0), Vec2::splat(92.0)),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                ui::text(&p, Pos2::new(full.center().x, full.top() + 188.0), Align2::CENTER_CENTER, "OTERIA", ui::pixel(36.0), c::TEXT);
                let g = p.layout_no_wrap("LAUNCHER".into(), ui::semi(11.0), c::MUTED);
                ui::spaced(&p, Pos2::new(full.center().x - (g.size().x + 7.0 * 3.4) / 2.0, full.top() + 210.0), "LAUNCHER", ui::semi(11.0), c::MUTED, 3.4);

                let x = full.left() + 70.0;
                let w = full.width() - 140.0;
                let btn = Rect::from_min_size(Pos2::new(full.center().x - 130.0, full.bottom() - 98.0), Vec2::new(260.0, 54.0));
                let (sub, label, enabled) = match &self.phase {
                    Phase::Welcome => (
                        format!("Le launcher sera installé dans {}", self.home.display()),
                        "INSTALLER",
                        self.source.is_some(),
                    ),
                    Phase::Installing { .. } => (String::new(), "INSTALLATION…", false),
                    Phase::Done { version } => (format!("Launcher v{version} installé. Raccourci ajouté au Bureau."), "LANCER OTERIA", true),
                    Phase::Failed(_) => (String::new(), "RÉESSAYER", self.source.is_some()),
                };
                match &self.phase {
                    Phase::Installing { step, fraction } => {
                        let track = Rect::from_min_size(Pos2::new(x, full.top() + 246.0), Vec2::new(w, 6.0));
                        p.rect_filled(track, CornerRadius::same(3), c::RAISED);
                        p.rect_filled(Rect::from_min_size(track.min, Vec2::new((w * fraction).max(6.0), 6.0)), CornerRadius::same(3), c::ACCENT);
                        ui::text(&p, track.left_bottom() + Vec2::new(0.0, 10.0), Align2::LEFT_TOP, step, ui::medium(12.5), c::TEXT);
                        ui::text(&p, track.right_bottom() + Vec2::new(0.0, 10.0), Align2::RIGHT_TOP, &format!("{:.0} %", fraction * 100.0), ui::semi(12.5), c::CYAN);
                    }
                    Phase::Failed(e) => {
                        ui::wrapped(&p, Pos2::new(x, full.top() + 240.0), e, ui::medium(12.5), c::BAD, w, 3);
                    }
                    _ => {
                        let g = p.layout(sub.clone(), ui::regular(13.0), c::MUTED, w);
                        p.galley(Pos2::new(full.center().x - g.size().x / 2.0, full.top() + 246.0), g, c::MUTED);
                        if matches!(self.phase, Phase::Welcome) && self.source.is_none() {
                            ui::wrapped(&p, Pos2::new(x, full.top() + 268.0), "Aucune source de mise à jour trouvée : place manifest.json à côté de ce programme.", ui::medium(12.0), c::WARN, w, 2);
                        }
                    }
                }
                let resp = ui.interact(btn, ui.id().with("go"), if enabled { Sense::click() } else { Sense::hover() });
                let fill = if !enabled { c::RAISED } else if resp.hovered() { c::ACCENT_HOVER } else { c::ACCENT };
                p.rect_filled(btn, CornerRadius::same(12), fill);
                p.rect_stroke(btn, CornerRadius::same(12), Stroke::new(1.0_f32, ui::alpha(Color32::WHITE, 0.1)), egui::StrokeKind::Inside);
                ui::text(&p, btn.center(), Align2::CENTER_CENTER, label, ui::semi(19.0), if enabled { Color32::WHITE } else { c::MUTED });
                ui::text(&p, Pos2::new(full.center().x, full.bottom() - 22.0), Align2::CENTER_CENTER, "Fork d'Horizon (MIT) · pas un produit officiel Minecraft", ui::regular(11.0), c::FAINT);
                if resp.clicked() {
                    match self.phase {
                        Phase::Welcome | Phase::Failed(_) => self.start(),
                        Phase::Done { .. } => {
                            self.start_launcher();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        _ => {}
                    }
                }
            });
        ctx.request_repaint_after(Duration::from_millis(
            if matches!(self.phase, Phase::Installing { .. }) || self.shot.is_some() {
                16
            } else {
                200
            },
        ));
    }
}
