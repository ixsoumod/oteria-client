//! The HUD editor (`docs/pvp.md`): the PvP & HUD widgets on the game, each in a frame the
//! player drags anywhere (snapping to the screen's edges and middle and to the other
//! widgets), resizes with the wheel and sends back to its corner with a right click. In the
//! middle, Oteria's glass toolbar: the hint, Reset and Done (the widgets' style is on the mods
//! page). Opened from the PvP & HUD page, the mods page or its key. Nothing here reaches the
//! server.
use super::{Game, Screen};
use crate::ui_theme::{Face, Ui};
use horizon_client::{
    hud::{HudBatch, rgba},
    lang,
    pvp_hud::{self, Layout, Place, WidgetId},
};

#[derive(Default)]
pub(super) struct HudEditor {
    /// The widget held, and where the cursor took it from its top left corner.
    drag: Option<(WidgetId, f32, f32)>,
    /// The lines the dragged box snapped to: a vertical one at x, a horizontal one at y.
    guides: (Option<f32>, Option<f32>),
    /// Opened from the PvP & HUD page: Done goes back there.
    from_options: bool,
    /// Opened from Oteria's mods page: Done goes back there.
    pub(super) to_oteria: bool,
    /// Where Reset and Done were drawn (GUI pixels), for clicks.
    buttons: [[f32; 4]; 2],
}

/// A frame's colour, and the held or hovered one's.
const FRAME: u32 = 0xB0FF_FFFF;
const FRAME_HOT: u32 = 0xFF2B_BFE9;
const GUIDE: u32 = 0xFF2B_BFE9;
/// The toolbar's glass and its Done, as `0xRRGGBB` (`oteria_menu::argb`).
const PANEL: u32 = 0x0B_1119;
const ACCENT: u32 = 0x2B_BFE9;

fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

impl Game<'_> {
    /// Opens the editor over the game, from the page (`from_options`) or the key.
    pub(super) fn open_hud_editor(&mut self, from_options: bool) {
        if self.player.is_none() || self.title.is_some() {
            return;
        }
        self.keys.clear();
        self.pvp.editor = Some(HudEditor {
            from_options,
            ..HudEditor::default()
        });
        self.screen = Screen::HudEditor;
        self.capture(false);
    }
    pub(super) fn close_hud_editor(&mut self) {
        let editor = self.pvp.editor.take();
        let from_options = editor.as_ref().is_some_and(|e| e.from_options);
        self.settings.save(&Self::options_path());
        if editor.is_some_and(|e| e.to_oteria) {
            self.screen = Screen::Playing;
            self.open_oteria_mods();
        } else if from_options {
            self.open_options_at(false, horizon_client::options_ui::Page::Pvp);
        } else {
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }
    /// The editor key, from the game.
    pub(super) fn hud_editor_key_press(&mut self, name: &str) -> bool {
        if self.screen != Screen::Playing
            || self.settings.extras.pvp.editor_key != name
            || name == "key.keyboard.unknown"
        {
            return false;
        }
        self.open_hud_editor(false);
        self.screen == Screen::HudEditor
    }
    /// Whether the HUD draws the editor's stand-ins for widgets that have nothing to show.
    pub(super) fn hud_editing(&self) -> bool {
        self.screen == Screen::HudEditor
    }
    /// The topmost widget under the cursor.
    fn widget_under_cursor(&self) -> Option<(WidgetId, [f32; 4])> {
        let (cx, cy) = self.cursor;
        self.pvp
            .boxes
            .iter()
            .rev()
            .copied()
            .find(|(_, [x, y, w, h])| cx >= *x && cx < x + w && cy >= *y && cy < y + h)
    }
    pub(super) fn draw_hud_editor(&mut self, batch: &mut HudBatch) {
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        let colour = |c: u32| {
            let [a, r, g, b] = c.to_be_bytes();
            rgba(r, g, b, a)
        };
        let held = self.pvp.editor.as_ref().and_then(|e| e.drag).map(|d| d.0);
        let hot = held.or(self.widget_under_cursor().map(|(id, _)| id));
        for &(id, [x, y, bw, bh]) in &self.pvp.boxes {
            let c = colour(if Some(id) == hot { FRAME_HOT } else { FRAME });
            batch.fill(atlas, x - 1.0, y - 1.0, bw + 2.0, 1.0, c);
            batch.fill(atlas, x - 1.0, y + bh, bw + 2.0, 1.0, c);
            batch.fill(atlas, x - 1.0, y, 1.0, bh, c);
            batch.fill(atlas, x + bw, y, 1.0, bh, c);
        }
        if let Some(e) = &self.pvp.editor {
            if let Some(x) = e.guides.0 {
                batch.fill(atlas, x, 0.0, 1.0, h, colour(GUIDE));
            }
            if let Some(y) = e.guides.1 {
                batch.fill(atlas, 0.0, y, w, 1.0, colour(GUIDE));
            }
        }
        // The toolbar, in Oteria's style: the hint over Reset and Done.
        let argb = super::oteria_menu::argb;
        let mut ui = Ui::new(atlas, batch, self.cursor);
        let (d, scale) = (ui.density, ui.batch.scale);
        let gui = |v: f32| v * d / scale;
        let hint = lang::get_or(
            "oteria.hud.hint",
            "Drag the widgets · Wheel: size · Right click: back to its corner",
        );
        let (bw, bh, gap) = (150.0, 34.0, 10.0);
        let pw = (ui.measure(Face::Regular, 12.0, &hint) + 40.0).max(2.0 * bw + gap + 32.0);
        let ph = 96.0;
        let (px, py) = ((ui.w - pw) / 2.0, (ui.h - ph) / 2.0);
        ui.rounded(px, py, pw, ph, 12.0, argb(PANEL, 0.82));
        ui.border(px, py, pw, ph, 12.0, argb(0xFF_FFFF, 0.08));
        ui.text_centered(Face::Regular, 12.0, px + pw / 2.0, py + 14.0, argb(0xFF_FFFF, 0.7), &hint);
        let cx = px + pw / 2.0;
        let places = [
            [cx - gap / 2.0 - bw, py + 44.0, bw, bh],
            [cx + gap / 2.0, py + 44.0, bw, bh],
        ];
        let labels = [
            lang::get_or("oteria.hud.reset", "Reset"),
            lang::get_or("oteria.hud.done", "Done"),
        ];
        for (k, (r, label)) in places.iter().zip(&labels).enumerate() {
            let over = inside(*r, ui.cursor);
            let fill = match k {
                0 => argb(0xFF_FFFF, if over { 0.16 } else { 0.08 }),
                _ => argb(ACCENT, if over { 0.95 } else { 0.78 }),
            };
            ui.rounded(r[0], r[1], r[2], r[3], 8.0, fill);
            ui.text_centered(Face::Medium, 12.0, r[0] + r[2] / 2.0, r[1] + 9.0, argb(0xFF_FFFF, 1.0), label);
            horizon_client::widgets::widget_cursor(over, true);
        }
        ui.finish();
        let rects = places.map(|r| r.map(gui));
        horizon_client::widgets::record("oteria-hud-reset", rects[0], true);
        horizon_client::widgets::record("oteria-hud-done", rects[1], true);
        if let Some(e) = &mut self.pvp.editor {
            e.buttons = rects;
        }
    }
    /// A press: a button, else the widget under the cursor (right button: back to its corner).
    pub(super) fn hud_editor_press(&mut self, right: bool) {
        let pressed = self
            .pvp
            .editor
            .as_ref()
            .and_then(|e| e.buttons.iter().position(|r| inside(*r, self.cursor)));
        match pressed {
            Some(_) if right => return,
            Some(0) => {
                self.audio.play_button_click();
                self.settings.extras.pvp.layout = Layout::default();
                self.settings.save(&Self::options_path());
                return;
            }
            Some(_) => {
                self.audio.play_button_click();
                self.close_hud_editor();
                return;
            }
            None => {}
        }
        let Some((id, [x, y, _, _])) = self.widget_under_cursor() else {
            return;
        };
        if right {
            *self.settings.extras.pvp.layout.get_mut(id) = Place::default();
            self.settings.save(&Self::options_path());
        } else if let Some(e) = &mut self.pvp.editor {
            e.drag = Some((id, self.cursor.0 - x, self.cursor.1 - y));
        }
    }
    /// The cursor moved: the held widget follows, snapping to edges near it.
    pub(super) fn hud_editor_drag(&mut self) {
        let Some((id, dx, dy)) = self.pvp.editor.as_ref().and_then(|e| e.drag) else {
            return;
        };
        let Some(&(_, [_, _, w, h])) = self.pvp.boxes.iter().find(|(b, _)| *b == id) else {
            return;
        };
        let screen = self.gui_size();
        let others = || self.pvp.boxes.iter().filter(|(b, _)| *b != id);
        let xs: Vec<(f32, f32)> = others().map(|(_, r)| (r[0], r[2])).collect();
        let ys: Vec<(f32, f32)> = others().map(|(_, r)| (r[1], r[3])).collect();
        let (x, gx) = pvp_hud::snap(self.cursor.0 - dx, w, screen.0, &xs);
        let (y, gy) = pvp_hud::snap(self.cursor.1 - dy, h, screen.1, &ys);
        let at = pvp_hud::to_fractions((x, y), (w, h), screen);
        self.settings.extras.pvp.layout.get_mut(id).at = Some(at);
        if let Some(e) = &mut self.pvp.editor {
            e.guides = (gx, gy);
        }
    }
    pub(super) fn hud_editor_release(&mut self) {
        if let Some(e) = &mut self.pvp.editor
            && e.drag.take().is_some()
        {
            e.guides = (None, None);
            self.settings.save(&Self::options_path());
        }
    }
    /// The wheel over a widget resizes it by 10%.
    pub(super) fn hud_editor_scroll(&mut self, amount: f64) {
        let Some((id, _)) = self.widget_under_cursor() else {
            return;
        };
        let place = self.settings.extras.pvp.layout.get_mut(id);
        let (lo, hi) = pvp_hud::SCALES;
        place.scale = if amount > 0.0 {
            place.scale.saturating_add(10).min(hi)
        } else {
            place.scale.saturating_sub(10).max(lo)
        };
        self.settings.save(&Self::options_path());
    }
}
