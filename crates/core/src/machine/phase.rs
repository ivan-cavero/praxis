//! Phase definitions and state machine.

use project_x_shared::types::Phase;

pub struct StateMachine {
    current: Phase,
    history: Vec<Phase>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            current: Phase::Idle,
            history: Vec::new(),
        }
    }

    pub fn current(&self) -> &Phase {
        &self.current
    }

    pub fn history(&self) -> &[Phase] {
        &self.history
    }
}