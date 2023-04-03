use std::collections::HashMap;
use std::sync::Arc;
use syntax::async_util::NameResolver;

#[derive(Clone)]
pub struct ImportManager {
    pub imports: Arc<HashMap<String, String>>,
    pub current: String,
    pub code_block_id: u32
}

impl ImportManager {
    pub fn new(current: String) -> Self {
        return Self {
            imports: Arc::new(HashMap::new()),
            current,
            code_block_id: 0
        }
    }

    pub fn get_full(&self, input: String) -> String {
        if let Some(found) = self.imports.get(&input) {
            return found.clone();
        }
        return input;
    }
}

impl NameResolver for ImportManager {
    fn resolve(&self, name: &String) -> &String {
        if let Some(found) = self.imports.get(name) {
            return found;
        }
        return name;
    }
}