use crate::llm::CactusLlm;
use crate::protocol::{AnalysisResult, ImpactedFile, SuggestedAction};
use anyhow::Result;
use super::diff::DiffFile;

const SYSTEM_PROMPT: &str = "You are a senior software engineer reviewing a git diff. \
Analyze the changes and respond with ONLY a JSON object (no markdown, no extra text) in this exact format:\n\
{\n\
  \"summary\": [\"one sentence description of change 1\", \"one sentence description of change 2\"],\n\
  \"risk_level\": \"low\",\n\
  \"risk_reasons\": [\"reason why this is risky\"],\n\
  \"suggested_actions\": [{\"label\": \"Short action title\", \"explanation\": \"Why to do this\"}]\n\
}\n\
risk_level must be exactly: low, med, or high. Keep summary to max 3 bullets. Max 2 risk_reasons. Max 3 suggested_actions.";

pub fn build_prompt(files: &[DiffFile], raw_diff: &str) -> String {
    let file_summary: Vec<String> = files
        .iter()
        .map(|f| format!("{} (+{} -{})", f.path, f.added_lines, f.removed_lines))
        .collect();

    let diff_excerpt = if raw_diff.len() > 3000 {
        format!("{}...[truncated]", &raw_diff[..3000])
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

    // Try to parse the JSON — strip markdown fences if the model added them
    let json_str = extract_json(&raw);
    let parsed: serde_json::Value = serde_json::from_str(json_str.trim()).unwrap_or_else(|_| {
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

    Ok(AnalysisResult {
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
    })
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
}
