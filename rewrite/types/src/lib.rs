pub trait Type {}

pub trait ComparableType: Type {
    fn compare(&self, other: &dyn ComparableType) -> bool;
}

pub struct RawType {
    raw: String,
}

impl RawType {
    pub fn new(raw: String) -> Self {
        return RawType { raw };
    }
}

impl Type for RawType {}
