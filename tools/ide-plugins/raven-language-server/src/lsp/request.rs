use crate::lsp::error::ResponseError;
use crate::lsp::Jsonable;

pub struct Request {
    id: u32,
    method: String,
    params: Vec<Box<dyn Jsonable>>
}
pub struct Response {
    id: u32,
    result: Option<Box<dyn Jsonable>>,
    error: Option<ResponseError>
}

pub struct Notification {
    method: String,
    params: Vec<Box<dyn Jsonable>>
}