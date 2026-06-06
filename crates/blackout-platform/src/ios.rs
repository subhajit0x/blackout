//! iOS implementation.
//!
//! CLEAN works fully today via `blackout-core`. iOS deliberately sandboxes apps
//! hard: there is no API to toggle Wi-Fi/Bluetooth or read most system state, so
//! LOCKDOWN/OPSEC are reported honestly. The share-extension flow (clean a photo
//! straight from the share sheet) is the natural iOS surface for CLEAN.

use crate::{check, unavailable, ActionResult, Capabilities, OpsecReport};

pub fn opsec_score() -> OpsecReport {
    OpsecReport {
        score: 0,
        checks: vec![check(
            "Device checks",
            "unknown",
            "iOS sandboxes system state; OPSEC reads need entitlements/native code (planned). Cleaning works today.",
            0,
        )],
    }
}

pub fn apply_level(_level: u32) -> Vec<ActionResult> {
    vec![unavailable(
        "Lockdown",
        "iOS does not allow apps to toggle radios or sensors. Cleaning files works; use the share sheet.",
    )]
}

pub fn panic_now() -> Vec<ActionResult> {
    vec![unavailable("Panic", "iOS does not permit apps to disable connectivity or lock the device.")]
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        platform: "iOS".into(),
        wifi: false,
        bluetooth: false,
        firewall: false,
        settings_deeplink: false, // iOS only lets an app open its *own* settings page
    }
}

pub fn open_settings(_pane: &str) -> bool {
    false
}

pub fn harden() -> Vec<ActionResult> {
    vec![unavailable("Harden", "iOS does not expose firewall/system hardening to apps.")]
}

pub fn apply_fix(_id: &str) -> Vec<ActionResult> {
    vec![unavailable("Fix", "iOS does not allow apps to change these system settings.")]
}
