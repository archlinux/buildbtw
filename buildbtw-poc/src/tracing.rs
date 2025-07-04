use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

/// - Create a subscriber for tokio-console if the tokio_unstable flag is enabled and `use_tokio_console` is true
/// - Create a formatting subscriber for outputting logs to stdout
/// - In the formatting subscriber, filter using the `RUST_LOG` env variable
/// - If `RUST_LOG` is not set, filter using the `verbose` argument:
///     - 0: error
///     - 1: warn
///     - 2: info
///     - 3: debug
///     - 4: trace
pub fn init(verbose: u8, use_tokio_console: bool) {
    let tracing_registry = tracing_subscriber::registry();

    let console_layer = if cfg!(tokio_unstable) && use_tokio_console {
        Some(console_subscriber::spawn())
    } else {
        None
    };

    let env_filter = EnvFilter::try_from_default_env().ok();

    let env_filter = env_filter.unwrap_or(match verbose {
        0 => EnvFilter::from("error"),
        1 => EnvFilter::from("warn"),
        2 => EnvFilter::from("info"),
        3 => EnvFilter::from("debug"),
        4 => EnvFilter::from("trace"),
        _ => EnvFilter::from("trace"),
    });
    let env_layer = tracing_subscriber::fmt::layer().with_filter(env_filter);

    tracing_registry.with(console_layer).with(env_layer).init();
}
