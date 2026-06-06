//! Video / HEIC / M4A metadata removal via `ffmpeg`.
//!
//! Stripping ISO-BMFF metadata atoms by hand means re-flowing media sample
//! offsets — error-prone and easy to corrupt. Until a vetted pure-Rust path
//! exists, we shell out to ffmpeg with `-map_metadata -1`, which rewrites the
//! container without metadata while copying the audio/video streams losslessly
//! (no re-encode). When ffmpeg is absent the caller reports the format as
//! unsupported instead of producing a broken file.

use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn clean_via_ffmpeg(input: &Path, output: &Path) -> Result<Vec<String>> {
    let result = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .args([
            "-map_metadata", "-1",
            "-map_chapters", "-1",
            "-c", "copy",
        ])
        .arg(output)
        .output()?;

    if !result.status.success() {
        let _ = std::fs::remove_file(output); // don't leave a partial file
        bail!(
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&result.stderr)
                .lines()
                .last()
                .unwrap_or("unknown error")
        );
    }

    Ok(vec![
        "Container & stream metadata (GPS location, device make/model, creation time, encoder tags, chapters)"
            .into(),
    ])
}
