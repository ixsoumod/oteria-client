//! The small window a launcher opens instead of itself when a newer launcher is out: Oteria's
//! logo, « Downloading Update », how far it is, and a green bar (350 × 450, the shape Lunar Client
//! uses). The launcher downloads its replacement here, with progress, then hands over to
//! `updater.exe self-update`, which swaps the exe and starts the new one. On failure it says so and
//! opens the launcher as it is (`--no-self-update`).
//!
//! It only moves by its top row, the one with the minimize dash.
//!
//! Testing: `OTERIA_UPDATE_POPUP_DEMO=<percent>` shows the window stopped at that percent and
//! changes nothing; with `OTERIA_LAUNCHER_SHOT=<png>` it saves a picture (and then closes, in
//! the demo).
//! `OTERIA_LANG=fr|en` picks the language.
use crate::ui::{self, Icons};
use crate::{install, lang, paths, Manifest, State};
use eframe::egui::{
    self, epaint::RectShape, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, Vec2,
};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

pub const SIZE: [f32; 2] = [350.0, 450.0];
const RADIUS: u8 = 10;
const BG: Color32 = Color32::from_rgb(0x19, 0x1a, 0x1f);
const EDGE: Color32 = Color32::from_rgb(0x2e, 0x30, 0x37);
const TITLE: Color32 = Color32::from_rgb(0xf4, 0xf5, 0xf7);
const GREY: Color32 = Color32::from_rgb(0x9c, 0x9f, 0xa8);
const BAD: Color32 = Color32::from_rgb(0xe0, 0x5a, 0x5a);
/// The bar: its empty track, its outline, and the fill's two ends.
const TRACK: Color32 = Color32::from_rgb(0x12, 0x2b, 0x1d);
const BAR_EDGE: Color32 = Color32::from_rgb(0x17, 0x6e, 0x3c);
const FILL_LEFT: Color32 = Color32::from_rgb(0x16, 0x9c, 0x4a);
const FILL_RIGHT: Color32 = Color32::from_rgb(0x2a, 0xd8, 0x6c);

pub struct Options {
    pub home: PathBuf,
    /// The launcher version being installed.
    pub version: String,
    /// The updater to hand over to (a copy, since it may replace itself).
    pub updater: PathBuf,
    /// `OTERIA_UPDATE_POPUP_DEMO`: stay at this percent and change nothing.
    pub demo: Option<f32>,
    pub shot: Option<PathBuf>,
}

/// What the download thread says.
enum Event {
    Progress(u64, u64),
    Done,
    Failed(String),
}

enum Stage {
    Downloading,
    /// Downloaded: the updater takes over and the window closes.
    Installing { since: Instant, handed_over: bool },
    Failed { message: String, since: Instant, reopened: bool },
}

struct Popup {
    opts: Options,
    french: bool,
    logo: TextureHandle,
    bar: TextureHandle,
    icons: Icons,
    rx: Option<Receiver<Event>>,
    stage: Stage,
    fraction: f32,
    frames: u32,
    shot_sent: bool,
}

/// Opens the popup; returns once it has closed (handed over, or failed and reopened the launcher).
pub fn run(opts: Options) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Oteria Launcher")
        .with_inner_size(SIZE)
        .with_resizable(false)
        .with_maximize_button(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_icon(ui::icon_data());
    eframe::run_native(
        "Oteria Launcher Update",
        eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(Popup::new(&cc.egui_ctx, opts)))),
    )
}

impl Popup {
    fn new(ctx: &egui::Context, opts: Options) -> Self {
        ui::install_fonts(ctx);
        // Rendered from the SVG at several times its drawn size: sharp on any screen scale.
        let px = ((104.0 * ctx.pixels_per_point().max(1.0) * 2.0) as u32).clamp(256, 768);
        let logo = ui::svg_texture(ctx, "update-logo", ui::LOGO_SVG, px, false)
            .or_else(|| ui::texture_from_png(ctx, "update-logo", ui::LOGO_PNG))
            .expect("logo");
        let rx = if opts.demo.is_some() {
            None
        } else {
            Some(download(opts.home.clone()))
        };
        Self {
            fraction: opts.demo.map_or(0.0, |p| (p / 100.0).clamp(0.0, 1.0)),
            opts,
            french: lang::french(),
            logo,
            bar: bar_texture(ctx),
            icons: Icons::load(ctx),
            rx,
            stage: Stage::Downloading,
            frames: 0,
            shot_sent: false,
        }
    }

    fn log(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.opts.home.join("launcher.log"))
        {
            let _ = writeln!(f, "[popup] {line}");
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            let mut ended = None;
            while let Ok(e) = rx.try_recv() {
                match e {
                    Event::Progress(done, total) => {
                        if total > 0 {
                            self.fraction = (done as f32 / total as f32).clamp(0.0, 1.0);
                        }
                    }
                    Event::Done => ended = Some(Ok(())),
                    Event::Failed(m) => ended = Some(Err(m)),
                }
            }
            match ended {
                Some(Ok(())) => {
                    self.rx = None;
                    self.fraction = 1.0;
                    self.log(&format!("launcher {} téléchargé", self.opts.version));
                    self.stage = Stage::Installing {
                        since: Instant::now(),
                        handed_over: false,
                    };
                }
                Some(Err(m)) => {
                    self.rx = None;
                    self.log(&format!("échec du téléchargement : {m}"));
                    self.stage = Stage::Failed {
                        message: m,
                        since: Instant::now(),
                        reopened: false,
                    };
                }
                None => {}
            }
        }
        match &mut self.stage {
            // A moment at 100 % so the full bar is seen, then the updater swaps the exe.
            Stage::Installing { since, handed_over } if !*handed_over
                && since.elapsed() > Duration::from_millis(500) =>
            {
                *handed_over = true;
                let spawned = install::quiet_command(&self.opts.updater)
                    .args(["self-update", "--wait-pid", &std::process::id().to_string()])
                    .current_dir(&self.opts.home)
                    .spawn();
                match spawned {
                    Ok(_) => {
                        self.log("mise à jour confiée à l'updater");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => {
                        let message = format!("updater.exe : {e}");
                        self.log(&message);
                        self.stage = Stage::Failed {
                            message,
                            since: Instant::now(),
                            reopened: false,
                        };
                    }
                }
            }
            // Long enough to read, then the launcher opens as it is.
            Stage::Failed {
                since, reopened, ..
            } if !*reopened && since.elapsed() > Duration::from_secs(4) =>
            {
                // Once: the window takes a frame or two to close.
                *reopened = true;
                if let Ok(me) = std::env::current_exe() {
                    let _ = std::process::Command::new(me)
                        .arg("--no-self-update")
                        .current_dir(&self.opts.home)
                        .spawn();
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
    }

    fn texts(&self) -> (String, String, Color32) {
        let pct = (self.fraction * 100.0).floor() as u32;
        let v = &self.opts.version;
        match (&self.stage, self.french) {
            (Stage::Failed { message, .. }, true) => (
                "Échec de la mise à jour".into(),
                format!("{message}\nOuverture du launcher…"),
                BAD,
            ),
            (Stage::Failed { message, .. }, false) => (
                "Update Failed".into(),
                format!("{message}\nOpening the launcher…"),
                BAD,
            ),
            (Stage::Installing { .. }, true) => (
                "Installation de la mise à jour".into(),
                format!("Le launcher {v} redémarre dans un instant…"),
                GREY,
            ),
            (Stage::Installing { .. }, false) => (
                "Installing Update".into(),
                format!("Launcher {v} is restarting in a moment..."),
                GREY,
            ),
            (Stage::Downloading, true) => (
                "Téléchargement de la mise à jour".into(),
                format!("La mise à jour du launcher vers {v} est en cours… {pct} %"),
                GREY,
            ),
            (Stage::Downloading, false) => (
                "Downloading Update".into(),
                format!("Launcher Update to {v} is currently in progress... {pct}%"),
                GREY,
            ),
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        let p = ui.painter().clone();
        p.rect(
            full,
            CornerRadius::same(RADIUS),
            BG,
            Stroke::new(1.0_f32, EDGE),
            StrokeKind::Inside,
        );

        // The top row: it moves the window, and holds the dash that minimizes it.
        let top = Rect::from_min_size(full.min, Vec2::new(full.width(), 40.0));
        let dash = Rect::from_center_size(
            Pos2::new(top.right() - 22.0, top.center().y),
            Vec2::new(30.0, 26.0),
        );
        let drag = ui.interact(
            Rect::from_min_max(top.min, Pos2::new(dash.left() - 2.0, top.bottom())),
            ui.id().with("popup-drag"),
            Sense::click_and_drag(),
        );
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let min = ui.interact(dash, ui.id().with("popup-min"), Sense::click());
        ui::hand(&min);
        if min.hovered() {
            ui::bubble(&p, dash.center(), ui::WINDOW_ICON + 16.0, 1.0);
        }
        if min.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let dash_color = if min.hovered() { Color32::WHITE } else { TITLE };
        self.icons.draw_fit(&p, "dash", dash.center(), ui::WINDOW_ICON, dash_color);

        // The logo.
        let logo = Rect::from_center_size(
            Pos2::new(full.center().x, full.top() + 128.0),
            Vec2::splat(116.0),
        );
        p.image(
            self.logo.id(),
            logo,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        // The title, shrunk to fit if a language makes it long, and the line under it.
        let (title, sub, sub_color) = self.texts();
        let inner = full.width() - 48.0;
        let mut size = 23.0;
        let mut galley = p.layout_no_wrap(title.clone(), ui::semi(size), TITLE);
        while galley.size().x > inner && size > 15.0 {
            size -= 0.5;
            galley = p.layout_no_wrap(title.clone(), ui::semi(size), TITLE);
        }
        let title_y = full.top() + 232.0;
        p.galley(
            Pos2::new(full.center().x - galley.size().x / 2.0, title_y - galley.size().y / 2.0),
            galley,
            TITLE,
        );
        let mut job = egui::text::LayoutJob::simple(sub, ui::regular(13.5), sub_color, inner);
        job.halign = egui::Align::Center;
        job.wrap.max_rows = 4;
        let sub = p.layout_job(job);
        p.galley(Pos2::new(full.center().x, title_y + 20.0), sub, sub_color);

        // The bar.
        let bar = Rect::from_min_size(
            Pos2::new(full.left() + 36.0, full.top() + 346.0),
            Vec2::new(full.width() - 72.0, 26.0),
        );
        p.rect(
            bar,
            CornerRadius::same(7),
            TRACK,
            Stroke::new(1.0_f32, BAR_EDGE),
            StrokeKind::Inside,
        );
        let k = self.fraction.clamp(0.0, 1.0);
        if k > 0.0 && !matches!(self.stage, Stage::Failed { .. }) {
            let inset = bar.shrink(2.0);
            // Never narrower than its own rounded ends.
            let w = (inset.width() * k).max(12.0);
            let fill = Rect::from_min_size(inset.min, Vec2::new(w, inset.height()));
            p.add(
                RectShape::filled(fill, CornerRadius::same(5), Color32::WHITE).with_texture(
                    self.bar.id(),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                ),
            );
        }
    }
}

/// The fill: green from left to right, a lighter band along its top as in Lunar's.
fn bar_texture(ctx: &egui::Context) -> TextureHandle {
    let (w, h) = (256usize, 32usize);
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        let t = y as f32 / (h - 1) as f32;
        // Top half a bit lighter, fading into the base colour.
        let light = ((0.5 - t) * 2.0).clamp(0.0, 1.0) * 0.22;
        for x in 0..w {
            let base = ui::lerp(FILL_LEFT, FILL_RIGHT, x as f32 / (w - 1) as f32);
            pixels.push(ui::lerp(base, Color32::WHITE, light));
        }
    }
    ctx.load_texture(
        "update-bar",
        egui::ColorImage {
            size: [w, h],
            source_size: Vec2::new(w as f32, h as f32),
            pixels,
        },
        egui::TextureOptions::LINEAR,
    )
}

/// Downloads the source's launcher to `OteriaLauncher.new.exe` (checked by SHA-256), which
/// `updater.exe self-update` then uses instead of downloading it again.
fn download(home: PathBuf) -> Receiver<Event> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = (|| {
            let state = State::load(&home);
            let source = state.source().ok_or("aucune source de mise à jour")?;
            let manifest = Manifest::load(&source)?;
            let dest = home.join(paths::LAUNCHER_NEW_EXE);
            let tx = tx.clone();
            install::fetch(&manifest, &manifest.launcher, &dest, &mut |done, total| {
                let _ = tx.send(Event::Progress(done, total));
            })
        })();
        let _ = tx.send(match result {
            Ok(()) => Event::Done,
            Err(e) => Event::Failed(e),
        });
    });
    rx
}

impl eframe::App for Popup {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ui::no_window_animations();
        self.tick(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| self.draw(ui));

        // Tests: a picture once the window has settled, then close.
        if let Some(path) = self.opts.shot.clone() {
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = shot {
                let r = ui::save_screenshot(&img, &path);
                self.log(&format!("capture {} : {r:?}", path.display()));
                self.opts.shot = None;
                // A real update carries on after its picture.
                if self.opts.demo.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            } else if !self.shot_sent {
                self.frames += 1;
                if self.frames > 10 {
                    self.shot_sent = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}
