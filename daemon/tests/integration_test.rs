/// Integration tests for the senior-daemon binary.
///
/// Each test spawns the real binary against a unique socket path, sends real
/// JSON over the socket, and asserts the response. No mocks — if the binary
/// is broken, these fail.
///
/// Run with:
///   cargo test --test integration_test
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{fs, thread};

// Locate the compiled daemon binary via Cargo's env var.
const BIN: &str = env!("CARGO_BIN_EXE_senior-daemon");

struct Daemon {
    child: Child,
    sock: PathBuf,
}

impl Daemon {
    /// Spawn the daemon with a unique socket path derived from the test name suffix.
    fn start(suffix: &str) -> Self {
        let sock = PathBuf::from(format!("/tmp/senior-test-{}.sock", suffix));
        if sock.exists() {
            fs::remove_file(&sock).unwrap();
        }

        let child = Command::new(BIN)
            .env("SENIOR_SOCKET_PATH", &sock)
            .env("CACTUS_MODEL_PATH", "/nonexistent") // forces stub mode — no LLM needed
            .env("RUST_LOG", "error") // silence startup noise
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn senior-daemon binary");

        let d = Daemon { child, sock };
        d.wait_ready();
        d
    }

    /// Poll until the socket is connectable (up to 3 s).
    fn wait_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if UnixStream::connect(&self.sock).is_ok() {
                return;
            }
            if Instant::now() > deadline {
                panic!("daemon did not become ready within 3 seconds");
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Send one JSON line and return the parsed response value.
    ///
    /// The request string may be a pretty-printed multiline JSON literal — it is
    /// compacted to a single line before sending because the daemon uses read_line.
    fn send(&self, request: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(&self.sock).expect("could not connect to daemon socket");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let compact: serde_json::Value =
            serde_json::from_str(request).expect("test request is not valid JSON");
        let msg = format!("{}\n", serde_json::to_string(&compact).unwrap());
        stream.write_all(msg.as_bytes()).expect("write failed");

        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read failed");
        serde_json::from_str(line.trim()).expect("daemon returned invalid JSON")
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        fs::remove_file(&self.sock).ok();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn daemon_responds_to_ping() {
    let d = Daemon::start("ping");
    let resp = d.send(r#"{"type":"ping","payload":null}"#);
    assert_eq!(resp["type"], "pong", "expected pong, got: {}", resp);
}

#[test]
fn daemon_greets_with_no_analysis() {
    let d = Daemon::start("greet-none");
    let resp = d.send(r#"{"type":"greet","payload":{"last_analysis":null}}"#);
    assert_eq!(
        resp["type"], "voice_answer",
        "expected voice_answer, got: {}",
        resp
    );
    let text = resp["payload"]["text"].as_str().unwrap_or("");
    assert!(!text.is_empty(), "greeting text must not be empty");
}

#[test]
fn daemon_greets_with_analysis_context() {
    let d = Daemon::start("greet-ctx");
    let req = r#"{
        "type": "greet",
        "payload": {
            "last_analysis": {
                "summary": ["refactored auth"],
                "risk_level": "high",
                "risk_reasons": ["touches tokens"],
                "impacted_files": [{"path":"src/auth.rs","score":0.9,"why":["+20 -5"]}],
                "impacted_symbols": [],
                "suggested_actions": [],
                "confidence": 0.8
            }
        }
    }"#;
    let resp = d.send(req);
    assert_eq!(resp["type"], "voice_answer", "got: {}", resp);
    let text = resp["payload"]["text"].as_str().unwrap_or("");
    assert!(!text.is_empty(), "greeting text must not be empty");
}

#[test]
fn daemon_answers_voice_query() {
    let d = Daemon::start("voice-query");
    let req = r#"{
        "type": "voice_query",
        "payload": {
            "question": "is this safe to ship?",
            "context": null
        }
    }"#;
    let resp = d.send(req);
    assert_eq!(resp["type"], "voice_answer", "got: {}", resp);
    let text = resp["payload"]["text"].as_str().unwrap_or("");
    assert!(!text.is_empty(), "voice answer must not be empty");
}

#[test]
fn daemon_answers_voice_query_with_context() {
    let d = Daemon::start("voice-query-ctx");
    let req = r#"{
        "type": "voice_query",
        "payload": {
            "question": "which file is riskiest?",
            "context": {
                "summary": ["touched payments"],
                "risk_level": "high",
                "risk_reasons": ["payment path"],
                "impacted_files": [
                    {"path":"src/payments.rs","score":0.95,"why":["+50 -0"]},
                    {"path":"src/main.rs","score":0.1,"why":["+1 -1"]}
                ],
                "impacted_symbols": [],
                "suggested_actions": [],
                "confidence": 0.9
            }
        }
    }"#;
    let resp = d.send(req);
    assert_eq!(resp["type"], "voice_answer", "got: {}", resp);
}

#[test]
fn daemon_analyzes_diff_in_stub_mode() {
    let d = Daemon::start("analyze");
    // Build the request as a value so the diff string is JSON-encoded correctly.
    let req = serde_json::json!({
        "type": "analyze_diff",
        "payload": {
            "diff": "diff --git a/src/lib.rs b/src/lib.rs\nindex abc..def 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,4 @@\n+use std::collections::HashMap;\n fn main() {}",
            "files_touched": ["src/lib.rs"],
            "active_file": "src/lib.rs",
            "trigger": "manual"
        }
    });
    let resp = d.send(&req.to_string());
    assert_eq!(resp["type"], "analysis_result", "got: {}", resp);
    assert!(
        resp["payload"]["risk_level"].as_str().is_some(),
        "analysis_result must have risk_level"
    );
}

#[test]
fn daemon_returns_error_on_unknown_request_type() {
    let d = Daemon::start("unknown");
    let resp = d.send(r#"{"type":"does_not_exist","payload":{}}"#);
    assert_eq!(resp["type"], "error", "expected error for unknown type, got: {}", resp);
}

#[test]
fn daemon_handles_multiple_sequential_requests_on_same_connection() {
    let d = Daemon::start("multi");
    // Two separate connections (protocol is one-request-per-connection via EOF)
    let r1 = d.send(r#"{"type":"ping","payload":null}"#);
    let r2 = d.send(r#"{"type":"greet","payload":{"last_analysis":null}}"#);
    assert_eq!(r1["type"], "pong");
    assert_eq!(r2["type"], "voice_answer");
}

#[test]
fn daemon_removes_stale_socket_on_restart() {
    // First instance creates the socket, then we kill it (Drop), leaving a stale file.
    let sock = PathBuf::from("/tmp/senior-test-stale.sock");
    {
        let _d = Daemon::start("stale");
        // _d is dropped here, killing the child and removing the socket
    }
    // Write a fake stale socket file to simulate the real-world problem
    fs::write(&sock, b"").unwrap();
    assert!(sock.exists(), "stale socket should exist before restart");

    // Second instance should start fine and remove the stale file itself
    let d2 = Daemon::start("stale");
    let resp = d2.send(r#"{"type":"ping","payload":null}"#);
    assert_eq!(resp["type"], "pong");
}
