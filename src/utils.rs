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
