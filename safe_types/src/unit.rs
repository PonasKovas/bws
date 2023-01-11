use std::fmt::Debug;

/// `#[repr(C)]` version of `()`
#[repr(C)]
#[derive(Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct SUnit {
    _private: [usize; 0],
}

impl SUnit {
    pub const fn new() -> Self {
        Self { _private: [] }
    }
}

impl Debug for SUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "()")
    }
}

impl Default for SUnit {
    fn default() -> Self {
        Self::new()
    }
}

impl From<()> for SUnit {
    fn from(_: ()) -> Self {
        Self::new()
    }
}

impl From<SUnit> for () {
    fn from(_: SUnit) -> Self {
        ()
    }
}
