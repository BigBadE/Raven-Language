use json::JsonValue;
use crate::lsp::error::ResponseError;
use crate::lsp::Jsonable;
use crate::requests::UnknownRequest;

pub struct Body {
    pub id: String,
    pub request: Box<dyn Request>
}

impl Body {
    pub fn from_json(json: JsonValue) -> Self {
        return Body {
            id: json["id"].dump(),
            request: Box::new(UnknownRequest {})
        };
    }
}

impl Jsonable for Body {
    fn to_json(&self) -> JsonValue {
        let result = self.request.result();
        let mut json = JsonValue::new_object();
        json.insert("jsonrpc", "2.0").unwrap();
        match result {
            Ok(found) => json.insert("result", found).unwrap(),
            Err(error) => json.insert("error", error.to_json()).unwrap()
        }
        return json;
    }
}

pub trait Request {
    fn result(&self) -> Result<JsonValue, ResponseError>;
}