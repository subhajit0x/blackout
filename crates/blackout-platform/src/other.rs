//! Fallback implementation for Linux, Windows and any other target.
//!
//! CLEAN works fully via `blackout-core`. Real LOCKDOWN/OPSEC for Linux (rfkill,
//! ufw, etc.) and Windows (Defender firewall, radio APIs) are planned; for now
//! these report honestly so the build compiles and behaves predictably anywhere.

use crate::{check, unavailable, ActionResult, Capabilities, OpsecReport};

pub fn opsec_score() -> OpsecReport {
    OpsecReport {
        score: 0,
        device: crate::device(std::env::consts::OS, "", ""),
        checks: vec![check(
            "On-device checks",
            "unknown",
            "Automatic checks for this OS are planned. The guide below covers the essentials.",
            0,
        )],
        guide: generic_guide(),
    }
}

fn generic_guide() -> Vec<crate::GuideStep> {
    use crate::step;
    vec![
        step("Encrypt your disk", "high",
            "Keeps your data unreadable if the device is lost or stolen.",
            "Windows: turn on BitLocker. Linux: use LUKS full-disk encryption.", None),
        step("Enable the firewall", "high",
            "Blocks unsolicited incoming connections from the network.",
            "Windows: Windows Security ▸ Firewall ▸ On. Linux: `sudo ufw enable`.", None),
        step("Require a login password & short auto-lock", "medium",
            "Stops physical access to an unattended machine.",
            "Set a strong account password and a short screen-lock timeout.", None),
        step("Keep the system updated", "medium",
            "Most real-world attacks exploit known, already-patched bugs.",
            "Install OS and application security updates promptly.", None),
        step("Use encrypted DNS or a trusted VPN", "low",
            "Stops your network and provider from logging the sites you visit.",
            "Enable DNS-over-HTTPS in your browser/OS, or connect a no-logs VPN.", None),
    ]
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
