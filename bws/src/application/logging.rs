use tracing_forest::ForestLayer;
use tracing_subscriber::{filter::LevelFilter, prelude::*, EnvFilter};

pub fn init() {
    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(ForestLayer::default())
        .init();
}
