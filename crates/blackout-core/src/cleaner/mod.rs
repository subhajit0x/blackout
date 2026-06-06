//! Dispatch layer: route a category + extension to the right cleaner and return
//! `(cleaned_bytes, removed_descriptions, notes)`.

use anyhow::{bail, Result};

use crate::filetype::FileCategory;

pub mod audio;
pub mod document;
pub mod image;
pub mod pdf;
pub mod video;

pub fn clean_bytes(
    category: FileCategory,
    ext: &str,
    bytes: Vec<u8>,
) -> Result<(Vec<u8>, Vec<String>, Vec<String>)> {
    let mut notes: Vec<String> = Vec::new();

    let (out, removed) = match category {
        FileCategory::Image => image::clean_image(bytes, ext)?,
        FileCategory::Pdf => pdf::clean_pdf(bytes)?,
        FileCategory::Text => {
            notes.push("Plain text carries no embedded metadata; copied unchanged.".into());
            (bytes, vec![])
        }
        FileCategory::Document => {
            notes.push(
                "Tracked changes and inline comments are not yet stripped (planned for a later build).".into(),
            );
            document::clean_office(bytes)?
        }
        FileCategory::Audio => match ext {
            "mp3" => audio::clean_mp3(bytes)?,
            "wav" => audio::clean_wav(bytes)?,
            // m4a is routed to ffmpeg by the caller; reaching here means a bad route.
            other => bail!("audio format '{other}' is not handled by the pure-Rust path"),
        },
        FileCategory::Video => {
            bail!("video is handled via the external-tool path, not clean_bytes")
        }
        FileCategory::Unknown => {
            bail!("unsupported file type")
        }
    };

    Ok((out, removed, notes))
}
