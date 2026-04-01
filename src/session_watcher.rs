use crate::session::SessionState;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub struct SessionWatcher {
    session_dir: PathBuf,
    sessions: HashMap<String, SessionState>,
}

impl SessionWatcher {
    pub fn new(session_dir: PathBuf) -> Self {
        Self {
            session_dir,
            sessions: HashMap::new(),
        }
    }

    pub fn scan_all(&mut self) -> Vec<SessionState> {
        let entries = match std::fs::read_dir(&self.session_dir) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                self.load_session(&path);
            }
        }

        let mut sessions: Vec<SessionState> = self.sessions.values().cloned().collect();
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }

    pub fn load_session(&mut self, path: &Path) -> Option<SessionState> {
        let content = std::fs::read_to_string(path).ok()?;
        let mut state: SessionState = serde_json::from_str(&content).ok()?;

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        state.session_id = session_id.clone();
        state.file_path = path.to_string_lossy().to_string();

        self.sessions.insert(session_id, state.clone());
        Some(state)
    }
}

pub async fn start_session_watcher(
    session_dir: PathBuf,
    tx: mpsc::Sender<Vec<SessionState>>,
) -> notify::Result<()> {
    let mut watcher = SessionWatcher::new(session_dir.clone());

    let sessions = watcher.scan_all();
    let _ = tx.send(sessions).await;

    // Channel to bridge notify's sync callback to async
    let (notify_tx, mut notify_rx) = mpsc::channel::<Event>(100);

    let mut fs_watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            let _ = notify_tx.blocking_send(event);
        }
    })?;

    std::fs::create_dir_all(&session_dir).ok();

    fs_watcher.watch(&session_dir, RecursiveMode::NonRecursive)?;

    while let Some(event) = notify_rx.recv().await {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                let sessions = watcher.scan_all();
                let _ = tx.send(sessions).await;
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_session_file(dir: &Path, id: &str, phase: &str, updated_at: u64) {
        let content = format!(
            r#"{{"phase":"{}","updated_at":{}}}"#,
            phase, updated_at
        );
        fs::write(dir.join(format!("{}.json", id)), content).unwrap();
    }

    #[test]
    fn scan_all_returns_empty_for_empty_dir() {
        let dir = TempDir::new().unwrap();
        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let sessions = watcher.scan_all();
        assert!(sessions.is_empty());
    }

    #[test]
    fn scan_all_loads_json_files_with_session_id_from_filename() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "abc-123", "idle", 100);
        write_session_file(dir.path(), "def-456", "implementing", 200);

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let sessions = watcher.scan_all();

        assert_eq!(sessions.len(), 2);
        // Sorted by updated_at desc
        assert_eq!(sessions[0].session_id, "def-456");
        assert_eq!(sessions[1].session_id, "abc-123");
    }

    #[test]
    fn scan_all_sorted_by_updated_at_descending() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "old", "idle", 10);
        write_session_file(dir.path(), "new", "planning", 300);
        write_session_file(dir.path(), "mid", "intake", 150);

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let sessions = watcher.scan_all();

        let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(ids, vec!["new", "mid", "old"]);
    }

    #[test]
    fn scan_all_ignores_non_json_files() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "valid", "idle", 100);
        fs::write(dir.path().join("notes.txt"), "not json").unwrap();
        fs::write(dir.path().join(".hidden"), "{}").unwrap();

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let sessions = watcher.scan_all();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "valid");
    }

    #[test]
    fn scan_all_skips_malformed_json() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "good", "idle", 100);
        fs::write(dir.path().join("bad.json"), "not valid json{{{").unwrap();

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let sessions = watcher.scan_all();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "good");
    }

    #[test]
    fn load_session_parses_and_sets_metadata() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "sess-1", "plan_review", 500);

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let path = dir.path().join("sess-1.json");
        let session = watcher.load_session(&path).unwrap();

        assert_eq!(session.session_id, "sess-1");
        assert_eq!(session.file_path, path.to_string_lossy());
        assert_eq!(session.updated_at, 500);
    }

    #[test]
    fn load_session_updates_internal_map() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "sess-1", "idle", 100);

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let path = dir.path().join("sess-1.json");
        watcher.load_session(&path);

        assert!(watcher.sessions.contains_key("sess-1"));

        // Update the file and reload
        write_session_file(dir.path(), "sess-1", "implementing", 200);
        let session = watcher.load_session(&path).unwrap();
        assert_eq!(session.updated_at, 200);
    }

    #[test]
    fn load_session_returns_none_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let result = watcher.load_session(Path::new("/nonexistent/file.json"));
        assert!(result.is_none());
    }

    #[test]
    fn load_session_returns_none_for_malformed_json() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("bad.json"), "{{invalid").unwrap();

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let result = watcher.load_session(&dir.path().join("bad.json"));
        assert!(result.is_none());
    }

    #[test]
    fn scan_all_tolerates_missing_directory() {
        let mut watcher = SessionWatcher::new(PathBuf::from("/tmp/nonexistent-session-dir-test"));
        let sessions = watcher.scan_all();
        assert!(sessions.is_empty());
    }

    #[test]
    fn scan_all_sets_file_path_on_each_session() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "test-id", "idle", 100);

        let mut watcher = SessionWatcher::new(dir.path().to_path_buf());
        let sessions = watcher.scan_all();

        assert_eq!(sessions.len(), 1);
        let expected_path = dir.path().join("test-id.json").to_string_lossy().to_string();
        assert_eq!(sessions[0].file_path, expected_path);
    }

    #[tokio::test]
    async fn start_watcher_sends_initial_scan() {
        let dir = TempDir::new().unwrap();
        write_session_file(dir.path(), "init-sess", "idle", 100);

        let (tx, mut rx) = mpsc::channel::<Vec<SessionState>>(10);
        let session_dir = dir.path().to_path_buf();

        // Spawn the watcher in a background task
        tokio::spawn(async move {
            let _ = start_session_watcher(session_dir, tx).await;
        });

        // Should receive initial scan
        let sessions = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            rx.recv(),
        )
        .await
        .expect("timeout waiting for initial scan")
        .expect("channel closed");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "init-sess");
    }

    #[tokio::test]
    async fn start_watcher_sends_update_on_file_change() {
        let dir = TempDir::new().unwrap();

        let (tx, mut rx) = mpsc::channel::<Vec<SessionState>>(10);
        let session_dir = dir.path().to_path_buf();

        tokio::spawn(async move {
            let _ = start_session_watcher(session_dir, tx).await;
        });

        // Consume initial scan (empty)
        let initial = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("channel closed");
        assert!(initial.is_empty());

        // Write a new session file
        // Small delay to ensure the watcher is set up
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        write_session_file(dir.path(), "new-sess", "planning", 200);

        // Should receive an update with the new session
        let updated = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        )
        .await
        .expect("timeout waiting for file change notification")
        .expect("channel closed");

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].session_id, "new-sess");
    }

    #[tokio::test]
    async fn start_watcher_creates_missing_directory() {
        let dir = TempDir::new().unwrap();
        let watch_dir = dir.path().join("nonexistent-subdir");

        let (tx, mut rx) = mpsc::channel::<Vec<SessionState>>(10);

        tokio::spawn(async move {
            let _ = start_session_watcher(watch_dir, tx).await;
        });

        // Should receive initial scan (empty, dir was created)
        let sessions = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("channel closed");
        assert!(sessions.is_empty());
    }
}
