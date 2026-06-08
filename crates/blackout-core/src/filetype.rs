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

/// Detect the real file type from its magic bytes, returning `(category, ext)`.
/// Lets us clean a file even when its name lies (a `.docx` renamed `.jpg`) or has
/// no extension — content is trusted over the filename. Returns None when the
/// bytes don't match a known cleanable type.
pub fn sniff(bytes: &[u8]) -> Option<(FileCategory, &'static str)> {
    let starts = |sig: &[u8]| bytes.len() >= sig.len() && &bytes[..sig.len()] == sig;
    let has = |needle: &[u8]| {
        let head = &bytes[..bytes.len().min(8192)];
        head.windows(needle.len()).any(|w| w == needle)
    };

    if starts(&[0xFF, 0xD8, 0xFF]) {
        return Some((FileCategory::Image, "jpg"));
    }
    if starts(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some((FileCategory::Image, "png"));
    }
    if starts(b"%PDF") {
        return Some((FileCategory::Pdf, "pdf"));
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" {
        if &bytes[8..12] == b"WEBP" {
            return Some((FileCategory::Image, "webp"));
        }
        if &bytes[8..12] == b"WAVE" {
            return Some((FileCategory::Audio, "wav"));
        }
    }
    if starts(b"ID3") || (bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0) {
        return Some((FileCategory::Audio, "mp3"));
    }
    // OOXML (docx/xlsx/pptx) are ZIPs — peek for the part that names the type.
    if starts(b"PK\x03\x04") {
        if has(b"word/") {
            return Some((FileCategory::Document, "docx"));
        }
        if has(b"xl/") {
            return Some((FileCategory::Document, "xlsx"));
        }
        if has(b"ppt/") {
            return Some((FileCategory::Document, "pptx"));
        }
        return None; // a plain zip — nothing we clean
    }
    // ISO-BMFF: '....ftyp<brand>' at offset 4.
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if matches!(brand, b"heic" | b"heix" | b"mif1" | b"heif") {
            return Some((FileCategory::Image, "heic"));
        }
        return Some((FileCategory::Video, "mp4"));
    }
    if starts(b"II*\0") || starts(b"MM\0*") {
        return Some((FileCategory::Image, "tiff"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{sniff, FileCategory};

    #[test]
    fn sniff_detects_real_types_by_magic() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some((FileCategory::Image, "jpg")));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n....").map(|x| x.0), Some(FileCategory::Image));
        assert_eq!(sniff(b"%PDF-1.4").map(|x| x.0), Some(FileCategory::Pdf));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt ").map(|x| x.0), Some(FileCategory::Audio));
        // OOXML zip that names itself a Word doc.
        assert_eq!(sniff(b"PK\x03\x04\x14\0\0\0word/document.xml"), Some((FileCategory::Document, "docx")));
    }

    #[test]
    fn sniff_returns_none_for_junk() {
        assert_eq!(sniff(&[]), None);
        assert_eq!(sniff(&[0u8; 64]), None);
        assert_eq!(sniff(b"just some text"), None);
        assert_eq!(sniff(b"PK\x03\x04 plain zip no office part"), None);
    }
}
