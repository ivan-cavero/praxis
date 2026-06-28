//! Phase transitions and conditions.

pub struct Transition {
    pub from: String,
    pub to: String,
}

impl Transition {
    pub fn new(from: &str, to: &str) -> Self {
        Self { from: from.to_string(), to: to.to_string() }
    }
}