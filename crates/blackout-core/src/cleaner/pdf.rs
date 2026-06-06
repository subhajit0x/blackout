//! PDF metadata removal.
//!
//! Two places carry identifying data:
//!   * the trailer's `/Info` dictionary (Author, Title, Creator, Producer, dates)
//!   * an XMP metadata stream referenced from the document catalog as `/Metadata`
//!
//! We remove both and rewrite the file.

use anyhow::Result;
use lopdf::Document;

pub fn clean_pdf(bytes: Vec<u8>) -> Result<(Vec<u8>, Vec<String>)> {
    let mut doc = Document::load_mem(&bytes)?;
    let mut removed = Vec::new();

    // ---- /Info dictionary in the trailer ----
    // The trailer may hold /Info either as an indirect reference (the common
    // case) or as a direct dictionary. Removing the trailer key alone leaves an
    // indirectly-referenced Info object orphaned but still written to the file,
    // so we must delete that object too.
    if let Some(info_obj) = doc.trailer.remove(b"Info") {
        if let Ok(info_ref) = info_obj.as_reference() {
            doc.delete_object(info_ref);
        }
        removed.push(
            "Document info dictionary (author, title, subject, keywords, creator, producer, creation/modification dates)"
                .into(),
        );
    }

    // ---- XMP /Metadata stream on the catalog ----
    // Resolve the catalog id first so the immutable borrow on `trailer` is
    // dropped before we take a mutable borrow on the document.
    let root_ref = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok());

    let mut metadata_ref = None;
    if let Some(root_ref) = root_ref {
        if let Ok(obj) = doc.get_object_mut(root_ref) {
            if let Ok(catalog) = obj.as_dict_mut() {
                if let Some(meta) = catalog.remove(b"Metadata") {
                    metadata_ref = meta.as_reference().ok();
                    removed.push("XMP metadata stream (application history, identifiers)".into());
                }
            }
        }
    }
    if let Some(meta_ref) = metadata_ref {
        doc.delete_object(meta_ref);
    }

    let mut out = Vec::with_capacity(bytes.len());
    doc.save_to(&mut out)?;
    Ok((out, removed))
}
