use camino::Utf8PathBuf;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BuildConfig {
    pub builds_dir: Utf8PathBuf,
    /// Non-optional directory provided by the gitlab runner. Allows caching stuff between separate runs. Currently unused.
    pub cache_dir: Utf8PathBuf,
}
