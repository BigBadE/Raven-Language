use json::{Error, JsonValue};
use crate::lsp::body::Body;

pub mod body;
pub mod request;
pub mod error;

pub struct Packet {
    pub body: Body
}

impl Packet {
    pub fn new(body: Body) -> Self {
        return Self {
            body
        }
    }

    pub fn serialize(&self) -> String {
        let body = self.body.to_json().dump();
        return format!("Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n{}", body.len(), body);
    }
    
    pub fn parse(input: &String) -> Result<Self, Error> {
        return Ok(Packet {
            body: Body::from_json(json::parse(input)?)
        });
    }
}

pub trait Jsonable {
    fn to_json(&self) -> JsonValue;
}