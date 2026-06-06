//! Image metadata removal.
//!
//! JPEG: handled by a hand-written segment filter that drops *every* metadata
//! segment — EXIF and XMP (APP1), IPTC/Photoshop (APP13), vendor APPn blocks,
//! and JPEG comments (COM) — while keeping the structural and colour segments
//! (JFIF/APP0, ICC/APP2, Adobe/APP14) so the image renders identically. This is
//! stricter than EXIF-only stripping: IPTC is a primary carrier of author,
//! copyright, caption and location data and must not survive.
//!
//! WebP / TIFF: handled by `img-parts`, which strips EXIF and ICC blocks while
//! leaving the compressed image data byte-for-byte intact (lossless, fast).
//!
//! PNG: handled by a small hand-written chunk filter that drops the ancillary
//! chunks that carry identifying data (eXIf, text chunks, timestamps) while
//! preserving everything needed to render the image.

use anyhow::{bail, Result};
use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};
use std::collections::BTreeSet;

pub fn clean_image(bytes: Vec<u8>, ext: &str) -> Result<(Vec<u8>, Vec<String>)> {
    match ext {
        "png" => clean_png(bytes),
        "jpg" | "jpeg" => clean_jpeg(bytes),
        "webp" => clean_via_img_parts(bytes),
        // tif/tiff are categorized as images but img-parts can't strip them.
        // Fail loudly: never pass a file through as "clean" while it may still
        // carry EXIF/GPS — a false "all clear" is the worst outcome here.
        other => bail!(
            "{other} metadata removal isn't supported yet — the file was left unchanged \
             rather than reported clean while still carrying metadata"
        ),
    }
}

/// Marker bytes (the second byte of an `FF xx` marker) for JPEG segments that
/// carry metadata and are safe to drop. Everything not listed here is preserved.
///   E1=APP1 (EXIF + XMP), E3..EC/EF=vendor APPn, ED=APP13 (IPTC/Photoshop),
///   FE=COM (comment).
/// Deliberately kept: E0=APP0 (JFIF density), E2=APP2 (ICC colour),
///   EE=APP14 (Adobe colour-transform flag).
const JPEG_DROP: &[u8] = &[
    0xE1, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEF, 0xFE,
];

fn jpeg_marker_label(m: u8) -> &'static str {
    match m {
        0xE1 => "EXIF & XMP (APP1)",
        0xED => "IPTC / Photoshop (APP13)",
        0xFE => "embedded comment (COM)",
        _ => "vendor metadata (APPn)",
    }
}

fn clean_jpeg(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        bail!("file has a .jpg/.jpeg name but is not a valid JPEG");
    }

    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[0..2]); // SOI

    let mut stripped: BTreeSet<&'static str> = BTreeSet::new();
    let mut i = 2usize;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            // Not a marker boundary — copy the remainder verbatim and stop.
            out.extend_from_slice(&bytes[i..]);
            break;
        }
        let m = bytes[i + 1];

        // Fill byte: a lone extra 0xFF before the real marker.
        if m == 0xFF {
            out.push(0xFF);
            i += 1;
            continue;
        }
        // Start of scan: entropy-coded image data follows to EOF. Copy verbatim.
        if m == 0xDA {
            out.extend_from_slice(&bytes[i..]);
            break;
        }
        // Standalone markers with no length payload (RSTn, TEM, EOI).
        if m == 0xD9 || m == 0x01 || (0xD0..=0xD7).contains(&m) {
            out.extend_from_slice(&bytes[i..i + 2]);
            i += 2;
            if m == 0xD9 {
                break; // EOI
            }
            continue;
        }

        // Length-prefixed segment.
        if i + 4 > bytes.len() {
            out.extend_from_slice(&bytes[i..]);
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        let seg_end = i + 2 + len;
        if len < 2 || seg_end > bytes.len() {
            // Malformed length; bail out safely by copying the rest.
            out.extend_from_slice(&bytes[i..]);
            break;
        }

        if JPEG_DROP.contains(&m) {
            stripped.insert(jpeg_marker_label(m));
        } else {
            out.extend_from_slice(&bytes[i..seg_end]);
        }
        i = seg_end;
    }

    let mut removed = Vec::new();
    if !stripped.is_empty() {
        removed.push(format!(
            "JPEG metadata segments: {} (GPS, camera make/model, author, copyright, captions)",
            stripped.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok((out, removed))
}

fn clean_via_img_parts(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    let mut removed = Vec::new();

    let parsed = DynImage::from_bytes(Bytes::from(bytes.clone()))?;
    let mut img = match parsed {
        Some(img) => img,
        // Recognized extension but not a parseable image — don't claim success.
        None => bail!("file has an image extension but isn't a readable image; left unchanged"),
    };

    if img.exif().is_some() {
        removed.push(
            "EXIF metadata (GPS coordinates, camera make/model, lens, timestamps, serial numbers)"
                .into(),
        );
        img.set_exif(None);
    }
    if img.icc_profile().is_some() {
        removed.push("ICC colour profile".into());
        img.set_icc_profile(None);
    }
    // WebP can also carry an XMP packet that the EXIF/ICC API doesn't touch — it
    // holds author, copyright, location and edit history. Strip it explicitly.
    if let DynImage::WebP(ref mut webp) = img {
        use img_parts::webp::CHUNK_XMP;
        if webp.has_chunk(CHUNK_XMP) {
            removed.push("XMP metadata (author, copyright, location, edit history)".into());
            webp.remove_chunks_by_id(CHUNK_XMP);
        }
    }

    let mut out = Vec::with_capacity(bytes.len());
    img.encoder().write_to(&mut out)?;
    Ok((out, removed))
}

const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Ancillary PNG chunks that can carry identifying information and are safe to
/// drop without affecting how the image renders.
const PNG_STRIP: &[&[u8; 4]] = &[b"tEXt", b"zTXt", b"iTXt", b"eXIf", b"tIME"];

fn clean_png(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    if bytes.len() < 8 || bytes[0..8] != PNG_SIG {
        bail!("file has a .png name but is not a valid PNG");
    }

    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&PNG_SIG);

    let mut stripped: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut i = 8usize;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let ctype = &bytes[i + 4..i + 8];
        let chunk_total = 12 + len; // length(4) + type(4) + data(len) + crc(4)
        if i + chunk_total > bytes.len() {
            break; // truncated; stop cleanly
        }

        let is_iend = ctype == b"IEND";
        if PNG_STRIP.iter().any(|s| s.as_slice() == ctype) {
            stripped.insert(String::from_utf8_lossy(ctype).into_owned());
        } else {
            out.extend_from_slice(&bytes[i..i + chunk_total]);
        }

        i += chunk_total;
        if is_iend {
            break;
        }
    }

    let mut removed = Vec::new();
    if !stripped.is_empty() {
        removed.push(format!(
            "PNG metadata chunks: {} (text comments, embedded EXIF, modification time)",
            stripped.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok((out, removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(typ: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut crc_input = typ.to_vec();
        crc_input.extend_from_slice(data);
        let crc = crc32(&crc_input);
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(typ);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc.to_be_bytes());
        out
    }

    // Minimal CRC32 (PNG/zlib polynomial) so the test fixture is well-formed.
    fn crc32(buf: &[u8]) -> u32 {
        let mut crc: u32 = 0xffff_ffff;
        for &b in buf {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        crc ^ 0xffff_ffff
    }

    fn app_segment(marker: u8, payload: &[u8]) -> Vec<u8> {
        let len = (payload.len() + 2) as u16;
        let mut v = vec![0xFF, marker];
        v.extend_from_slice(&len.to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn jpeg_strips_iptc_xmp_and_comments_keeps_color() {
        let mut input = vec![0xFF, 0xD8]; // SOI
        input.extend(app_segment(0xE0, b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00")); // keep
        input.extend(app_segment(0xE1, b"Exif\x00\x00secret-gps-and-camera")); // drop
        input.extend(app_segment(0xED, b"Photoshop 3.0\x00IPTC-author-jdoe")); // drop
        input.extend(app_segment(0xE2, b"ICC_PROFILE\x00color-data")); // keep
        input.extend(app_segment(0xEE, b"Adobe-transform")); // keep
        input.extend(app_segment(0xFE, b"a secret comment")); // drop (COM)
        input.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]); // SOS marker + minimal len
        input.extend_from_slice(b"\xde\xad\xbe\xef"); // entropy data
        input.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let (out, removed) = clean_jpeg(input).unwrap();

        // Metadata gone.
        assert!(!out.windows(15).any(|w| w == b"secret-gps-and-"), "EXIF removed");
        assert!(!out.windows(11).any(|w| w == b"IPTC-author"), "IPTC removed");
        assert!(!out.windows(8).any(|w| w == b"a secret"), "comment removed");
        // Colour/structure kept.
        assert!(out.windows(4).any(|w| w == b"JFIF"), "JFIF kept");
        assert!(out.windows(11).any(|w| w == b"ICC_PROFILE"), "ICC kept");
        assert!(out.windows(5).any(|w| w == b"Adobe"), "Adobe kept");
        // Image data intact.
        assert!(out.windows(4).any(|w| w == b"\xde\xad\xbe\xef"), "scan data kept");
        assert_eq!(&out[out.len() - 2..], &[0xFF, 0xD9], "EOI preserved");
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn png_removes_text_chunks_keeps_image_data() {
        let mut input = PNG_SIG.to_vec();
        input.extend(chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0]));
        input.extend(chunk(b"tEXt", b"Author\x00secret-person"));
        input.extend(chunk(b"IDAT", b"\x08\x1d\x01keepme"));
        input.extend(chunk(b"IEND", b""));

        let (out, removed) = clean_png(input).unwrap();
        assert!(!out.windows(4).any(|w| w == b"tEXt"), "tEXt chunk removed");
        assert!(!out.windows(13).any(|w| w == b"secret-person"), "text content gone");
        assert!(out.windows(4).any(|w| w == b"IDAT"), "image data preserved");
        assert!(out.windows(4).any(|w| w == b"IHDR"), "header preserved");
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn tiff_errors_instead_of_falsely_reporting_clean() {
        // img-parts can't strip TIFF metadata. The cleaner must surface that as an
        // error, never silently pass the file through as if it had been cleaned —
        // a false "all clear" on a file that still carries EXIF/GPS is unacceptable.
        let little_endian_tiff_header = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00];
        assert!(clean_image(little_endian_tiff_header, "tiff").is_err());
    }
}
