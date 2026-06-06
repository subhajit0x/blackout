//! BLACKOUT app — Tauri backend.
//!
//! CLEAN is handled by `clean` (wrapping the portable `blackout-core` engine).
//! OPSEC / LOCKDOWN / PANIC are thin command wrappers over `blackout-platform`,
//! whose behaviour is cfg-gated per OS — so this same backend compiles and runs
//! on macOS, Windows, Linux, iOS and Android.

mod clean;
mod watch;

use blackout_platform as bp;

#[tauri::command]
fn opsec_score() -> bp::OpsecReport {
    bp::opsec_score()
}

#[tauri::command]
fn apply_level(level: u32) -> Vec<bp::ActionResult> {
    bp::apply_level(level)
}

#[tauri::command]
fn panic_now() -> Vec<bp::ActionResult> {
    bp::panic_now()
}

#[tauri::command]
fn capabilities() -> bp::Capabilities {
    bp::capabilities()
}

#[tauri::command]
fn open_settings(pane: String) -> bool {
    bp::open_settings(&pane)
}

#[tauri::command]
fn harden_now() -> Vec<bp::ActionResult> {
    bp::harden()
}

#[tauri::command]
fn apply_fix(id: String) -> Vec<bp::ActionResult> {
    bp::apply_fix(&id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(watch::WatchState::default())
        .invoke_handler(tauri::generate_handler![
            // CLEAN
            clean::inspect_files,
            clean::clean_files,
            clean::reveal_path,
            // OPSEC / LOCKDOWN / PANIC (portable, cfg-gated platform layer)
            opsec_score,
            apply_level,
            panic_now,
            capabilities,
            open_settings,
            harden_now,
            apply_fix,
            // Auto-clean watched folder (desktop)
            watch::start_watch,
            watch::stop_watch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
