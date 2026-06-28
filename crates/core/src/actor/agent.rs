//! Agent trait — implemented by all specialized agents.

pub trait Agent: Send + Sync {
    fn role(&self) -> &str;
    fn model(&self) -> &str;
    fn asi_score(&self) -> f32;
    fn context_pressure(&self) -> f32;
}