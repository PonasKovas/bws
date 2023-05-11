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

    // logging to files
    let file_appender = tracing_appender::rolling::daily("logs", "bws.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    if crate::cli::OPT.save_logs {
        subscriber
            .with(
                tracing_subscriber::fmt::Layer::new()
                    .with_ansi(false)
                    .with_writer(non_blocking),
            )
            .init();
    } else {
        subscriber.init();
    }
}
