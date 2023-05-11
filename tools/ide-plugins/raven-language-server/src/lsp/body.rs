use json::JsonValue;
use crate::lsp::Jsonable;

pub struct Body {
    pub request: Box<dyn Request>
}

impl Body {
    pub fn from_json(json: JsonValue) -> Self {
        todo!()
    }
}
impl Jsonable for Body {
    fn to_json(&self) -> JsonValue {
        let mut json = self.request.to_json();
        json.insert("jsonrpc", "2.0").unwrap();
        return json;
    }
}

pub trait Request: Jsonable {

}