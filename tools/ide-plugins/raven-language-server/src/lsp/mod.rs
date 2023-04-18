use json::JsonValue;
use crate::lsp::body::Body;

pub mod body;
pub mod request;

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
        return format!("Content-Length:{}\r\n\r\n{}", body.len(), body);
    }
}

pub trait Jsonable {
    fn to_json(&self) -> JsonValue;
}