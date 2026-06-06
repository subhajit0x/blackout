//! Extract the *actual values* of exposed metadata — so a user sees "📍 28.6139,
//! 77.2090" and "Canon EOS R6", not just "EXIF present". This is read-only and
//! entirely offline: we surface coordinates, never look them up on a network.

use crate::filetype::FileCategory;
use serde::Serialize;
use std::io::Cursor;

/// One concrete piece of exposed metadata.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Short label, e.g. "Location", "Camera", "Author".
    pub label: String,
    /// The real value found in the file.
    pub value: String,
    /// Category for the UI: "location" | "device" | "date" | "identity" | "software" | "other".
    pub kind: String,
}

impl Finding {
    fn new(label: &str, value: impl Into<String>, kind: &str) -> Self {
        Finding { label: label.into(), value: value.into(), kind: kind.into() }
    }
}

pub fn extract(category: FileCategory, ext: &str, bytes: &[u8]) -> Vec<Finding> {
    match category {
        FileCategory::Image => image_findings(bytes),
        FileCategory::Pdf => pdf_findings(bytes),
        FileCategory::Document if matches!(ext, "docx" | "xlsx" | "pptx") => office_findings(bytes),
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Images — EXIF via kamadak-exif (handles JPEG/TIFF/HEIF/PNG/WebP containers).
// ---------------------------------------------------------------------------

fn image_findings(bytes: &[u8]) -> Vec<Finding> {
    // EXIF first, then merge IPTC (APP13) + XMP (APP1) — the latter two are how
    // Lightroom/Photoshop exports leak author, copyright and city.
    let mut out = exif_findings(bytes);
    merge(&mut out, jpeg_segment_findings(bytes));
    out
}

/// Skip duplicates (the same author can appear in EXIF, IPTC *and* XMP).
fn merge(into: &mut Vec<Finding>, more: Vec<Finding>) {
    for f in more {
        if !into.iter().any(|e| e.label == f.label && e.value == f.value) {
            into.push(f);
        }
    }
}

fn exif_findings(bytes: &[u8]) -> Vec<Finding> {
    use exif::{In, Tag};

    let mut out = Vec::new();
    let exif = match exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(e) => e,
        Err(_) => return out, // no EXIF (or unreadable) — IPTC/XMP handled separately
    };

    // GPS location → decimal degrees.
    if let (Some(lat), Some(lon)) = (
        gps_decimal(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        gps_decimal(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    ) {
        out.push(Finding::new("Location", format!("{lat:.5}, {lon:.5}"), "location"));
    }

    let pull = |tag: Tag| -> Option<String> {
        exif.get_field(tag, In::PRIMARY).map(|f| {
            f.display_value().with_unit(&exif).to_string().trim_matches('"').trim().to_string()
        })
    };

    let make = pull(Tag::Make).unwrap_or_default();
    let model = pull(Tag::Model).unwrap_or_default();
    // Many cameras repeat the make inside the model ("Canon" + "Canon EOS R6").
    let device = if model.to_lowercase().starts_with(&make.to_lowercase()) && !make.is_empty() {
        model.clone()
    } else {
        format!("{make} {model}").trim().to_string()
    };
    if !device.is_empty() {
        out.push(Finding::new("Camera", device, "device"));
    }
    if let Some(lens) = pull(Tag::LensModel).filter(|s| !s.is_empty()) {
        out.push(Finding::new("Lens", lens, "device"));
    }
    if let Some(dt) = pull(Tag::DateTimeOriginal).or_else(|| pull(Tag::DateTime)).filter(|s| !s.is_empty()) {
        out.push(Finding::new("Captured", dt, "date"));
    }
    if let Some(artist) = pull(Tag::Artist).filter(|s| !s.is_empty()) {
        out.push(Finding::new("Author", artist, "identity"));
    }
    if let Some(c) = pull(Tag::Copyright).filter(|s| !s.is_empty()) {
        out.push(Finding::new("Copyright", c, "identity"));
    }
    if let Some(sw) = pull(Tag::Software).filter(|s| !s.is_empty()) {
        out.push(Finding::new("Software", sw, "software"));
    }
    out
}

fn gps_decimal(exif: &exif::Exif, coord: exif::Tag, refr: exif::Tag) -> Option<f64> {
    use exif::{In, Value};
    let c = exif.get_field(coord, In::PRIMARY)?;
    let r = exif.get_field(refr, In::PRIMARY)?;
    if let Value::Rational(ref v) = c.value {
        if v.len() >= 3 {
            let deg = v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0;
            let r_str = r.display_value().to_string();
            let sign = if r_str.contains('S') || r_str.contains('W') { -1.0 } else { 1.0 };
            return Some(deg * sign);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// IPTC (APP13) + XMP (APP1) — walk JPEG segments and read identity fields.
// ---------------------------------------------------------------------------

fn jpeg_segment_findings(bytes: &[u8]) -> Vec<Finding> {
    let mut out = Vec::new();
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return out;
    }
    const XMP_HDR: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    let mut i = 2usize;
    while i + 4 <= bytes.len() && bytes[i] == 0xFF {
        let m = bytes[i + 1];
        if m == 0xDA || m == 0xD9 {
            break;
        }
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        let seg = &bytes[i + 4..i + 2 + len];
        if m == 0xED {
            out.extend(parse_iptc(seg));
        } else if m == 0xE1 && seg.starts_with(XMP_HDR) {
            out.extend(parse_xmp(&seg[XMP_HDR.len()..]));
        }
        i += 2 + len;
    }
    out
}

/// IPTC IIM datasets live as 0x1C 0x02 <dataset> <len:u16> <value>.
fn parse_iptc(seg: &[u8]) -> Vec<Finding> {
    fn label(ds: u8) -> Option<(&'static str, &'static str)> {
        match ds {
            5 => Some(("Title", "other")),
            55 => Some(("Date created", "date")),
            80 => Some(("Author", "identity")),
            85 => Some(("Author title", "identity")),
            90 => Some(("City", "place")),
            95 => Some(("State", "place")),
            101 => Some(("Country", "place")),
            105 => Some(("Headline", "other")),
            110 => Some(("Credit", "identity")),
            115 => Some(("Source", "identity")),
            116 => Some(("Copyright", "identity")),
            120 => Some(("Caption", "other")),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut j = 0usize;
    while j + 5 <= seg.len() {
        if seg[j] == 0x1C && seg[j + 1] == 0x02 {
            let ds = seg[j + 2];
            let l = u16::from_be_bytes([seg[j + 3], seg[j + 4]]) as usize;
            let start = j + 5;
            if start + l <= seg.len() {
                if let Some((lbl, kind)) = label(ds) {
                    let v = String::from_utf8_lossy(&seg[start..start + l]).trim().to_string();
                    if !v.is_empty() {
                        out.push(Finding::new(lbl, v, kind));
                    }
                }
                j = start + l;
                continue;
            }
        }
        j += 1;
    }
    out
}

fn parse_xmp(xml_bytes: &[u8]) -> Vec<Finding> {
    let xml = String::from_utf8_lossy(xml_bytes);
    let tags: &[(&str, &str, &str)] = &[
        ("dc:creator", "Author", "identity"),
        ("dc:rights", "Copyright", "identity"),
        ("photoshop:Credit", "Credit", "identity"),
        ("photoshop:City", "City", "place"),
        ("photoshop:State", "State", "place"),
        ("photoshop:Country", "Country", "place"),
        ("Iptc4xmpCore:Location", "Location", "place"),
        ("xmp:CreatorTool", "Software", "software"),
        ("tiff:Make", "Camera make", "device"),
        ("tiff:Model", "Camera model", "device"),
        ("exif:DateTimeOriginal", "Captured", "date"),
    ];
    let mut out = Vec::new();
    for (tag, lbl, kind) in tags {
        if let Some(v) = xmp_value(&xml, tag) {
            out.push(Finding::new(lbl, v, kind));
        }
    }
    out
}

/// Read an XMP value as either `<tag ...>text</tag>` (stripping nested rdf:li
/// markup) or the attribute form `tag="value"`.
fn xmp_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    if let Some(s) = xml.find(&open) {
        if let Some(gt) = xml[s..].find('>') {
            let inner_start = s + gt + 1;
            let close = format!("</{tag}>");
            if let Some(e) = xml[inner_start..].find(&close) {
                let text = strip_tags(&xml[inner_start..inner_start + e]);
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    let attr = format!("{tag}=\"");
    if let Some(s) = xml.find(&attr) {
        let start = s + attr.len();
        if let Some(e) = xml[start..].find('"') {
            let v = xml[start..start + e].trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Video / HEIC / M4A — via ffprobe (when ffmpeg is installed). Path-based.
// ---------------------------------------------------------------------------

pub fn video_findings(path: &std::path::Path) -> Vec<Finding> {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "flat", "-show_entries", "format_tags"])
        .arg(path)
        .output();
    let text = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return vec![],
    };

    let val = |key: &str| -> Option<String> {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("format.tags.{key}=")))
            .map(|v| v.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty())
    };

    let mut out = Vec::new();
    // Apple stores GPS as ISO-6709, e.g. "+28.6139+077.2090+000.000/".
    if let Some(loc) = val("com.apple.quicktime.location.ISO6709").or_else(|| val("location")) {
        match parse_iso6709(&loc) {
            Some((lat, lon)) => out.push(Finding::new("Location", format!("{lat:.5}, {lon:.5}"), "location")),
            None => out.push(Finding::new("Location", loc, "place")),
        }
    }
    if let Some(mk) = val("com.apple.quicktime.make") {
        out.push(Finding::new("Device make", mk, "device"));
    }
    if let Some(md) = val("com.apple.quicktime.model") {
        out.push(Finding::new("Device model", md, "device"));
    }
    if let Some(sw) = val("com.apple.quicktime.software").or_else(|| val("encoder")) {
        out.push(Finding::new("Software", sw, "software"));
    }
    if let Some(ct) = val("creation_time") {
        out.push(Finding::new("Created", ct, "date"));
    }
    out
}

fn parse_iso6709(s: &str) -> Option<(f64, f64)> {
    // "+DD.DDDD+DDD.DDDD.../" — find the two signed numbers.
    let body = s.trim_end_matches('/');
    let bytes = body.as_bytes();
    let mut splits = Vec::new();
    for (idx, &b) in bytes.iter().enumerate() {
        if (b == b'+' || b == b'-') && idx != 0 {
            splits.push(idx);
        }
    }
    if splits.is_empty() {
        return None;
    }
    let lat: f64 = body[..splits[0]].parse().ok()?;
    let lon_end = splits.get(1).copied().unwrap_or(body.len());
    let lon: f64 = body[splits[0]..lon_end].parse().ok()?;
    Some((lat, lon))
}

// ---------------------------------------------------------------------------
// PDF — the /Info dictionary.
// ---------------------------------------------------------------------------

fn pdf_findings(bytes: &[u8]) -> Vec<Finding> {
    let mut out = Vec::new();
    let doc = match lopdf::Document::load_mem(bytes) {
        Ok(d) => d,
        Err(_) => return out,
    };
    let info_ref = doc.trailer.get(b"Info").ok().and_then(|o| o.as_reference().ok());
    let info = match info_ref.and_then(|r| doc.get_object(r).ok()).and_then(|o| o.as_dict().ok()) {
        Some(d) => d,
        None => return out,
    };
    let map = [
        (&b"Author"[..], "Author", "identity"),
        (&b"Title"[..], "Title", "other"),
        (&b"Creator"[..], "Creator app", "software"),
        (&b"Producer"[..], "Producer", "software"),
        (&b"CreationDate"[..], "Created", "date"),
        (&b"ModDate"[..], "Modified", "date"),
    ];
    for (key, label, kind) in map {
        if let Ok(obj) = info.get(key) {
            if let Ok(s) = obj.as_str() {
                let val = String::from_utf8_lossy(s).trim().to_string();
                if !val.is_empty() {
                    // Only the date fields get date-formatting — never the text fields.
                    let display = if kind == "date" { clean_pdf_date(&val) } else { val };
                    out.push(Finding::new(label, display, kind));
                }
            }
        }
    }
    out
}

/// PDF dates look like "D:20240122072100Z" — show the readable part.
fn clean_pdf_date(s: &str) -> String {
    let d = s.strip_prefix("D:").unwrap_or(s);
    if d.len() >= 8 {
        let (y, rest) = d.split_at(4);
        let (mo, rest) = rest.split_at(2);
        let (da, _) = rest.split_at(2);
        return format!("{y}-{mo}-{da}");
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// Office (docx/xlsx/pptx) — docProps/core.xml + app.xml.
// ---------------------------------------------------------------------------

fn office_findings(bytes: &[u8]) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut archive = match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(a) => a,
        Err(_) => return out,
    };

    let read = |archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str| -> Option<String> {
        use std::io::Read;
        let mut f = archive.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };

    if let Some(core) = read(&mut archive, "docProps/core.xml") {
        for (tag, label, kind) in [
            ("dc:creator", "Author", "identity"),
            ("cp:lastModifiedBy", "Last modified by", "identity"),
            ("dcterms:created", "Created", "date"),
            ("dcterms:modified", "Modified", "date"),
        ] {
            if let Some(v) = xml_text(&core, tag) {
                out.push(Finding::new(label, v, kind));
            }
        }
    }
    if let Some(app) = read(&mut archive, "docProps/app.xml") {
        for (tag, label, kind) in [
            ("Company", "Company", "identity"),
            ("Manager", "Manager", "identity"),
            ("Application", "Application", "software"),
        ] {
            if let Some(v) = xml_text(&app, tag) {
                out.push(Finding::new(label, v, kind));
            }
        }
    }
    out
}

/// Tiny tag-text extractor (avoids pulling a full XML parser for a few fields).
fn xml_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let val = xml[start..end].trim();
    if val.is_empty() { None } else { Some(val.to_string()) }
}
