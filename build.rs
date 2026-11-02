fn main() {
    // Embed the application icon into the Windows executable.
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/app-icon/icon.ico");
            if let Err(err) = res.compile() {
                println!("cargo:warning=failed to embed icon: {err}");
            }
        }
    }
    println!("cargo:rerun-if-changed=assets/app-icon/icon.ico");
}
