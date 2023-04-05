#![feature(try_trait_v2)]

use std::sync::{Arc, Mutex};
use anyhow::Error;
use syntax::syntax::Syntax;

pub mod tokens;

pub async fn parse(syntax: Arc<Mutex<Syntax>>, name: String, file: String) -> Result<(), Error> {
    todo!()
}