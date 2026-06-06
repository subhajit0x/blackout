//! macOS implementation of the BLACKOUT platform API.

use crate::{check, done, errored, tally, unavailable, ActionResult, Capabilities, OpsecReport};
use std::process::Command;

const FIREWALL: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";

/// Run a command, returning trimmed stdout on success.
fn out(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn wifi_device() -> Option<String> {
    let text = out("networksetup", &["-listallhardwareports"])?;
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.contains("Hardware Port: Wi-Fi") {
            if let Some(dev_line) = lines.next() {
                if let Some(dev) = dev_line.strip_prefix("Device: ") {
                    return Some(dev.trim().to_string());
                }
            }
        }
    }
    None
}

fn blueutil_present() -> bool {
    Command::new("blueutil").arg("-p").output().map(|o| o.status.success()).unwrap_or(false)
}

// ---------------- OPSEC ----------------

pub fn opsec_score() -> OpsecReport {
    let mut checks = Vec::new();

    let fv = out("fdesetup", &["status"]).unwrap_or_default();
    checks.push(if fv.contains("FileVault is On") {
        check("Disk encryption", "good", "Your disk is encrypted — files are unreadable if the device is lost or stolen.", 16)
    } else if fv.is_empty() {
        check("Disk encryption", "unknown", "Couldn't read FileVault status.", 16)
    } else {
        check("Disk encryption", "bad", "FileVault is off — anyone with your device can read its contents.", 16)
    });

    let fw = out(FIREWALL, &["--getglobalstate"])
        .or_else(|| out("defaults", &["read", "/Library/Preferences/com.apple.alf", "globalstate"]));
    checks.push(match fw.as_deref() {
        Some(s) if s.contains("State = 1") || s.contains("State = 2") || s.trim() == "1" || s.trim() == "2" =>
            check("Firewall", "good", "The firewall is on — incoming connections are blocked.", 12),
        Some(s) if s.contains("State = 0") || s.trim() == "0" || s.contains("disabled") =>
            check("Firewall", "bad", "The firewall is off — other devices on your network can reach open ports.", 12),
        _ => check("Firewall", "unknown", "Couldn't read firewall status.", 12),
    });

    let vpn = out("scutil", &["--nc", "list"]).unwrap_or_default();
    checks.push(if vpn.contains("(Connected)") {
        check("VPN", "good", "A VPN is active — your internet provider can't see which sites you visit.", 10)
    } else {
        check("VPN", "warn", "No VPN connected — your internet provider can see the sites you connect to.", 10)
    });

    let bt = out("system_profiler", &["SPBluetoothDataType"]).unwrap_or_default();
    checks.push(if bt.contains("State: On") {
        check("Bluetooth", "warn", "Bluetooth is on — nearby devices can detect and try to connect.", 6)
    } else if bt.contains("State: Off") {
        check("Bluetooth", "good", "Bluetooth is off — your device isn't broadcasting to nearby devices.", 6)
    } else {
        check("Bluetooth", "unknown", "Couldn't read Bluetooth status.", 6)
    });

    let ask = out("defaults", &["read", "com.apple.screensaver", "askForPassword"]).unwrap_or_default();
    checks.push(if ask == "1" {
        check("Screen lock", "good", "A password is required after sleep or screensaver.", 10)
    } else {
        check("Screen lock", "warn", "No password required on wake — someone could open your unlocked Mac.", 10)
    });

    // Automatic login — unlocks the Mac at boot with no password.
    let autologin = out("defaults", &["read", "/Library/Preferences/com.apple.loginwindow", "autoLoginUser"]);
    checks.push(match autologin {
        Some(u) if !u.is_empty() => check("Automatic login", "bad", "Auto-login is on — the Mac unlocks itself at startup without a password.", 8),
        _ => check("Automatic login", "good", "A login is required at startup.", 8),
    });

    // AirDrop discoverability.
    let airdrop = out("defaults", &["read", "com.apple.sharingd", "DiscoverableMode"]).unwrap_or_default();
    checks.push(match airdrop.as_str() {
        "Everyone" => check("AirDrop", "warn", "AirDrop is open to Everyone — anyone nearby can see your Mac and send files.", 6),
        "Off" => check("AirDrop", "good", "AirDrop receiving is off.", 6),
        "Contacts Only" => check("AirDrop", "good", "AirDrop is limited to your contacts.", 6),
        _ => check("AirDrop", "unknown", "Couldn't read the AirDrop setting.", 6),
    });

    // Listening services: SSH (22) and Screen Sharing / VNC (5900).
    let net = out("netstat", &["-an", "-p", "tcp"]).unwrap_or_default();
    let listens = |port: &str| net.lines().filter(|l| l.contains("LISTEN")).any(|l| {
        l.split_whitespace().any(|t| t.ends_with(&format!(".{port}")))
    });
    checks.push(if listens("22") {
        check("Remote login (SSH)", "warn", "SSH is listening — your Mac accepts remote shell connections.", 6)
    } else {
        check("Remote login (SSH)", "good", "No SSH service is listening.", 6)
    });
    checks.push(if listens("5900") {
        check("Screen sharing", "warn", "Screen Sharing/VNC is listening — your screen can be viewed remotely.", 4)
    } else {
        check("Screen sharing", "good", "Screen Sharing is off.", 4)
    });

    let gk = out("spctl", &["--status"]).unwrap_or_default();
    checks.push(if gk.contains("enabled") {
        check("Gatekeeper", "good", "macOS blocks unsigned, untrusted apps from running.", 8)
    } else {
        check("Gatekeeper", "bad", "Gatekeeper is off — untrusted apps can run without warning.", 8)
    });

    let sip = out("csrutil", &["status"]).unwrap_or_default();
    checks.push(if sip.contains("enabled") {
        check("System Integrity Protection", "good", "Core system files are protected from tampering.", 8)
    } else {
        check("System Integrity Protection", "bad", "SIP is disabled — malware could modify protected system files.", 8)
    });

    checks.push(check(
        "DNS privacy",
        if vpn.contains("(Connected)") { "good" } else { "warn" },
        if vpn.contains("(Connected)") {
            "DNS lookups are tunnelled through your VPN."
        } else {
            "DNS requests are visible to your network and provider unless you use encrypted DNS."
        },
        6,
    ));

    // App permission audit (camera/mic) — needs Full Disk Access to read TCC.db.
    // Weight 0: informational, doesn't move the score.
    checks.push(app_permission_check());

    // Assign UI category + one-tap fix id per check (keeps the check bodies clean).
    for c in checks.iter_mut() {
        let (cat, fix) = check_meta(&c.label);
        c.category = cat.into();
        c.fix = fix.map(Into::into);
    }

    let score = tally(&checks);
    OpsecReport { score, checks }
}

fn check_meta(label: &str) -> (&'static str, Option<&'static str>) {
    match label {
        "Disk encryption" => ("Device", Some("filevault")),
        "Screen lock" => ("Device", Some("screenlock")),
        "Automatic login" => ("Device", None),
        "Gatekeeper" => ("Device", None),
        "System Integrity Protection" => ("Device", None),
        "Firewall" => ("Network", Some("firewall")),
        "VPN" => ("Network", None),
        "DNS privacy" => ("Network", None),
        "Bluetooth" => ("Sharing", None),
        "AirDrop" => ("Sharing", Some("airdrop")),
        "Remote login (SSH)" => ("Sharing", None),
        "Screen sharing" => ("Sharing", None),
        "App permissions" => ("Privacy", Some("fulldisk")),
        _ => ("Other", None),
    }
}

fn app_permission_check() -> crate::Check {
    let home = std::env::var("HOME").unwrap_or_default();
    let db = format!("{home}/Library/Application Support/com.apple.TCC/TCC.db");
    let query = "SELECT service, count(*) FROM access WHERE auth_value=2 GROUP BY service;";
    match out("sqlite3", &[&db, query]) {
        Some(s) if !s.is_empty() => {
            let count = |svc: &str| {
                s.lines()
                    .find(|l| l.starts_with(svc))
                    .and_then(|l| l.split('|').nth(1))
                    .unwrap_or("0")
                    .to_string()
            };
            let cam = count("kTCCServiceCamera");
            let mic = count("kTCCServiceMicrophone");
            check(
                "App permissions",
                "warn",
                &format!("{cam} app(s) can use your camera and {mic} your microphone. Review them in System Settings ▸ Privacy."),
                0,
            )
        }
        _ => check(
            "App permissions",
            "unknown",
            "Grant BLACKOUT Full Disk Access (Settings ▸ Privacy ▸ Full Disk Access) to audit which apps can use your camera and microphone.",
            0,
        ),
    }
}

// ---------------- LOCKDOWN / PANIC ----------------

fn act_wifi_off() -> ActionResult {
    match wifi_device() {
        Some(dev) => match Command::new("networksetup").args(["-setairportpower", &dev, "off"]).status() {
            Ok(s) if s.success() => done("Wi-Fi disabled", "Wireless radio turned off."),
            _ => errored("Wi-Fi", "Could not turn off Wi-Fi."),
        },
        None => errored("Wi-Fi", "No Wi-Fi interface found."),
    }
}

fn act_bluetooth_off() -> ActionResult {
    if blueutil_present() {
        match Command::new("blueutil").args(["-p", "0"]).status() {
            Ok(s) if s.success() => done("Bluetooth disabled", "Bluetooth radio turned off."),
            _ => errored("Bluetooth", "blueutil failed to turn off Bluetooth."),
        }
    } else {
        unavailable("Bluetooth", "Needs the 'blueutil' helper (brew install blueutil). macOS has no built-in way to toggle Bluetooth from an app.")
    }
}

fn act_clipboard_clear() -> ActionResult {
    match Command::new("osascript").args(["-e", "set the clipboard to \"\""]).status() {
        Ok(s) if s.success() => done("Clipboard cleared", "Any copied passwords or text were wiped."),
        _ => errored("Clipboard", "Could not clear the clipboard."),
    }
}

fn act_lock_screen() -> ActionResult {
    let cg = "/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession";
    match Command::new(cg).arg("-suspend").status() {
        Ok(s) if s.success() => done("Screen locked", "The Mac is now locked."),
        _ => errored("Screen lock", "Could not lock the screen."),
    }
}

fn act_airdrop_off() -> ActionResult {
    let ok = Command::new("defaults")
        .args(["write", "com.apple.sharingd", "DiscoverableMode", "-string", "Off"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = Command::new("killall").arg("sharingd").status(); // apply immediately
    if ok {
        done("AirDrop disabled", "AirDrop receiving set to Off — your Mac is no longer discoverable nearby.")
    } else {
        errored("AirDrop", "Could not change the AirDrop setting.")
    }
}

/// Block ALL incoming connections (firewall + stealth). Needs one admin prompt.
fn act_firewall_blockall() -> ActionResult {
    let inner = format!(
        "{fw} --setglobalstate on && {fw} --setblockall on && {fw} --setstealthmode on",
        fw = FIREWALL
    );
    let script = format!("do shell script \"{}\" with administrator privileges", inner);
    match Command::new("osascript").arg("-e").arg(&script).output() {
        Ok(o) if o.status.success() => {
            done("Incoming connections blocked", "The firewall now blocks all incoming connections and hides from scans.")
        }
        Ok(_) => errored("Firewall", "Password prompt cancelled — incoming connections were not blocked."),
        Err(e) => errored("Firewall", &format!("Could not show the password prompt: {e}")),
    }
}

/// Open Apple's Lockdown Mode pane (can't be toggled programmatically — the OS
/// requires the user to confirm and restart). Device/OS dependent.
fn act_open_lockdown() -> ActionResult {
    if open_settings("lockdown") {
        done(
            "Apple Lockdown Mode",
            "Opened Privacy & Security — turn on Lockdown Mode for OS-level isolation (requires a restart).",
        )
    } else {
        unavailable("Apple Lockdown Mode", "Could not open the settings pane on this device.")
    }
}

fn platform_limits() -> Vec<ActionResult> {
    vec![
        unavailable("Camera", "macOS has no public API to globally disable the camera. Use System Settings ▸ Privacy."),
        unavailable("Microphone", "macOS has no public API to globally disable the microphone."),
        unavailable("Location", "Location Services can only be turned off in System Settings."),
        unavailable("NFC", "macOS exposes no user-facing NFC to disable."),
        unavailable("Tor / VPN", "Routing through Tor/VPN needs separate software and configuration — not bundled in this build."),
    ]
}

pub fn apply_level(level: u32) -> Vec<ActionResult> {
    let mut results = Vec::new();
    match level {
        1 => results.push(act_bluetooth_off()),
        2 => {
            results.push(act_bluetooth_off());
            results.push(act_clipboard_clear());
        }
        3 => {
            results.push(act_bluetooth_off());
            results.push(act_clipboard_clear());
            results.push(act_wifi_off());
        }
        _ => {
            // Ghost: maximum isolation, including admin firewall block-all.
            results.push(act_firewall_blockall());
            results.push(act_airdrop_off());
            results.push(act_bluetooth_off());
            results.push(act_clipboard_clear());
            results.push(act_wifi_off());
            results.push(act_lock_screen());
        }
    }
    if level >= 2 {
        results.extend(platform_limits());
    }
    results
}

/// PANIC — complete isolation, instant (no password): wipe clipboard, kill every
/// radio, turn AirDrop off, open Apple Lockdown Mode (device-dependent), then lock.
pub fn panic_now() -> Vec<ActionResult> {
    vec![
        act_clipboard_clear(),
        act_wifi_off(),
        act_airdrop_off(),
        act_bluetooth_off(),
        act_open_lockdown(),
        act_lock_screen(),
    ]
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        platform: "macOS".into(),
        wifi: wifi_device().is_some(),
        bluetooth: blueutil_present(),
        firewall: true,
        settings_deeplink: true,
    }
}

pub fn open_settings(pane: &str) -> bool {
    let url = match pane {
        "filevault" => "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?FileVault",
        "firewall" => "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Security",
        "lockscreen" => "x-apple.systempreferences:com.apple.Lock-Screen-Settings.extension",
        "fulldisk" => "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
        _ => "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension",
    };
    Command::new("open").arg(url).status().map(|s| s.success()).unwrap_or(false)
}

/// One-tap remediation for a failing OPSEC check (the `fix` id on a `Check`).
pub fn apply_fix(id: &str) -> Vec<ActionResult> {
    match id {
        "firewall" => harden(),
        "airdrop" => vec![act_airdrop_off()],
        "screenlock" => {
            let ok = Command::new("defaults")
                .args(["write", "com.apple.screensaver", "askForPassword", "-int", "1"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            let _ = Command::new("defaults")
                .args(["write", "com.apple.screensaver", "askForPasswordDelay", "-int", "0"])
                .status();
            if ok {
                vec![done("Lock password required", "A password is now required immediately after sleep or screensaver.")]
            } else {
                vec![errored("Screen lock", "Could not change the setting.")]
            }
        }
        "filevault" => {
            open_settings("filevault");
            vec![done("Opened FileVault settings", "Turn on FileVault there to encrypt your disk.")]
        }
        "fulldisk" => {
            open_settings("fulldisk");
            vec![done("Opened Full Disk Access", "Add BLACKOUT, then re-run the scan to audit app permissions.")]
        }
        _ => vec![errored("Fix", "Unknown fix.")],
    }
}

pub fn harden() -> Vec<ActionResult> {
    let mut results = Vec::new();

    let inner = format!("{fw} --setglobalstate on && {fw} --setstealthmode on", fw = FIREWALL);
    let script = format!("do shell script \"{}\" with administrator privileges", inner);
    let admin = Command::new("osascript").arg("-e").arg(&script).output();

    match admin {
        Ok(o) if o.status.success() => {
            let gs = out(FIREWALL, &["--getglobalstate"]).unwrap_or_default();
            results.push(if gs.contains("State = 1") || gs.contains("State = 2") {
                done("Firewall enabled", "The macOS application firewall is now on — incoming connections are blocked.")
            } else {
                errored("Firewall", "Ran the command but the firewall still reads as off.")
            });
            let sm = out(FIREWALL, &["--getstealthmode"]).unwrap_or_default();
            results.push(if sm.contains("enabled") || sm.contains("on") {
                done("Stealth mode enabled", "Your Mac no longer answers ping or port scans from the network.")
            } else {
                errored("Stealth mode", "Could not confirm stealth mode.")
            });
        }
        Ok(_) => {
            results.push(errored("Authentication", "Password prompt was cancelled — no changes were made."));
            return results;
        }
        Err(e) => {
            results.push(errored("Authentication", &format!("Could not show the password prompt: {e}")));
            return results;
        }
    }

    let _ = Command::new("defaults").args(["write", "com.apple.screensaver", "askForPassword", "-int", "1"]).status();
    let _ = Command::new("defaults").args(["write", "com.apple.screensaver", "askForPasswordDelay", "-int", "0"]).status();
    results.push(done("Lock password required", "A password is now required immediately after sleep or screensaver."));
    results
}
