use crate::safe_types::*;

#[repr(C)]
pub struct VTable {
    pub log: extern "C" fn(target: SStr, level: LogLevel, message: SStr),
    pub cmd_arg: extern "C" fn(SStr, u32, SStr, SStr, SStr, bool),
    pub cmd_flag: extern "C" fn(SStr, u32, SStr, SStr),
}

#[repr(C)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
