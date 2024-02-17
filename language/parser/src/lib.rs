#![feature(get_mut_unchecked)]
#![feature(let_chains)]
extern crate core;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::{fs, path};

use anyhow::Error;

use data::tokens::{Token, TokenTypes};
use data::{Readable, SourceSet};
use syntax::async_util::{HandleWrapper, NameResolver, UnparsedType};
use syntax::program::syntax::Syntax;

use crate::parser::top_parser::parse_top;
use crate::parser::util::ParserUtils;
use crate::tokens::tokenizer::Tokenizer;

/// The Raven parser
pub mod parser;
/// The Raven tokenizer
pub mod tokens;

/// Parses a file into the syntax
pub async fn parse(syntax: Arc<Mutex<Syntax>>, handle: Arc<Mutex<HandleWrapper>>, name: String, file: Box<dyn Readable>) {
    let buffer = file.contents();
    let mut parser_utils = ParserUtils {
        buffer: buffer.as_bytes(),
        index: 0,
        tokens: file.read(),
        syntax,
        file: file.hash(),
        file_name: name.clone(),
        imports: ImportNameResolver::new(name.clone()),
        handle,
    };

    parse_top(&mut parser_utils);
}

/// Basic name resolver implementation
#[derive(Clone)]
pub struct ImportNameResolver {
    /// The current file imports
    pub imports: Vec<String>,
    /// The current generics
    pub generics: HashMap<String, Vec<UnparsedType>>,
    /// The parent type
    pub parent: Option<UnparsedType>,
    /// Last ID used on a code block label
    pub last_id: u32,
}

impl ImportNameResolver {
    /// Creates a new name resolver
    pub fn new(base: String) -> Self {
        return Self { imports: vec![base], generics: HashMap::default(), parent: None, last_id: 0 };
    }
}

impl NameResolver for ImportNameResolver {
    fn imports(&self) -> &Vec<String> {
        return &self.imports;
    }

    fn generic(&self, name: &String) -> Option<Vec<UnparsedType>> {
        return self.generics.get(name).cloned();
    }

    fn generics(&self) -> &HashMap<String, Vec<UnparsedType>> {
        return &self.generics;
    }

    fn boxed_clone(&self) -> Box<dyn NameResolver> {
        return Box::new(self.clone());
    }
}

/// A simple source set of a single file/folder
#[derive(Clone, Debug)]
pub struct FileSourceSet {
    /// The path of the file/folder
    pub root: PathBuf,
}

/// A wrapper around the PathBuf type, used for implementing traits on it
pub struct FilePath {
    /// The path to the file
    pub path: PathBuf,
}

impl Hash for FilePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl Readable for FilePath {
    fn read(&self) -> Vec<Token> {
        let binding = self.contents();
        let mut tokenizer = Tokenizer::new(binding.as_bytes());
        let mut tokens = Vec::default();
        loop {
            tokens.push(tokenizer.next());
            if tokens.last().unwrap().token_type == TokenTypes::EOF {
                break;
            }
        }

        return tokens;
    }

    fn contents(&self) -> String {
        fs::read_to_string(&self.path.clone())
            .unwrap_or_else(|_| panic!("Failed to read source file: {}", self.path.to_str().unwrap()))
    }

    fn path(&self) -> String {
        return self.path.to_str().unwrap().to_string();
    }

    fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::default();
        Hash::hash(&self, &mut hasher);
        return hasher.finish();
    }
}

impl SourceSet for FileSourceSet {
    fn get_files(&self) -> Vec<Box<dyn Readable>> {
        let mut output = Vec::default();
        read_recursive(self.root.clone(), &mut output)
            .unwrap_or_else(|_| panic!("Failed to read source files! Make sure {:?} exists", self.root));
        return output;
    }

    fn relative(&self, other: &dyn Readable) -> String {
        let name =
            other.path().replace(self.root.to_str().unwrap(), "").replace(path::MAIN_SEPARATOR, "::").replace('/', "::");
        if name.len() == 0 {
            let path = other.path();
            let name: &str = path.split(path::MAIN_SEPARATOR).last().unwrap();
            return name[0..name.len() - 3].to_string();
        }
        return name.as_str()[2..name.len() - 3].to_string();
    }

    fn cloned(&self) -> Box<dyn SourceSet> {
        return Box::new(self.clone());
    }
}

/// Recursively reads a folder/file into the list of files
fn read_recursive(base: PathBuf, output: &mut Vec<Box<dyn Readable>>) -> Result<(), Error> {
    if fs::metadata(&base)?.file_type().is_dir() {
        for file in fs::read_dir(&base)? {
            let file = file?;
            read_recursive(file.path(), output)?;
        }
    } else {
        output.push(Box::new(FilePath { path: base }));
    }
    return Ok(());
}
