const COMMANDS: &[&str] = &["opsec_facts", "open_panel", "clear_clipboard"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();

    // Permissions we need declared in the merged AndroidManifest. ACCESS_NETWORK_STATE
    // is what lets us read the VPN transport; the rest are read-only state queries.
    let perms = [
        r#"<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />"#,
        r#"<uses-permission android:name="android.permission.ACCESS_WIFI_STATE" />"#,
        r#"<uses-permission android:name="android.permission.BLUETOOTH" android:maxSdkVersion="30" />"#,
        r#"<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />"#,
        // See every installed app so we can surface sideloaded / suspicious ones
        // (a security tool — not for Play Store distribution).
        r#"<uses-permission android:name="android.permission.QUERY_ALL_PACKAGES" />"#,
    ];
    tauri_plugin::mobile::update_android_manifest(
        "BLACKOUT DROID PLUGIN",
        "manifest",
        perms.join("\n"),
    )
    .expect("failed to rewrite AndroidManifest.xml");
}
