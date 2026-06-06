//! Auto-clean a watched folder. Point BLACKOUT at e.g. your Screenshots folder
//! and every new file that lands there gets its metadata stripped automatically
//! into a `BLACKOUT-clean` subfolder. Desktop-only (mobile has no folder watcher).

#[cfg(desktop)]
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// Holds the live watcher; dropping it stops watching. Boxed as `Any` so the
/// struct is identical on every platform (the watcher type is desktop-only).
#[derive(Default)]
pub struct WatchState {
    watcher: Mutex<Option<Box<dyn std::any::Any + Send>>>,
}

#[cfg(desktop)]
#[derive(Clone, Serialize)]
pub struct WatchEvent {
    pub name: String,
    pub status: String,
    pub removed: Vec<String>,
}

#[cfg(desktop)]
const CLEAN_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "tif", "tiff", "heic", "heif", "mp3", "wav", "m4a", "pdf",
    "docx", "xlsx", "pptx", "txt",
];

#[cfg(desktop)]
fn is_cleanable(p: &std::path::Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| CLEAN_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[tauri::command]
pub fn start_watch(path: String, app: AppHandle, state: State<WatchState>) -> Result<String, String> {
    let folder = PathBuf::from(&path);
    if !folder.is_dir() {
        return Err("That isn't a folder.".into());
    }
    let out_dir = folder.join("BLACKOUT-clean");

    #[cfg(not(desktop))]
    {
        let _ = (&app, &state, &out_dir);
        Err("Folder watching is available on desktop only.".into())
    }
    #[cfg(desktop)]
    {
        let watcher = desktop::spawn(folder, out_dir.clone(), app)?;
        *state.watcher.lock().unwrap() = Some(watcher);
        Ok(out_dir.to_string_lossy().to_string())
    }
}

#[tauri::command]
pub fn stop_watch(state: State<WatchState>) {
    // Dropping the boxed watcher stops the OS-level file events.
    *state.watcher.lock().unwrap() = None;
}

#[cfg(desktop)]
mod desktop {
    use super::{is_cleanable, WatchEvent};
    use blackout_core::{clean_file, ffmpeg_available};
    use notify::{EventKind, RecursiveMode, Watcher};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tauri::{AppHandle, Emitter};

    pub fn spawn(
        folder: PathBuf,
        out_dir: PathBuf,
        app: AppHandle,
    ) -> Result<Box<dyn std::any::Any + Send>, String> {
        let ffmpeg = ffmpeg_available();
        let seen = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));
        let out2 = out_dir.clone();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event = match res {
                Ok(e) => e,
                Err(_) => return,
            };
            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                return;
            }
            for p in event.paths {
                // Skip our own output folder, non-files, and non-cleanable types.
                if p.starts_with(&out2) || !is_cleanable(&p) || !p.is_file() {
                    continue;
                }
                // De-dup: the OS fires several events per save.
                {
                    let mut s = seen.lock().unwrap();
                    if !s.insert(p.clone()) {
                        continue;
                    }
                }
                let report = clean_file(&p, &out2, ffmpeg);
                let _ = app.emit(
                    "watch-cleaned",
                    WatchEvent {
                        name: p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string(),
                        status: report.status,
                        removed: report.removed,
                    },
                );
            }
        })
        .map_err(|e| e.to_string())?;

        watcher
            .watch(&folder, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;
        Ok(Box::new(watcher))
    }
}
