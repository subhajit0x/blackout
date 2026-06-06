//! Audio metadata removal for the pure-Rust formats.
//!
//! MP3: ID3v2 tags sit at the start of the file, ID3v1 in the last 128 bytes.
//! Both are length-delimited containers, so we can excise them without a decoder.
//!
//! WAV: a RIFF container of chunks. We keep the structural chunks (fmt , data,
//! fact) and drop the ones used for tagging (LIST/INFO, id3, bext, iXML).

use anyhow::{bail, Result};
use std::collections::BTreeSet;

pub fn clean_mp3(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    let mut removed = Vec::new();
    let len = bytes.len();

    // ---- ID3v2 header at the start ----
    let mut start = 0usize;
    if len > 10 && &bytes[0..3] == b"ID3" {
        // bytes[6..10] is a synchsafe (7-bit) integer: the tag size excluding header.
        let size = synchsafe(&bytes[6..10]) as usize;
        let has_footer = bytes[5] & 0x10 != 0;
        let total = 10 + size + if has_footer { 10 } else { 0 };
        if total <= len {
            start = total;
            removed.push(
                "ID3v2 tags (artist, album, title, comments, embedded cover art, encoder)".into(),
            );
        }
    }

    // ---- ID3v1 trailer in the final 128 bytes ----
    let mut end = len;
    if end >= start + 128 && &bytes[end - 128..end - 125] == b"TAG" {
        end -= 128;
        removed.push("ID3v1 tag (title, artist, album, year, comment)".into());
    }

    Ok((bytes[start..end].to_vec(), removed))
}

fn synchsafe(b: &[u8]) -> u32 {
    ((b[0] as u32 & 0x7f) << 21)
        | ((b[1] as u32 & 0x7f) << 14)
        | ((b[2] as u32 & 0x7f) << 7)
        | (b[3] as u32 & 0x7f)
}

/// RIFF chunk IDs that carry tagging/metadata rather than audio structure.
const WAV_STRIP: &[&[u8; 4]] = &[b"LIST", b"id3 ", b"ID3 ", b"bext", b"iXML", b"_PMX", b"cue "];

pub fn clean_wav(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("file has a .wav name but is not a valid RIFF/WAVE file");
    }

    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&bytes[0..12]); // RIFF, size (fixed up below), WAVE

    let mut stripped: BTreeSet<String> = BTreeSet::new();
    let mut i = 12usize;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let sz = u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
        let padded = sz + (sz & 1); // chunks are word-aligned
        let total = 8 + padded;

        if i + total > bytes.len() {
            // Last chunk runs past EOF (common for streamed/truncated data chunks):
            // copy the remainder verbatim and stop.
            out.extend_from_slice(&bytes[i..]);
            break;
        }

        if WAV_STRIP.iter().any(|s| s.as_slice() == id) {
            stripped.insert(String::from_utf8_lossy(id).trim().to_string());
        } else {
            out.extend_from_slice(&bytes[i..i + total]);
        }
        i += total;
    }

    // Fix up the RIFF chunk size to reflect what we kept.
    let new_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&new_size.to_le_bytes());

    let mut removed = Vec::new();
    if !stripped.is_empty() {
        removed.push(format!(
            "WAV metadata chunks: {} (recording info, broadcast metadata, embedded tags)",
            stripped.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok((out, removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synchsafe_bytes(n: u32) -> [u8; 4] {
        [
            ((n >> 21) & 0x7f) as u8,
            ((n >> 14) & 0x7f) as u8,
            ((n >> 7) & 0x7f) as u8,
            (n & 0x7f) as u8,
        ]
    }

    #[test]
    fn mp3_strips_id3v2_and_id3v1() {
        let body = b"TIT2\x00\x00\x00\x05\x00\x00\x00secret"; // a fake frame
        let mut input = Vec::new();
        input.extend_from_slice(b"ID3\x04\x00\x00");
        input.extend_from_slice(&synchsafe_bytes(body.len() as u32));
        input.extend_from_slice(body);
        let audio_marker = b"\xff\xfb\x90\x00audio-data";
        input.extend_from_slice(audio_marker);
        let mut v1 = vec![0u8; 128];
        v1[0..3].copy_from_slice(b"TAG");
        input.extend_from_slice(&v1);

        let (out, removed) = clean_mp3(input).unwrap();
        assert_eq!(&out, audio_marker, "only the audio payload should remain");
        assert_eq!(removed.len(), 2, "both ID3v2 and ID3v1 reported");
        assert_ne!(&out[0..3], b"ID3");
        assert!(out.len() < 128 || &out[out.len() - 128..out.len() - 125] != b"TAG");
    }

    #[test]
    fn wav_drops_list_chunk_and_fixes_size() {
        let fmt = b"fmt \x10\x00\x00\x00\x01\x00\x01\x00\x40\x1f\x00\x00\x40\x1f\x00\x00\x01\x00\x08\x00";
        let info = b"INFOIARTsecretdj"; // not length-correct, but inside a LIST we wrap properly
        let mut list = Vec::new();
        list.extend_from_slice(b"LIST");
        list.extend_from_slice(&(info.len() as u32).to_le_bytes());
        list.extend_from_slice(info);
        let data = b"data\x04\x00\x00\x00\x80\x80\x80\x80";

        let mut body = Vec::new();
        body.extend_from_slice(b"WAVE");
        body.extend_from_slice(fmt);
        body.extend_from_slice(&list);
        body.extend_from_slice(data);
        let mut input = Vec::new();
        input.extend_from_slice(b"RIFF");
        input.extend_from_slice(&(body.len() as u32).to_le_bytes());
        input.extend_from_slice(&body);

        let (out, removed) = clean_wav(input).unwrap();
        assert!(!out.windows(4).any(|w| w == b"LIST"), "LIST chunk removed");
        assert!(!out.windows(8).any(|w| w == b"secretdj"), "tag content gone");
        assert_eq!(removed.len(), 1);
        // RIFF size field must match remaining bytes.
        let declared = u32::from_le_bytes([out[4], out[5], out[6], out[7]]) as usize;
        assert_eq!(declared, out.len() - 8, "RIFF size fixed up");
    }
}
