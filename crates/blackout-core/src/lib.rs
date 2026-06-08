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
        Err(_) => return CleanReport::error(input, category.label(), "Couldn't read this file — it may have moved or be locked."),
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
        Err(_) => return CleanReport::error(input, category.label(), "Couldn't read this file — it may have moved or be locked."),
    };

    match cleaner::clean_bytes(category, &ext, bytes) {
        Ok((out_bytes, removed, notes)) => {
            if fs::write(&out_path, &out_bytes).is_err() {
                return CleanReport::error(input, category.label(), "Couldn't save the cleaned copy to the output folder.");
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

// ---------------------------------------------------------------------------
// Filesystem-free API (mobile / content-URI inputs).
//
// On Android/iOS the "path" the picker hands us is a content URI, not a real
// file — `std::fs` can't read it. These functions take the bytes the platform
// already read for us, so the same proven cleaners run everywhere. They never
// return a scary error: anything we can't process degrades to a calm status.
// ---------------------------------------------------------------------------

/// Clean already-read bytes. Returns the report plus the cleaned bytes to save
/// (None when there's nothing to write — unsupported format or unreadable file).
pub fn clean_named_bytes(filename: &str, bytes: Vec<u8>) -> (CleanReport, Option<Vec<u8>>) {
    let ext = filetype::ext_of(Path::new(filename));
    let category = filetype::categorize(&ext);
    let before = bytes.len() as u64;
    let source = PathBuf::from(filename);

    // Container formats (video/HEIC/M4A) need ffmpeg + real paths — say so calmly.
    if EXTERNAL_EXTS.contains(&ext.as_str()) {
        return (
            soft(source, category.label(), "unsupported",
                &format!("{} files are cleaned on desktop (needs ffmpeg) — not on this device yet.", ext.to_uppercase()),
                before),
            None,
        );
    }

    match cleaner::clean_bytes(category, &ext, bytes) {
        Ok((out_bytes, removed, notes)) => {
            let status = if removed.is_empty() { "copied" } else { "cleaned" };
            let after = out_bytes.len() as u64;
            (
                CleanReport {
                    source,
                    category: category.label().into(),
                    status: status.into(),
                    output: None,
                    removed,
                    notes,
                    findings: vec![],
                    bytes_before: Some(before),
                    bytes_after: Some(after),
                },
                Some(out_bytes),
            )
        }
        // Never surface a raw error to the user — report softly, write nothing.
        Err(_) => (
            soft(source, category.label(), "skipped",
                "We couldn't process this file, so it was left unchanged.", before),
            None,
        ),
    }
}

/// Inspect already-read bytes: report the exposed metadata without writing.
pub fn inspect_named_bytes(filename: &str, bytes: Vec<u8>) -> CleanReport {
    let ext = filetype::ext_of(Path::new(filename));
    let category = filetype::categorize(&ext);
    let before = bytes.len() as u64;
    let source = PathBuf::from(filename);
    let findings = findings::extract(category, &ext, &bytes);

    let (status, removed, notes) = match cleaner::clean_bytes(category, &ext, bytes) {
        Ok((_out, removed, notes)) => {
            (if removed.is_empty() { "copied" } else { "cleaned" }, removed, notes)
        }
        Err(_) => ("skipped", vec![], vec!["Nothing to inspect for this file here.".to_string()]),
    };
    CleanReport {
        source,
        category: category.label().into(),
        status: status.into(),
        output: None,
        removed,
        notes,
        findings,
        bytes_before: Some(before),
        bytes_after: None,
    }
}

/// A calm, non-error report (no "os error", no stack-ish text).
fn soft(source: PathBuf, category: &str, status: &str, note: &str, before: u64) -> CleanReport {
    CleanReport {
        source,
        category: category.into(),
        status: status.into(),
        output: None,
        removed: vec![],
        notes: vec![note.into()],
        findings: vec![],
        bytes_before: Some(before),
        bytes_after: None,
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

#[cfg(test)]
mod bytes_api_tests {
    use super::*;

    #[test]
    fn garbage_input_never_panics_and_degrades_softly() {
        // A file claiming to be a JPEG but full of junk must NOT error/panic.
        let (report, out) = clean_named_bytes("photo.jpg", vec![0u8; 16]);
        assert_ne!(report.status, "error", "must never surface an error status");
        assert!(out.is_none(), "nothing valid to write");
    }

    #[test]
    fn container_formats_report_unsupported_not_error() {
        let (report, out) = clean_named_bytes("clip.mp4", vec![0u8; 8]);
        assert_eq!(report.status, "unsupported");
        assert!(out.is_none());
    }

    #[test]
    fn plain_text_is_copied_through() {
        let (report, out) = clean_named_bytes("notes.txt", b"hello".to_vec());
        assert_eq!(report.status, "copied");
        assert_eq!(out.as_deref(), Some(&b"hello"[..]));
    }

    #[test]
    fn unknown_extension_does_not_error() {
        // Some random file type the user dropped in — stay calm, never crash.
        let (report, _out) = clean_named_bytes("data.xyz", vec![1, 2, 3]);
        assert_ne!(report.status, "error");
    }
}
