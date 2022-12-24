pub mod mut_slice;
pub mod slice;
pub mod str;
pub mod string;
pub mod tuples;
pub mod vec;

pub use crate::safe_types::str::SStr;
pub use mut_slice::SMutSlice;
pub use slice::SSlice;
pub use string::SString;
pub use tuples::STuple2;
pub use vec::SVec;
