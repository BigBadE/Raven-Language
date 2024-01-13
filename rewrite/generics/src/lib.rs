use types::{ComparableType, Type};

pub struct GenericType {}

impl Type for GenericType {}

impl ComparableType for GenericType {
    fn comparable(other: &Self) {
        todo!()
    }
}
