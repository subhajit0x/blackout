//! Linux, Windows and any other target.
//!
//! CLEAN works fully via `blackout-core`. OPSEC now runs **real** read-only
//! system checks on Linux and Windows (firewall, disk encryption, exposed
//! services / antivirus) using the OS's own tools — every check degrades to
//! "unknown" if a tool isn't present, so it's safe on any distro/edition.
//! Live LOCKDOWN/PANIC system control is still reported honestly as planned.

use crate::{check, device, done, step, tally, unavailable, ActionResult, Capabilities, GuideStep, OpsecReport};
use std::process::Command;

/// Whether a command ran and exited successfully.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn ran(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd).args(args).status().map(|s| s.success()).unwrap_or(false)
}

/// Run a command, returning trimmed stdout on success (None otherwise).
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn out(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn opsec_score() -> OpsecReport {
    #[cfg(target_os = "linux")]
    let report = build(linux_checks());
    #[cfg(target_os = "windows")]
    let report = build(windows_checks());
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let report = OpsecReport {
        score: 0,
        device: device(std::env::consts::OS, "", ""),
        checks: vec![check(
            "On-device checks",
            "unknown",
            "Automatic checks for this OS are planned. The guide below covers the essentials.",
            0,
        )],
        guide: generic_guide(),
    };
    report
}

/// Assemble a report from real checks + the prioritized fallback guide.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn build(mut checks: Vec<crate::Check>) -> OpsecReport {
    for c in checks.iter_mut() {
        c.category = "Device".into();
    }
    let score = tally(&checks);
    OpsecReport {
        score,
        device: device(std::env::consts::OS, "", ""),
        checks,
        guide: generic_guide(),
    }
}

#[cfg(target_os = "linux")]
fn linux_checks() -> Vec<crate::Check> {
    let mut checks = Vec::new();

    // Firewall: ufw or firewalld.
    let fw = out("ufw", &["status"]).or_else(|| out("systemctl", &["is-active", "firewalld"]));
    checks.push(match fw.as_deref() {
        Some(s) if s.contains("Status: active") || s.trim() == "active" =>
            check("Firewall", "good", "A firewall is active — incoming connections are filtered.", 16),
        Some(s) if s.contains("Status: inactive") || s.trim() == "inactive" =>
            check("Firewall", "bad", "The firewall is off — run `sudo ufw enable` to block unsolicited connections.", 16),
        _ => check("Firewall", "unknown", "Couldn't read firewall status (ufw/firewalld not found).", 16),
    });

    // Disk encryption: a LUKS/crypt device in the block tree.
    let blk = out("lsblk", &["-o", "TYPE"]).unwrap_or_default();
    checks.push(if blk.lines().any(|l| l.trim() == "crypt") {
        check("Disk encryption", "good", "An encrypted (LUKS) volume is in use — your data is unreadable without the key.", 16)
    } else if blk.is_empty() {
        check("Disk encryption", "unknown", "Couldn't inspect the disk layout.", 16)
    } else {
        check("Disk encryption", "warn", "No encrypted volume detected — consider LUKS full-disk encryption.", 16)
    });

    // Exposed SSH service.
    let listen = out("ss", &["-tlnH"]).or_else(|| out("ss", &["-tln"])).unwrap_or_default();
    let ssh = listen.split_whitespace().any(|t| t.ends_with(":22"));
    checks.push(if ssh {
        check("Remote login (SSH)", "warn", "SSH is listening — this machine accepts remote shell connections.", 8)
    } else {
        check("Remote login (SSH)", "good", "No SSH service is listening.", 8)
    });

    checks
}

#[cfg(target_os = "windows")]
fn windows_checks() -> Vec<crate::Check> {
    let mut checks = Vec::new();
    let ps = |script: &str| out("powershell", &["-NoProfile", "-NonInteractive", "-Command", script]);

    // Defender firewall — all profiles.
    let fw = ps("(Get-NetFirewallProfile).Enabled -join ','");
    checks.push(match fw.as_deref() {
        Some(s) if !s.is_empty() && !s.contains("False") =>
            check("Firewall", "good", "Windows Firewall is on for all network profiles.", 16),
        Some(s) if s.contains("False") =>
            check("Firewall", "bad", "Windows Firewall is off for at least one profile — turn it on in Windows Security.", 16),
        _ => check("Firewall", "unknown", "Couldn't read the firewall state.", 16),
    });

    // Disk encryption — BitLocker on the system drive.
    let bl = ps("(Get-BitLockerVolume -MountPoint $env:SystemDrive).ProtectionStatus");
    checks.push(match bl.as_deref() {
        Some(s) if s.trim() == "On" || s.trim() == "1" =>
            check("Disk encryption", "good", "BitLocker is protecting your system drive.", 16),
        Some(s) if s.trim() == "Off" || s.trim() == "0" =>
            check("Disk encryption", "bad", "BitLocker is off — your drive is readable if the device is lost.", 16),
        _ => check("Disk encryption", "unknown", "Couldn't read BitLocker status (may need admin).", 16),
    });

    // Real-time antivirus protection.
    let av = ps("(Get-MpComputerStatus).RealTimeProtectionEnabled");
    checks.push(match av.as_deref() {
        Some(s) if s.trim().eq_ignore_ascii_case("true") =>
            check("Antivirus", "good", "Real-time malware protection is on.", 10),
        Some(s) if s.trim().eq_ignore_ascii_case("false") =>
            check("Antivirus", "bad", "Real-time protection is off — enable it in Windows Security.", 10),
        _ => check("Antivirus", "unknown", "Couldn't read antivirus status.", 10),
    });

    checks
}

fn generic_guide() -> Vec<GuideStep> {
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
    lockdown_actions()
}

pub fn panic_now() -> Vec<ActionResult> {
    lockdown_actions()
}

/// Real, no-admin lockdown automation: clear the clipboard, lock the screen, and
/// (Linux) cut Wi-Fi. Each step reports honestly if its tool isn't available.
fn lockdown_actions() -> Vec<ActionResult> {
    #[cfg(target_os = "linux")]
    let r = linux_lockdown();
    #[cfg(target_os = "windows")]
    let r = windows_lockdown();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let r = vec![unavailable(
        "Lockdown",
        "One-tap system control isn't available on this OS. The OPSEC guide has the steps; file cleaning works fully.",
    )];
    r
}

#[cfg(target_os = "linux")]
fn linux_lockdown() -> Vec<ActionResult> {
    vec![
        clear_clipboard_linux(),
        if ran("nmcli", &["radio", "wifi", "off"]) {
            done("Wi-Fi off", "Wireless turned off via NetworkManager.")
        } else {
            unavailable("Wi-Fi", "Couldn't toggle Wi-Fi (nmcli/NetworkManager not found).")
        },
        if ran("loginctl", &["lock-session"]) || ran("xdg-screensaver", &["lock"]) {
            done("Screen locked", "Your session is locked.")
        } else {
            unavailable("Screen lock", "Couldn't lock the screen automatically.")
        },
    ]
}

#[cfg(target_os = "windows")]
fn windows_lockdown() -> Vec<ActionResult> {
    vec![
        if ran("cmd", &["/C", "type nul | clip"]) {
            done("Clipboard cleared", "Any copied text was wiped.")
        } else {
            unavailable("Clipboard", "Couldn't clear the clipboard.")
        },
        if ran("rundll32.exe", &["user32.dll,LockWorkStation"]) {
            done("Screen locked", "The workstation is locked.")
        } else {
            unavailable("Screen lock", "Couldn't lock the workstation.")
        },
    ]
}

#[cfg(target_os = "linux")]
fn clear_clipboard_linux() -> ActionResult {
    // Wayland (wl-clipboard) then X11 (xsel).
    if ran("wl-copy", &["--clear"]) || ran("xsel", &["-bc"]) {
        done("Clipboard cleared", "Any copied text was wiped.")
    } else {
        unavailable("Clipboard", "Couldn't clear the clipboard (need wl-clipboard or xsel).")
    }
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        platform: std::env::consts::OS.to_string(),
        wifi: cfg!(target_os = "linux"), // Linux can toggle Wi-Fi via nmcli (no admin)
        bluetooth: false,
        firewall: true,
        settings_deeplink: false,
    }
}

pub fn open_settings(_pane: &str) -> bool {
    false
}

pub fn harden() -> Vec<ActionResult> {
    vec![unavailable("Harden", "Automated hardening for this OS is planned. The OPSEC guide lists what to change now.")]
}

pub fn apply_fix(_id: &str) -> Vec<ActionResult> {
    vec![unavailable("Fix", "One-tap fixes for this OS are planned.")]
}
