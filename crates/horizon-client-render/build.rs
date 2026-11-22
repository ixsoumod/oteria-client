//! Builds the rigged knives copied on this computer into `private/cosmetics/` (not in git) into
//! the executable, so a build handed to someone else draws them too (`oteria_art::knife_rig_bytes`).
//! Without the folder, nothing is built in and the game reads the files from disk as before.
//! The wearables copied into `private/cosmetics/wear/` are built in the same way.

use std::fmt::Write as _;
use std::path::PathBuf;

/// `oteria_art::KNIFE_RIGS`, and each knife's files.
const KNIVES: [&str; 6] = ["butterfly", "karambit", "skeleton", "stiletto", "talon", "push"];
const FILES: [&str; 4] = ["geo.json", "animation.json", "png", "icon.png"];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let dir = manifest.join("../../private/cosmetics");
    println!("cargo:rerun-if-changed=build.rs");
    let mut table = String::from("&[\n");
    if dir.is_dir() {
        // A knife added or taken away changes the folder.
        println!("cargo:rerun-if-changed={}", dir.display());
        for knife in KNIVES {
            for ext in FILES {
                let file = format!("{knife}_rig.{ext}");
                let path = dir.join(&file);
                if path.is_file() {
                    println!("cargo:rerun-if-changed={}", path.display());
                    let _ = writeln!(table, "    ({file:?}, include_bytes!({:?})),", path.display().to_string());
                }
            }
        }
    }
    table.push(']');
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it")).join("knife_rigs.rs");
    std::fs::write(out, table).expect("OUT_DIR is writable");
    wear(&dir.join("wear"));
}

/// The wearables (`oteria_art::CAPES`, `HATS`, `PETS`) copied into `private/cosmetics/wear/`:
/// every PNG and model there, built in.
fn wear(dir: &std::path::Path) {
    let mut table = String::from("&[\n");
    if dir.is_dir() {
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .expect("the folder reads")
            .flatten()
            .map(|e| e.path())
            .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("png" | "json")))
            .collect();
        files.sort();
        for path in files {
            let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
                continue;
            };
            println!("cargo:rerun-if-changed={}", path.display());
            let _ = writeln!(table, "    ({file:?}, include_bytes!({:?})),", path.display().to_string());
        }
    }
    table.push(']');
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it")).join("wear.rs");
    std::fs::write(out, table).expect("OUT_DIR is writable");
}
