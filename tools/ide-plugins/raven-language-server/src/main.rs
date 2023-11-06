use std::io;
use std::io::{BufRead, Read};
use crate::lsp::Packet;

pub mod lsp;
pub mod requests;
pub mod stdio;

pub fn main() {
    let mut input = io::stdin().lock();
    let mut line = String::new();
    loop {
        stdio::stdio_transport();
        input.read_line(&mut line).unwrap();
        panic!("{}", line);
        let content_length = line.split(": ").last().unwrap().trim().parse::<u64>().unwrap();
        line.clear();
        input.read_line(&mut line).unwrap();
        println!("Content-Length: 59\r\n\r\n{{\"jsonrpc\": \"2.0\", \"method\": \"initialized\", \"params\": {{}}");
        line.clear();
        input.read_line(&mut line).unwrap();
        println!("Got it!");
        input.read_line(&mut line).unwrap();
        panic!("Line: {}", line);
        let _content_type = line.split(": ").last().unwrap();

        line.clear();
        assert_eq!(content_length as usize, input.read_line(&mut line).unwrap());
        let input = match Packet::parse(&line) {
            Ok(output) => output,
            Err(_) => panic!("Malformed JSON!")
        };
        println!("{}", input.serialize());
    }
}

fn run_server() {

}