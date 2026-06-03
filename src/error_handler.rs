use color_eyre::Result;

/// Install the error handler
///
/// It's set up to minimize distracting information in the default case and to provide extra
/// information in the verbose mode.
pub fn init(verbose: u8) -> Result<()> {
    // Hide spantraces if we're not verbose.
    if verbose == 0 && std::env::var("RUST_SPANTRACE").is_err() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("RUST_SPANTRACE", "0");
        }
    }

    let mut eyre_builder = color_eyre::config::HookBuilder::new();

    // If colors aren't supported, we'll use the empty theme which effectively disables color output.
    // See also:
    // - https://github.com/eyre-rs/eyre/issues/236
    // - https://github.com/eyre-rs/eyre/issues/237
    // - https://github.com/eyre-rs/eyre/pull/238
    if !colored::control::ShouldColorize::from_env().should_colorize() {
        eyre_builder = eyre_builder.theme(color_eyre::config::Theme::new());
    }

    // Custom color_eyre config so that we hide the env nagging section to get cleaner output in the
    // default case.
    eyre_builder.panic_section("If you believe this is a program error, consider reporting a bug at https://gitlab.archlinux.org/archlinux/buildbtw")
        // Reduce noise when verbosity level is 0.
        .display_env_section(verbose != 0)
        // Also reduce noise, and prevent line numbers causing test snapshots to change often.
        .display_location_section(verbose != 0)
        .install()?;

    Ok(())
}
