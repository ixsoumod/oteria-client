//! The mod menu's icons (`game/oteria_menu.rs`): Tabler's outline icons (MIT,
//! `branding/icons/tabler/LICENSE`) in white, 32 texels square, packed into the GUI atlas.
//! Written by `branding/make_icons.py`: edit that, not this.

pub const BOX: &str = "gui/sprites/oteria_icons/box";
pub const BULB: &str = "gui/sprites/oteria_icons/bulb";
pub const CLOCK: &str = "gui/sprites/oteria_icons/clock";
pub const COPY: &str = "gui/sprites/oteria_icons/copy";
pub const CROSSHAIR: &str = "gui/sprites/oteria_icons/crosshair";
pub const DROPLET: &str = "gui/sprites/oteria_icons/droplet";
pub const EYE: &str = "gui/sprites/oteria_icons/eye";
pub const EYE_OFF: &str = "gui/sprites/oteria_icons/eye-off";
pub const FEATHER: &str = "gui/sprites/oteria_icons/feather";
pub const FLAME: &str = "gui/sprites/oteria_icons/flame";
pub const FLAME_OFF: &str = "gui/sprites/oteria_icons/flame-off";
pub const FLASK: &str = "gui/sprites/oteria_icons/flask";
pub const GAUGE: &str = "gui/sprites/oteria_icons/gauge";
pub const KEYBOARD: &str = "gui/sprites/oteria_icons/keyboard";
pub const LAYOUT: &str = "gui/sprites/oteria_icons/layout";
pub const LOCK: &str = "gui/sprites/oteria_icons/lock";
pub const LOGOUT: &str = "gui/sprites/oteria_icons/logout";
pub const MAP: &str = "gui/sprites/oteria_icons/map";
pub const MAP_PIN: &str = "gui/sprites/oteria_icons/map-pin";
pub const MICROPHONE: &str = "gui/sprites/oteria_icons/microphone";
pub const MICROPHONE_OFF: &str = "gui/sprites/oteria_icons/microphone-off";
pub const MOON_STARS: &str = "gui/sprites/oteria_icons/moon_stars";
pub const MOUSE: &str = "gui/sprites/oteria_icons/mouse";
pub const NUMBERS: &str = "gui/sprites/oteria_icons/numbers";
pub const PENCIL: &str = "gui/sprites/oteria_icons/pencil";
pub const PLUS: &str = "gui/sprites/oteria_icons/plus";
pub const POINTER: &str = "gui/sprites/oteria_icons/pointer";
pub const REFRESH: &str = "gui/sprites/oteria_icons/refresh";
pub const RIPPLE: &str = "gui/sprites/oteria_icons/ripple";
pub const RULER_2: &str = "gui/sprites/oteria_icons/ruler-2";
pub const RUN: &str = "gui/sprites/oteria_icons/run";
pub const SEARCH: &str = "gui/sprites/oteria_icons/search";
pub const SERVER: &str = "gui/sprites/oteria_icons/server";
pub const SETTINGS: &str = "gui/sprites/oteria_icons/settings";
pub const SHIELD: &str = "gui/sprites/oteria_icons/shield";
pub const SPARKLES: &str = "gui/sprites/oteria_icons/sparkles";
pub const SUN: &str = "gui/sprites/oteria_icons/sun";
pub const SWORD: &str = "gui/sprites/oteria_icons/sword";
pub const TRASH: &str = "gui/sprites/oteria_icons/trash";
pub const USER: &str = "gui/sprites/oteria_icons/user";
pub const USERS: &str = "gui/sprites/oteria_icons/users";
pub const VOLUME: &str = "gui/sprites/oteria_icons/volume";
pub const VOLUME_OFF: &str = "gui/sprites/oteria_icons/volume-off";
pub const WALK: &str = "gui/sprites/oteria_icons/walk";
pub const WIFI: &str = "gui/sprites/oteria_icons/wifi";
pub const WIFI_OFF: &str = "gui/sprites/oteria_icons/wifi-off";
pub const X: &str = "gui/sprites/oteria_icons/x";

/// Each icon's sprite and PNG: the 32 px ones, then the 20 px ones (`small`).
pub(crate) const ALL: [(&str, &[u8]); 94] = [
    (BOX, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/box.png")),
    (BULB, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/bulb.png")),
    (CLOCK, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/clock.png")),
    (COPY, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/copy.png")),
    (CROSSHAIR, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/crosshair.png")),
    (DROPLET, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/droplet.png")),
    (EYE, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/eye.png")),
    (EYE_OFF, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/eye-off.png")),
    (FEATHER, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/feather.png")),
    (FLAME, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/flame.png")),
    (FLAME_OFF, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/flame-off.png")),
    (FLASK, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/flask.png")),
    (GAUGE, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/gauge.png")),
    (KEYBOARD, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/keyboard.png")),
    (LAYOUT, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/layout.png")),
    (LOCK, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/lock.png")),
    (LOGOUT, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/logout.png")),
    (MAP, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/map.png")),
    (MAP_PIN, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/map-pin.png")),
    (MICROPHONE, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/microphone.png")),
    (MICROPHONE_OFF, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/microphone-off.png")),
    (MOON_STARS, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/moon_stars.png")),
    (MOUSE, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/mouse.png")),
    (NUMBERS, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/numbers.png")),
    (PENCIL, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/pencil.png")),
    (PLUS, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/plus.png")),
    (POINTER, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/pointer.png")),
    (REFRESH, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/refresh.png")),
    (RIPPLE, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/ripple.png")),
    (RULER_2, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/ruler-2.png")),
    (RUN, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/run.png")),
    (SEARCH, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/search.png")),
    (SERVER, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/server.png")),
    (SETTINGS, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/settings.png")),
    (SHIELD, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/shield.png")),
    (SPARKLES, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/sparkles.png")),
    (SUN, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/sun.png")),
    (SWORD, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/sword.png")),
    (TRASH, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/trash.png")),
    (USER, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/user.png")),
    (USERS, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/users.png")),
    (VOLUME, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/volume.png")),
    (VOLUME_OFF, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/volume-off.png")),
    (WALK, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/walk.png")),
    (WIFI, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/wifi.png")),
    (WIFI_OFF, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/wifi-off.png")),
    (X, include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/x.png")),
    ("gui/sprites/oteria_icons_small/box", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/box.png")),
    ("gui/sprites/oteria_icons_small/bulb", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/bulb.png")),
    ("gui/sprites/oteria_icons_small/clock", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/clock.png")),
    ("gui/sprites/oteria_icons_small/copy", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/copy.png")),
    ("gui/sprites/oteria_icons_small/crosshair", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/crosshair.png")),
    ("gui/sprites/oteria_icons_small/droplet", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/droplet.png")),
    ("gui/sprites/oteria_icons_small/eye", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/eye.png")),
    ("gui/sprites/oteria_icons_small/eye-off", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/eye-off.png")),
    ("gui/sprites/oteria_icons_small/feather", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/feather.png")),
    ("gui/sprites/oteria_icons_small/flame", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/flame.png")),
    ("gui/sprites/oteria_icons_small/flame-off", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/flame-off.png")),
    ("gui/sprites/oteria_icons_small/flask", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/flask.png")),
    ("gui/sprites/oteria_icons_small/gauge", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/gauge.png")),
    ("gui/sprites/oteria_icons_small/keyboard", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/keyboard.png")),
    ("gui/sprites/oteria_icons_small/layout", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/layout.png")),
    ("gui/sprites/oteria_icons_small/lock", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/lock.png")),
    ("gui/sprites/oteria_icons_small/logout", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/logout.png")),
    ("gui/sprites/oteria_icons_small/map", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/map.png")),
    ("gui/sprites/oteria_icons_small/map-pin", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/map-pin.png")),
    ("gui/sprites/oteria_icons_small/microphone", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/microphone.png")),
    ("gui/sprites/oteria_icons_small/microphone-off", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/microphone-off.png")),
    ("gui/sprites/oteria_icons_small/moon_stars", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/moon_stars.png")),
    ("gui/sprites/oteria_icons_small/mouse", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/mouse.png")),
    ("gui/sprites/oteria_icons_small/numbers", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/numbers.png")),
    ("gui/sprites/oteria_icons_small/pencil", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/pencil.png")),
    ("gui/sprites/oteria_icons_small/plus", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/plus.png")),
    ("gui/sprites/oteria_icons_small/pointer", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/pointer.png")),
    ("gui/sprites/oteria_icons_small/refresh", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/refresh.png")),
    ("gui/sprites/oteria_icons_small/ripple", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/ripple.png")),
    ("gui/sprites/oteria_icons_small/ruler-2", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/ruler-2.png")),
    ("gui/sprites/oteria_icons_small/run", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/run.png")),
    ("gui/sprites/oteria_icons_small/search", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/search.png")),
    ("gui/sprites/oteria_icons_small/server", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/server.png")),
    ("gui/sprites/oteria_icons_small/settings", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/settings.png")),
    ("gui/sprites/oteria_icons_small/shield", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/shield.png")),
    ("gui/sprites/oteria_icons_small/sparkles", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/sparkles.png")),
    ("gui/sprites/oteria_icons_small/sun", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/sun.png")),
    ("gui/sprites/oteria_icons_small/sword", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/sword.png")),
    ("gui/sprites/oteria_icons_small/trash", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/trash.png")),
    ("gui/sprites/oteria_icons_small/user", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/user.png")),
    ("gui/sprites/oteria_icons_small/users", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/users.png")),
    ("gui/sprites/oteria_icons_small/volume", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/volume.png")),
    ("gui/sprites/oteria_icons_small/volume-off", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/volume-off.png")),
    ("gui/sprites/oteria_icons_small/walk", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/walk.png")),
    ("gui/sprites/oteria_icons_small/wifi", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/wifi.png")),
    ("gui/sprites/oteria_icons_small/wifi-off", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/wifi-off.png")),
    ("gui/sprites/oteria_icons_small/x", include_bytes!("../../horizon-client/assets/horizon/textures/gui/icons/small/x.png")),
];

/// The 20 px version of `icon`, for the menu's chrome.
pub fn small(icon: &str) -> String {
    icon.replacen("/oteria_icons/", "/oteria_icons_small/", 1)
}
