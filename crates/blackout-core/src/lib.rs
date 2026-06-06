//! BLACKOUT CLEAN — the metadata-removal engine.
//!
//! This crate is intentionally platform-agnostic and dependency-light. It takes a
//! file in, produces a privacy-safe copy out, and reports exactly what was removed.
//! Front-ends (CLI today; desktop GUI and mobile bindings later) wrap this engine
//! without re-implementing any cleaning logic.
//!
//! Design rules that match the BLACKOUT philosophy:
//!   * Everything is local. No network, no telemetry, no accounts.
//!   * Originals are never modified — we only ever write copies.
//!   * We tell the user, in plain language, what we stripped.

use std::fs;
use std::path::{Path, PathBuf};

mod cleaner;
mod filetype;
mod findings;

pub use filetype::FileCategory;
pub use findings::Finding;

/// Extensions handled by spawning an external `ffmpeg` process. These are
/// ISO-BMFF / container formats whose metadata cannot be stripped safely in a
/// few hundred lines of pure Rust (removing atoms shifts media offsets). When
/// `ffmpeg` is absent we report them honestly rather than corrupting the file.
const EXTERNAL_EXTS: &[&str] = &["mp4", "mov", "avi", "mkv", "m4a", "heic", "heif"];

/// Outcome of cleaning a single file. Serializable so a `--json` front-end gets
/// the same information a human does.
#[derive(Debug, serde::Serialize)]
pub struct CleanReport {
    pub source: PathBuf,
    pub category: String,
    /// "cleaned" | "copied" | "unsupported" | "error"
    pub status: String,
    pub output: Option<PathBuf>,
    /// Human-readable descriptions of what was stripped.
    pub removed: Vec<String>,
    /// Caveats / informational notes (e.g. limitations, install hints).
    pub notes: Vec<String>,
    /// The actual exposed values found (GPS, camera, author, …). Populated by
    /// `inspect_file`; empty after a clean.
    pub findings: Vec<Finding>,
    pub bytes_before: Option<u64>,
    pub bytes_after: Option<u64>,
}

impl CleanReport {
    fn error(source: &Path, category: &str, msg: impl Into<String>) -> Self {
        CleanReport {
            source: source.to_path_buf(),
            category: category.to_string(),
            status: "error".into(),
            output: None,
            removed: vec![],
            notes: vec![msg.into()],
            findings: vec![],
            bytes_before: None,
            bytes_after: None,
        }
    }
}

/// Whether the host has a usable `ffmpeg` binary. Checked once per run by the
/// caller and threaded in, so we don't fork a process per file.
pub fn ffmpeg_available() -> bool {
    cleaner::video::ffmpeg_available()
}

/// Inspect a file and report what exposed metadata it carries — without writing
/// anything. This runs the same parsers as `clean_file` but discards output,
/// so "what would be removed" and "what gets removed" never drift apart.
pub fn inspect_file(input: &Path) -> CleanReport {
    let ext = filetype::ext_of(input);
    let category = filetype::categorize(&ext);

    if EXTERNAL_EXTS.contains(&ext.as_str()) {
        let mut notes = vec![
            "Container format — inspected via ffmpeg when available.".to_string(),
        ];
        if !ffmpeg_available() {
            notes.push("ffmpeg not installed; cannot enumerate metadata for this format yet.".into());
        }
        return CleanReport {
            source: input.to_path_buf(),
            category: category.label().into(),
            status: "unsupported".into(),
            output: None,
            removed: vec!["Container & stream metadata (location, device make/model, creation time)".into()],
            notes,
            findings: findings::video_findings(input),
            bytes_before: fs::metadata(input).ok().map(|m| m.len()),
            bytes_after: None,
        };
    }

    let bytes = match fs::read(input) {
        Ok(b) => b,
        Err(e) => return CleanReport::error(input, category.label(), format!("cannot read file: {e}")),
    };
    let before = bytes.len() as u64;
    // Pull the real values *before* the bytes are consumed by the cleaner.
    let found = findings::extract(category, &ext, &bytes);

    match cleaner::clean_bytes(category, &ext, bytes) {
        Ok((_out, removed, notes)) => CleanReport {
            source: input.to_path_buf(),
            category: category.label().into(),
            status: if removed.is_empty() { "copied".into() } else { "cleaned".into() },
            output: None,
            removed,
            notes,
            findings: found,
            bytes_before: Some(before),
            bytes_after: None,
        },
        Err(e) => CleanReport::error(input, category.label(), e.to_string()),
    }
}

/// Clean a file, writing a privacy-safe copy into `out_dir`. The original is
/// never touched. `ffmpeg_ok` is passed in so the caller probes for ffmpeg once.
pub fn clean_file(input: &Path, out_dir: &Path, ffmpeg_ok: bool) -> CleanReport {
    let ext = filetype::ext_of(input);
    let category = filetype::categorize(&ext);
    let before = fs::metadata(input).ok().map(|m| m.len());

    let out_path = match unique_output_path(input, out_dir) {
        Ok(p) => p,
        Err(e) => return CleanReport::error(input, category.label(), e.to_string()),
    };

    // External-tool formats (video, HEIC, m4a) go through ffmpeg.
    if EXTERNAL_EXTS.contains(&ext.as_str()) {
        if !ffmpeg_ok {
            return CleanReport {
                source: input.to_path_buf(),
                category: category.label().into(),
                status: "unsupported".into(),
                output: None,
                removed: vec![],
                notes: vec![format!(
                    "'{ext}' needs ffmpeg, which isn't installed. Install it with: brew install ffmpeg"
                )],
                findings: vec![],
                bytes_before: before,
                bytes_after: None,
            };
        }
        return match cleaner::video::clean_via_ffmpeg(input, &out_path) {
            Ok(removed) => {
                let after = fs::metadata(&out_path).ok().map(|m| m.len());
                CleanReport {
                    source: input.to_path_buf(),
                    category: category.label().into(),
                    status: "cleaned".into(),
                    output: Some(out_path),
                    removed,
                    notes: vec![],
                    findings: vec![],
                    bytes_before: before,
                    bytes_after: after,
                }
            }
            Err(e) => CleanReport::error(input, category.label(), e.to_string()),
        };
    }

    // Pure-Rust, byte-oriented formats.
    let bytes = match fs::read(input) {
        Ok(b) => b,
        Err(e) => return CleanReport::error(input, category.label(), format!("cannot read file: {e}")),
    };

    match cleaner::clean_bytes(category, &ext, bytes) {
        Ok((out_bytes, removed, notes)) => {
            if let Err(e) = fs::write(&out_path, &out_bytes) {
                return CleanReport::error(input, category.label(), format!("cannot write output: {e}"));
            }
            CleanReport {
                source: input.to_path_buf(),
                category: category.label().into(),
                status: if removed.is_empty() { "copied".into() } else { "cleaned".into() },
                output: Some(out_path),
                removed,
                notes,
                findings: vec![],
                bytes_before: before,
                bytes_after: Some(out_bytes.len() as u64),
            }
        }
        Err(e) => CleanReport::error(input, category.label(), e.to_string()),
    }
}

/// Pick a non-colliding output path inside `out_dir`, preserving the original
/// filename where possible (`photo.jpg`, then `photo-1.jpg`, ...).
fn unique_output_path(input: &Path, out_dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let name = input
        .file_name()
        .map(|n| n.to_owned())
        .unwrap_or_else(|| std::ffi::OsString::from("cleaned"));
    let candidate = out_dir.join(&name);
    if !candidate.exists() {
        return Ok(candidate);
    }
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("cleaned");
    let ext = input.extension().and_then(|s| s.to_str());
    for n in 1..10_000 {
        let fname = match ext {
            Some(e) => format!("{stem}-{n}.{e}"),
            None => format!("{stem}-{n}"),
        };
        let p = out_dir.join(fname);
        if !p.exists() {
            return Ok(p);
        }
    }
    Ok(candidate)
}
