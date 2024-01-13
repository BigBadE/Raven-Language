use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;

/// A top element in the program
pub trait TopElement {
    /// The finalized version of this trait
    type Finalized;
}

/// Holds the top elements and the wakers requiring those elements.
/// Wakers are used to allow tasks to wait for an element to be parsed and added
pub struct TopElementManager<T>
where
    T: TopElement,
{
    /// Types and their data, added immediately after parsing
    pub types: HashMap<String, Arc<T>>,
    /// A list of data sorted by the data's ID. Guaranteed to be in ID order.
    pub sorted: Vec<Arc<T>>,
    /// Data sorted by its finalized type, which contains the finalized code. Added after finalization.
    pub data: HashMap<Arc<T>, Arc<T::Finalized>>,
    /// Wakers waiting on a type to be added to the types hashmap, waked after the type is added to types
    pub wakers: HashMap<String, Vec<Waker>>,
}
