//! Android implementation.
//!
//! CLEAN (file metadata removal) works fully today via `blackout-core` — it's
//! pure Rust. Radio/sensor control and OPSEC reads need a native (Kotlin/JNI)
//! plugin; until that lands these report honestly rather than faking success.

use crate::{check, unavailable, ActionResult, Capabilities, OpsecReport};

pub fn opsec_score() -> OpsecReport {
    OpsecReport {
        score: 0,
        device: crate::device("Android", "", ""),
        checks: vec![check(
            "On-device checks",
            "unknown",
            "Automatic checks need a native plugin (planned). The guide below is accurate for Android — follow it.",
            0,
        )],
        guide: android_guide(),
    }
}

fn android_guide() -> Vec<crate::GuideStep> {
    use crate::step;
    vec![
        step("Set a strong screen lock", "high",
            "A 6-digit PIN or password (not a pattern) is your first defense if the phone is lost or seized.",
            "Settings ▸ Security ▸ Screen lock ▸ PIN/Password, then add a fingerprint.", None),
        step("Confirm storage encryption", "high",
            "Encryption keeps your data unreadable without the lock. Modern Android encrypts once a lock is set.",
            "Settings ▸ Security ▸ Encryption & credentials — verify it says 'Encrypted'.", None),
        step("Review app permissions", "medium",
            "Apps routinely over-ask for camera, microphone and location.",
            "Settings ▸ Privacy ▸ Permission manager — revoke Camera/Mic/Location from apps that don't need them.", None),
        step("Delete your advertising ID", "medium",
            "Stops apps from linking your activity across the system for ad targeting.",
            "Settings ▸ Privacy ▸ Ads ▸ Delete advertising ID.", None),
        step("Turn off Wi-Fi & Bluetooth scanning", "medium",
            "Stores and apps track your location via radio scans even when Wi-Fi/Bluetooth are 'off'.",
            "Settings ▸ Location ▸ Wi-Fi scanning & Bluetooth scanning ▸ Off.", None),
        step("Use Private DNS", "low",
            "Encrypts your DNS so your carrier or network can't log the sites you visit.",
            "Settings ▸ Network & internet ▸ Private DNS ▸ enter a trusted provider hostname.", None),
        step("Learn the Lockdown shortcut", "medium",
            "Lockdown instantly hides notifications and disables biometrics if you're stopped or detained.",
            "Settings ▸ Display ▸ Lock screen ▸ enable 'Show Lockdown option', then hold the power button.", None),
    ]
}

pub fn apply_level(_level: u32) -> Vec<ActionResult> {
    vec![unavailable(
        "Lockdown",
        "Radio/sensor control on Android requires a native plugin (planned). File metadata cleaning works now.",
    )]
}

pub fn panic_now() -> Vec<ActionResult> {
    vec![unavailable("Panic", "Android panic actions require a native plugin (planned).")]
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        platform: "Android".into(),
        wifi: false,
        bluetooth: false,
        firewall: false,
        // The native plugin opens system Settings panels via Intents.
        settings_deeplink: true,
    }
}

pub fn open_settings(_pane: &str) -> bool {
    false // requires a native Intent; wired up with the Android plugin
}

pub fn harden() -> Vec<ActionResult> {
    vec![unavailable("Harden", "Android system hardening requires a native plugin (planned).")]
}

pub fn apply_fix(_id: &str) -> Vec<ActionResult> {
    vec![unavailable("Fix", "One-tap fixes require a native plugin on Android (planned).")]
}
