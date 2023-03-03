use std::collections::HashMap;
use crate::function::Function;

pub struct Program<'a> {
    pub static_functions: HashMap<String, Function<'a>>,
    pub package_name: Option<String>,
    pub operations: HashMap<String, String>,
}

impl<'a> Program<'a> {
    pub fn new() -> Self {
        return Self {
            static_functions: HashMap::new(),
            package_name: None,
            operations: HashMap::new()
        }
    }
}