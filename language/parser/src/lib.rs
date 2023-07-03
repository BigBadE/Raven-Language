#![feature(try_trait_v2, get_mut_unchecked)]
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use syntax::async_util::{NameResolver, UnparsedType};
use syntax::syntax::Syntax;
use crate::parser::top_parser::parse_top;
use crate::parser::util::ParserUtils;
use crate::tokens::tokenizer::Tokenizer;
use crate::tokens::tokens::TokenTypes;

pub mod parser;
pub mod tokens;

pub async fn parse(syntax: Arc<Mutex<Syntax>>, handle: Handle, name: String, file: String) {
    let mut tokenizer = Tokenizer::new(file.as_bytes());
    let mut tokens = Vec::new();
    loop {
        tokens.push(tokenizer.next());
        if tokens.last().unwrap().token_type == TokenTypes::EOF {
            break
        }
    }
    let mut parser_utils = ParserUtils {
        buffer: file.as_bytes(),
        index: 0,
        tokens,
        syntax,
        file: name.clone(),
        imports: ImportNameResolver::new(name),
        handle,
    };
    parse_top(&mut parser_utils);
}

#[derive(Clone)]
pub struct ImportNameResolver {
    pub imports: Vec<String>,
    pub generics: HashMap<String, Vec<UnparsedType>>,
    pub parent: Option<String>,
    pub last_id: u32
}

impl ImportNameResolver {
    pub fn new(base: String) -> Self {
        return Self {
            imports: vec!(base),
            generics: HashMap::new(),
            parent: None,
            last_id: 0
        }
    }
}

impl NameResolver for ImportNameResolver {
    fn imports(&self) -> &Vec<String> {
        return &self.imports;
    }

    fn generic(&self, name: &String) -> Option<Vec<UnparsedType>> {
        return self.generics.get(name).map(|types| types.clone());
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(self.clone());
    }
}