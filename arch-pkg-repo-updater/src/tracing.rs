use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

/// - Create a subscriber for tokio-console if the tokio_unstable flag is enabled and `use_tokio_console` is true
/// - Create a formatting subscriber for outputting logs to stdout
/// - In the formatting subscriber, filter using the `RUST_LOG` env variable
/// - If `RUST_LOG` is not set, filter using the `verbose` argument:
///     - 0: info
///     - 1: debug
///     - 2: trace
pub fn init(verbose: u8, use_tokio_console: bool) {
    let tracing_registry = tracing_subscriber::registry();

    let console_layer = if cfg!(tokio_unstable) && use_tokio_console {
        Some(console_subscriber::spawn())
    } else {
        None
    };

    let env_filter = EnvFilter::try_from_default_env().ok();

    let env_filter = env_filter.unwrap_or(if verbose == 2 {
        EnvFilter::from("trace")
    } else if verbose == 1 {
        EnvFilter::from("debug")
    } else {
        EnvFilter::from("info")
    });
    let env_layer = tracing_subscriber::fmt::layer().with_filter(env_filter);

    tracing_registry.with(console_layer).with(env_layer).init();
}
