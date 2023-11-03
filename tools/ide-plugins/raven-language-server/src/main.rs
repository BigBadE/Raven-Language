use std::io;
use std::io::BufRead;
use crate::lsp::Packet;

pub mod lsp;
pub mod requests;

pub fn main() {
    let mut input = io::stdin().lock();
    let mut line = String::new();
    loop {
        input.read_line(&mut line).unwrap();
        let content_length = line.split(": ").last().unwrap().parse::<u64>().unwrap();
        line.clear();
        input.read_line(&mut line).unwrap();
        let _content_type = line.split(": ").last().unwrap();

        line.clear();
        assert!(content_length == input.read_line(&mut line).unwrap());
        let input = match Packet::parse(&line) {
            Ok(output) => output,
            Err(_) => panic!("Malformed JSON!")
        };
        println!("{}", input.serialize());
    }
}

fn run_server() {

}