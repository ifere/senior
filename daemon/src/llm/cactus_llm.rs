use anyhow::{anyhow, Result};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Mutex;
use tracing::debug;

extern "C" {
    fn cactus_init(
        model_path: *const c_char,
        corpus_dir: *const c_char,
        cache_index: bool,
    ) -> *mut c_void;

    fn cactus_complete(
        model: *mut c_void,
        messages_json: *const c_char,
        response_buffer: *mut c_char,
        buffer_size: usize,
        options_json: *const c_char,
        tools_json: *const c_char,
        callback: Option<extern "C" fn(*const c_char, u32, *mut c_void)>,
        user_data: *mut c_void,
    ) -> c_int;

    fn cactus_destroy(model: *mut c_void);

    fn cactus_reset(model: *mut c_void);

    fn cactus_get_last_error() -> *const c_char;
}

pub struct CactusLlm {
    model: Mutex<*mut c_void>,
}

// Safety: cactus_model_t is an opaque pointer, access serialized via Mutex
unsafe impl Send for CactusLlm {}
unsafe impl Sync for CactusLlm {}

impl CactusLlm {
    pub fn new(model_path: &str) -> Result<Self> {
        let model_path_c = CString::new(model_path)?;
        let corpus_dir_c = CString::new("")?;

        let model = unsafe {
            cactus_init(model_path_c.as_ptr(), corpus_dir_c.as_ptr(), false)
        };

        if model.is_null() {
            let err = unsafe {
                let ptr = cactus_get_last_error();
                if ptr.is_null() {
                    "unknown error".to_string()
                } else {
                    CStr::from_ptr(ptr).to_string_lossy().into_owned()
                }
            };
            return Err(anyhow!("cactus_init failed: {}", err));
        }

        Ok(Self {
            model: Mutex::new(model),
        })
    }

    pub fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let messages = serde_json::json!([
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_message }
        ]);
        let messages_c = CString::new(messages.to_string())?;

        let options = serde_json::json!({
            "max_tokens": 512,
            "temperature": 0.1
        });
        let options_c = CString::new(options.to_string())?;

        let raw_json = self.run_complete(messages_c, options_c, std::ptr::null())?;
        parse_cactus_response(&raw_json)
    }

    /// Call the model using native function-calling (tools_json + force_tools).
    /// Returns the `function_calls` array from the cactus response envelope.
    pub fn complete_with_tools(
        &self,
        user_message: &str,
        tools_json: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let messages = serde_json::json!([
            { "role": "user", "content": user_message }
        ]);
        let messages_c = CString::new(messages.to_string())?;

        let options = serde_json::json!({
            "max_tokens": 512,
            "temperature": 0.1,
            "force_tools": true
        });
        let options_c = CString::new(options.to_string())?;
        let tools_c = CString::new(tools_json)?;

        let raw_json = self.run_complete(messages_c, options_c, tools_c.as_ptr())?;
        debug!("cactus raw response (tools): {}", raw_json);

        let parsed: serde_json::Value = serde_json::from_str(&raw_json)
            .map_err(|e| anyhow!("failed to parse cactus envelope: {}: {}", e, raw_json))?;

        if parsed["success"].as_bool() != Some(true) {
            let err = parsed["error"].as_str().unwrap_or("unknown error");
            return Err(anyhow!("cactus returned failure: {}", err));
        }

        let calls = parsed["function_calls"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(calls)
    }

    fn run_complete(
        &self,
        messages_c: CString,
        options_c: CString,
        tools_ptr: *const c_char,
    ) -> Result<String> {
        // 8KB response buffer
        let mut response_buf: Vec<c_char> = vec![0; 8192];

        let model = self.model.lock().unwrap();
        // Clear any cached conversation state from previous calls so each
        // invocation starts from a fresh context.
        unsafe { cactus_reset(*model) };
        let ret = unsafe {
            cactus_complete(
                *model,
                messages_c.as_ptr(),
                response_buf.as_mut_ptr(),
                response_buf.len(),
                options_c.as_ptr(),
                tools_ptr,
                None,
                std::ptr::null_mut(),
            )
        };

        if ret < 0 {
            let err = unsafe {
                let ptr = cactus_get_last_error();
                if ptr.is_null() {
                    "unknown error".to_string()
                } else {
                    CStr::from_ptr(ptr).to_string_lossy().into_owned()
                }
            };
            return Err(anyhow!("cactus_complete failed (ret={}): {}", ret, err));
        }

        let raw_json = unsafe {
            CStr::from_ptr(response_buf.as_ptr())
                .to_string_lossy()
                .into_owned()
        };

        debug!("cactus raw response: {}", raw_json);
        Ok(raw_json)
    }
}

/// Parse the cactus response envelope: `{"success":true,"response":"..."}`.
/// Extracted as a pure function so it can be unit tested without FFI.
fn parse_cactus_response(raw_json: &str) -> Result<String> {
    let parsed: serde_json::Value = serde_json::from_str(raw_json)
        .map_err(|e| anyhow!("failed to parse cactus response JSON: {}: {}", e, raw_json))?;

    if parsed["success"].as_bool() != Some(true) {
        let err = parsed["error"].as_str().unwrap_or("unknown error");
        return Err(anyhow!("cactus returned failure: {}", err));
    }

    let text = parsed["response"]
        .as_str()
        .ok_or_else(|| anyhow!("cactus response missing 'response' field: {}", raw_json))?
        .to_string();

    Ok(text)
}

impl Drop for CactusLlm {
    fn drop(&mut self) {
        let model = self.model.lock().unwrap();
        if !model.is_null() {
            unsafe { cactus_destroy(*model) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_envelope_returns_response_text() {
        let json = r#"{"success":true,"response":"Hello world"}"#;
        let result = parse_cactus_response(json);
        assert_eq!(result.unwrap(), "Hello world");
    }

    #[test]
    fn parse_envelope_with_extra_fields_returns_response_text() {
        let json = r#"{"success":true,"response":"hi","confidence":0.85,"tokens":12}"#;
        let result = parse_cactus_response(json);
        assert_eq!(result.unwrap(), "hi");
    }

    #[test]
    fn parse_failure_envelope_with_error_field() {
        let json = r#"{"success":false,"error":"out of memory"}"#;
        let err = parse_cactus_response(json).unwrap_err();
        assert!(err.to_string().contains("out of memory"), "got: {}", err);
    }

    #[test]
    fn parse_failure_envelope_without_error_field_uses_unknown() {
        let json = r#"{"success":false}"#;
        let err = parse_cactus_response(json).unwrap_err();
        assert!(err.to_string().contains("unknown error"), "got: {}", err);
    }

    #[test]
    fn parse_success_true_but_missing_response_field_is_error() {
        let json = r#"{"success":true}"#;
        let err = parse_cactus_response(json).unwrap_err();
        assert!(err.to_string().contains("missing 'response' field"), "got: {}", err);
    }

    #[test]
    fn parse_response_field_non_string_is_error() {
        let json = r#"{"success":true,"response":42}"#;
        let err = parse_cactus_response(json).unwrap_err();
        assert!(err.to_string().contains("missing 'response' field"), "got: {}", err);
    }

    #[test]
    fn parse_invalid_json_returns_parse_error() {
        let err = parse_cactus_response("not json at all").unwrap_err();
        assert!(
            err.to_string().contains("failed to parse cactus response JSON"),
            "got: {}", err
        );
    }

    #[test]
    fn parse_empty_string_returns_parse_error() {
        let err = parse_cactus_response("").unwrap_err();
        assert!(
            err.to_string().contains("failed to parse cactus response JSON"),
            "got: {}", err
        );
    }

    #[test]
    fn parse_json_null_is_treated_as_failure() {
        // null["success"].as_bool() is None, not Some(true) → failure path
        let err = parse_cactus_response("null").unwrap_err();
        assert!(err.to_string().contains("cactus returned failure"), "got: {}", err);
    }

    #[test]
    fn parse_response_empty_string_is_ok() {
        // empty string response is valid — caller decides if it's useful
        let json = r#"{"success":true,"response":""}"#;
        let result = parse_cactus_response(json);
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    #[ignore] // Run with: CACTUS_MODEL_PATH=... cargo test -- --ignored
    fn test_cactus_complete_live() {
        let model_path = std::env::var("CACTUS_MODEL_PATH")
            .expect("CACTUS_MODEL_PATH must be set to run live tests");
        let llm = CactusLlm::new(&model_path).expect("failed to init model");
        let result = llm.complete("You are helpful. Be brief.", "Say hello in one word.");
        assert!(result.is_ok(), "complete failed: {:?}", result);
        let text = result.unwrap();
        assert!(!text.is_empty(), "got empty response");
        println!("LLM response: {}", text);
    }
}
