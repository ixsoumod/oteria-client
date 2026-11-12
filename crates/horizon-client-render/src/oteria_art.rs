//! The right-shift menu's art (`game/oteria_menu.rs`): Oteria's logo in white and its buttons'
//! icons, from `branding/` (`make_icons.py`). Packed into the GUI atlas beside the controller art
//! and drawn one texel to a screen pixel, so they stay sharp at any GUI scale.

/// The logo over the menu's buttons, 160 texels square.
pub const LOGO: &str = "gui/sprites/oteria_logo";
/// The Boost button's rocket, 32 texels square.
pub const ROCKET: &str = "gui/sprites/oteria_rocket";
/// The Cosmetics button's t-shirt, 32 texels square.
pub const SHIRT: &str = "gui/sprites/oteria_shirt";

const ART: [(&str, &[u8]); 3] = [
    (
        LOGO,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_logo.png"),
    ),
    (
        ROCKET,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_rocket.png"),
    ),
    (
        SHIRT,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_shirt.png"),
    ),
];

/// Each `(sprite, width, height, RGBA texels)`, for the atlas.
pub fn images() -> impl Iterator<Item = (String, u32, u32, Vec<u8>)> {
    ART.iter().map(|&(name, png)| {
        let (w, h, pixels) = crate::pad_glyph::decode(png).expect("Oteria's menu art is PNG");
        (name.to_owned(), w, h, pixels)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_art_is_white_on_nothing_at_its_sizes() {
        let sizes = [(LOGO, 160), (ROCKET, 32), (SHIRT, 32)];
        for (name, w, h, px) in images() {
            let side = sizes.iter().find(|s| s.0 == name).expect("known art").1;
            assert_eq!((w, h), (side, side), "{name}");
            assert_eq!(px[3], 0, "{name}: its corner is clear");
            let drawn: Vec<_> = px.chunks(4).filter(|p| p[3] > 128).collect();
            assert!(drawn.len() > (w * h / 10) as usize, "{name}: drawn");
            assert!(drawn.iter().all(|p| p[0] > 200 && p[1] > 200 && p[2] > 200), "{name}: white");
        }
    }
}
