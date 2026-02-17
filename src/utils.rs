//! Various smaller helper functions that are universally useful

use std::io;
use std::path::Path;

use tokio::fs;

/// Delete the file at `path` only if it exists
pub async fn remove_file_if_exists<P>(path: P) -> io::Result<()>
where
    P: AsRef<Path>,
{
    match fs::remove_file(path.as_ref()).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Creates a static regex and returns a reference to it.
///
/// See the [crate level documentation][crate] for more information.
#[macro_export]
macro_rules! regex {
    ($re:expr $(,)?) => {{
        static RE: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new($re).expect("invalid regex"));
        &RE
    }};
}
