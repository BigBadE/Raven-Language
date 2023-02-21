use std::fmt::{Display, Formatter};

pub struct Ident {
    pub value: String
}

impl Ident {
    pub fn new(value: String) -> Self {
        return Self {
            value
        }
    }
}

impl Display for Ident {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.value);
    }
}