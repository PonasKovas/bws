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
    let mut command = CLAP_COMMAND_BUILDER.lock().unwrap();

    // Option fuckery because command builder needs ownership when doing anything with it
    let new = command.take().unwrap().arg(
        clap::builder::Arg::new(id.as_str().to_owned())
            .short(char::from_u32(short).expect("`short` is not a valid utf8 char"))
            .long(long.as_str().to_owned())
            .value_name(value_name.as_str().to_owned())
            .help(help.as_str().to_owned())
            .required(required),
    );
    command.replace(new);
}

pub extern "C" fn cmd_flag(id: SStr, short: u32, long: SStr, help: SStr) {
    let mut command = CLAP_COMMAND_BUILDER.lock().unwrap();

    // Option fuckery because command builder needs ownership when doing anything with it
    let new = command.take().unwrap().arg(
        clap::builder::Arg::new(id.as_str().to_owned())
            .short(char::from_u32(short).expect("`short` is not a valid utf8 char"))
            .long(long.as_str().to_owned())
            .help(help.as_str().to_owned())
            .action(clap::builder::ArgAction::SetTrue),
    );
    command.replace(new);
}

pub extern "C" fn get_cmd_arg(id: SStr) -> SOption<SString> {
    CLAP_MATCHES
        .get()
        .expect("clap matches not parsed yet!")
        .try_get_one::<String>(id.into())
        .ok()
        .flatten()
        .map(|s| s.clone().into())
        .into()
}

pub extern "C" fn get_cmd_flag(id: SStr) -> bool {
    match CLAP_MATCHES
        .get()
        .expect("clap matches not parsed yet!")
        .try_get_one::<bool>(id.into())
    {
        Ok(Some(&true)) => true,
        _ => false,
    }
}
