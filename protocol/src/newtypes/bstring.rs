use crate::{FromBytes, ToBytes};
use std::ops::Deref;

/// Length-**B**ound **String**
///
/// Immutable by design
#[derive(ToBytes, FromBytes, Debug, Clone, PartialEq)]
pub struct BString<const MAX: usize>(String);

impl<const MAX: usize> BString<MAX> {
    /// Fails if string too long
    pub fn new(s: String) -> Option<Self> {
        if s.len() > MAX {
            return None;
        }

        Some(Self(s))
    }
    pub fn to_inner(self) -> String {
        self.0
    }
    // panics if resulting string too long
    pub fn mutate<F: FnOnce(&mut String)>(&mut self, f: F) {
        f(&mut self.0);

        if self.0.len() > MAX {
            panic!("BString too long");
        }
    }
}

impl<const MAX: usize> Deref for BString<MAX> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
