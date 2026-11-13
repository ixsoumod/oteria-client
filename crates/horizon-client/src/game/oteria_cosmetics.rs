//! The Cosmetics page of Oteria's menu (its t-shirt button): Oteria's cosmetics, free for every
//! player, each a card with its picture, its name, what it does and Equip / Remove. For now the
//! butterfly knife (`oteria_knife`). Only this client draws them for now: others will see them once
//! Oteria's cosmetics service runs.

use crate::ui_theme::{Face, Ui};
use horizon_client::lang;
use horizon_client::oteria_art;

const PANEL: u32 = 0x0B_1119;
const WHITE: u32 = 0xFF_FFFF;
const ACCENT: u32 = 0x2B_BFE9;
const OFF: u32 = 0xB3_3346;

fn argb(rgb: u32, alpha: f32) -> u32 {
    super::oteria_menu::argb(rgb, alpha)
}
fn inside([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
fn tr(key: &str, english: &str) -> String {
    lang::get_or(key, english)
}

/// What the page shows of the player's cosmetics.
pub(super) struct CosmeticsView {
    pub knife: bool,
    /// The inspect key's name, as the keyboard shows it.
    pub inspect_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CosmeticsAction {
    ToggleKnife,
}

#[derive(Default)]
pub(super) struct CosmeticsMenu {
    hits: Vec<([f32; 4], CosmeticsAction)>,
    hover: f32,
}

impl CosmeticsMenu {
    pub(super) fn click(&mut self, cursor: (f32, f32)) -> Option<CosmeticsAction> {
        self.hits.iter().rev().find(|(r, _)| inside(*r, cursor)).map(|h| h.1)
    }

    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        view: &CosmeticsView,
        alpha: f32,
        rise: f32,
        dt: f32,
        gui: &dyn Fn(f32) -> f32,
    ) {
        self.hits.clear();
        let a = alpha;
        let pointer = ui.cursor;
        let (pw, ph) = (520.0, 330.0);
        let (px, py) = ((ui.w - pw) / 2.0, (ui.h - ph) / 2.0 + rise);
        ui.rounded(px, py, pw, ph, 14.0, argb(PANEL, 0.86 * a));
        ui.border(px, py, pw, ph, 14.0, argb(WHITE, 0.08 * a));
        let title = tr("oteria.cosmetics.title", "Cosmetics");
        ui.text_centered(Face::SemiBold, 15.0, px + pw / 2.0, py + 14.0, argb(WHITE, a), &title);
        let free = tr("oteria.cosmetics.free", "Free for every Oteria player.");
        ui.text_centered(Face::Regular, 11.0, px + pw / 2.0, py + 36.0, argb(WHITE, 0.5 * a), &free);

        // The knife's card.
        let card = [px + 22.0, py + 66.0, pw - 44.0, 200.0];
        let over = inside(card, pointer);
        self.hover += ((if over { 1.0 } else { 0.0 }) - self.hover) * (1.0 - (-dt * 14.0).exp());
        let lit = if view.knife { ACCENT } else { WHITE };
        let fill = (if view.knife { 0.14 } else { 0.05 }) + 0.04 * self.hover;
        ui.rounded(card[0], card[1], card[2], card[3], 12.0, argb(lit, fill * a));
        ui.border(card[0], card[1], card[2], card[3], 12.0, argb(lit, if view.knife { 0.6 } else { 0.1 } * a));
        // Its picture, a texel to a dp, on whole pixels.
        let (pic_w, pic_h) = (192.0, 96.0);
        let (ix, iy) = (card[0] + 18.0, card[1] + (card[3] - pic_h) / 2.0);
        ui.rounded(ix - 6.0, iy - 6.0, pic_w + 12.0, pic_h + 12.0, 10.0, argb(0x00_0000, 0.3 * a));
        if let Some(r) = ui.atlas.sprites.get(oteria_art::KNIFE_PREVIEW).copied() {
            let k = ui.batch.scale;
            let snap = |v: f32| (gui(v) * k).round() / k;
            ui.batch.rect_uv(snap(ix), snap(iy), gui(pic_w), gui(pic_h), r, argb(WHITE, a));
        }
        let tx = ix + pic_w + 24.0;
        let name = tr("oteria.cosmetics.knife", "Oteria butterfly knife");
        ui.text(Face::SemiBold, 15.0, tx, card[1] + 26.0, argb(WHITE, a), &name);
        let kind = tr("oteria.cosmetics.knife.kind", "Sword skin");
        ui.text(Face::Medium, 11.0, tx, card[1] + 50.0, argb(ACCENT, a), &kind);
        let about = tr(
            "oteria.cosmetics.knife.about",
            "Your swords become Oteria's butterfly knife in your hand.",
        );
        ui.text(Face::Regular, 11.0, tx, card[1] + 74.0, argb(WHITE, 0.7 * a), &about);
        let inspect = tr("oteria.cosmetics.knife.inspect", "Press %s to inspect it.").replacen("%s", &view.inspect_key, 1);
        ui.text(Face::Regular, 11.0, tx, card[1] + 92.0, argb(WHITE, 0.7 * a), &inspect);
        // Equip / Remove.
        let b = [tx, card[1] + card[3] - 22.0 - 38.0, 160.0, 38.0];
        let bover = inside(b, pointer);
        let (label, colour) = if view.knife {
            (tr("oteria.cosmetics.remove", "Remove"), OFF)
        } else {
            (tr("oteria.cosmetics.equip", "Equip"), ACCENT)
        };
        ui.rounded(b[0], b[1], b[2], b[3], 9.0, argb(colour, if bover { 1.0 } else { 0.82 } * a));
        ui.text_centered(Face::Medium, 12.0, b[0] + b[2] / 2.0, b[1] + (b[3] - 17.0) / 2.0, argb(WHITE, a), &label);
        horizon_client::widgets::widget_cursor(bover, true);
        self.hits.push((b.map(gui), CosmeticsAction::ToggleKnife));
        horizon_client::widgets::record("oteria-cosmetics-knife", b.map(gui), true);

        let soon = tr("oteria.cosmetics.soon", "More cosmetics are coming. Others will see yours once Oteria's servers are up.");
        ui.text_centered(Face::Regular, 11.0, px + pw / 2.0, py + ph - 34.0, argb(WHITE, 0.45 * a), &soon);
    }
}
