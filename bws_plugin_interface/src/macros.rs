#[macro_export]
macro_rules! error {
    ($vtable:path, $($arg:tt)+) => {
        ($vtable.log)(
            $crate::safe_types::SStr::from(::std::module_path!()),
            $crate::vtable::LogLevel::Error,
            $crate::safe_types::SStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! warn {
    ($vtable:path, $($arg:tt)+) => {
        ($vtable.log)(
            $crate::safe_types::SStr::from(::std::module_path!()),
            $crate::vtable::LogLevel::Warn,
            $crate::safe_types::SStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! info {
    ($vtable:path, $($arg:tt)+) => {
        ($vtable.log)(
            $crate::safe_types::SStr::from(::std::module_path!()),
            $crate::vtable::LogLevel::Info,
            $crate::safe_types::SStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! debug {
    ($vtable:path, $($arg:tt)+) => {
        ($vtable.log)(
            $crate::safe_types::SStr::from(::std::module_path!()),
            $crate::vtable::LogLevel::Debug,
            $crate::safe_types::SStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! trace {
    ($vtable:path, $($arg:tt)+) => {
        ($vtable.log)(
            $crate::safe_types::SStr::from(::std::module_path!()),
            $crate::vtable::LogLevel::Trace,
            $crate::safe_types::SStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}
