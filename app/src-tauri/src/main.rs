#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "--uninstall") {
        std::process::exit(tunelito_lib::uninstall_windows());
    }
    tunelito_lib::run();
}
