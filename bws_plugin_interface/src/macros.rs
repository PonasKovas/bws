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

/// A macro that helps with callback definition
///
/// # Usage
///
/// ```ignore
/// callback!{ <callback_name>(vtable, [event_id], [data: &<data_type>]) {
///     // function body
/// }}
/// ```
///
/// Where:
/// - `callback_name` is an identifier for the callback. A function is created with this name
/// - `vtable` is the variable name for the [`VTable`](crate::vtable::VTable) (Required)
/// - `event_id` is the variable name for the event id (`usize`) (Optional)
/// - `data` is the variable name for data (Optional)
/// - `data_type` is the type of data that is expected. The data pointer is converted to a reference of that type
/// automatically, so make sure you provide the correct type. (Required if `data` present)
///
/// The function body has to return [`bool`]: `false` meaning to stop event and execute no further callbacks for this instance.
#[macro_export]
macro_rules! callback {
    ($callback_name:ident ($vtable:ident, $event_id:ident, $data:ident : & $data_type:ty $(,)? ) $block:tt ) => {
        extern "C" fn $callback_name(
            $vtable: &'static $crate::vtable::VTable,
            $event_id: usize,
            $data: *const (),
        ) -> bool {
            let reference: &$data_type = unsafe { &*($data as *const $data_type) };

            fn inner(
                $vtable: &'static $crate::vtable::VTable,
                $event_id: usize,
                $data: &$data_type,
            ) -> bool {
                $block
            }

            inner($vtable, $event_id, reference)
        }
    };
    ($callback_name:ident ($vtable:ident, $data:ident : & $data_type:ty $(,)? ) $block:tt ) => {
        extern "C" fn $callback_name(
            $vtable: &'static $crate::vtable::VTable,
            event_id: usize,
            $data: *const (),
        ) -> bool {
            let reference: &$data_type = unsafe { &*($data as *const $data_type) };

            fn inner(
                $vtable: &'static $crate::vtable::VTable,
                _: usize,
                $data: &$data_type,
            ) -> bool {
                $block
            }

            inner($vtable, event_id, reference)
        }
    };
    ($callback_name:ident ($vtable:ident, $event_id:ident $(,)? ) $block:tt ) => {
        extern "C" fn $callback_name(
            $vtable: &'static $crate::vtable::VTable,
            $event_id: usize,
            _: *const (),
        ) -> bool {
            $block
        }
    };
    ($callback_name:ident ($vtable:ident $(,)? ) $block:tt ) => {
        extern "C" fn $callback_name(
            $vtable: &'static $crate::vtable::VTable,
            _: usize,
            _: *const (),
        ) -> bool {
            $block
        }
    };
}

/// **Highly unsafe!** Retrieves the API of a different plugin
///
/// # Usage
///
/// ```ignore
/// let api: &'static <API> = get_plugin_api!(<vtable>, <plugin_name>, <API>);
/// ```
///
/// Where:
/// - `API` is the type of the API. Usually should be exposed by the specific plugin interface library.
/// It is very important to use the correct type here, there are no checks and nothing (besides a segfault)
/// will stop you from making mistakes here, so be extra careful.
/// - `vtable` is [`&VTable`](crate::vtable::VTable)
/// - `plugin_name` is a string with the needed plugin name.
#[macro_export]
macro_rules! get_plugin_api {
    ($vtable:path, $plugin:expr, $api_type:ty) => {
        unsafe { ($vtable.get_plugin_vtable)($plugin.into()).into_vtable::<$api_type>() }.unwrap()
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
                impl __ToEventId for &String {
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
