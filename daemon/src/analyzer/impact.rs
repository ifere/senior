use crate::llm::CactusLlm;
use crate::protocol::{AnalysisResult, ImpactedFile, SuggestedAction};
use anyhow::Result;
use super::diff::DiffFile;
use tracing::{debug, warn};

const SYSTEM_PROMPT: &str = "You are a code reviewer. Output ONLY a JSON object. No markdown. No explanation. Example output:\n\
{\"summary\":[\"added input validation\"],\"risk_level\":\"low\",\"risk_reasons\":[],\"suggested_actions\":[{\"label\":\"Add tests\",\"explanation\":\"Cover new validation logic\"}]}\n\
Rules: risk_level is low, med, or high. summary max 3 items. suggested_actions max 3 items.";

pub fn build_prompt(files: &[DiffFile], raw_diff: &str) -> String {
    let file_summary: Vec<String> = files
        .iter()
        .map(|f| format!("{} (+{} -{})", f.path, f.added_lines, f.removed_lines))
        .collect();

    let diff_excerpt = if raw_diff.chars().count() > 3000 {
        let truncated: String = raw_diff.chars().take(3000).collect();
        format!("{}...[truncated]", truncated)
    } else {
        raw_diff.to_string()
    };

    format!(
        "Files changed:\n{}\n\nDiff:\n```\n{}\n```",
        file_summary.join("\n"),
        diff_excerpt
    )
}

pub fn analyze(llm: &CactusLlm, files: &[DiffFile], raw_diff: &str) -> Result<AnalysisResult> {
    let prompt = build_prompt(files, raw_diff);
    let raw = llm.complete(SYSTEM_PROMPT, &prompt)?;
    debug!("llm text output: {}", raw);
    Ok(parse_analysis_json(&raw, files))
}

/// Parse LLM text output into an AnalysisResult. Pure function — no LLM call, fully testable.
pub fn parse_analysis_json(text: &str, files: &[DiffFile]) -> AnalysisResult {
    let json_str = extract_json(text);
    let parsed: serde_json::Value = serde_json::from_str(json_str.trim()).unwrap_or_else(|e| {
        warn!("json parse failed ({}), raw text was: {:?}", e, text);
        serde_json::json!({
            "summary": ["Changes analyzed (LLM parse error)"],
            "risk_level": "low",
            "risk_reasons": [],
            "suggested_actions": []
        })
    });

    let impacted_files: Vec<ImpactedFile> = files
        .iter()
        .map(|f| ImpactedFile {
            path: f.path.clone(),
            score: normalize_score(f.added_lines + f.removed_lines),
            why: vec![format!("+{} -{} lines", f.added_lines, f.removed_lines)],
        })
        .collect();

    AnalysisResult {
        summary: parsed["summary"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(|| vec!["Analysis complete".to_string()]),
        risk_level: parsed["risk_level"]
            .as_str()
            .unwrap_or("low")
            .to_string(),
        risk_reasons: parsed["risk_reasons"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        impacted_files,
        impacted_symbols: vec![],
        suggested_actions: parsed["suggested_actions"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        Some(SuggestedAction {
                            label: v["label"].as_str()?.to_string(),
                            explanation: v["explanation"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        confidence: parsed["confidence"].as_f64().unwrap_or(0.75) as f32,
    }
}

fn extract_json(raw: &str) -> &str {
    // Strip ```json ... ``` if model wrapped output in markdown
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            return &raw[start..=end];
        }
    }
    raw
}

fn normalize_score(lines: usize) -> f32 {
    match lines {
        0..=10 => 0.3,
        11..=50 => 0.6,
        _ => 0.9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_includes_file_names() {
        let files = vec![DiffFile {
            path: "src/foo.ts".to_string(),
            added_lines: 5,
            removed_lines: 2,
            hunks: vec![],
        }];
        let prompt = build_prompt(&files, "diff content");
        assert!(prompt.contains("src/foo.ts"));
        assert!(prompt.contains("+5 -2"));
    }

    #[test]
    fn test_build_prompt_truncates_large_diff() {
        let files: Vec<DiffFile> = vec![];
        let prompt = build_prompt(&files, &"x".repeat(5000));
        assert!(prompt.contains("[truncated]"));
    }

    #[test]
    fn test_extract_json_strips_markdown() {
        let wrapped = "```json\n{\"key\": \"val\"}\n```";
        assert_eq!(extract_json(wrapped), "{\"key\": \"val\"}");
    }

    #[test]
    fn test_normalize_score() {
        assert_eq!(normalize_score(5), 0.3);
        assert_eq!(normalize_score(25), 0.6);
        assert_eq!(normalize_score(100), 0.9);
    }

    // --- normalize_score boundary coverage ---

    #[test]
    fn test_normalize_score_zero() {
        assert_eq!(normalize_score(0), 0.3);
    }

    #[test]
    fn test_normalize_score_boundary_ten() {
        // 10 is the last value in the 0..=10 arm.
        assert_eq!(normalize_score(10), 0.3);
    }

    #[test]
    fn test_normalize_score_boundary_eleven() {
        // 11 is the first value in the 11..=50 arm.
        assert_eq!(normalize_score(11), 0.6);
    }

    #[test]
    fn test_normalize_score_boundary_fifty() {
        assert_eq!(normalize_score(50), 0.6);
    }

    #[test]
    fn test_normalize_score_boundary_fifty_one() {
        assert_eq!(normalize_score(51), 0.9);
    }

    // --- extract_json ---

    #[test]
    fn test_extract_json_returns_raw_when_no_braces() {
        // No '{' → the function must return the full input unchanged.
        let raw = "no json here";
        assert_eq!(extract_json(raw), raw);
    }

    #[test]
    fn test_extract_json_opening_brace_but_no_closing() {
        // Has '{' but no '}' → return full input.
        let raw = "{ oops no close";
        assert_eq!(extract_json(raw), raw);
    }

    #[test]
    fn test_extract_json_with_nested_braces() {
        // Outermost braces should be selected, not inner ones.
        let raw = r#"prefix {"outer": {"inner": 1}} suffix"#;
        assert_eq!(extract_json(raw), r#"{"outer": {"inner": 1}}"#);
    }

    #[test]
    fn test_extract_json_bare_object() {
        // No surrounding noise — should return as-is.
        let raw = r#"{"key":"val"}"#;
        assert_eq!(extract_json(raw), r#"{"key":"val"}"#);
    }

    // --- build_prompt ---

    #[test]
    fn test_build_prompt_with_no_files() {
        // Empty file list — prompt should still contain the diff.
        let files: Vec<DiffFile> = vec![];
        let prompt = build_prompt(&files, "diff content here");
        assert!(prompt.contains("diff content here"));
        assert!(prompt.contains("Files changed:"));
    }

    #[test]
    fn test_build_prompt_with_multiple_files() {
        let files = vec![
            DiffFile { path: "a.ts".into(), added_lines: 1, removed_lines: 0, hunks: vec![] },
            DiffFile { path: "b.rs".into(), added_lines: 5, removed_lines: 3, hunks: vec![] },
        ];
        let prompt = build_prompt(&files, "diff");
        assert!(prompt.contains("a.ts (+1 -0)"));
        assert!(prompt.contains("b.rs (+5 -3)"));
    }

    // --- parse_analysis_json ---

    #[test]
    fn test_parse_well_formed_json_extracts_all_fields() {
        let files = vec![DiffFile { path: "auth.ts".into(), added_lines: 5, removed_lines: 2, hunks: vec![] }];
        let json = r#"{"summary":["added input validation"],"risk_level":"med","risk_reasons":["no tests"],"suggested_actions":[{"label":"Add tests","explanation":"Cover new logic"}]}"#;
        let result = parse_analysis_json(json, &files);
        assert_eq!(result.summary, vec!["added input validation"]);
        assert_eq!(result.risk_level, "med");
        assert_eq!(result.risk_reasons, vec!["no tests"]);
        assert_eq!(result.suggested_actions.len(), 1);
        assert_eq!(result.suggested_actions[0].label, "Add tests");
        assert_eq!(result.suggested_actions[0].explanation, "Cover new logic");
    }

    #[test]
    fn test_parse_markdown_fenced_json_is_extracted() {
        let json = "```json\n{\"summary\":[\"refactored auth\"],\"risk_level\":\"high\",\"risk_reasons\":[\"auth logic changed\"],\"suggested_actions\":[]}\n```";
        let result = parse_analysis_json(json, &[]);
        assert_eq!(result.risk_level, "high");
        assert_eq!(result.summary, vec!["refactored auth"]);
    }

    #[test]
    fn test_parse_non_json_returns_llm_parse_error_summary() {
        let result = parse_analysis_json("I changed some things in the auth flow.", &[]);
        assert_eq!(result.summary, vec!["Changes analyzed (LLM parse error)"]);
        assert_eq!(result.risk_level, "low");
    }

    #[test]
    fn test_parse_empty_text_returns_fallback() {
        let result = parse_analysis_json("", &[]);
        assert_eq!(result.summary, vec!["Changes analyzed (LLM parse error)"]);
        assert_eq!(result.risk_level, "low");
    }

    #[test]
    fn test_parse_missing_risk_reasons_defaults_to_empty_vec() {
        let json = r#"{"summary":["fix typo"],"risk_level":"low","suggested_actions":[]}"#;
        let result = parse_analysis_json(json, &[]);
        assert!(result.risk_reasons.is_empty());
    }

    #[test]
    fn test_parse_missing_suggested_actions_defaults_to_empty_vec() {
        let json = r#"{"summary":["fix typo"],"risk_level":"low","risk_reasons":[]}"#;
        let result = parse_analysis_json(json, &[]);
        assert!(result.suggested_actions.is_empty());
    }

    #[test]
    fn test_parse_high_risk_level_preserved() {
        let json = r#"{"summary":["deleted prod table"],"risk_level":"high","risk_reasons":["data loss"],"suggested_actions":[]}"#;
        let result = parse_analysis_json(json, &[]);
        assert_eq!(result.risk_level, "high");
    }

    #[test]
    fn test_parse_json_with_preamble_text_extracts_embedded_json() {
        // Model sometimes outputs prose before the JSON object.
        let raw = r#"Sure, here is my analysis: {"summary":["added logging"],"risk_level":"low","risk_reasons":[],"suggested_actions":[]}"#;
        let result = parse_analysis_json(raw, &[]);
        assert_eq!(result.risk_level, "low");
        assert_eq!(result.summary, vec!["added logging"]);
    }

    #[test]
    fn test_parse_impacted_files_scored_by_line_count() {
        let files = vec![
            DiffFile { path: "small.ts".into(), added_lines: 3,  removed_lines: 0,  hunks: vec![] },
            DiffFile { path: "mid.rs".into(),   added_lines: 10, removed_lines: 10, hunks: vec![] },
            DiffFile { path: "large.go".into(), added_lines: 50, removed_lines: 30, hunks: vec![] },
        ];
        let json = r#"{"summary":[],"risk_level":"low","risk_reasons":[],"suggested_actions":[]}"#;
        let result = parse_analysis_json(json, &files);
        assert_eq!(result.impacted_files[0].score, 0.3); // 3 lines  → low band
        assert_eq!(result.impacted_files[1].score, 0.6); // 20 lines → mid band
        assert_eq!(result.impacted_files[2].score, 0.9); // 80 lines → high band
    }

    #[test]
    fn test_parse_impacted_files_why_label_shows_added_and_removed() {
        let files = vec![DiffFile { path: "x.ts".into(), added_lines: 7, removed_lines: 3, hunks: vec![] }];
        let json = r#"{"summary":[],"risk_level":"low","risk_reasons":[],"suggested_actions":[]}"#;
        let result = parse_analysis_json(json, &files);
        assert_eq!(result.impacted_files[0].why, vec!["+7 -3 lines"]);
    }

    #[test]
    fn test_parse_no_files_produces_empty_impacted_list() {
        let json = r#"{"summary":["minor fix"],"risk_level":"low","risk_reasons":[],"suggested_actions":[]}"#;
        let result = parse_analysis_json(json, &[]);
        assert!(result.impacted_files.is_empty());
    }

    // --- SYSTEM_PROMPT structure ---

    #[test]
    fn test_system_prompt_contains_all_risk_level_values() {
        assert!(SYSTEM_PROMPT.contains("low"),  "prompt must mention 'low'");
        assert!(SYSTEM_PROMPT.contains("med"),  "prompt must mention 'med'");
        assert!(SYSTEM_PROMPT.contains("high"), "prompt must mention 'high'");
    }

    #[test]
    fn test_system_prompt_contains_required_output_keys() {
        assert!(SYSTEM_PROMPT.contains("risk_level"),       "prompt must include 'risk_level'");
        assert!(SYSTEM_PROMPT.contains("summary"),          "prompt must include 'summary'");
        assert!(SYSTEM_PROMPT.contains("suggested_actions"),"prompt must include 'suggested_actions'");
    }
}
