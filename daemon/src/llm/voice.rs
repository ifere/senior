use crate::llm::CactusLlm;
use crate::protocol::AnalysisResult;
use anyhow::Result;

pub const SYSTEM_PROMPT: &str = "You are Senior, a voice-first AI pair programmer. \
    Be conversational and brief — max 2 sentences. \
    You are speaking aloud, not writing. No markdown, no lists, just natural spoken words.";

pub fn greet(llm: &CactusLlm, last_analysis: Option<&AnalysisResult>) -> Result<String> {
    llm.complete(SYSTEM_PROMPT, &build_greet_prompt(last_analysis))
}

pub fn answer(llm: &CactusLlm, question: &str, context: Option<&AnalysisResult>) -> Result<String> {
    llm.complete(SYSTEM_PROMPT, &build_answer_prompt(question, context))
}

pub fn build_greet_prompt(last_analysis: Option<&AnalysisResult>) -> String {
    match last_analysis {
        None => "The developer has no uncommitted changes. \
            Greet them warmly and ask what they would like to work on.".to_string(),
        Some(a) => format!(
            "The developer changed {} file(s): {}. Risk: {}. Summary: {}. \
            Give a brief spoken greeting summarising what they changed and the risk level.",
            a.impacted_files.len(),
            a.impacted_files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join(", "),
            a.risk_level,
            a.summary.join(", ")
        ),
    }
}

pub fn build_answer_prompt(question: &str, context: Option<&AnalysisResult>) -> String {
    match context {
        None => format!(
            "The developer asked: \"{}\". There is no current analysis context. Answer briefly.",
            question
        ),
        Some(a) => format!(
            "The developer asked: \"{}\". Context — risk: {}, changed files: {}, summary: {}. \
            Answer briefly in natural spoken language.",
            question,
            a.risk_level,
            a.impacted_files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join(", "),
            a.summary.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ImpactedFile;

    fn make_analysis() -> AnalysisResult {
        AnalysisResult {
            summary: vec!["refactored auth flow".to_string()],
            risk_level: "med".to_string(),
            risk_reasons: vec!["touches auth tokens".to_string()],
            impacted_files: vec![ImpactedFile {
                path: "src/auth.rs".to_string(),
                score: 0.6,
                why: vec!["+10 -3 lines".to_string()],
            }],
            impacted_symbols: vec![],
            suggested_actions: vec![],
            confidence: 0.8,
        }
    }

    #[test]
    fn test_greet_prompt_no_analysis_mentions_no_changes() {
        let prompt = build_greet_prompt(None);
        assert!(
            prompt.contains("no uncommitted changes") || prompt.contains("no changes"),
            "prompt was: {}", prompt
        );
    }

    #[test]
    fn test_greet_prompt_with_analysis_mentions_risk_and_files() {
        let a = make_analysis();
        let prompt = build_greet_prompt(Some(&a));
        assert!(prompt.contains("med"), "should mention risk level, got: {}", prompt);
        assert!(prompt.contains("auth.rs"), "should mention file, got: {}", prompt);
    }

    #[test]
    fn test_answer_prompt_includes_question() {
        let prompt = build_answer_prompt("is this safe to ship?", None);
        assert!(prompt.contains("is this safe to ship?"));
    }

    #[test]
    fn test_answer_prompt_with_context_includes_file_and_risk() {
        let a = make_analysis();
        let prompt = build_answer_prompt("which file first?", Some(&a));
        assert!(prompt.contains("which file first?"));
        assert!(prompt.contains("auth.rs"));
        assert!(prompt.contains("med"));
    }

    #[test]
    fn test_system_prompt_is_brief_and_spoken() {
        assert!(
            SYSTEM_PROMPT.contains("brief") || SYSTEM_PROMPT.contains("short"),
            "prompt should say brief"
        );
        assert!(
            SYSTEM_PROMPT.contains("aloud") || SYSTEM_PROMPT.contains("spoken") || SYSTEM_PROMPT.contains("speaking"),
            "prompt should say speaking aloud"
        );
    }
}
