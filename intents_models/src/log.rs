use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt as _};

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        // TODO: Create env var to check if in prod or develop and use different formats
        .with(fmt::layer().json().pretty().with_ansi(true))
        .init();
}
