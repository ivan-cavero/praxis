//! OpenAI-compatible provider (Groq, Together, etc).

pub struct OpenAICompatibleProvider;

impl OpenAICompatibleProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAICompatibleProvider {
    fn default() -> Self {
        Self::new()
    }
}
