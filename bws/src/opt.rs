use clap::ArgMatches;

pub fn collect_opt() -> ArgMatches {
    clap::App::new("bws")
        .version(clap::crate_version!())
        .about("A light-weight and modular minecraft server")
        .arg(
            clap::Arg::new("disable_timestamps")
                .env("BWS_DISABLE_TIMESTAMPS")
                .long("disable_timestamps")
                .help("If set, the application will not log timestamps.")
                .long_help("If set, the application will not log timestamps. Useful when using with systemd, because it logs timestamps by itself.")
        )
        .get_matches()
}
