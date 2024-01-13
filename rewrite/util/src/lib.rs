use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem;

use indexmap::map::Values;
use indexmap::IndexMap;

pub mod source;
pub mod top_elements;

#[derive(Default)]
pub struct GenericMap {
    map: HashMap<TypeId, Box<dyn Any>>,
}

#[derive(Default)]
pub struct IndexedGenericMap {
    map: IndexMap<TypeId, Box<dyn Any>>,
}

impl GenericMap {
    pub fn get_type<T: 'static>(&self) -> &T {
        // SAFETY
        // The type id is associated with the Any value, so the type is known to be correct
        return unsafe { mem::transmute::<&Box<dyn Any>, &Box<T>>(&self.map[&TypeId::of::<T>()]) };
    }

    pub fn initialize<T: 'static>(&mut self, value: T) {
        if self.map.contains_key(&TypeId::of::<T>()) {
            panic!("Tried to add duplicate to generic map!")
        }
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }
}

impl IndexedGenericMap {
    pub fn get_type<T: 'static>(&self) -> &T {
        // SAFETY
        // The type id is associated with the Any value, so the type is known to be correct
        return unsafe { mem::transmute::<&Box<dyn Any>, &Box<T>>(&self.map[&TypeId::of::<T>()]) };
    }

    pub fn get_type_mut<T: 'static>(&mut self) -> &mut T {
        // SAFETY
        // The type id is associated with the Any value, so the type is known to be correct
        return unsafe { mem::transmute::<&mut Box<dyn Any>, &mut Box<T>>(self.map.get_mut(&TypeId::of::<T>()).unwrap()) };
    }

    pub fn initialize<T: 'static>(&mut self, value: T) {
        if self.map.contains_key(&TypeId::of::<T>()) {
            panic!("Tried to add duplicate to generic map!")
        }
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn iter(&self) -> Values<'_, TypeId, Box<dyn Any>> {
        return self.map.values();
    }
}

#[cfg(test)]
mod test {
    use std::any::Any;
    use std::mem;

    use crate::{GenericMap, IndexedGenericMap};

    #[test]
    fn test_generic_map() {
        let mut testing = GenericMap::default();
        let vec = vec![1, 2, 3];
        testing.initialize(vec.clone());
        assert_eq!(testing.get_type::<Vec<u64>>(), &vec)
    }

    #[test]
    fn test_index_map() {
        let mut testing = IndexedGenericMap::default();
        testing.initialize(1u64);
        testing.initialize("test");
        testing.initialize(vec![12]);
        let mut index = 0;
        for value in testing.iter() {
            unsafe {
                if index == 0 {
                    assert_eq!(**mem::transmute::<&Box<dyn Any>, &Box<u64>>(value), 1);
                    index += 1;
                } else if index == 1 {
                    assert_eq!(**mem::transmute::<&Box<dyn Any>, &Box<&str>>(value), "test");
                } else {
                    assert_eq!(**mem::transmute::<&Box<dyn Any>, &Box<&str>>(value), "test");
                }
            }
        }
    }
}
