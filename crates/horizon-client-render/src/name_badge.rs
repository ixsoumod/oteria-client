//! Oteria's badge before an Oteria player's name, over their head and in the player list: its
//! logo in white on nothing, as Lunar's or Badlion's. `branding/make_logo.py` draws it and
//! `branding/make_icons.py` renders the texture; it is packed into the GUI atlas with the
//! controller art (`pad_glyph::images`) and drawn 8 text pixels square.

/// The atlas sprite, where the GUI's sprites are: name tags draw it by this name.
pub const SPRITE: &str = "gui/sprites/oteria_badge";
/// Its `blitSprite` id: what a [`crate::hud_layout::GuiDraw::Sprite`] names (the player list),
/// which looks it up under `gui/sprites/`.
pub const GUI_SPRITE: &str = "oteria_badge";
/// The size it is drawn at, in text pixels: a line of text's height.
pub const DRAWN: i32 = 8;

pub(crate) const PNG: &[u8] =
    include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_badge.png");

#[cfg(test)]
mod tests {
    use super::*;

    /// The player list draws a `GuiDraw::Sprite`, which `hud_layout::paint` looks up under
    /// `gui/sprites/`: its id must lead to the packed name, or the badge is silently dropped.
    #[test]
    fn the_player_lists_id_finds_the_packed_sprite() {
        assert_eq!(format!("gui/sprites/{GUI_SPRITE}"), SPRITE);
    }

    #[test]
    fn the_badge_is_packed_square_white_and_clear_around() {
        let (_, w, h, px) = crate::pad_glyph::images()
            .find(|i| i.0 == SPRITE)
            .expect("the badge is packed");
        assert_eq!((w, h), (32, 32), "four texels a text pixel");
        let at = |x: u32, y: u32| &px[((y * w + x) * 4) as usize..][..4];
        assert_eq!(at(0, 0)[3], 0, "the corners are clear");
        let drawn: Vec<_> = (0..w * h).map(|i| &px[(i * 4) as usize..][..4]).filter(|p| p[3] > 128).collect();
        assert!(drawn.len() > 100, "the waves are drawn");
        assert!(drawn.iter().all(|p| p[0] > 200 && p[1] > 200 && p[2] > 200), "in white");
    }
}
