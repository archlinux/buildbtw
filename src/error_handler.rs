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

    // Custom color_eyre config so that we hide the env nagging section to get cleaner output in the
    // default case.
    color_eyre::config::HookBuilder::new()
        .panic_section("If you believe this is a program error, consider reporting a bug at https://gitlab.archlinux.org/archlinux/buildbtw")
        .display_env_section(verbose != 0)
        .install()?;

    Ok(())
}
