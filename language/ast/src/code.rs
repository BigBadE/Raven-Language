use std::fmt::{Display, Formatter};

pub struct Expression {
    effect: Box<dyn Effect>
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{};", self.effect);
    }
}

pub trait Effect: Display {

}