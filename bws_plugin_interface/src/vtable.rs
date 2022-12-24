use crate::safe_types::*;

#[repr(C)]
pub struct VTable {
    pub log: extern "C" fn(target: SStr, level: LogLevel, message: SStr),
}

#[repr(C)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
