use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt as _};

pub fn init_tracing(prod_format: bool) {
    if prod_format {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().json().flatten_event(true).with_ansi(false))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            // TODO: Create env var to check if in prod or develop and use different formats
            .with(fmt::layer().json().pretty().with_ansi(true))
            .init();
    }
}
