/// A macro that helps with plugin definition
///
/// # Usage
///
/// ```ignore
/// bws_plugin_interface::plugin! {
///     depend "other_plugin_dependency" = "0.1",
///     depend "you_can_have_as_many_dependencies_as_you_want" = "<2.0",
///     ...
///     depend "or_you_can_have_none" = "=1.0.0",
///     
///     api MY_API, // optional, can be ommited if your plugin doesn't expose an API
///
///     init_fn init,
///     start_fn start,
/// }
///
/// // Again, optional. Only if you need to expose an API for other plugins
/// // If you do use this, make sure MyApi is #[repr(C)]
/// static MY_API: MyApi = MyApi { ... };
///
/// // This function will be called once to let the plugin initialize
/// // You can add command line flags, arguments here and register event callbacks
/// // For everything else you should use the "start" event (register a callback for it in this function)
/// fn init(vtable: &'static InitVTable) {
///     ...
/// }
/// ```
#[macro_export]
macro_rules! plugin {
    ($(depend $depname:literal = $depversion:literal ,)* $(api $api:path ,)? init_fn $init_fn:path , start_fn $start_fn:path $(,)? ) => {
        #[no_mangle]
        static BWS_ABI: u64 = $crate::ABI;

        #[no_mangle]
        static BWS_PLUGIN_ROOT: $crate::BwsPlugin = $crate::BwsPlugin {
            name: $crate::safe_types::SStr::from_str(env!("CARGO_PKG_NAME")),
            version: $crate::safe_types::SStr::from_str(env!("CARGO_PKG_VERSION")),
            dependencies: $crate::safe_types::SSlice::from_slice(&[
                $( $crate::safe_types::STuple2( $crate::safe_types::SStr::from_str( $depname ), $crate::safe_types::SStr::from_str( $depversion ) ), )*
            ]),
            init_fn: {
                extern "C" fn ___init(plugin_id: usize, vtable: &'static $crate::vtable::InitVTable) -> $crate::safe_types::MaybePanicked<$crate::safe_types::SResult> {

                    $crate::safe_types::MaybePanicked::new(move || {
                        $crate::global::set_plugin_id(plugin_id);

                        let r: ::std::result::Result<(), ()> = $init_fn (vtable);

                        if r.is_ok() {
                            $crate::safe_types::SResult::Ok($crate::safe_types::SUnit::new())
                        } else {
                            $crate::safe_types::SResult::Err($crate::safe_types::SUnit::new())
                        }

                    })
                }
                ___init
            },
            vtable_fn: {
                extern "C" fn ___vtable(vtable: &'static $crate::vtable::VTable) -> $crate::safe_types::MaybePanicked {
                    $crate::safe_types::MaybePanicked::new(move || {
                        $crate::global::set_vtable(vtable);
                    })
                }
                ___vtable
            },
            start_fn: {
                extern "C" fn ___start() -> $crate::safe_types::MaybePanicked<$crate::safe_types::SResult> {
                    $crate::safe_types::MaybePanicked::new(move || {
                        let r: ::std::result::Result<(), ()> = $start_fn ();

                        if r.is_ok() {
                            $crate::safe_types::SResult::Ok($crate::safe_types::SUnit::new())
                        } else {
                            $crate::safe_types::SResult::Err($crate::safe_types::SUnit::new())
                        }
                    })
                }
                ___start
            },
            api: {
                // Default, if API not given
                let api: $crate::safe_types::SOption<$crate::plugin_api::PluginApiPtr> = $crate::safe_types::SOption::None;

                $(
                    let api = $crate::safe_types::SOption::Some(
                        $crate::plugin_api::PluginApiPtr::new(& $api)
                    );
                )?

                api
            },
        };
    };
}

//////////////////////
// Log macros below //
//////////////////////

#[doc(hidden)]
#[macro_export]
macro_rules! __bws_log {
    ($vtable:expr, $level:ident, $($arg:tt)+) => {
        ($vtable.log)(
            $crate::global::get_plugin_id(),
            $crate::safe_types::SStr::from(::std::module_path!()),
            $crate::vtable::LogLevel::$level,
            $crate::safe_types::SStr::from(::std::format!($($arg)+).as_str()),
        ).unwrap();
    };
}

/// Logs an error
///
/// # Usage
///
/// ```ignore
/// error!(vtable, "error: {:?}", e);
/// ```
#[macro_export]
macro_rules! error {
    ($vtable:path, $message:literal $($arg:tt)*) => {
        $crate::__bws_log!($vtable, Error, $message $($arg)*);
    };
    ($message:literal $($arg:tt)*) => {
        $crate::__bws_log!($crate::global::get_vtable(), Error, $message $($arg)*);
    };
}

/// Logs a warning
///
/// # Usage
///
/// ```ignore
/// warn!(vtable, "caution: {:?}", e);
/// ```
#[macro_export]
macro_rules! warn {
    ($vtable:path, $message:literal $($arg:tt)*) => {
        $crate::__bws_log!($vtable, Warn, $message $($arg)*);
    };
    ($message:literal $($arg:tt)*) => {
        $crate::__bws_log!($crate::global::get_vtable(), Warn, $message $($arg)*);
    };
}

/// Logs a message
///
/// # Usage
///
/// ```ignore
/// info!(vtable, "info: {:?}", e);
/// ```
#[macro_export]
macro_rules! info {
    ($vtable:path, $message:literal $($arg:tt)*) => {
        $crate::__bws_log!($vtable, Info, $message $($arg)*);
    };
    ($message:literal $($arg:tt)*) => {
        $crate::__bws_log!($crate::global::get_vtable(), Info, $message $($arg)*);
    };
}

/// Logs a debug message
///
/// # Usage
///
/// ```ignore
/// debug!(vtable, "debug info: {:?}", e);
/// ```
#[macro_export]
macro_rules! debug {
    ($vtable:path, $message:literal $($arg:tt)*) => {
        $crate::__bws_log!($vtable, Debug, $message $($arg)*);
    };
    ($message:literal $($arg:tt)*) => {
        $crate::__bws_log!($crate::global::get_vtable(), Debug, $message $($arg)*);
    };
}

/// Logs a message on the trace level
///
/// # Usage
///
/// ```ignore
/// trace!(vtable, "trace: {:?}", e);
/// ```
#[macro_export]
macro_rules! trace {
    ($vtable:path, $message:literal $($arg:tt)*) => {
        $crate::__bws_log!($vtable, Trace, $message $($arg)*);
    };
    ($message:literal $($arg:tt)*) => {
        $crate::__bws_log!($crate::global::get_vtable(), Trace, $message $($arg)*);
    };
}
