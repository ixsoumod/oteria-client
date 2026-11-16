//! The HUD editor (`docs/pvp.md`): the PvP & HUD widgets on the game, each in a frame the
//! player drags anywhere (snapping to the screen's edges and middle and to the other
//! widgets), resizes by a corner's square (proportionally, between `pvp_hud::SCALES`) or with
//! the wheel, and sends back to its corner with a right click. At the top, one line says
//! Escape saves; it goes back to the game, wherever the editor was opened from (the PvP & HUD
//! page, Oteria's menu or its key). Nothing here reaches the server.
use super::{Game, Screen};
use crate::ui_theme::{Face, Ui};
use horizon_client::{
    hud::{HudBatch, rgba},
    lang,
    pvp_hud::{self, Place, WidgetId},
};

#[derive(Default)]
pub(super) struct HudEditor {
    /// The widget held, and where the cursor took it from its top left corner.
    drag: Option<(WidgetId, f32, f32)>,
    /// The lines the dragged box snapped to: a vertical one at x, a horizontal one at y.
    guides: (Option<f32>, Option<f32>),
    /// A corner held: the widget, its opposite corner (which stays), and its size at 100%.
    resize: Option<(WidgetId, (f32, f32), (f32, f32))>,
}

/// The corners' squares: their side, and how far from a corner a press still takes it.
const HANDLE: f32 = 3.0;
const HANDLE_REACH: f32 = 3.5;

/// The corner of `[x, y, w, h]` near `(px, py)`, and the corner across from it.
fn corner_near([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
    [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
        .into_iter()
        .find(|&(cx, cy)| (px - cx).abs() <= HANDLE_REACH && (py - cy).abs() <= HANDLE_REACH)
        .map(|(cx, cy)| ((cx, cy), (2.0 * x + w - cx, 2.0 * y + h - cy)))
}

/// The scale (percent, within `SCALES`) a box of `base` size takes to reach `cursor` from
/// `anchor`, keeping its proportions; and its top left there.
fn resized(
    anchor: (f32, f32),
    base: (f32, f32),
    cursor: (f32, f32),
) -> (u8, (f32, f32), (f32, f32)) {
    let (lo, hi) = pvp_hud::SCALES;
    let (dx, dy) = (cursor.0 - anchor.0, cursor.1 - anchor.1);
    let k = (dx.abs() / base.0.max(1.0)).max(dy.abs() / base.1.max(1.0));
    let scale = (k * 100.0).round().clamp(f32::from(lo), f32::from(hi)) as u8;
    let size = (
        base.0 * f32::from(scale) / 100.0,
        base.1 * f32::from(scale) / 100.0,
    );
    // The held corner goes where the cursor is; the opposite one stays.
    let x = if dx < 0.0 {
        anchor.0 - size.0
    } else {
        anchor.0
    };
    let y = if dy < 0.0 {
        anchor.1 - size.1
    } else {
        anchor.1
    };
    (scale, (x, y), size)
}

/// A frame's colour, and the held or hovered one's.
const FRAME: u32 = 0xB0FF_FFFF;
const FRAME_HOT: u32 = 0xFF2B_BFE9;
const GUIDE: u32 = 0xFF2B_BFE9;

impl Game<'_> {
    /// Opens the editor over the game (from the PvP & HUD page, Oteria's menu or the key).
    pub(super) fn open_hud_editor(&mut self, _from_options: bool) {
        if self.player.is_none() || self.title.is_some() {
            return;
        }
        self.keys.clear();
        self.pvp.editor = Some(HudEditor::default());
        self.screen = Screen::HudEditor;
        self.capture(false);
    }
    /// Escape: the layout saved, and back to the game.
    pub(super) fn close_hud_editor(&mut self) {
        self.pvp.editor = None;
        self.settings.save(&Self::options_path());
        self.screen = Screen::Playing;
        self.capture(true);
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
        let resizing = self.pvp.editor.as_ref().and_then(|e| e.resize).map(|r| r.0);
        let hot = resizing.or(hot);
        for &(id, [x, y, bw, bh]) in &self.pvp.boxes {
            let c = colour(if Some(id) == hot { FRAME_HOT } else { FRAME });
            batch.fill(atlas, x - 1.0, y - 1.0, bw + 2.0, 1.0, c);
            batch.fill(atlas, x - 1.0, y + bh, bw + 2.0, 1.0, c);
            batch.fill(atlas, x - 1.0, y, 1.0, bh, c);
            batch.fill(atlas, x + bw, y, 1.0, bh, c);
            // A square on each corner: drag one to resize.
            for (cx, cy) in [(x, y), (x + bw, y), (x, y + bh), (x + bw, y + bh)] {
                let over =
                    corner_near([x, y, bw, bh], self.cursor).is_some_and(|(at, _)| at == (cx, cy));
                let h = if over { HANDLE + 1.0 } else { HANDLE };
                let (hx, hy) = (cx - h / 2.0 - 0.5, cy - h / 2.0 - 0.5);
                batch.fill(
                    atlas,
                    hx - 0.5,
                    hy - 0.5,
                    h + 1.0,
                    h + 1.0,
                    rgba(0, 0, 0, 160),
                );
                batch.fill(
                    atlas,
                    hx,
                    hy,
                    h,
                    h,
                    colour(if Some(id) == hot {
                        FRAME_HOT
                    } else {
                        0xFFFF_FFFF
                    }),
                );
            }
        }
        if let Some(e) = &self.pvp.editor {
            if let Some(x) = e.guides.0 {
                batch.fill(atlas, x, 0.0, 1.0, h, colour(GUIDE));
            }
            if let Some(y) = e.guides.1 {
                batch.fill(atlas, 0.0, y, w, 1.0, colour(GUIDE));
            }
        }
        // At the top, one line: Escape saves; under it, how to move them, dimmer. Each with a
        // shadow, over whatever the world shows.
        let argb = super::oteria_menu::argb;
        let mut ui = Ui::new(atlas, batch, self.cursor);
        let save = lang::get_or("oteria.hud.esc", "Press Esc to save");
        let hint = lang::get_or(
            "oteria.hud.hint",
            "Drag the widgets · A corner or the wheel: size · Right click: back to its corner",
        );
        let cx = ui.w / 2.0;
        for (dy, alpha) in [(1.0, 0.55), (0.0, 1.0)] {
            let colour = if dy > 0.0 {
                argb(0x00_0000, alpha)
            } else {
                argb(0xFF_FFFF, alpha)
            };
            ui.text_centered(Face::SemiBold, 15.0, cx + dy, 18.0 + dy, colour, &save);
        }
        for (dy, alpha) in [(1.0, 0.45), (0.0, 0.75)] {
            let colour = if dy > 0.0 {
                argb(0x00_0000, alpha)
            } else {
                argb(0xFF_FFFF, alpha)
            };
            ui.text_centered(Face::Regular, 11.0, cx + dy, 42.0 + dy, colour, &hint);
        }
        ui.finish();
    }
    /// A press on the widget under the cursor (right button: back to its corner).
    pub(super) fn hud_editor_press(&mut self, right: bool) {
        // A corner's square: resizing, around the opposite corner.
        if !right {
            let held = self.pvp.boxes.iter().rev().find_map(|&(id, rect)| {
                let (_, anchor) = corner_near(rect, self.cursor)?;
                let scale = f32::from(self.settings.extras.pvp.layout.get(id).scale) / 100.0;
                Some((id, anchor, (rect[2] / scale, rect[3] / scale)))
            });
            if let Some(held) = held {
                if let Some(e) = &mut self.pvp.editor {
                    e.resize = Some(held);
                }
                return;
            }
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
        if let Some((id, anchor, base)) = self.pvp.editor.as_ref().and_then(|e| e.resize) {
            let (scale, at, size) = resized(anchor, base, self.cursor);
            let screen = self.gui_size();
            let place = self.settings.extras.pvp.layout.get_mut(id);
            place.scale = scale;
            place.at = Some(pvp_hud::to_fractions(at, size, screen));
            return;
        }
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
            && (e.drag.take().is_some() | e.resize.take().is_some())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_resizes_around_the_opposite_one_in_proportion() {
        let rect = [10.0, 10.0, 40.0, 20.0];
        // The bottom right corner, held: the top left stays.
        let (held, anchor) = corner_near(rect, (49.0, 31.0)).unwrap();
        assert_eq!((held, anchor), ((50.0, 30.0), (10.0, 10.0)));
        let (scale, at, size) = resized(anchor, (40.0, 20.0), (90.0, 30.0));
        assert_eq!((scale, at, size), (200, (10.0, 10.0), (80.0, 40.0)));
        // The top left, dragged in: smaller, its bottom right where it was.
        let (scale, at, _) = resized((50.0, 30.0), (40.0, 20.0), (30.0, 20.0));
        assert_eq!((scale, at), (50, (30.0, 20.0)));
        // Never past the limits.
        assert_eq!(
            resized((0.0, 0.0), (40.0, 20.0), (1.0, 1.0)).0,
            pvp_hud::SCALES.0
        );
        assert_eq!(
            resized((0.0, 0.0), (40.0, 20.0), (900.0, 1.0)).0,
            pvp_hud::SCALES.1
        );
        assert!(
            corner_near(rect, (30.0, 20.0)).is_none(),
            "the middle is a move"
        );
    }
}
