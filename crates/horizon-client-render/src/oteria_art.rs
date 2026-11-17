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
/// AppleSkin's icons (public domain: `THIRD_PARTY_NOTICES.md`), the top 81x45 texels of its
/// `icons.png`: the saturation outlines (row 0), the exhaustion bar (row 18).
pub const APPLESKIN: &str = "gui/sprites/oteria_appleskin";
pub const APPLESKIN_SIZE: (u32, u32) = (81, 45);

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
/// Knives rigged with their arms and animated (Bedrock geometry and animations, as gun packs
/// ship them: `game/oteria_knife_rig.rs`), each copied on this computer into
/// `private/cosmetics/` (not in git, like the knife above): `<name>_rig.geo.json`,
/// `<name>_rig.animation.json`, its texture `<name>_rig.png` and its picture for the Cosmetics
/// page `<name>_rig.icon.png`. The one chosen there, when it is there, is the knife drawn.
pub const KNIFE_RIGS: [&str; 6] = ["butterfly", "karambit", "skeleton", "stiletto", "talon", "push"];
/// Oteria's own wearables (`game/oteria_wear.rs`), drawn on the player in every view: a cape
/// that hangs and flies as cloth, wings, and an aura. Each comes in styles: an id, its English
/// name, and its two colours (`0xRRGGBB`): the main one and its light.
#[derive(Debug, PartialEq, Eq)]
pub struct WearStyle {
    pub id: &'static str,
    pub english: &'static str,
    pub main: u32,
    pub light: u32,
}
const fn style(id: &'static str, english: &'static str, main: u32, light: u32) -> WearStyle {
    WearStyle { id, english, main, light }
}
pub const CAPES: [WearStyle; 4] = [
    style("oteria", "Oteria cape", 0x5B_2BB5, 0xC9_A2FF),
    style("crimson", "Crimson cape", 0x8E_1328, 0xFF_7A86),
    style("midnight", "Midnight cape", 0x14_2240, 0x6F_C4FF),
    style("gold", "Royal cape", 0x7A_4A06, 0xFF_D866),
];
pub const WINGS: [WearStyle; 3] = [
    style("dragon", "Dragon wings", 0x3A_1466, 0xB0_6CFF),
    style("angel", "Angel wings", 0xC8_D4E8, 0xFF_FFFF),
    style("demon", "Demon wings", 0x2A_0A0E, 0xE0_2A3A),
];
pub const AURAS: [WearStyle; 3] = [
    style("violet", "Violet aura", 0x8A_3CFF, 0xE2_C6FF),
    style("frost", "Frost aura", 0x2F_9BFF, 0xD6_F3FF),
    style("ember", "Ember aura", 0xFF_5A1F, 0xFF_E08A),
];
/// The style of `id` in `all`.
pub fn wear_style(all: &'static [WearStyle], id: &str) -> Option<&'static WearStyle> {
    all.iter().find(|s| s.id == id)
}

/// A rigged knife's texture's sprite.
pub fn knife_rig_sprite(name: &str) -> String {
    format!("gui/sprites/oteria_knife_rig/{name}")
}
/// A rigged knife's picture's sprite.
pub fn knife_rig_icon(name: &str) -> String {
    format!("gui/sprites/oteria_knife_rig/{name}_icon")
}

/// The converted knife's file with `extension`, when there is one: under the folder the game
/// runs in, beside the executable, or in the repository the executable was built in (two folders
/// up from `target/<profile>/`), as `oteria.cmd` runs it.
pub fn knife_private_path(extension: &str) -> Option<std::path::PathBuf> {
    private_path(&format!("{KNIFE_PRIVATE_FILE}.{extension}"))
}
/// A rigged knife's file with `extension` (`geo.json`, `animation.json`, `png`, `icon.png`),
/// looked for as `knife_private_path` looks.
pub fn knife_rig_path(name: &str, extension: &str) -> Option<std::path::PathBuf> {
    private_path(&format!("private/cosmetics/{name}_rig.{extension}"))
}
/// The rigged knives' files built into this executable (`build.rs`), when the computer it was
/// built on had them in `private/cosmetics/`.
static BUILT_IN_RIGS: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/knife_rigs.rs"));
/// A rigged knife's file with `extension`: the copy on disk (`knife_rig_path`) when there is
/// one, so a knife can be changed without a rebuild, else the one built in.
pub fn knife_rig_bytes(name: &str, extension: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    if let Some(bytes) = knife_rig_path(name, extension).and_then(|p| std::fs::read(p).ok()) {
        return Some(std::borrow::Cow::Owned(bytes));
    }
    let file = format!("{name}_rig.{extension}");
    BUILT_IN_RIGS
        .iter()
        .find(|(f, _)| *f == file)
        .map(|(_, bytes)| std::borrow::Cow::Borrowed(*bytes))
}
fn private_path(file: &str) -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok();
    let dirs = [
        std::env::current_dir().ok(),
        exe.as_ref().and_then(|e| e.parent()).map(|d| d.to_path_buf()),
        exe.as_ref().and_then(|e| e.ancestors().nth(3)).map(|d| d.to_path_buf()),
    ];
    dirs.into_iter().flatten().map(|d| d.join(file)).find(|p| p.is_file())
}

const ART: [(&str, &[u8]); 7] = [
    (
        APPLESKIN,
        include_bytes!("../../horizon-client/assets/horizon/textures/gui/oteria_appleskin.png"),
    ),
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
    let rig = KNIFE_RIGS.iter().flat_map(|&knife| {
        [(knife_rig_sprite(knife), "png"), (knife_rig_icon(knife), "icon.png")]
    });
    let rig = rig.filter_map(|(sprite, ext)| {
        let png = knife_rig_bytes(sprite.rsplit('/').next()?.trim_end_matches("_icon"), ext)?;
        let (w, h, pixels) = crate::pad_glyph::decode(&png)?;
        Some((sprite, w, h, pixels))
    });
    ART.iter()
        .chain(crate::oteria_icons::ALL.iter())
        .map(|&(name, png)| {
            let (w, h, pixels) = crate::pad_glyph::decode(png).expect("Oteria's menu art is PNG");
            (name.to_owned(), w, h, pixels)
        })
        .chain(private)
        .chain(rig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_art_is_white_on_nothing_at_its_sizes() {
        let sizes = [(LOGO, 160), (ROCKET, 32), (SHIRT, 32), (ELYTRA, 16)];
        for (name, w, h, px) in images() {
            // Knives copied on this computer are whatever they are.
            if name == KNIFE_PRIVATE || name.starts_with("gui/sprites/oteria_knife_rig/") {
                continue;
            }
            // The knife's texture and picture are in colour, at their own sizes.
            if name == KNIFE || name == KNIFE_PREVIEW {
                assert_eq!((w, h), if name == KNIFE { (64, 64) } else { (192, 96) }, "{name}");
                continue;
            }
            if name == APPLESKIN {
                assert_eq!((w, h), APPLESKIN_SIZE);
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
