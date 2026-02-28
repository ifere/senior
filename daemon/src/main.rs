mod protocol;

use anyhow::Result;
use protocol::{AnalysisResult, ImpactedFile, Request, Response, SuggestedAction};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info};

const SOCKET_PATH: &str = "/tmp/callmeout.sock";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Remove stale socket
    if Path::new(SOCKET_PATH).exists() {
        std::fs::remove_file(SOCKET_PATH)?;
    }

    let listener = UnixListener::bind(SOCKET_PATH)?;
    info!("callmeout daemon listening on {}", SOCKET_PATH);

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                error!("connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(stream: UnixStream) -> Result<()> {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(trimmed) {
            Ok(Request::Ping) => Response::Pong,
            Ok(Request::AnalyzeDiff(payload)) => {
                // Stub: returns placeholder until LLM is wired in Task 5
                Response::AnalysisResult(AnalysisResult {
                    summary: vec![
                        format!("Changed {} file(s)", payload.files_touched.len()),
                        "Analysis pending LLM integration".to_string(),
                    ],
                    risk_level: "low".to_string(),
                    risk_reasons: vec!["Stub response — LLM not loaded yet".to_string()],
                    impacted_files: payload
                        .files_touched
                        .iter()
                        .map(|p| ImpactedFile {
                            path: p.clone(),
                            score: 0.5,
                            why: vec!["File was directly modified".to_string()],
                        })
                        .collect(),
                    impacted_symbols: vec![],
                    suggested_actions: vec![SuggestedAction {
                        label: "Review changes".to_string(),
                        explanation: "Inspect the modified files manually".to_string(),
                    }],
                    confidence: 0.0,
                })
            }
            Err(e) => Response::Error {
                message: format!("parse error: {}", e),
            },
        };

        let mut out = serde_json::to_string(&response)?;
        out.push('\n');
        writer.write_all(out.as_bytes()).await?;
    }

    Ok(())
}
