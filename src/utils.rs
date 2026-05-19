//! Various smaller helper functions that are universally useful

use std::io;
use std::path::Path;

use color_eyre::Result;
use port_check::is_local_ipv4_port_free;
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

/// Find a free port and return it.
/// We do it using a named lock because that can be reliably passed between processes.
/// If we would bind to port 0 instead, we'd have no way to find out which port we'll
/// get before actually starting to listen on it.
///
/// You can drop the returned lock guard once your process started listening on it and there's no chance of another process taking it.
pub async fn free_port() -> Result<(u16, Option<named_lock::NamedLockGuard>)> {
    let mut port_candidate = 32000;
    Ok(loop {
        if is_local_ipv4_port_free(port_candidate) {
            // We'll make the port part of the lock so that we can quickly find an unused port.
            let lock_name = format!("buildbtw-test-port-{port_candidate}");
            if let Ok(guard) = tokio::task::spawn_blocking(move || {
                named_lock::NamedLock::create(&lock_name)?.try_lock()
            })
            .await?
            {
                break (port_candidate, Some(guard));
            }
        }
        port_candidate += 1;
    })
}
