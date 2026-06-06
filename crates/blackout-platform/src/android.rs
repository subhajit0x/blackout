//! Android implementation.
//!
//! CLEAN (file metadata removal) works fully today via `blackout-core` — it's
//! pure Rust. Radio/sensor control and OPSEC reads need a native (Kotlin/JNI)
//! plugin; until that lands these report honestly rather than faking success.

use crate::{check, unavailable, ActionResult, Capabilities, OpsecReport};

pub fn opsec_score() -> OpsecReport {
    OpsecReport {
        score: 0,
        checks: vec![check(
            "Device checks",
            "unknown",
            "OPSEC checks on Android need a native integration (planned). Metadata cleaning already works.",
            0,
        )],
    }
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
        settings_deeplink: true, // Android can open system Settings via an Intent (native plugin)
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
