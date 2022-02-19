use abi_stable::std_types::RStr;

#[repr(C)]
pub struct VTable {
    pub log: fn(target: RStr, level: LogLevel, message: RStr),
}

#[repr(C)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
