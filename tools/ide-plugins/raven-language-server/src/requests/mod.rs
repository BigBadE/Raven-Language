use json::JsonValue;
use crate::lsp::body::Request;
use crate::lsp::error::{METHOD_NOT_FOUND, ResponseError};

pub struct UnknownRequest {

}

impl Request for UnknownRequest {
    fn result(&self) -> Result<JsonValue, ResponseError> {
        return Err(ResponseError { code: METHOD_NOT_FOUND, message: "Unknown request".to_string(), data: None })
    }
}