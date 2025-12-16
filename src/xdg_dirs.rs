//! Interactions with the XDG basedir specification

use color_eyre::eyre::ContextCompat;

/// Return the XDG project directories for the buildbtw project.
pub fn new() -> color_eyre::Result<directories::ProjectDirs> {
    directories::ProjectDirs::from("org", "archlinux", "buildbtw")
        .wrap_err("XDG directories not found")
}
