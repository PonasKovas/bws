mod cmd;
mod linear_search;
mod plugins;
mod vtable;

use anyhow::{Context, Result};
pub use linear_search::LinearSearch;
use once_cell::sync::OnceCell;

static END_PROGRAM: OnceCell<()> = OnceCell::new();

fn main() -> Result<()> {
    // Attempt to load plugins
    plugins::load_plugins().context(
        "Failed to load plugins. You have to resolve this error before you can launch BWS.",
    )?;

    // Parse env and command line args
    cmd::parse();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(if cmd::get_flag("disable_timestamps") {
            None
        } else {
            Some(Default::default())
        })
        .parse_default_env()
        .init();

    // Start the plugins
    plugins::start_plugins().context("Couldn't start plugins")?;

    // block the thread until notification on END_PROGRAM is received
    END_PROGRAM.wait();

    Ok(())
}
