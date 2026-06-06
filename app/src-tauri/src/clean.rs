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
}

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

    let (mut cleaned, mut copied, mut skipped, mut errored) = (0, 0, 0, 0);
    for r in &reports {
        match r.status.as_str() {
            "cleaned" => cleaned += 1,
            "copied" => copied += 1,
            "unsupported" => skipped += 1,
            _ => errored += 1,
        }
    }

    CleanRunResult {
        out_dir: out_dir.to_string_lossy().to_string(),
        reports,
        cleaned,
        copied,
        skipped,
        errored,
        ffmpeg,
    }
}

/// Reveal a file or folder in Finder.
#[tauri::command]
pub fn reveal_path(path: String) {
    let _ = std::process::Command::new("open").arg(&path).spawn();
}
