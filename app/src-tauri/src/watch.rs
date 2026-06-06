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
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter};

    /// Dropping this stops both the OS watcher and the debounce worker thread.
    pub struct WatchGuard {
        _watcher: notify::RecommendedWatcher,
        running: Arc<AtomicBool>,
    }
    impl Drop for WatchGuard {
        fn drop(&mut self) {
            self.running.store(false, Ordering::Relaxed);
        }
    }

    /// A new file fires several events while it's still being written. We record
    /// the time of the *last* event per path and only clean once it has been
    /// quiet (write finished) for SETTLE — so we never read a half-written file.
    const SETTLE: Duration = Duration::from_millis(300);

    pub fn spawn(
        folder: PathBuf,
        out_dir: PathBuf,
        app: AppHandle,
    ) -> Result<Box<dyn std::any::Any + Send>, String> {
        let ffmpeg = ffmpeg_available();
        let pending: Arc<Mutex<HashMap<PathBuf, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
        let done: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let running = Arc::new(AtomicBool::new(true));

        // --- notify callback: just record "this path changed at T" ---
        let (p_cb, d_cb, out_cb) = (pending.clone(), done.clone(), out_dir.clone());
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let event = match res {
                Ok(e) => e,
                Err(_) => return,
            };
            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                return;
            }
            for p in event.paths {
                if p.starts_with(&out_cb) || !is_cleanable(&p) {
                    continue;
                }
                if d_cb.lock().unwrap().contains(&p) {
                    continue; // already cleaned this one
                }
                p_cb.lock().unwrap().insert(p, Instant::now());
            }
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(&folder, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;

        // --- worker: clean files once they've settled ---
        let run2 = running.clone();
        let out2 = out_dir.clone();
        std::thread::spawn(move || {
            // How many times we'll re-check a still-empty file before concluding
            // it's genuinely empty (not mid-write) and giving up — avoids an
            // empty dropped file looping in the queue forever.
            const MAX_RETRIES: u8 = 8;
            let mut attempts: HashMap<PathBuf, u8> = HashMap::new();
            while run2.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(120));
                let ready: Vec<PathBuf> = {
                    let mut pend = pending.lock().unwrap();
                    let now = Instant::now();
                    let ready: Vec<PathBuf> = pend
                        .iter()
                        .filter(|(_, t)| now.duration_since(**t) >= SETTLE)
                        .map(|(p, _)| p.clone())
                        .collect();
                    for p in &ready {
                        pend.remove(p);
                    }
                    ready
                };
                for p in ready {
                    if !p.is_file() {
                        attempts.remove(&p);
                        continue;
                    }
                    // Still empty/being written? Re-queue a few times, then give up.
                    if std::fs::metadata(&p).map(|m| m.len() == 0).unwrap_or(true) {
                        let n = attempts.entry(p.clone()).or_insert(0);
                        *n += 1;
                        if *n <= MAX_RETRIES {
                            pending.lock().unwrap().insert(p, Instant::now());
                        } else {
                            attempts.remove(&p);
                        }
                        continue;
                    }
                    attempts.remove(&p);
                    done.lock().unwrap().insert(p.clone());
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
            }
        });

        Ok(Box::new(WatchGuard { _watcher: watcher, running }))
    }
}
