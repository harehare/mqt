//! Event-driven file watching for `--watch`, backed by the OS's native file
//! system notifications (inotify / FSEvents / ReadDirectoryChangesW) via the
//! `notify` crate, instead of polling files on a timer.

use notify_debouncer_mini::{
    DebounceEventResult, Debouncer, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

/// Watches one or more files for changes and reports changed paths.
///
/// Internally watches each file's *parent directory* rather than the file
/// itself, so that editor save patterns that replace the file (write to a
/// temp file, then rename over the original) are still detected reliably.
pub struct FileWatcher {
    debouncer: Debouncer<RecommendedWatcher>,
    receiver: Receiver<DebounceEventResult>,
    watched_dirs: HashSet<PathBuf>,
}

impl FileWatcher {
    pub fn new() -> notify_debouncer_mini::notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let debouncer = new_debouncer(Duration::from_millis(300), tx)?;
        Ok(Self {
            debouncer,
            receiver: rx,
            watched_dirs: HashSet::new(),
        })
    }

    /// Start watching `path` for changes. No-op if its parent directory is
    /// already watched.
    pub fn watch_file(&mut self, path: &Path) {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        if self.watched_dirs.insert(dir.clone()) {
            let _ = self
                .debouncer
                .watcher()
                .watch(&dir, RecursiveMode::NonRecursive);
        }
    }

    /// Drain all pending (already debounced) change notifications without
    /// blocking, returning the paths that changed.
    pub fn drain_changed_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        while let Ok(result) = self.receiver.try_recv() {
            if let Ok(events) = result {
                paths.extend(events.into_iter().map(|event| event.path));
            }
        }
        paths
    }
}

/// Whether `a` and `b` refer to the same file on disk, accounting for one
/// being relative and the other canonicalized (or vice versa).
pub fn paths_refer_to_same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_refer_to_same_file_identical() {
        let path = Path::new("foo.md");
        assert!(paths_refer_to_same_file(path, path));
    }

    #[test]
    fn test_paths_refer_to_same_file_canonicalized() {
        let tmp_dir = std::env::temp_dir();
        let file_path = tmp_dir.join(format!("mq_tui_watcher_test_{}.md", std::process::id()));
        std::fs::write(&file_path, "content").unwrap();

        let relative_looking = file_path.clone();
        assert!(paths_refer_to_same_file(&relative_looking, &file_path));

        std::fs::remove_file(&file_path).ok();
    }

    #[test]
    fn test_paths_refer_to_same_file_different() {
        assert!(!paths_refer_to_same_file(
            Path::new("/tmp/does-not-exist-a.md"),
            Path::new("/tmp/does-not-exist-b.md")
        ));
    }

    #[test]
    fn test_watch_file_detects_change() {
        let tmp_dir = std::env::temp_dir();
        let file_path = tmp_dir.join(format!("mq_tui_watcher_live_{}.md", std::process::id()));
        std::fs::write(&file_path, "before").unwrap();

        let mut watcher = FileWatcher::new().unwrap();
        watcher.watch_file(&file_path);

        // Give the OS watcher a moment to register before we write.
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(&file_path, "after").unwrap();

        // The debouncer waits ~300ms before emitting; poll a little longer
        // than that for the change notification.
        let mut found = false;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(50));
            if watcher
                .drain_changed_paths()
                .iter()
                .any(|p| paths_refer_to_same_file(p, &file_path))
            {
                found = true;
                break;
            }
        }

        std::fs::remove_file(&file_path).ok();
        assert!(found, "expected a change notification for the watched file");
    }
}
