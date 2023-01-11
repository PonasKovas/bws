// incremented on each incompatible ABI change
pub const ABI: u32 = 0;

pub mod maybe_panicked;
pub mod mut_slice;
pub mod option;
pub mod result;
pub mod slice;
pub mod str;
pub mod string;
pub mod tuples;
pub mod unit;
pub mod vec;

pub use crate::str::SStr;
pub use maybe_panicked::MaybePanicked;
pub use mut_slice::SMutSlice;
pub use option::SOption;
pub use result::SResult;
pub use slice::SSlice;
pub use string::SString;
pub use tuples::STuple2;
pub use unit::SUnit;
pub use vec::SVec;
