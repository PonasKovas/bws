/// A macro that helps with plugin definition
///
/// # Usage
///
/// ```
/// bws_plugin_interface::plugin! {
///     depend "other_plugin_dependency" = "0.1",
///     depend "you_can_have_as_many_dependencies_as_you_want" = "<2.0",
///
///     provide "api_name" = "0.2.2" with MY_API,
///     provide "my_api" with MY_API,   // uses crate version
///
///     CMD[
///         ARG{
///             id: "my_arg",
///             short: 'm', // optional
///             long: "my_arg",
///             help: "My argument takes an integer.",
///             value_name: "INTEGER",
///             required: true,
///         },
///         FLAG{
///             id: "my_flag",
///             short: 'n', // optional
///             long: "my_flag",
///             help: "Enables functionality",
///         },
///     ],
///
///     start(start),
/// }
///
/// static MY_API: u32 = 0;
///
/// fn start() {
///     //info!("My plugin started");
///     //if get_vtable().get_cmd_flag("debug") {
///     //    warn!("--debug flag set!");
///     //}
/// }
/// ```
#[macro_export]
macro_rules! plugin {
    (
        $(depend $depname:literal = $depversion:literal ,)*
        $(provide $providename:literal $( = $provideversion:literal )? with $provider:path ,)*
        $(CMD[
            $(ARG{ id: $arg_id:literal, $(short: $arg_short:literal,)? long: $arg_long:literal, help: $arg_help:literal, value_name: $arg_value_name:literal, required: $arg_required:literal $(,)?},)*
            $(FLAG{ id: $flag_id:literal, $(short: $flag_short:literal,)? long: $flag_long:literal, help: $flag_help:literal $(,)?},)*
        ],)?
        start($start_fn:path) $(,)?
    ) => {const _: () = {
        use $crate::ironties::types::{MaybePanicked, SUnit, SSlice, SStr, SOption, STuple2};
        use $crate::ironties::{TypeLayout, TypeInfo};

        #[no_mangle]
        static BWS_ABI: SStr<'static> = $crate::ABI;

        #[no_mangle]
        static BWS_PLUGIN: $crate::BwsPlugin = $crate::BwsPlugin {
            name: SStr::new(env!("CARGO_PKG_NAME")),
            depends_on: SSlice::new(&[
                $( STuple2( SStr::new( $depname ), SStr::new( $depversion ) ), )*
            ]),
            provides: SSlice::new(&[
                $( $crate::Api{
                    name: SStr::new($providename),
                    version: SStr::new({ env!("CARGO_PKG_VERSION") $(; $provideversion)? }),
                    vtable: &$provider as *const _ as *const (),
                    vtable_layout: {
                        const fn get_layout<T: TypeInfo>(_: &T) -> extern "C" fn() -> MaybePanicked<TypeLayout> {
                            extern "C" fn __layout<T: TypeInfo>() -> MaybePanicked<TypeLayout> {
                                MaybePanicked::new(|| T::layout())
                            }
                            __layout::<T>
                        }

                        get_layout(&$provider)
                    },
                }, )*
            ]),
            cmd: SSlice::new(&[
                $(
                    $( $crate::Cmd{
                        id: SStr::new($arg_id),
                        short: { SOption::<char>::None $(; SOption::Some($arg_short) )? },
                        long: SStr::new($arg_long),
                        help: SStr::new($arg_help),
                        kind: $crate::CmdKind::Argument{
                            value_name: SStr::new($arg_value_name),
                            required: $arg_required,
                        },
                    }, )*
                    $( $crate::Cmd{
                        id: SStr::new($flag_id),
                        short: { SOption::<char>::None $(; SOption::Some($flag_short) )? },
                        long: SStr::new($flag_long),
                        help: SStr::new($flag_help),
                        kind: $crate::CmdKind::Flag,
                    }, )*
                )?
            ]),
            start: {
                extern "C" fn __start(plugin_id: usize, vtable: &'static $crate::VTable) -> MaybePanicked<SUnit>{
                    MaybePanicked::new(move || {
                        $crate::global::set_plugin_id(plugin_id);
                        $crate::global::set_vtable(vtable);

                        let _: () = $start_fn();

                        SUnit::new()
                    })
                }

                __start
            },
        };
    };};
}

//////////////////////
// Log macros below //
//////////////////////

/// Logs an error
///
/// # Usage
///
/// ```
/// # use bws_plugin_interface::error;
/// # fn example<T: std::fmt::Debug>(e: T) {
/// error!("error: {:?}", e);
/// # }
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::vtable().log(::std::module_path!(), $crate::LogLevel::Error, &::std::format!($($arg)+));
    };
}

/// Logs a warning
///
/// # Usage
///
/// ```
/// # use bws_plugin_interface::warn;
/// # fn example<T: std::fmt::Debug>(e: T) {
/// warn!("warning: {:?}", e);
/// # }
/// ```
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::vtable().log(::std::module_path!(), $crate::LogLevel::Warn, &::std::format!($($arg)+));
    };
}

/// Logs a message
///
/// # Usage
///
/// ```
/// # use bws_plugin_interface::info;
/// # fn example<T: std::fmt::Debug>(e: T) {
/// info!("info: {:?}", e);
/// # }
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::vtable().log(::std::module_path!(), $crate::LogLevel::Info, &::std::format!($($arg)+));
    };
}

/// Logs a debug message
///
/// # Usage
///
/// ```
/// # use bws_plugin_interface::debug;
/// # fn example<T: std::fmt::Debug>(e: T) {
/// debug!("debug info: {:?}", e);
/// # }
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::vtable().log(::std::module_path!(), $crate::LogLevel::Debug, &::std::format!($($arg)+));
    };
}

/// Logs a message on the trace level
///
/// # Usage
///
/// ```
/// # use bws_plugin_interface::trace;
/// # fn example<T: std::fmt::Debug>(e: T) {
/// trace!("trace info: {:?}", e);
/// # }
/// ```
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::vtable().log(::std::module_path!(), $crate::LogLevel::Trace, &::std::format!($($arg)+));
    };
}
