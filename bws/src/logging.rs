use tracing_forest::ForestLayer;
use tracing_subscriber::{filter::LevelFilter, prelude::*, EnvFilter};

pub fn init() {
    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(ForestLayer::default());

    if crate::cli::OPT.save_logs {
        // logging to files
        let file_appender = tracing_appender::rolling::daily("logs", "bws.log");

        subscriber
            .with(
                tracing_subscriber::fmt::Layer::new()
                    .with_ansi(false)
                    .with_writer(file_appender),
            )
            .init();
    } else {
        subscriber.init();
    }
}
