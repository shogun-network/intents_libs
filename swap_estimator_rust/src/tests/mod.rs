use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

#[cfg(test)]
pub fn init_tracing_in_tests() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().json().pretty().with_ansi(true))
        .try_init()
        .ok();
}
