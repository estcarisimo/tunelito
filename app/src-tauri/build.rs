fn main() {
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
