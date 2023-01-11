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
    _plugin_id: usize,
    id: SStr,
    short: u32,
    long: SStr,
    value_name: SStr,
    help: SStr,
    required: bool,
) -> MaybePanicked {
    MaybePanicked::new(move || {
        let mut command = CLAP_COMMAND_BUILDER.lock().unwrap();

        // Option fuckery because command builder needs ownership when doing anything with it
        let new = command.take().unwrap().arg(
            clap::builder::Arg::new(id.into_str().to_owned())
                .short(char::from_u32(short).expect("`short` is not a valid utf8 char"))
                .long(long.into_str().to_owned())
                .value_name(value_name.into_str().to_owned())
                .help(help.into_str().to_owned())
                .required(required),
        );
        command.replace(new);
    })
}

pub extern "C" fn cmd_flag(
    _plugin_id: usize,
    id: SStr,
    short: u32,
    long: SStr,
    help: SStr,
) -> MaybePanicked {
    MaybePanicked::new(move || {
        let mut command = CLAP_COMMAND_BUILDER.lock().unwrap();

        // Option fuckery because command builder needs ownership when doing anything with it
        let new = command.take().unwrap().arg(
            clap::builder::Arg::new(id.into_str().to_owned())
                .short(char::from_u32(short).expect("`short` is not a valid utf8 char"))
                .long(long.into_str().to_owned())
                .help(help.into_str().to_owned())
                .action(clap::builder::ArgAction::SetTrue),
        );
        command.replace(new);
    })
}

pub extern "C" fn get_cmd_arg(_plugin_id: usize, id: SStr) -> MaybePanicked<SOption<SString>> {
    MaybePanicked::new(move || {
        SOption::from_option(
            CLAP_MATCHES
                .get()
                .expect("clap matches not parsed yet!")
                .try_get_one::<String>(id.into())
                .ok()
                .flatten()
                .map(|s| s.clone().into()),
        )
    })
}

pub extern "C" fn get_cmd_flag(_plugin_id: usize, id: SStr) -> MaybePanicked<bool> {
    MaybePanicked::new(move || {
        matches!(
            CLAP_MATCHES
                .get()
                .expect("clap matches not parsed yet!")
                .try_get_one::<bool>(id.into()),
            Ok(Some(&true))
        )
    })
}
