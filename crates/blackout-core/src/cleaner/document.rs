//! Office Open XML (.docx/.xlsx/.pptx) metadata removal.
//!
//! These files are ZIP archives. The identifying metadata lives in a few parts
//! under `docProps/`:
//!   * core.xml   — author, last-modified-by, created/modified dates, revision
//!   * app.xml    — company, manager, application, template, total edit time
//!   * custom.xml — arbitrary custom properties
//!
//! We rewrite core.xml, app.xml and custom.xml as valid-but-empty property
//! documents, copying every other part byte-for-byte so the document stays valid
//! and openable. We deliberately *replace* (rather than delete) these parts:
//! `[Content_Types].xml` and `_rels/.rels` reference them, so removing the parts
//! would leave dangling references that make Office prompt to "repair" the file.

use anyhow::Result;
use std::io::{Cursor, Read, Write};
use zip::write::FileOptions;
use zip::CompressionMethod;

const EMPTY_CORE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"></cp:coreProperties>"#;

const EMPTY_APP: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"></Properties>"#;

const EMPTY_CUSTOM: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"></Properties>"#;

pub fn clean_office(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes))?;

    let mut out_buf: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut had_core = false;
    let mut had_app = false;
    let mut had_custom = false;

    {
        let mut zw = zip::ZipWriter::new(Cursor::new(&mut out_buf));
        let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();

            // Skip directory entries; the writer recreates structure from file paths.
            if name.ends_with('/') {
                continue;
            }

            match name.as_str() {
                "docProps/core.xml" => {
                    had_core = true;
                    zw.start_file(name, opts)?;
                    zw.write_all(EMPTY_CORE.as_bytes())?;
                }
                "docProps/app.xml" => {
                    had_app = true;
                    zw.start_file(name, opts)?;
                    zw.write_all(EMPTY_APP.as_bytes())?;
                }
                "docProps/custom.xml" => {
                    had_custom = true;
                    zw.start_file(name, opts)?;
                    zw.write_all(EMPTY_CUSTOM.as_bytes())?;
                }
                _ => {
                    let mut data = Vec::with_capacity(entry.size() as usize);
                    entry.read_to_end(&mut data)?;
                    zw.start_file(name, opts)?;
                    zw.write_all(&data)?;
                }
            }
        }
        zw.finish()?;
    }

    let mut removed = Vec::new();
    if had_core {
        removed.push(
            "Core properties (author, last-modified-by, created/modified dates, revision number)"
                .into(),
        );
    }
    if had_app {
        removed.push(
            "Extended properties (company, manager, application, template, total editing time)"
                .into(),
        );
    }
    if had_custom {
        removed.push("Custom document properties".into());
    }
    Ok((out_buf, removed))
}
