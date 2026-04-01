use crate::event::HookEvent;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

/// Start listening on a Unix socket, parsing each line as a HookEvent
/// and forwarding it through the mpsc sender.
///
/// Removes a stale socket file if one exists at the path before binding.
/// Each accepted connection is handled concurrently in a spawned task.
/// Malformed JSON lines are logged to stderr and skipped.
pub async fn start_listener(
    socket_path: PathBuf,
    tx: mpsc::Sender<HookEvent>,
) -> std::io::Result<()> {
    match std::fs::remove_file(&socket_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let listener = UnixListener::bind(&socket_path)?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let reader = BufReader::new(stream);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        match serde_json::from_str::<HookEvent>(&line) {
                            Ok(event) => {
                                let _ = tx.send(event).await;
                            }
                            Err(e) => {
                                eprintln!("Malformed event JSON: {}", e);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("Socket accept error: {}", e);
            }
        }
    }
}

/// Remove the socket file if it exists. Best-effort, ignores errors.
pub fn cleanup_socket(socket_path: &Path) {
    let _ = std::fs::remove_file(socket_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    use std::time::Duration;

    #[tokio::test]
    async fn listener_receives_valid_hook_event() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        let (tx, mut rx) = mpsc::channel::<HookEvent>(16);

        let path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            start_listener(path_clone, tx).await.unwrap();
        });

        // Give listener time to bind
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Connect and send a valid JSON line
        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let json_line = r#"{"hook_event_name":"PreToolUse","session_id":"test-123","tool_name":"Read"}"#;
        stream.write_all(format!("{}\n", json_line).as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        // Receive the event
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed without event");

        assert_eq!(event.hook_event_name, "PreToolUse");
        assert_eq!(event.session_id, "test-123");

        handle.abort();
        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn listener_skips_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("test_malformed.sock");
        let (tx, mut rx) = mpsc::channel::<HookEvent>(16);

        let path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            start_listener(path_clone, tx).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        // Send malformed JSON followed by valid JSON
        stream.write_all(b"not valid json\n").await.unwrap();
        let valid = r#"{"hook_event_name":"Stop","session_id":"s1"}"#;
        stream.write_all(format!("{}\n", valid).as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        // Only the valid event should arrive
        assert_eq!(event.hook_event_name, "Stop");

        handle.abort();
        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn listener_removes_stale_socket() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("stale.sock");

        // Create a stale file
        std::fs::write(&socket_path, "stale").unwrap();
        assert!(socket_path.exists());

        let (tx, _rx) = mpsc::channel::<HookEvent>(16);
        let path_clone = socket_path.clone();
        let handle = tokio::spawn(async move {
            start_listener(path_clone, tx).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should be able to connect (stale file was removed and socket rebound)
        let result = UnixStream::connect(&socket_path).await;
        assert!(result.is_ok(), "Failed to connect after stale socket removal");

        handle.abort();
        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn cleanup_socket_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("cleanup.sock");
        std::fs::write(&socket_path, "data").unwrap();
        assert!(socket_path.exists());

        cleanup_socket(&socket_path);
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn cleanup_socket_noop_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("nonexistent.sock");
        // Should not panic
        cleanup_socket(&socket_path);
    }
}
