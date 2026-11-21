//! The player's language, for the few windows that speak English to non-French players (the
//! launcher update popup). French for any French Windows (France, Canada, Belgium, Switzerland…),
//! English for everything else.

/// Whether Windows' display language is French (any region). `OTERIA_LANG=fr|en` overrides it.
pub fn french() -> bool {
    if let Ok(v) = std::env::var("OTERIA_LANG") {
        return v.to_ascii_lowercase().starts_with("fr");
    }
    system_french()
}

#[cfg(windows)]
fn system_french() -> bool {
    // A LANGID's low 10 bits are the primary language; LANG_FRENCH is 0x0c for every region
    // (fr-FR 0x040c, fr-CA 0x0c0c, fr-BE 0x080c, fr-CH 0x100c…).
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    id & 0x3ff == 0x0c
}

#[cfg(not(windows))]
fn system_french() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.to_ascii_lowercase().starts_with("fr"))
}
