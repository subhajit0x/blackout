# BLACKOUT

> **One Tap. Less Exposure.**

A privacy-first, offline, open-source app that reduces digital residue. No accounts,
no cloud, no telemetry, no network calls. Everything runs locally on your device.

This is a **cross-platform monorepo**: one Rust core, one shared UI, building for
macOS, Windows, Linux, **iOS and Android** from the same codebase.

> **Free & open source. Not on any app store — only here.**
> 📥 **[How to install →](INSTALL.md)** · Android (direct APK) · iPhone (build on a Mac) · Desktop
>
> Android users sideload a signed APK from [Releases](../../releases). iPhone users
> build it on their Mac with a free Apple ID. Or build any platform from source —
> that's the point of open source: don't trust the binary, make your own.

---

## Modules

| Module | What it does | Status |
|---|---|---|
| **CLEAN** | Strip hidden metadata (GPS, camera, author, IPTC, …) from files before sharing. Originals never touched. | ✅ Works on every platform (pure Rust) |
| **OPSEC SCORE** | Read-only exposure scorecard in plain language. | ✅ macOS · Android · Windows · Linux · 🟡 iOS guide |
| **LOCKDOWN** | Reduce device exposure by privacy level. | ✅ macOS · Android (panels + clipboard) · 🟡 Win/Linux/iOS guide |
| **PANIC** | One tap: wipe clipboard, kill radios, lock screen. | ✅ macOS · Android · 🟡 Win/Linux/iOS guide |

The honesty rule: anything an OS won't let an app do (Apple Lockdown Mode, global
camera/mic disable, iOS radio toggles) is **clearly labelled "not available"** or
deep-links you to the system toggle — never faked.

### What actually works on each platform

| | macOS | Windows / Linux | Android | iOS |
|---|---|---|---|---|
| **CLEAN** (metadata removal) | ✅ full | ✅ full | ✅ full | ✅ full |
| **OPSEC** score (live checks) | ✅ 13 checks + fixes | ✅ core checks | ✅ 9 checks + fixes | 🟡 guide only |
| **OPSEC** device guide | ✅ | ✅ | ✅ | ✅ |
| **LOCKDOWN / PANIC** actions | ✅ real (radios, AirDrop, firewall, lock) | 🟡 planned | 🟡 guide only* | 🟡 guide only* |
| Finder right-click clean | ✅ | — | — | — |
| Auto-clean watched folder | ✅ | ✅ | — | — |

\* Android/iOS sandbox apps from silently toggling radios — the **OPSEC guide tells
you exactly how to do it in Settings** for your device. A native Android plugin
(opens Wi-Fi/Bluetooth/Airplane panels + clears clipboard) is the planned next step.

So: **metadata cleaning and the device-tailored OPSEC guide work everywhere**;
full live hardening is a macOS feature today and rolls out per-platform.

---

## Repository layout (designed for portability)

```
blackout/
├── crates/
│   ├── blackout-core/        # CLEAN engine — pure Rust, ZERO OS-specific code.
│   │                         #   No C deps → cross-compiles anywhere.
│   ├── blackout-platform/    # OPSEC / LOCKDOWN / PANIC. ONE public API,
│   │   └── src/              #   implemented per-OS behind cfg gates:
│   │       ├── lib.rs        #     shared types + dispatch
│   │       ├── macos.rs      #     #[cfg(target_os = "macos")]
│   │       ├── ios.rs        #     #[cfg(target_os = "ios")]
│   │       ├── android.rs    #     #[cfg(target_os = "android")]
│   │       └── other.rs      #     Linux / Windows / fallback
│   └── blackout-cli/         # terminal front-end (desktop)
└── app/                      # universal Tauri app — ONE web UI for all platforms
    ├── src/                  #   frontend: index.html · main.js · styles.css
    └── src-tauri/
        ├── src/              #   thin command layer → core + platform
        └── gen/
            ├── apple/        #   iOS Xcode project   (tauri ios init)
            └── android/      #   Android Studio proj (tauri android init)
```

**Why this is portable:** all OS-specific code lives in exactly one place
(`blackout-platform`, behind `cfg` gates). Add support for a new OS = add one file.
The engine and UI never change. The CLEAN engine has no C dependencies, so it
cross-compiles to any target without a C toolchain.

---

## Building

### Desktop (macOS / Windows / Linux)
```bash
cd app
npm install
npm run tauri build        # → .app/.dmg (mac), .msi (win), .deb/.AppImage (linux)
npm run tauri dev          # hot-reload dev mode
```

### iOS  (needs Xcode + CocoaPods)
```bash
cd app
npm run tauri ios init     # one-time: generates gen/apple/*.xcodeproj
npm run tauri ios dev      # run on Simulator
npm run tauri ios build    # archive .ipa (requires an Apple signing identity)
```

### Android  (needs Android SDK + NDK)
```bash
export ANDROID_HOME=/path/to/android-sdk
export NDK_HOME="$ANDROID_HOME/ndk/<version>"
cd app
npm run tauri android init # one-time: generates gen/android Studio project
npm run tauri android dev  # run on emulator/device
npm run tauri android build
```

### CLI / library crates
```bash
cargo build -p blackout-cli --release     # terminal tool
cargo test                                # engine + platform unit tests
```

### Cross-compile check (proves portability)
```bash
rustup target add aarch64-apple-ios aarch64-linux-android
cargo build -p blackout-core -p blackout-platform --target aarch64-apple-ios
cargo build -p blackout-core -p blackout-platform --target aarch64-linux-android
```

---

## Verified portability

| Target | core | platform | full app |
|---|---|---|---|
| macOS (aarch64) | ✅ | ✅ | ✅ runs |
| iOS device (aarch64-apple-ios) | ✅ | ✅ | ✅ compiles |
| iOS Simulator (aarch64-apple-ios-sim) | ✅ | ✅ | ✅ compiles |
| Android arm64 (aarch64-linux-android) | ✅ | ✅ | ✅ links (NDK) |

iOS and Android projects are scaffolded under `app/src-tauri/gen/`. Full device
builds need the usual signing (iOS) and an emulator/device (Android).

---

## Principles

- **Local only** — no network, ever. No telemetry, analytics, accounts, or ads.
- **Originals are sacred** — CLEAN only writes copies.
- **Honest** — if an OS blocks something, we say so; we never fake an action.
- **Open source** — MIT OR Apache-2.0.
