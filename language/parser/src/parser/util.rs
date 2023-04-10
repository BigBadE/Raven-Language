use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use syntax::async_util::StructureGetter;
use syntax::function::Function;
use syntax::ParsingError;
use syntax::r#struct::Struct;
use syntax::syntax::Syntax;
use crate::ImportNameResolver;
use crate::tokens::tokens::Token;

pub struct ParserUtils<'a> {
    pub buffer: &'a [u8],
    pub tokens: Vec<Token>,
    pub syntax: Arc<Mutex<Syntax>>,
    pub file: String,
    pub imports: ImportNameResolver,
    pub handle: Handle
}

impl<'a> ParserUtils<'a> {
    pub fn get_struct(&self, token: Token, name: String) -> StructureGetter {
        return StructureGetter::new(self.syntax.clone(),
                                    token.make_error(format!("Failed to find type named {}", &name)),
                                    name, Box::new(self.imports.clone()));
    }

    pub async fn add_struct(&self, token: &Token, structure: impl Future<Output=Result<Struct, ParsingError>>) {
        let mut locked = self.syntax.lock().unwrap();
        let structure = match structure.await {
            Ok(structure) => structure,
            Err(error) => {
                locked.add_struct(None, Arc::new(Struct::new_poisoned(format!("${}", self.file), error)));
                return;
            }
        };

        for function in &structure.functions {
            locked.add_function(token.make_error(format!("Duplicate function {}", function.name)), function.clone());
        }
        locked.add_struct(Some(token.make_error(format!("Duplicate structure {}", structure.name))),
                                          Arc::new(structure));
    }

    pub async fn add_function(&self, token: &Token, function: impl Future<Output=Result<Function, ParsingError>>) {
        let mut locked = self.syntax.lock().unwrap();
        let function = match function.await {
            Ok(function) => function,
            Err(error) => {
                locked.add_struct(None, Arc::new(Struct::new_poisoned(format!("${}", self.file), error)));
                return;
            }
        };

        let function = Arc::new(function);
        locked.add_function(token.make_error(format!("Duplicate structure {}", function.name)),
                                               function);
    }
}