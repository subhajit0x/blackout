//! File classification by extension. Extension is sufficient for the front-end
//! UX (the user picked the file); each cleaner additionally validates magic
//! bytes before touching the contents, so a misnamed file fails safe.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    Image,
    Video,
    Document,
    Audio,
    Pdf,
    Text,
    Unknown,
}

impl FileCategory {
    pub fn label(self) -> &'static str {
        match self {
            FileCategory::Image => "image",
            FileCategory::Video => "video",
            FileCategory::Document => "document",
            FileCategory::Audio => "audio",
            FileCategory::Pdf => "pdf",
            FileCategory::Text => "text",
            FileCategory::Unknown => "unknown",
        }
    }
}

pub fn ext_of(p: &Path) -> String {
    p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

pub fn categorize(ext: &str) -> FileCategory {
    match ext {
        "jpg" | "jpeg" | "png" | "webp" | "tif" | "tiff" | "heic" | "heif" => FileCategory::Image,
        "mp4" | "mov" | "avi" | "mkv" => FileCategory::Video,
        "mp3" | "wav" | "m4a" => FileCategory::Audio,
        "pdf" => FileCategory::Pdf,
        "docx" | "xlsx" | "pptx" => FileCategory::Document,
        "txt" => FileCategory::Text,
        _ => FileCategory::Unknown,
    }
}
