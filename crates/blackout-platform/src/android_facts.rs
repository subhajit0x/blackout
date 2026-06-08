//! Build a real Android OPSEC report from live facts gathered by the native
//! plugin. Kept platform-agnostic (plain bools in) so it compiles everywhere and
//! the app's command layer can call it on Android. Mirrors the macOS scoring.

use crate::{check, device, step, tally, GuideStep, OpsecReport};

/// Live device state read by the native Android plugin.
#[derive(Debug, Default, Clone)]
pub struct AndroidFacts {
    pub vpn_active: bool,
    pub wifi_on: bool,
    pub bluetooth_on: bool,
    pub airplane_on: bool,
    pub screen_lock_set: bool,
    pub developer_options: bool,
    pub location_on: bool,
    pub sdk_int: i32,
    pub os_version: String,
    pub model: String,
}

/// `(panel id for open_panel, how-to text)` for a failing check, by label.
fn how(label: &str) -> Option<(&'static str, &'static str)> {
    Some(match label {
        "Screen lock" => ("security", "Settings ▸ Security ▸ Screen lock ▸ set a PIN or password (this also encrypts the device)."),
        "Encrypted tunnel (VPN/Tor)" => ("wifi", "Connect a trusted no-logs VPN, or route through Tor with Orbot, before using untrusted networks."),
        "Bluetooth" => ("bluetooth", "Turn Bluetooth off when you're not actively using it."),
        "Airplane mode" => ("airplane", "Toggle Airplane mode to cut every radio at once."),
        "Developer options" => ("security", "Settings ▸ System ▸ Developer options ▸ turn off (and disable USB debugging)."),
        "Location services" => ("location", "Settings ▸ Location ▸ turn off, or revoke location from apps that don't need it."),
        _ => return None,
    })
}

fn cat(label: &str) -> &'static str {
    match label {
        "Screen lock" => "Device",
        "Developer options" => "Device",
        "Encrypted tunnel (VPN/Tor)" => "Network",
        "Location services" => "Privacy",
        _ => "Sharing",
    }
}

pub fn opsec_from_facts(f: &AndroidFacts) -> OpsecReport {
    let mut checks = vec![
        if f.screen_lock_set {
            check("Screen lock", "good", "A secure lock is set — your storage is encrypted and protected if the device is lost.", 18)
        } else {
            check("Screen lock", "bad", "No secure lock — anyone can open the phone and the storage isn't encrypted.", 18)
        },
        if f.vpn_active {
            check("Encrypted tunnel (VPN/Tor)", "good", "An encrypted tunnel is active — your provider can't see which sites you visit.", 14)
        } else {
            check("Encrypted tunnel (VPN/Tor)", "warn", "No VPN/Tor tunnel — your carrier or network can log the sites you connect to.", 14)
        },
        if f.bluetooth_on {
            check("Bluetooth", "warn", "Bluetooth is on — nearby devices can detect and try to pair.", 8)
        } else {
            check("Bluetooth", "good", "Bluetooth is off — not broadcasting to nearby devices.", 8)
        },
        if f.developer_options {
            check("Developer options", "warn", "Developer options are on — USB debugging can expose the device to a connected computer.", 8)
        } else {
            check("Developer options", "good", "Developer options are off.", 8)
        },
        if f.location_on {
            check("Location services", "warn", "Location is on — apps with permission can track where you are.", 6)
        } else {
            check("Location services", "good", "Location services are off.", 6)
        },
        if f.airplane_on {
            check("Airplane mode", "good", "Airplane mode is on — every radio is cut.", 6)
        } else {
            check("Airplane mode", "warn", "Radios are live. Airplane mode is the fastest way to cut them all at once.", 6)
        },
    ];

    for c in checks.iter_mut() {
        c.category = cat(&c.label).into();
        c.fix = how(&c.label).map(|(panel, _)| panel.to_string());
    }

    // Prioritized guide from the failing checks (worst first).
    let mut failing: Vec<&crate::Check> = checks
        .iter()
        .filter(|c| matches!(c.status.as_str(), "bad" | "warn"))
        .collect();
    failing.sort_by(|a, b| {
        let sev = |s: &str| if s == "bad" { 0 } else { 1 };
        sev(&a.status).cmp(&sev(&b.status)).then(b.weight.cmp(&a.weight))
    });
    let mut guide: Vec<GuideStep> = Vec::new();
    for c in failing {
        if let Some((panel, how_to)) = how(&c.label) {
            let sev = if c.status == "bad" { "high" } else { "medium" };
            guide.push(step(&c.label, sev, &c.detail, how_to, Some(panel)));
        }
    }

    let score = tally(&checks);
    let dev = device("Android", &f.os_version, &f.model);
    OpsecReport { score, device: dev, checks, guide }
}
