fn main() {
    // The launcher's version can be set at build time (tools/oteria/dist_test.ps1 builds two).
    println!("cargo:rerun-if-env-changed=OTERIA_LAUNCHER_VERSION");
    println!("cargo:rerun-if-env-changed=OTERIA_MSA_CLIENT_ID");
    version_covers();
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/oteria.ico");
        res.set("ProductName", "Oteria Client");
        res.set("FileDescription", "Oteria Client");
        let _ = res.compile();
    }
}

/// The Versions page's covers: Mojang's update artwork, never in the repository. They are made into
/// `private/launcher-versions/` (ignored by git) by `tools/oteria/make_version_covers.py`, and embedded
/// from there when present; without them the build embeds empty files and the page draws plain cards.
fn version_covers() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dir = std::path::Path::new("../private/launcher-versions");
    println!("cargo:rerun-if-changed=../private/launcher-versions");
    for name in ["26.png", "1.21.png", "1.20.png"] {
        let src = dir.join(name);
        println!("cargo:rerun-if-changed={}", src.display());
        let bytes = std::fs::read(&src).unwrap_or_default();
        std::fs::write(out.join(format!("cover-{name}")), bytes).unwrap();
    }
}
