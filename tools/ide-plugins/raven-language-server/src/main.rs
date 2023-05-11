use std::io;
use std::io::{BufRead, Stdin};
use crate::lsp::Packet;

pub mod lsp;

pub fn main() {
    let mut input = io::stdin().lock();
    let mut line = String::new();
    loop {
        input.read_line(&mut line).unwrap();
        let input = match Packet::parse(&line) {
            Ok(output) => output,
            Err(error) => panic!("Malformed JSON!")
        };
    }
}

fn run_server() {

}