mod analyzer;
mod llm;
mod protocol;
mod store;

use anyhow::Result;
use protocol::{Request, Response};
use std::path::Path;
use std::sync::Arc;
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

    let audit = Arc::new(store::AuditLog::open("/tmp/callmeout-audit.db")?);

    let model_path = std::env::var("CACTUS_MODEL_PATH").unwrap_or_else(|_| {
        tracing::warn!("CACTUS_MODEL_PATH not set, trying default dev path");
        "/Users/chilly/dev/cactus/weights/functiongemma-270m-it".to_string()
    });

    let llm: Option<Arc<llm::CactusLlm>> = match llm::CactusLlm::new(&model_path) {
        Ok(l) => {
            info!("Cactus LLM loaded from {}", model_path);
            Some(Arc::new(l))
        }
        Err(e) => {
            tracing::warn!("Cactus LLM not available ({}), running in stub mode", e);
            None
        }
    };

    loop {
        let (stream, _) = listener.accept().await?;
        let audit = audit.clone();
        let llm = llm.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, audit, llm).await {
                error!("connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(
    stream: UnixStream,
    audit: Arc<store::AuditLog>,
    llm: Option<Arc<llm::CactusLlm>>,
) -> Result<()> {
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
                if let Err(e) = audit.log("analyze_diff", &payload.active_file) {
                    tracing::warn!("audit log write failed: {}", e);
                }
                let files = analyzer::diff::parse_diff(&payload.diff);
                match &llm {
                    Some(llm_ref) => {
                        // LLM inference is synchronous C FFI — move to blocking thread
                        // so the tokio async runtime stays responsive for other connections
                        let llm_clone = llm_ref.clone();
                        let files_clone = files.clone();
                        let diff_clone = payload.diff.clone();
                        match tokio::task::spawn_blocking(move || {
                            analyzer::impact::analyze(&llm_clone, &files_clone, &diff_clone)
                        }).await {
                            Ok(Ok(result)) => Response::AnalysisResult(result),
                            Ok(Err(e)) => Response::Error { message: e.to_string() },
                            Err(e) => Response::Error { message: format!("inference panicked: {}", e) },
                        }
                    },
                    None => Response::AnalysisResult(protocol::AnalysisResult {
                        summary: vec![
                            format!("Stub: {} file(s) changed", files.len()),
                            "Set CACTUS_MODEL_PATH to enable real analysis".to_string(),
                        ],
                        risk_level: "low".to_string(),
                        risk_reasons: vec!["LLM not loaded".to_string()],
                        impacted_files: files.iter().map(|f| protocol::ImpactedFile {
                            path: f.path.clone(),
                            score: 0.5,
                            why: vec![format!("+{} -{} lines", f.added_lines, f.removed_lines)],
                        }).collect(),
                        impacted_symbols: vec![],
                        suggested_actions: vec![],
                        confidence: 0.0,
                    }),
                }
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
