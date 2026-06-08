//! BLACKOUT native Android bridge.
//!
//! A thin Tauri mobile plugin exposing the few things Android only lets *native*
//! code do: read live OPSEC state (VPN/Tor tunnel, Wi-Fi, Bluetooth, airplane,
//! screen-lock), open the right system Settings panel, and clear the clipboard.
//! Everything is read-only or user-initiated — we never silently toggle radios
//! (Android forbids that), matching BLACKOUT's "honest, never faked" rule.

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Runtime};
#[cfg(target_os = "android")]
use tauri::Manager;

/// Live device state read from Android APIs. All best-effort; unknown → false/0.
/// camelCase to match the Kotlin JSObject keys sent over the mobile bridge.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Facts {
    /// An encrypted tunnel (VPN or Tor-via-Orbot) is the active transport.
    pub vpn_active: bool,
    pub wifi_on: bool,
    pub bluetooth_on: bool,
    pub airplane_on: bool,
    /// A PIN/password/biometric lock is set (device encryption is keyed to this).
    pub screen_lock_set: bool,
    pub developer_options: bool,
    pub usb_debugging: bool,
    pub location_on: bool,
    pub nfc_on: bool,
    /// Days since the OS security patch level, -1 if unknown.
    pub patch_age_days: i32,
    pub sdk_int: i32,
    pub os_version: String,
    pub model: String,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct PanelArg<'a> {
    panel: &'a str,
}

#[cfg(target_os = "android")]
#[derive(Deserialize, Default)]
struct OkResp {
    ok: bool,
}

/// Holds the live handle to the Kotlin plugin (Android only).
#[cfg(target_os = "android")]
pub struct BlackoutDroid<R: Runtime>(tauri::plugin::PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("blackout-droid")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api
                    .register_android_plugin("com.plugin.blackout_droid", "BlackoutDroidPlugin")?;
                _app.manage(BlackoutDroid(handle));
            }
            Ok(())
        })
        .build()
}

/// Read live OPSEC state. Returns all-default off Android or if the bridge fails.
pub fn opsec_facts<R: Runtime>(app: &AppHandle<R>) -> Facts {
    #[cfg(target_os = "android")]
    {
        if let Some(state) = app.try_state::<BlackoutDroid<R>>() {
            return state
                .inner()
                .0
                .run_mobile_plugin::<Facts>("opsecFacts", serde_json::json!({}))
                .unwrap_or_default();
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
    Facts::default()
}

/// Open a system Settings panel. `panel` ∈ wifi|bluetooth|airplane|location|
/// security|privacy. Returns whether the intent was launched.
pub fn open_panel<R: Runtime>(app: &AppHandle<R>, panel: &str) -> bool {
    #[cfg(target_os = "android")]
    {
        if let Some(state) = app.try_state::<BlackoutDroid<R>>() {
            return state
                .inner()
                .0
                .run_mobile_plugin::<OkResp>("openPanel", PanelArg { panel })
                .map(|r| r.ok)
                .unwrap_or(false);
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, panel);
    false
}

/// Wipe the clipboard (any copied passwords/text). Returns success.
pub fn clear_clipboard<R: Runtime>(app: &AppHandle<R>) -> bool {
    #[cfg(target_os = "android")]
    {
        if let Some(state) = app.try_state::<BlackoutDroid<R>>() {
            return state
                .inner()
                .0
                .run_mobile_plugin::<OkResp>("clearClipboard", serde_json::json!({}))
                .map(|r| r.ok)
                .unwrap_or(false);
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
    false
}
