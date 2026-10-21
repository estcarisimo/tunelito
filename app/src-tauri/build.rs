use std::fs;
use std::path::PathBuf;

fn main() {
    write_embedded_bins();
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_state",
            "add_key",
            "remove_key",
            "turn_on",
            "turn_off",
            "update_settings",
            "refresh",
        ]),
    ))
    .expect("failed to run tauri-build");
}

/// When `TUNELITO_EMBED_DIR` contains `cloudflared.exe` and `tunelito-priv.exe`,
/// the portable Windows exe carries both. Other builds get empty stubs.
fn write_embedded_bins() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("embedded_bins.rs");
    let dir = std::env::var("TUNELITO_EMBED_DIR").unwrap_or_default();
    let cloudflared = PathBuf::from(&dir).join("cloudflared.exe");
    let priv_helper = PathBuf::from(&dir).join("tunelito-priv.exe");
    println!("cargo:rerun-if-env-changed=TUNELITO_EMBED_DIR");
    if dir.is_empty() || !cloudflared.is_file() || !priv_helper.is_file() {
        fs::write(
            dest,
            "pub fn embedded_cloudflared() -> Option<&'static [u8]> { None }\n\
             #[allow(dead_code)]\n\
             pub fn embedded_priv() -> Option<&'static [u8]> { None }\n",
        )
        .expect("embed stub");
        return;
    }
    fs::copy(&cloudflared, out_dir.join("cloudflared.exe")).expect("copy cloudflared");
    fs::copy(&priv_helper, out_dir.join("tunelito-priv.exe")).expect("copy priv");
    println!("cargo:rerun-if-changed={}", cloudflared.display());
    println!("cargo:rerun-if-changed={}", priv_helper.display());
    fs::write(
        dest,
        "pub fn embedded_cloudflared() -> Option<&'static [u8]> {\n\
             Some(include_bytes!(concat!(env!(\"OUT_DIR\"), \"/cloudflared.exe\")))\n\
         }\n\
         #[allow(dead_code)]\n\
         pub fn embedded_priv() -> Option<&'static [u8]> {\n\
             Some(include_bytes!(concat!(env!(\"OUT_DIR\"), \"/tunelito-priv.exe\")))\n\
         }\n",
    )
    .expect("embed rs");
}
