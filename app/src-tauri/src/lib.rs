mod embed;
mod ops;
#[cfg(windows)]
mod windows_host;

use ops::Runtime;
use tauri::{Manager, RunEvent};
use tunelito_core::{Settings, ViewModel};

#[derive(Debug, serde::Serialize)]
struct ApiError {
    code: String,
    detail: Option<String>,
}

impl From<tunelito_core::Error> for ApiError {
    fn from(err: tunelito_core::Error) -> Self {
        Self {
            code: err.code.to_string(),
            detail: err.detail,
        }
    }
}

#[tauri::command]
fn get_state(runtime: tauri::State<'_, Runtime>) -> ViewModel {
    runtime.get_state()
}

#[tauri::command]
fn add_key(runtime: tauri::State<'_, Runtime>, text: String) -> Result<ViewModel, ApiError> {
    runtime.add_key(&text).map_err(ApiError::from)
}

#[tauri::command]
fn remove_key(
    runtime: tauri::State<'_, Runtime>,
    fingerprint: String,
) -> Result<ViewModel, ApiError> {
    runtime.remove_key(&fingerprint).map_err(ApiError::from)
}

#[tauri::command]
fn turn_on(runtime: tauri::State<'_, Runtime>) -> Result<ViewModel, ApiError> {
    runtime.turn_on().map_err(ApiError::from)
}

#[tauri::command]
fn turn_off(runtime: tauri::State<'_, Runtime>, revoke_keys: bool) -> Result<ViewModel, ApiError> {
    runtime.turn_off(revoke_keys).map_err(ApiError::from)
}

#[tauri::command]
fn update_settings(
    runtime: tauri::State<'_, Runtime>,
    settings: Settings,
) -> Result<ViewModel, ApiError> {
    runtime.update_settings(settings).map_err(ApiError::from)
}

#[tauri::command]
fn refresh(runtime: tauri::State<'_, Runtime>) -> Result<ViewModel, ApiError> {
    runtime.refresh().map_err(ApiError::from)
}

#[cfg(windows)]
pub fn uninstall_windows() -> i32 {
    windows_host::uninstall_exit()
}

pub fn run() {
    let runtime = Runtime::load();
    tauri::Builder::default()
        .manage(runtime)
        .invoke_handler(tauri::generate_handler![
            get_state,
            add_key,
            remove_key,
            turn_on,
            turn_off,
            update_settings,
            refresh,
        ])
        .build(tauri::generate_context!())
        .expect("tunelito failed to build its window")
        .run(|app, event| {
            if matches!(event, RunEvent::ExitRequested { .. }) {
                app.state::<Runtime>().shutdown();
            }
        });
}
