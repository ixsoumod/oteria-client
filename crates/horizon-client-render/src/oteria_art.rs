//! The right-shift menu's art (`game/oteria_menu.rs`): Oteria's logo in white and its buttons'
//! icons, from `branding/` (`make_icons.py`). Packed into the GUI atlas beside the controller art
//! and drawn one texel to a screen pixel, so they stay sharp at any GUI scale.

/// The logo over the menu's buttons, 160 texels square.
pub const LOGO: &str = "gui/sprites/oteria_logo";
/// The Boost button's rocket, 32 texels square.
pub const ROCKET: &str = "gui/sprites/oteria_rocket";
/// The Cosmetics button's t-shirt, 32 texels square.
pub const SHIRT: &str = "gui/sprites/oteria_shirt";

/// Elytra Indicator's icon, 16 texels square (from the ElytraIndicator mod, MIT:
/// `THIRD_PARTY_NOTICES.md`), white and see-through: drawn in the crosshair's colour.
pub const ELYTRA: &str = "gui/sprites/oteria_elytra";

/// Oteria's butterfly knife (`game/oteria_knife.rs`): its 64x64 texture, read by the hand
/// renderer, and the Cosmetics page's 192x96 picture of it
/// (`branding/cosmetics/make_balisong.py`).
pub const KNIFE: &str = "gui/sprites/oteria_balisong";
pub const KNIFE_PREVIEW: &str = "gui/sprites/oteria_balisong_preview";
/// A knife converted on this computer into `private/cosmetics/` (not in git: models licensed to
/// whoever downloaded them, `branding/cosmetics/convert_glb_balisong.py`): its texture, when the
/// game starts in the repository and finds it.
pub const KNIFE_PRIVATE: &str = "gui/sprites/oteria_balisong_private";
pub const KNIFE_PRIVATE_FILE: &str = "private/cosmetics/balisong_leopham";

/// The converted knife's file with `extension`, when there is one: under the folder the game
/// runs in, beside the executable, or in the repository the executable was built in (two folders
/// up from `target/<profile>/`), as `oteria.cmd` runs it.
pub fn knife_private_path(extension: &str) -> Option<std::path::PathBuf> {
    let file = format!("{KNIFE_PRIVATE_FILE}.{extension}");
    let exe = std::env::current_exe().ok();
    let dirs = [
        std::env::current_dir().ok(),
        exe.as_ref().and_then(|e| e.parent()).map(|d| d.to_path_buf()),
        exe.as_ref().and_then(|e| e.ancestors().nth(3)).map(|d| d.to_path_buf()),
    ];
    dirs.into_iter().flatten().map(|d| d.join(&file)).find(|p| p.is_file())
}

const ART: [(&str, &[u8]); 6] = [
    (
        KNIFE,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_balisong.png"),
    ),
    (
        KNIFE_PREVIEW,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_balisong_preview.png"),
    ),
    (
        ELYTRA,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_elytra.png"),
    ),
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

/// Each `(sprite, width, height, RGBA texels)`, for the atlas: this art, then the mod menu's
/// icons (`crate::oteria_icons`).
pub fn images() -> impl Iterator<Item = (String, u32, u32, Vec<u8>)> {
    let private = knife_private_path("png")
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|png| crate::pad_glyph::decode(&png))
        .map(|(w, h, pixels)| (KNIFE_PRIVATE.to_owned(), w, h, pixels));
    ART.iter()
        .chain(crate::oteria_icons::ALL.iter())
        .map(|&(name, png)| {
            let (w, h, pixels) = crate::pad_glyph::decode(png).expect("Oteria's menu art is PNG");
            (name.to_owned(), w, h, pixels)
        })
        .chain(private)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_art_is_white_on_nothing_at_its_sizes() {
        let sizes = [(LOGO, 160), (ROCKET, 32), (SHIRT, 32), (ELYTRA, 16)];
        for (name, w, h, px) in images() {
            // The knife's texture and picture are in colour, at their own sizes.
            if name == KNIFE || name == KNIFE_PREVIEW {
                assert_eq!((w, h), if name == KNIFE { (64, 64) } else { (192, 96) }, "{name}");
                continue;
            }
            // The mod menu's icons are 32 texels square too, their small ones 20.
            let small = if name.contains("_small/") { 20 } else { 32 };
            let side = sizes.iter().find(|s| s.0 == name).map_or(small, |s| s.1);
            assert_eq!((w, h), (side, side), "{name}");
            assert_eq!(px[3], 0, "{name}: its corner is clear");
            // The elytra is drawn see-through (its texels at 155 of 255).
            let drawn: Vec<_> = px.chunks(4).filter(|p| p[3] > 128).collect();
            // Outline icons are thin lines: a twenty-fifth of the square is plenty.
            assert!(drawn.len() > (w * h / 25) as usize, "{name}: drawn");
            assert!(drawn.iter().all(|p| p[0] > 200 && p[1] > 200 && p[2] > 200), "{name}: white");
        }
    }
}
