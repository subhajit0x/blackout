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
        device: crate::device("iOS", "", ""),
        checks: vec![check(
            "On-device checks",
            "unknown",
            "iOS sandboxes system state, so auto-checks need entitlements (planned). The guide below is accurate for iPhone/iPad.",
            0,
        )],
        guide: ios_guide(),
    }
}

fn ios_guide() -> Vec<crate::GuideStep> {
    use crate::step;
    vec![
        step("Use a strong passcode", "high",
            "A 6-digit or alphanumeric passcode protects everything, including your Keychain and Face ID enrolment.",
            "Settings ▸ Face ID & Passcode ▸ Change Passcode ▸ Custom Alphanumeric Code.", None),
        step("Turn on Stolen Device Protection", "high",
            "Requires Face ID + a delay before sensitive changes if someone has your passcode.",
            "Settings ▸ Face ID & Passcode ▸ Stolen Device Protection ▸ On.", None),
        step("Enable Lockdown Mode (high-risk)", "high",
            "Apple's strongest protection against targeted mercenary spyware.",
            "Settings ▸ Privacy & Security ▸ Lockdown Mode ▸ Turn On Lockdown Mode.", None),
        step("Limit lock-screen access", "medium",
            "Stops someone holding your locked phone from using Control Center or USB data.",
            "Settings ▸ Face ID & Passcode ▸ under 'Allow Access When Locked' turn off Control Center & Accessories.", None),
        step("Review tracking & permissions", "medium",
            "Cut off cross-app tracking and needless camera/mic/location access.",
            "Settings ▸ Privacy & Security ▸ Tracking (off), then review Camera, Microphone and Location Services.", None),
        step("Enable iCloud Private Relay", "low",
            "Hides your IP address and DNS from networks and the sites you visit.",
            "Settings ▸ [your name] ▸ iCloud ▸ Private Relay ▸ On.", None),
    ]
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
