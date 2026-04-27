#[cfg(senior_has_cactus)]
pub mod cactus_llm;
#[cfg(not(senior_has_cactus))]
pub mod cactus_llm_stub;
pub mod voice;
#[cfg(senior_has_cactus)]
pub use cactus_llm::CactusLlm;
#[cfg(not(senior_has_cactus))]
pub use cactus_llm_stub::CactusLlm;
