//! A looping H.264 video drawn as an egui texture: the launcher's banner background.
//!
//! The clip is a raw H.264 stream (Annex B, Constrained Baseline, no B-frames), made with ffmpeg
//! from the recording (`tools/oteria/make_menu_loop.ps1`). A thread decodes it with OpenH264 one
//! frame ahead of the window (a small bounded queue, so it waits rather than racing ahead), and the
//! window uploads a frame whenever its time has come. At the end it starts over.
use eframe::egui::{self, TextureHandle};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use std::sync::mpsc::{sync_channel, Receiver};
use std::time::{Duration, Instant};

struct Frame {
    size: [usize; 2],
    rgba: Vec<u8>,
}

pub struct VideoLoop {
    rx: Receiver<Frame>,
    texture: Option<TextureHandle>,
    /// When the next frame is due.
    due: Instant,
    frame: Duration,
}

impl VideoLoop {
    /// Starts decoding `stream` (played at `fps`) in the background.
    pub fn start(stream: &'static [u8], fps: f32) -> Self {
        let (tx, rx) = sync_channel::<Frame>(3);
        std::thread::Builder::new()
            .name("menu-video".into())
            .spawn(move || loop {
                let Ok(mut decoder) = Decoder::new() else { return };
                let mut any = false;
                for nal in openh264::nal_units(stream) {
                    let Ok(Some(yuv)) = decoder.decode(nal) else {
                        continue;
                    };
                    let (w, h) = yuv.dimensions();
                    let mut rgba = vec![0; w * h * 4];
                    yuv.write_rgba8(&mut rgba);
                    any = true;
                    // The window is gone: so is this thread.
                    if tx.send(Frame { size: [w, h], rgba }).is_err() {
                        return;
                    }
                }
                if !any {
                    return;
                }
            })
            .ok();
        Self {
            rx,
            texture: None,
            due: Instant::now(),
            frame: Duration::from_secs_f32(1.0 / fps.max(1.0)),
        }
    }

    /// This moment's frame (none until the first one is decoded), and asks for a repaint when the
    /// next one is due.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<&TextureHandle> {
        let now = Instant::now();
        if now >= self.due {
            if let Ok(f) = self.rx.try_recv() {
                let image = egui::ColorImage::from_rgba_unmultiplied(f.size, &f.rgba);
                match &mut self.texture {
                    Some(t) => t.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.texture =
                            Some(ctx.load_texture("menu-video", image, egui::TextureOptions::LINEAR))
                    }
                }
                // Keep the pace, but never try to catch up on a long stall.
                self.due = (self.due + self.frame).max(now);
            }
        }
        ctx.request_repaint_after(
            self.due
                .saturating_duration_since(now)
                .max(Duration::from_millis(4)),
        );
        self.texture.as_ref()
    }
}
