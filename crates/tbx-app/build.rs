fn main() {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    slint_build::compile("ui/main.slint").expect("Slint UI failed to compile");
    #[cfg(target_os = "windows")]
    embed_icon();
}
#[cfg(target_os = "windows")]
fn embed_icon() {
    let icon_path = std::path::Path::new("assets/icon.ico");
    if !icon_path.exists() {
        println!("cargo:warning=assets/icon.ico not found — .exe will use the default Windows icon. Place a 256×256 .ico file at crates/tbx-app/assets/icon.ico to embed it.");
        return;
    }
    let mut res = winres::WindowsResource::new();
    res.set_icon(icon_path.to_str().unwrap());
    if let Ok(version) = std::env::var("CARGO_PKG_VERSION") {
        res.set("FileVersion", &version);
        res.set("ProductVersion", &version);
    }
    res.set("ProductName", "TexelBox");
    res.set("FileDescription", "TexelBox — PBR Texture Tools for Game Developers");
    res.set("LegalCopyright", "© TexelBox");
    res.compile().expect("failed to embed Windows icon/resources");
}
