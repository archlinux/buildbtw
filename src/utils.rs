//! Various smaller helper functions that are universally useful

use std::io;
use std::path::Path;

use color_eyre::Result;
use port_check::with_free_ipv4_port;
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
pub async fn free_port() -> Result<(u16, named_lock::NamedLockGuard)> {
    // Keep the lock files in a subdirectory so they don't pollute the
    // top-level temp dir (named-lock doesn't support cleaning them up).
    let lock_dir = std::env::temp_dir().join("buildbtw-test-port-locks");
    fs::create_dir_all(&lock_dir).await?;
    Ok(tokio::task::spawn_blocking(move || {
        loop {
            if let Some((lock, port)) = with_free_ipv4_port(|port| {
                // We'll make the port part of the lock so that we can quickly find an unused port.
                named_lock::NamedLock::with_path(lock_dir.join(format!("{port}.lock")))?.try_lock()
            }) {
                break (port, lock);
            }
        }
    })
    .await?)
}

/// Convert any unicode string to an ascii "slug" (useful for safe file names/url components)
///
/// The returned "slug" will consist of a-z, 0-9, and '-' or '.'. Furthermore, a slug will
/// never contain more than one '-' or '.' in a row and will never start or end with '-' or '.'.
///
/// ```rust
/// use self::utils::slugify;
///
/// assert_eq!(slugify("My Test String!!!1!1"), "my-test-string-1-1");
/// assert_eq!(slugify("test\nit   now!"), "test-it-now");
/// assert_eq!(slugify("  --test_-_cool"), "test-cool");
/// assert_eq!(slugify("Æúű--cool?"), "cool");
/// assert_eq!(slugify("You & Me"), "you-me");
/// assert_eq!(slugify("user@example.com"), "user-example.com");
/// ```
pub fn slugify<S: AsRef<str>>(s: S) -> String {
    let s = s.as_ref();
    let mut slug = String::with_capacity(s.len());
    // Starts with true to avoid leading - or .
    let mut prev_is_dash_or_dot = true;
    {
        let mut push_char = |x: u8| {
            match x {
                b'a'..=b'z' | b'0'..=b'9' => {
                    prev_is_dash_or_dot = false;
                    slug.push(x.into());
                }
                b'A'..=b'Z' => {
                    prev_is_dash_or_dot = false;
                    // Manual lowercasing as Rust to_lowercase() is unicode
                    // aware and therefore much slower
                    slug.push((x - b'A' + b'a').into());
                }
                c => {
                    if !prev_is_dash_or_dot {
                        if c == b'.' {
                            slug.push(c.into());
                        } else {
                            slug.push('-');
                        }
                        prev_is_dash_or_dot = true;
                    }
                }
            }
        };

        for c in s.chars() {
            if c.is_ascii() {
                (push_char)(c as u8);
            } else {
                (push_char)(b'-');
            }
        }
    }

    if slug.ends_with('-') || slug.ends_with('.') {
        slug.pop();
    }
    // We likely reserved more space than needed.
    slug.shrink_to_fit();
    slug
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("", "")]
    #[case("test\nit   now!", "test-it-now")]
    #[case("test\tit   now!", "test-it-now")]
    #[case("  --test_-_cool", "test-cool")]
    #[case("Æúű--cool?", "cool")]
    #[case("You & Me", "you-me")]
    #[case("user@example.com", "user-example.com")]
    #[case("upgrade-git-smash-1.0", "upgrade-git-smash-1.0")]
    #[case("1.?", "1")]
    #[case("1.?2", "1.2")]
    #[case("../foo", "foo")]
    #[case("foo/../../bar", "foo-bar")]
    #[case("..", "")]
    #[case("...---...", "")]
    fn test_slugify(#[case] s: &str, #[case] expected: &str) {
        assert_eq!(slugify(s), expected);
        // check conversion is idempotent
        assert_eq!(slugify(slugify(s)), expected);
    }
}
