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
fn opsec_score(app: tauri::AppHandle) -> bp::OpsecReport {
    #[cfg(target_os = "android")]
    {
        let f = blackout_droid::opsec_facts(&app);
        return bp::android_facts::opsec_from_facts(&bp::android_facts::AndroidFacts {
            vpn_active: f.vpn_active,
            wifi_on: f.wifi_on,
            bluetooth_on: f.bluetooth_on,
            airplane_on: f.airplane_on,
            screen_lock_set: f.screen_lock_set,
            developer_options: f.developer_options,
            usb_debugging: f.usb_debugging,
            location_on: f.location_on,
            nfc_on: f.nfc_on,
            patch_age_days: f.patch_age_days,
            sdk_int: f.sdk_int,
            os_version: f.os_version,
            model: f.model,
        });
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        bp::opsec_score()
    }
}

#[tauri::command]
fn apply_level(app: tauri::AppHandle, level: u32) -> Vec<bp::ActionResult> {
    #[cfg(target_os = "android")]
    {
        let _ = level;
        return android_lockdown(&app);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        bp::apply_level(level)
    }
}

#[tauri::command]
fn panic_now(app: tauri::AppHandle) -> Vec<bp::ActionResult> {
    #[cfg(target_os = "android")]
    return android_lockdown(&app);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        bp::panic_now()
    }
}

#[tauri::command]
fn capabilities() -> bp::Capabilities {
    bp::capabilities()
}

#[tauri::command]
fn open_settings(app: tauri::AppHandle, pane: String) -> bool {
    #[cfg(target_os = "android")]
    return blackout_droid::open_panel(&app, &pane);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        bp::open_settings(&pane)
    }
}

#[tauri::command]
fn harden_now(app: tauri::AppHandle) -> Vec<bp::ActionResult> {
    #[cfg(target_os = "android")]
    return android_lockdown(&app);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        bp::harden()
    }
}

#[tauri::command]
fn apply_fix(app: tauri::AppHandle, id: String) -> Vec<bp::ActionResult> {
    #[cfg(target_os = "android")]
    {
        let ok = blackout_droid::open_panel(&app, &id);
        return vec![action(
            if ok { "done" } else { "unavailable" },
            "Opened settings",
            if ok {
                "Opened the relevant Settings panel — make the change there."
            } else {
                "Couldn't open that panel on this device."
            },
        )];
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        bp::apply_fix(&id)
    }
}

/// "Am I hacked?" — list installed apps with threat flags (Android).
#[tauri::command]
fn list_apps(app: tauri::AppHandle) -> serde_json::Value {
    #[cfg(target_os = "android")]
    return serde_json::to_value(blackout_droid::list_apps(&app))
        .unwrap_or_else(|_| serde_json::json!([]));
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        serde_json::json!([])
    }
}

#[tauri::command]
fn uninstall_app(app: tauri::AppHandle, pkg: String) -> bool {
    #[cfg(target_os = "android")]
    return blackout_droid::uninstall_app(&app, &pkg);
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, pkg);
        false
    }
}

#[tauri::command]
fn open_app_settings(app: tauri::AppHandle, pkg: String) -> bool {
    #[cfg(target_os = "android")]
    return blackout_droid::open_app_settings(&app, &pkg);
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, pkg);
        false
    }
}

/// Android lockdown/panic: clear the clipboard and jump to Airplane mode (apps
/// can't toggle radios, so we open the panel — honest, never faked).
#[cfg(target_os = "android")]
fn android_lockdown(app: &tauri::AppHandle) -> Vec<bp::ActionResult> {
    let cleared = blackout_droid::clear_clipboard(app);
    let opened = blackout_droid::open_panel(app, "airplane");
    vec![
        action(
            if cleared { "done" } else { "error" },
            "Clipboard cleared",
            if cleared { "Any copied passwords or text were wiped." } else { "Couldn't clear the clipboard." },
        ),
        action(
            if opened { "done" } else { "unavailable" },
            "Airplane mode",
            if opened { "Opened Airplane mode — turn it on to cut every radio." } else { "Couldn't open Airplane settings." },
        ),
    ]
}

#[cfg(target_os = "android")]
fn action(status: &str, label: &str, detail: &str) -> bp::ActionResult {
    bp::ActionResult { label: label.into(), status: status.into(), detail: detail.into() }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    // Android: file access (content:// URIs → Downloads) + native OPSEC bridge
    // (VPN/Bluetooth/etc. state, settings panels, clipboard).
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(tauri_plugin_android_fs::init())
        .plugin(blackout_droid::init());

    builder
        .manage(watch::WatchState::default())
        .manage(clean::LastCleaned::default())
        .invoke_handler(tauri::generate_handler![
            // CLEAN
            clean::inspect_files,
            clean::clean_files,
            clean::clean_picked,
            clean::share_cleaned,
            clean::reveal_path,
            // OPSEC / LOCKDOWN / PANIC (portable, cfg-gated platform layer)
            opsec_score,
            apply_level,
            panic_now,
            capabilities,
            open_settings,
            harden_now,
            apply_fix,
            // "Am I hacked?" app inventory (Android)
            list_apps,
            uninstall_app,
            open_app_settings,
            // Auto-clean watched folder (desktop)
            watch::start_watch,
            watch::stop_watch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
