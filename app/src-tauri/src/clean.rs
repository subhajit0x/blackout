//! CLEAN module commands — thin wrappers over the proven `blackout-core` engine.

use blackout_core::{clean_file, ffmpeg_available, inspect_file, CleanReport};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct CleanRunResult {
    pub out_dir: String,
    pub reports: Vec<CleanReport>,
    pub cleaned: usize,
    pub copied: usize,
    pub skipped: usize,
    pub errored: usize,
    pub ffmpeg: bool,
    /// Android: at least one cleaned file was saved and can be shared.
    pub shareable: bool,
}

/// Android: the URIs (JSON-encoded) of the files saved by the last clean, so the
/// user can share them straight to another app.
#[derive(Default)]
pub struct LastCleaned(pub std::sync::Mutex<Vec<String>>);

/// Where cleaned copies land: ~/Desktop/BLACKOUT-clean (falls back to home).
fn default_out_dir() -> PathBuf {
    let base = dirs::desktop_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("BLACKOUT-clean")
}

/// Inspect files (report only, nothing written).
#[tauri::command]
pub fn inspect_files(paths: Vec<String>) -> Vec<CleanReport> {
    paths
        .iter()
        .map(|p| inspect_file(Path::new(p)))
        .collect()
}

/// Clean files into the output directory, returning a per-file report + tallies.
#[tauri::command]
pub fn clean_files(paths: Vec<String>) -> CleanRunResult {
    let out_dir = default_out_dir();
    let ffmpeg = ffmpeg_available();

    let reports: Vec<CleanReport> = paths
        .iter()
        .map(|p| clean_file(Path::new(p), &out_dir, ffmpeg))
        .collect();

    let (cleaned, copied, skipped, errored) = tally(&reports);
    CleanRunResult {
        out_dir: out_dir.to_string_lossy().to_string(),
        reports,
        cleaned,
        copied,
        skipped,
        errored,
        ffmpeg,
        shareable: false,
    }
}

/// Count outcomes for the summary banner. "skipped" and "unsupported" both mean
/// "nothing was written, but nothing went wrong" — kept out of the error count.
fn tally(reports: &[CleanReport]) -> (usize, usize, usize, usize) {
    let (mut cleaned, mut copied, mut skipped, mut errored) = (0, 0, 0, 0);
    for r in reports {
        match r.status.as_str() {
            "cleaned" => cleaned += 1,
            "copied" => copied += 1,
            "unsupported" | "skipped" => skipped += 1,
            _ => errored += 1,
        }
    }
    (cleaned, copied, skipped, errored)
}

/// Reveal a file or folder in Finder (desktop only; a no-op elsewhere).
#[tauri::command]
pub fn reveal_path(path: String) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let opener = if cfg!(target_os = "windows") { "explorer" }
            else if cfg!(target_os = "macos") { "open" }
            else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(&path).spawn();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let _ = path;
}

// ---------------------------------------------------------------------------
// Android: the picker hands back content:// URIs that std::fs can't read. We use
// the platform file API to read the bytes, clean them with the same engine, and
// save the clean copy into the public Downloads folder. This never surfaces a
// raw error — a file we can't handle is reported calmly and skipped.
// ---------------------------------------------------------------------------

/// Pick files and clean them in place on platforms without real file paths
/// (Android). On desktop this isn't used (drag-drop / Browse give real paths).
#[tauri::command]
pub async fn clean_picked(app: tauri::AppHandle) -> CleanRunResult {
    #[cfg(target_os = "android")]
    {
        // The picker is a blocking UI call — run it off the main thread.
        return tauri::async_runtime::spawn_blocking(move || clean_picked_blocking(&app))
            .await
            .unwrap_or_else(|_| empty_result());
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        empty_result()
    }
}

fn empty_result() -> CleanRunResult {
    CleanRunResult {
        out_dir: "Downloads/BLACKOUT-clean".into(),
        reports: vec![],
        cleaned: 0,
        copied: 0,
        skipped: 0,
        errored: 0,
        ffmpeg: false,
        shareable: false,
    }
}

#[cfg(target_os = "android")]
fn clean_picked_blocking(app: &tauri::AppHandle) -> CleanRunResult {
    use blackout_core::clean_named_bytes;
    use tauri::Manager;
    use tauri_plugin_android_fs::{AndroidFsExt, PublicGeneralPurposeDir};

    let api = app.android_fs();
    // Legacy external-storage permission (Android 9 and below); a no-op on 10+.
    let _ = api.public_storage().request_permission();

    let uris = match api.file_picker().pick_files(None, &["*/*"], false) {
        Ok(u) => u,
        Err(_) => return empty_result(), // cancelled or unavailable — stay calm
    };

    let mut reports: Vec<CleanReport> = Vec::new();
    let mut saved_uris: Vec<String> = Vec::new();
    for uri in &uris {
        let name = api.get_name(uri).unwrap_or_else(|_| "file".to_string());
        let bytes = match api.read(uri) {
            Ok(b) => b,
            Err(_) => {
                reports.push(soft_skip(&name, "Couldn't read this file."));
                continue;
            }
        };
        let (mut report, out) = clean_named_bytes(&name, bytes);
        if let Some(out_bytes) = out {
            let rel = format!("BLACKOUT-clean/{name}");
            let dest = api.public_storage().create_new_file(
                None,
                PublicGeneralPurposeDir::Download,
                rel.as_str(),
                None,
            );
            match dest.and_then(|d| api.write(&d, &out_bytes).map(|_| d)) {
                Ok(d) => {
                    if let Ok(json) = d.to_json_string() {
                        saved_uris.push(json);
                    }
                }
                Err(_) => {
                    report.status = "skipped".to_string();
                    report.notes = vec!["Cleaned, but couldn't save to Downloads.".to_string()];
                }
            }
        }
        reports.push(report);
    }

    // Remember what we saved so the user can share it in one tap.
    let shareable = !saved_uris.is_empty();
    *app.state::<LastCleaned>().0.lock().unwrap() = saved_uris;

    let (cleaned, copied, skipped, errored) = tally(&reports);
    CleanRunResult {
        out_dir: "Downloads/BLACKOUT-clean".into(),
        reports,
        cleaned,
        copied,
        skipped,
        errored,
        ffmpeg: false,
        shareable,
    }
}

/// Android: open the system share sheet for the files saved by the last clean.
#[tauri::command]
pub async fn share_cleaned(app: tauri::AppHandle) {
    #[cfg(target_os = "android")]
    {
        let _ = tauri::async_runtime::spawn_blocking(move || share_cleaned_blocking(&app)).await;
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

#[cfg(target_os = "android")]
fn share_cleaned_blocking(app: &tauri::AppHandle) {
    use tauri::Manager;
    use tauri_plugin_android_fs::{AndroidFsExt, FileUri};

    let jsons = app.state::<LastCleaned>().0.lock().unwrap().clone();
    let uris: Vec<FileUri> = jsons
        .iter()
        .filter_map(|j| FileUri::from_json_str(j).ok())
        .collect();
    if !uris.is_empty() {
        let _ = app.android_fs().file_opener().share_files(uris.iter());
    }
}

/// A calm "nothing written, nothing broke" report.
#[cfg(target_os = "android")]
fn soft_skip(name: &str, note: &str) -> CleanReport {
    CleanReport {
        source: PathBuf::from(name),
        category: "file".into(),
        status: "skipped".into(),
        output: None,
        removed: vec![],
        notes: vec![note.into()],
        findings: vec![],
        bytes_before: None,
        bytes_after: None,
    }
}
