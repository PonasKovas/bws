/// iIncremented on each incompatible ABI change
pub const ABI: u32 = 0;

mod maybe_panicked;
mod mut_slice;
mod option;
mod result;
mod slice;
mod str;
mod string;
mod tuples;
mod unit;
mod vec;

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
