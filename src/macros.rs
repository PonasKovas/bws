#[macro_export]
macro_rules! error {
    (@$gstate:ident, $($arg:tt)+) => {
        ($gstate.vtable.log)(
            ::abi_stable::std_types::RStr::from(::std::module_path!()),
            $crate::global_state::vtable::LogLevel::Error,
            ::abi_stable::std_types::RStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! warn {
    (@$gstate:ident, $($arg:tt)+) => {
        ($gstate.vtable.log)(
            ::abi_stable::std_types::RStr::from(::std::module_path!()),
            $crate::global_state::vtable::LogLevel::Warn,
            ::abi_stable::std_types::RStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! info {
    (@$gstate:ident, $($arg:tt)+) => {
        ($gstate.vtable.log)(
            ::abi_stable::std_types::RStr::from(::std::module_path!()),
            $crate::global_state::vtable::LogLevel::Info,
            ::abi_stable::std_types::RStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! debug {
    (@$gstate:ident, $($arg:tt)+) => {
        ($gstate.vtable.log)(
            ::abi_stable::std_types::RStr::from(::std::module_path!()),
            $crate::global_state::vtable::LogLevel::Debug,
            ::abi_stable::std_types::RStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}

#[macro_export]
macro_rules! trace {
    (@$gstate:ident, $($arg:tt)+) => {
        ($gstate.vtable.log)(
            ::abi_stable::std_types::RStr::from(::std::module_path!()),
            $crate::global_state::vtable::LogLevel::Trace,
            ::abi_stable::std_types::RStr::from(::std::format!($($arg)+).as_str()),
        );
    };
}
