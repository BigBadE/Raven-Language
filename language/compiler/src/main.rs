use std::{env, fs};
use crate::compiler::Compiler;

pub mod compiler;
pub mod types;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    fs::write(args.get(2).unwrap(), Compiler::new().compile(
        fs::read_to_string(args.get(1).unwrap()).unwrap())).unwrap();
}