# Installing BLACKOUT

BLACKOUT is **not** on any app store — it's open-source and distributed only here
on GitHub. Pick your platform.

---

## 📱 Android — direct install (no Play Store)

**The easy way (prebuilt APK):**

1. Go to the [**Releases**](../../releases) page.
2. Download `BLACKOUT-arm64.apk` (works on virtually all modern phones).
3. Tap the downloaded file. Android will say *"For your security, your phone
   isn't allowed to install unknown apps from this source."* → tap **Settings**
   → enable **Allow from this source** → go back → **Install**.
4. Open BLACKOUT. Done.

> The APK is signed so it installs and updates cleanly. It is **not** debuggable.
> Because it's not from the Play Store, Android shows the one-time "unknown
> source" prompt — that's expected for any sideloaded app.

**Build it yourself instead** (recommended if you don't trust a prebuilt binary —
the whole point of open source):

```bash
# Prereqs: Android Studio (SDK + NDK), Java 17, Rust, Node
export ANDROID_HOME=~/Library/Android/sdk            # or your SDK path
export NDK_HOME="$ANDROID_HOME/ndk/<version>"
git clone <this-repo> && cd blackout/app
npm install
npm run tauri android init
npm run tauri android build --apk --target aarch64
# APK appears under app/src-tauri/gen/android/app/build/outputs/apk/
```

---

## 🍎 iPhone / iPad — build it yourself on a Mac

Apple doesn't allow sideloading signed apps the way Android does, so iOS users
**build and install from their own Mac** using a **free Apple ID** (no paid
Developer account needed — the app stays valid for 7 days, then just rebuild).

```bash
# Prereqs on the Mac: Xcode (from the App Store), Rust, Node, CocoaPods
git clone <this-repo> && cd blackout/app
npm install
npm run tauri ios init
npm run tauri ios dev          # builds + runs on a connected iPhone or the Simulator
```

If `ios dev` can't sign:
1. `open app/src-tauri/gen/apple/blackout-app.xcodeproj` in Xcode.
2. Select the **blackout_app** target → **Signing & Capabilities**.
3. Set **Team** to your personal Apple ID (Xcode ▸ Settings ▸ Accounts ▸ add
   your Apple ID — it's free).
4. Plug in your iPhone, select it as the run target, press ▶.
5. On the phone: **Settings ▸ General ▸ VPN & Device Management** → trust your
   developer certificate. Launch BLACKOUT.

> On iOS, the **CLEAN** module works fully. LOCKDOWN/OPSEC/PANIC are limited by
> Apple's sandbox (the app says so honestly) — the natural iOS surface for CLEAN
> is the share sheet.

---

## 💻 Desktop — macOS / Windows / Linux

**Prebuilt:** download the installer for your OS from [Releases](../../releases)
(`.dmg` on macOS, `.msi`/`.exe` on Windows, `.AppImage`/`.deb` on Linux).

> macOS note: the prebuilt `.dmg` is signed for development, not notarized, so on
> a Mac that isn't the build machine, Gatekeeper shows a warning. Right-click the
> app → **Open** → **Open** the first time. (Notarization needs a paid Apple
> Developer ID — see the README.) Or build it yourself:

```bash
git clone <this-repo> && cd blackout/app
npm install
npm run tauri build        # produces the installer for your current OS
# also: `npm run tauri dev` for a hot-reload dev build
```

The optional **Finder right-click "Clean with BLACKOUT"** Quick Action:
```bash
bash app/quickaction/install-quickaction.sh
```

---

## Build prerequisites (all platforms)

| Tool | Why | Install |
|---|---|---|
| **Rust** | the engine + backend | https://rustup.rs |
| **Node + npm** | the Tauri tooling & UI | https://nodejs.org |
| **Xcode** | macOS & iOS builds | Mac App Store |
| **Android Studio** | Android SDK + NDK | https://developer.android.com/studio |
| **Java 17** | Android Gradle build | `brew install openjdk@17` |

Verify the engine compiles for every target:
```bash
rustup target add aarch64-apple-ios aarch64-linux-android
cargo build -p blackout-core -p blackout-platform --target aarch64-apple-ios
cargo build -p blackout-core -p blackout-platform --target aarch64-linux-android
```
