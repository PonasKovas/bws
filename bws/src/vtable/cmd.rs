use bws_plugin_interface::{
    safe_types::*,
    vtable::{LogLevel, VTable},
};
use once_cell::sync::{Lazy, OnceCell};
use std::sync::Mutex;

pub static CLAP_COMMAND_BUILDER: Lazy<Mutex<Option<clap::builder::Command>>> = Lazy::new(|| {
    Mutex::new(Some(
        clap::builder::Command::new("BWS")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Beautiful and Wholesome Server"),
    ))
});

pub static CLAP_MATCHES: OnceCell<clap::parser::ArgMatches> = OnceCell::new();

/// Registers a command line argument for the application
///
///  - `id` - unique name for the argument, can be used later to retrieve the set value
///  - `short` - a `char` in the form of `u32` (example `'p' as u32`) defining the short way to set the argument
///  - `long` - the long way to set the argument
///  - `value_name` - the name/type of value that is expected (convention is to use all uppercase here)
///  - `help` - the help string
///  - `required` - whether the argument is mandatory
///
/// The function will panic if `short` is not a valid `char`
pub extern "C" fn cmd_arg(
    id: SStr,
    short: u32,
    long: SStr,
    value_name: SStr,
    help: SStr,
    required: bool,
) {
    let mut command = CLAP_COMMAND_BUILDER
        .lock()
        .expect("Couldn't lock Clap command builder");
    // Option fuckery because command builder needs ownership when doing anything with it
    let new = command.take().unwrap().arg(
        clap::builder::Arg::new(id.as_str().to_owned())
            .short(char::from_u32(short).unwrap())
            .long(long.as_str().to_owned())
            .value_name(value_name.as_str().to_owned())
            .help(help.as_str().to_owned())
            .required(required),
    );
    command.replace(new);
}

/// Registers a command line flag for the application
///
///  - `id` - unique name for the flag, can be used later to check if it was set
///  - `short` - a `char` in the form of `u32` (example `'p' as u32`) defining the short way to set the flag
///  - `long` - the long way to set the flag
///  - `help` - the help string
///
/// The function will panic if `short` is not a valid `char`
pub extern "C" fn cmd_flag(id: SStr, short: u32, long: SStr, help: SStr) {
    let mut command = CLAP_COMMAND_BUILDER
        .lock()
        .expect("Couldn't lock Clap command builder");
    // Option fuckery because command builder needs ownership when doing anything with it
    let new = command.take().unwrap().arg(
        clap::builder::Arg::new(id.as_str().to_owned())
            .short(char::from_u32(short).unwrap())
            .long(long.as_str().to_owned())
            .help(help.as_str().to_owned())
            .action(clap::builder::ArgAction::SetTrue),
    );
    command.replace(new);
}

/// Retrieves a command line argument, if it was set
///
///  - `id` - the unique name of the argument. (Must match with the one given when registering the argument with `cmd_arg`)
pub extern "C" fn get_cmd_arg(id: SStr) -> SOption<SString> {
    match CLAP_MATCHES
        .get()
        .expect("clap matches not parsed yet!")
        .try_get_one::<String>(id.into())
    {
        Ok(m) => m.map(|x| x.clone().into()).into(),
        Err(_) => SOption::None,
    }
}

/// Checks if a command line flag was set
///
///  - `id` - the unique name of the flag. (Must match with the one given when registering the flag with `cmd_flag`)
pub extern "C" fn get_cmd_flag(id: SStr) -> bool {
    match CLAP_MATCHES
        .get()
        .expect("clap matches not parsed yet!")
        .try_get_one::<bool>(id.into())
    {
        Ok(m) => *m.unwrap_or(&false),
        Err(_) => false,
    }
}
