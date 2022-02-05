//! Just a convenience trait for performing a linear search
//! on [(K, V)] slices

use std::borrow::Borrow;

pub trait LinearSearch<'a> {
    type Key;
    type Value;

    /// panics if key not found
    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Value
    where
        Self::Key: Borrow<Q>;

    /// panics if key not found
    fn search_by_val<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Key
    where
        Self::Value: Borrow<Q>;
}

impl<'a, K, T> LinearSearch<'a> for [(K, T)] {
    type Key = K;
    type Value = T;

    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Value
    where
        K: Borrow<Q>,
    {
        &self.iter().find(|e| e.0.borrow() == key).unwrap().1
    }

    fn search_by_val<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Key
    where
        T: Borrow<Q>,
    {
        &self.iter().find(|e| e.1.borrow() == key).unwrap().0
    }
}
