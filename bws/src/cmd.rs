use bws_plugin_interface::ironties::types::FfiSafeEquivalent;
use clap::{Arg, ArgAction, ArgMatches, Command};
use once_cell::sync::OnceCell;

pub static OPT: OnceCell<ArgMatches> = OnceCell::new();

pub fn parse() {
    let mut command = Command::new("BWS")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Beautiful and Wholesome Server")
        .arg(
            Arg::new("disable_timestamps")
                .long("disable_timestamps")
                .help("Don't print timestamps in logs (useful when timestamps already provided externally)")
                .action(ArgAction::SetTrue),
        );

    // Add flags and args from plugins
    //////////////////////////////////

    let plugins = crate::plugins::PLUGINS.get().unwrap();

    for plugin in plugins {
        for cmd in plugin.plugin.cmd {
            let arg = Arg::new(cmd.id.into_normal())
                .long(cmd.long.into_normal())
                .help(format!("{} [{}]", cmd.help, plugin.plugin.name));

            let arg = if let Some(short) = cmd.short.into_normal() {
                arg.short(short)
            } else {
                arg
            };

            let arg = match cmd.kind {
                bws_plugin_interface::CmdKind::Argument {
                    value_name,
                    required,
                } => arg.value_name(value_name.into_normal()).required(required),
                bws_plugin_interface::CmdKind::Flag => arg.action(ArgAction::SetTrue),
            };

            command = command.arg(arg);
        }
    }

    // Get matches
    //////////////

    let matches = command.get_matches();

    OPT.set(matches).unwrap();
}

pub fn get_arg(arg: &str) -> Option<&'static str> {
    OPT.get()
        .expect("clap matches not parsed yet!")
        .try_get_raw(arg)
        .expect("Attempted to check an argument which wasn't registered")
        .map(|mut s| {
            s.next()
                .unwrap()
                .to_str()
                .expect("Command line arguments must be valid unicode")
        })
}

pub fn get_flag(arg: &str) -> bool {
    matches!(
        OPT.get()
            .expect("clap matches not parsed yet!")
            .try_get_one::<bool>(arg)
            .expect("Attempted to check a flag which wasn't registered"),
        Some(&true)
    )
}
