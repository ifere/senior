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

    #[test]
    fn test_serialize_error_response() {
        let resp = Response::Error { message: "something broke".to_string() };
        let json = serde_json::to_string(&resp).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["type"], "error");
        assert_eq!(val["payload"]["message"], "something broke");
    }

    #[test]
    fn test_serialize_analysis_result() {
        let result = Response::AnalysisResult(super::AnalysisResult {
            summary: vec!["changed auth flow".to_string()],
            risk_level: "high".to_string(),
            risk_reasons: vec!["touches tokens".to_string()],
            impacted_files: vec![],
            impacted_symbols: vec![],
            suggested_actions: vec![],
            confidence: 0.9,
        });
        let json = serde_json::to_string(&result).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["type"], "analysis_result");
        assert_eq!(val["payload"]["risk_level"], "high");
        assert_eq!(val["payload"]["confidence"], 0.9);
    }

    #[test]
    fn test_deserialize_invalid_json_returns_error() {
        let bad = "not json at all";
        let result = serde_json::from_str::<Request>(bad);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_unknown_type_returns_error() {
        // Valid JSON but type field is unrecognised.
        let bad = r#"{"type":"unknown_command","payload":{}}"#;
        let result = serde_json::from_str::<Request>(bad);
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_diff_payload_all_fields_accessible() {
        let raw = r#"{"type":"analyze_diff","payload":{"diff":"the diff","files_touched":["a.ts","b.ts"],"active_file":"a.ts","trigger":"save"}}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        if let Request::AnalyzeDiff(payload) = req {
            assert_eq!(payload.diff, "the diff");
            assert_eq!(payload.files_touched, vec!["a.ts", "b.ts"]);
            assert_eq!(payload.active_file, "a.ts");
            assert_eq!(payload.trigger, "save");
        } else {
            panic!("expected AnalyzeDiff");
        }
    }

    #[test]
    fn test_impacted_file_serializes_correctly() {
        let f = super::ImpactedFile {
            path: "src/auth.rs".to_string(),
            score: 0.9,
            why: vec!["big change".to_string()],
        };
        let json = serde_json::to_string(&f).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["path"], "src/auth.rs");
        assert_eq!(val["score"], 0.9);
        assert_eq!(val["why"][0], "big change");
    }

    #[test]
    fn test_suggested_action_serializes_correctly() {
        let action = super::SuggestedAction {
            label: "Add tests".to_string(),
            explanation: "This path has no coverage".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["label"], "Add tests");
        assert_eq!(val["explanation"], "This path has no coverage");
    }
}
