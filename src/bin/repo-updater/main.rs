//! Tool for cloning all Arch package source repositories and keeping them
//! up-to-date.

use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    buildbtw::tracing::init(0, false)?;

    Ok(())
}
