use std::collections::HashMap;
use crate::TopElement;

pub struct Program {
    pub classes: HashMap<String, Vec<TopElement>>
}

impl Program {
    pub fn new() -> Self {
        return Self {
            classes: HashMap::new()
        }
    }
}