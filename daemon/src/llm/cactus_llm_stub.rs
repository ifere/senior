use anyhow::{anyhow, Result};

pub struct CactusLlm;

impl CactusLlm {
    pub fn new(_model_path: &str) -> Result<Self> {
        Err(anyhow!(
            "Cactus support is not compiled in. Set CACTUS_LIB_DIR at build time to enable local LLM inference."
        ))
    }

    pub fn complete(&self, _system_prompt: &str, _user_message: &str) -> Result<String> {
        Err(anyhow!(
            "Cactus support is not compiled in. Rebuild with CACTUS_LIB_DIR set to use LLM completion."
        ))
    }

    pub fn complete_with_tools(
        &self,
        _user_message: &str,
        _tools_json: &str,
    ) -> Result<Vec<serde_json::Value>> {
        Err(anyhow!(
            "Cactus support is not compiled in. Rebuild with CACTUS_LIB_DIR set to use tool calling."
        ))
    }
}
