use crate::lsp::Jsonable;

pub struct Request {
    id: u32,
    method: String,
    params: Vec<Box<dyn Jsonable>>
}