mod slice;
mod string;
mod tuples;
mod vec;

pub use slice::BwsSlice;
pub use string::{BwsStr, BwsString};
pub use tuples::{Tuple2, Tuple3};
pub use vec::BwsVec;

pub type RegisterPlugin =
    unsafe extern "C" fn(BwsStr, BwsStr, BwsSlice) -> Tuple2<RegisterCallback, RegisterSubPlugin>;
pub type RegisterCallback = unsafe extern "C" fn(BwsStr, *const ());
pub type RegisterSubPlugin = unsafe extern "C" fn(BwsStr) -> RegisterSubPluginCallback;
pub type RegisterSubPluginCallback = unsafe extern "C" fn(BwsStr, *const ());
