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
///     on_load init
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
    ($(depend $depname:literal = $depversion:literal ,)* $(api $api:path ,)? on_load $on_load:path) => {
        #[no_mangle]
        static BWS_ABI: u64 = $crate::ABI;

        #[no_mangle]
        static BWS_PLUGIN_ROOT: $crate::BwsPlugin = $crate::BwsPlugin {
            name: $crate::safe_types::SStr::from_str(env!("CARGO_PKG_NAME")),
            version: $crate::safe_types::SStr::from_str(env!("CARGO_PKG_VERSION")),
            dependencies: $crate::safe_types::SSlice::from_slice(&[
                $( $crate::safe_types::STuple2( $crate::safe_types::SStr::from_str( $depname ), $crate::safe_types::SStr::from_str( $depversion ) ), )*
            ]),
            on_load: {
                extern "C" fn __bws_plugin_on_load(vtable: &'static $crate::vtable::InitVTable) {
                    // make sure on_load isnt executed more than once
                    static ONCE: ::std::sync::Once = ::std::sync::Once::new();
                    ONCE.call_once(move || {
                        ( $on_load )(vtable);
                    });
                }
                __bws_plugin_on_load
            },
            api: {
                // Default, if api not given
                let api = $crate::PluginApi::new();
                $( let api = $crate::PluginApi::from(& $api); )?
                api
            },
        };
    };
}

/// Registers a callback for an event
///
/// # Usage
///
/// ```ignore
/// add_event_callback!(<vtable>, <event_name>, <callback>, [priority]);
/// ```
///
/// - `vtable` is either [`&VTable`](crate::vtable::VTable) or [`&InitVTable`](crate::vtable::InitVTable)
/// - `event_name` is the name of the event (string)
/// - `callback` is the callback function pointer of type [`EventFn`](crate::vtable::EventFn)
/// - `priority` is an optional [`f64`] meaning the priority of the callback (less means it will be executed before others) (Default: `0.0`)
#[macro_export]
macro_rules! add_event_callback {
    ($vtable:path, $event_name:expr, $callback:path $(, $priority:expr )? $(,)? ) => {
        ($vtable.add_event_callback)(
            ($vtable.get_event_id)($event_name.into()),
            env!("CARGO_PKG_NAME").into(),
            $callback,
            {
                let priority = 0f64;
                $( let priority = $priority; )?
                priority
            },
        );
    };
}

/// Retrieves the numerical ID of the given event
///
/// # Usage
///
/// ```ignore
/// let id: usize = get_event_id!(<vtable>, <event_name>);
/// ```
///
/// - `vtable` is either [`&VTable`](crate::vtable::VTable) or [`&InitVTable`](crate::vtable::InitVTable)
/// - `event_name` is the name of the event (string)
#[macro_export]
macro_rules! get_event_id {
    ($vtable:path, $event_name:expr $(,)? ) => {
        ($vtable.get_event_id)($event_name.into())
    };
}

/// Fires an event
///
/// # Usage
///
/// ```ignore
/// let success: bool = fire_event!(<vtable>, <event>, [data]);
/// ```
///
/// - `vtable` is [`&VTable`](crate::vtable::VTable)
/// - `event` is either the string name of the event or it's numerical ID ([`usize`])
/// - `data` is an optional reference to arbitrary data that will be passed in the form of `*const ()`
#[macro_export]
macro_rules! fire_event {
    ($vtable:path, $event:expr $(, $data:expr )? $(,)? ) => {
        ($vtable.fire_event)({
                trait __ToEventId {
                    fn to_event_id(self, vtable: & $crate::vtable::VTable) -> usize;
                }
                impl __ToEventId for usize {
                    fn to_event_id(self, vtable: & $crate::vtable::VTable) -> usize {
                        self
                    }
                }
                impl __ToEventId for &str {
                    fn to_event_id(self, vtable: & $crate::vtable::VTable) -> usize {
                        (vtable.get_event_id)(self.into())
                    }
                }

                __ToEventId::to_event_id($event, $vtable)
            },
            {
                let data: *const () = ::std::ptr::null();
                $( let data = $data as *const _ as *const (); )?
                data
            }
        )
    };
}

//////////////////////
// Log macros below //
//////////////////////

/// Logs an error
///
/// # Usage
///
/// ```ignore
/// error!(vtable, "error: {:?}", e);
/// ```
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

/// Logs a warning
///
/// # Usage
///
/// ```ignore
/// warn!(vtable, "caution: {:?}", e);
/// ```
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

/// Logs a message
///
/// # Usage
///
/// ```ignore
/// info!(vtable, "info: {:?}", e);
/// ```
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

/// Logs a debug message
///
/// # Usage
///
/// ```ignore
/// debug!(vtable, "debug info: {:?}", e);
/// ```
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

/// Logs a message on the trace level
///
/// # Usage
///
/// ```ignore
/// trace!(vtable, "trace: {:?}", e);
/// ```
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
