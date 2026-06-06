//! Fallback implementation for Linux, Windows and any other target.
//!
//! CLEAN works fully via `blackout-core`. Real LOCKDOWN/OPSEC for Linux (rfkill,
//! ufw, etc.) and Windows (Defender firewall, radio APIs) are planned; for now
//! these report honestly so the build compiles and behaves predictably anywhere.

use crate::{check, unavailable, ActionResult, Capabilities, OpsecReport};

pub fn opsec_score() -> OpsecReport {
    OpsecReport {
        score: 0,
        checks: vec![check(
            "Device checks",
            "unknown",
            "OPSEC checks for this OS aren't wired up yet (planned). Metadata cleaning works today.",
            0,
        )],
    }
}

pub fn apply_level(_level: u32) -> Vec<ActionResult> {
    vec![unavailable("Lockdown", "System control for this OS is planned. File metadata cleaning works now.")]
}

pub fn panic_now() -> Vec<ActionResult> {
    vec![unavailable("Panic", "Panic actions for this OS are planned.")]
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        platform: std::env::consts::OS.to_string(),
        wifi: false,
        bluetooth: false,
        firewall: false,
        settings_deeplink: false,
    }
}

pub fn open_settings(_pane: &str) -> bool {
    false
}

pub fn harden() -> Vec<ActionResult> {
    vec![unavailable("Harden", "System hardening for this OS is planned.")]
}

pub fn apply_fix(_id: &str) -> Vec<ActionResult> {
    vec![unavailable("Fix", "One-tap fixes for this OS are planned.")]
}
