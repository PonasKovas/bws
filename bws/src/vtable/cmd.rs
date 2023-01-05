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
