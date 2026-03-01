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
            "max_tokens": 256,
            "temperature": 0.1
        });
        let options_c = CString::new(options.to_string())?;

        // 8KB response buffer
        let mut response_buf: Vec<c_char> = vec![0; 8192];

        let model = self.model.lock().unwrap();
        let ret = unsafe {
            cactus_complete(
                *model,
                messages_c.as_ptr(),
                response_buf.as_mut_ptr(),
                response_buf.len(),
                options_c.as_ptr(),
                std::ptr::null(),
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

        // Parse the cactus response envelope: {"success":true,"response":"..."}
        let parsed: serde_json::Value = serde_json::from_str(&raw_json)
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
    #[ignore] // Run with: CACTUS_MODEL_PATH=... cargo test -- --ignored
    fn test_cactus_complete_live() {
        let model_path = std::env::var("CACTUS_MODEL_PATH")
            .unwrap_or_else(|_| "/Users/chilly/dev/cactus/weights/functiongemma-270m-it".to_string());
        let llm = CactusLlm::new(&model_path).expect("failed to init model");
        let result = llm.complete("You are helpful. Be brief.", "Say hello in one word.");
        assert!(result.is_ok(), "complete failed: {:?}", result);
        let text = result.unwrap();
        assert!(!text.is_empty(), "got empty response");
        println!("LLM response: {}", text);
    }
}
