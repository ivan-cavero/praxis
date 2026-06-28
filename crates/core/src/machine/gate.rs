//! Quality gates — preconditions for phase transitions.

pub struct Gate {
    pub name: String,
}

impl Gate {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}