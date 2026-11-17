fn main() {
    // The launcher's version can be set at build time (tools/oteria/dist_test.ps1 builds two).
    println!("cargo:rerun-if-env-changed=OTERIA_LAUNCHER_VERSION");
    println!("cargo:rerun-if-env-changed=OTERIA_MSA_CLIENT_ID");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/oteria.ico");
        res.set("ProductName", "Oteria Client");
        res.set("FileDescription", "Oteria Client");
        let _ = res.compile();
    }
}
