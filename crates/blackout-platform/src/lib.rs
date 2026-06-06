//! BLACKOUT platform layer.
//!
//! One portable API — `opsec_score`, `apply_level`, `panic_now`, `harden`,
//! `open_settings`, `capabilities` — implemented per operating system behind
//! `cfg` gates. Every target (macOS, iOS, Android, Linux, Windows) gets a
//! compiling implementation: the OS that can do something does it for real, the
//! ones that can't say so honestly. The CLEAN engine (`blackout-core`) is
//! already pure Rust and portable; this crate is the only place OS-specific
//! code lives.

use serde::Serialize;

// ---------------------------------------------------------------------------
// Shared, portable data types (returned to any front-end as-is via serde).
// ---------------------------------------------------------------------------

/// A single read-only OPSEC check, in plain language.
#[derive(Debug, Serialize)]
pub struct Check {
    /// Grouping for the UI: "Device" | "Network" | "Sharing" | "Privacy" | "Other".
    pub category: String,
    pub label: String,
    /// "good" | "warn" | "bad" | "unknown"
    pub status: String,
    pub detail: String,
    pub weight: u32,
    /// If set, an id the UI can pass to `apply_fix` for a one-tap remediation.
    pub fix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpsecReport {
    pub score: u32,
    pub checks: Vec<Check>,
}

/// Result of a single LOCKDOWN/PANIC action.
#[derive(Debug, Serialize)]
pub struct ActionResult {
    pub label: String,
    /// "done" | "unavailable" | "error"
    pub status: String,
    pub detail: String,
}

/// What this build can actually control on the current device.
#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub platform: String,
    pub wifi: bool,
    pub bluetooth: bool,
    pub firewall: bool,
    pub settings_deeplink: bool,
}

// ---------------------------------------------------------------------------
// Constructors shared by every per-OS implementation.
// ---------------------------------------------------------------------------

pub(crate) fn check(label: &str, status: &str, detail: &str, weight: u32) -> Check {
    Check {
        category: "Other".into(),
        label: label.into(),
        status: status.into(),
        detail: detail.into(),
        weight,
        fix: None,
    }
}

impl Check {
    /// Builder: set the UI category.
    #[allow(dead_code)]
    pub(crate) fn cat(mut self, category: &str) -> Self {
        self.category = category.into();
        self
    }
}
#[allow(dead_code)] // used by some platform backends, not others
pub(crate) fn done(label: &str, detail: &str) -> ActionResult {
    ActionResult { label: label.into(), status: "done".into(), detail: detail.into() }
}
pub(crate) fn unavailable(label: &str, detail: &str) -> ActionResult {
    ActionResult { label: label.into(), status: "unavailable".into(), detail: detail.into() }
}
#[allow(dead_code)]
pub(crate) fn errored(label: &str, detail: &str) -> ActionResult {
    ActionResult { label: label.into(), status: "error".into(), detail: detail.into() }
}

/// Score = good→full weight, warn→half, else 0. Shared across implementations.
#[allow(dead_code)] // used by desktop backends, not the mobile stubs
pub(crate) fn tally(checks: &[Check]) -> u32 {
    let (mut earned, mut total) = (0u32, 0u32);
    for c in checks {
        total += c.weight;
        earned += match c.status.as_str() {
            "good" => c.weight,
            "warn" => c.weight / 2,
            _ => 0,
        };
    }
    if total == 0 { 0 } else { (earned * 100) / total }
}

// ---------------------------------------------------------------------------
// Per-OS dispatch. Exactly one module compiles per target.
// ---------------------------------------------------------------------------

#[cfg_attr(target_os = "macos", path = "macos.rs")]
#[cfg_attr(target_os = "ios", path = "ios.rs")]
#[cfg_attr(target_os = "android", path = "android.rs")]
#[cfg_attr(
    not(any(target_os = "macos", target_os = "ios", target_os = "android")),
    path = "other.rs"
)]
mod imp;

pub use imp::{
    apply_fix, apply_level, capabilities, harden, opsec_score, open_settings, panic_now,
};
