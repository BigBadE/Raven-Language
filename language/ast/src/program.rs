use std::collections::HashMap;
use crate::function::Function;
use crate::r#struct::Struct;

pub struct Program {
    pub elem_types: HashMap<String, Struct>,
    pub static_functions: HashMap<String, Function>,
    pub package_name: Option<String>,
    pub main: Option<String>
}

impl Program {
    pub fn new() -> Self {
        return Self {
            elem_types: HashMap::new(),
            static_functions: HashMap::new(),
            package_name: None,
            main: None
        }
    }

    pub fn set_main(&mut self, main: String) {
        if !self.main.is_some() {
            self.main = Some(main);
        } else {
            panic!("Tried to set already-set main!");
        }
    }
}