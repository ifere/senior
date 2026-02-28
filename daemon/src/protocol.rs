use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Request {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "analyze_diff")]
    AnalyzeDiff(AnalyzeDiffPayload),
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeDiffPayload {
    pub diff: String,
    pub files_touched: Vec<String>,
    pub active_file: String,
    pub trigger: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum Response {
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "analysis_result")]
    AnalysisResult(AnalysisResult),
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
    pub summary: Vec<String>,
    pub risk_level: String,
    pub risk_reasons: Vec<String>,
    pub impacted_files: Vec<ImpactedFile>,
    pub impacted_symbols: Vec<ImpactedSymbol>,
    pub suggested_actions: Vec<SuggestedAction>,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImpactedFile {
    pub path: String,
    pub score: f32,
    pub why: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImpactedSymbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub score: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct SuggestedAction {
    pub label: String,
    pub explanation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_ping() {
        let raw = r#"{"type":"ping","payload":null}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert!(matches!(req, Request::Ping));
    }

    #[test]
    fn test_deserialize_analyze_diff() {
        let raw = r#"{"type":"analyze_diff","payload":{"diff":"--- a/foo.ts\n+++ b/foo.ts","files_touched":["foo.ts"],"active_file":"foo.ts","trigger":"save"}}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert!(matches!(req, Request::AnalyzeDiff(_)));
    }

    #[test]
    fn test_serialize_pong() {
        let resp = Response::Pong;
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("pong"));
    }
}
