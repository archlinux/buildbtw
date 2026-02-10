//! Functionality for handling external secret values such as credentials for APIs.
//!
//! This is *not* concerned with data stored in the server database that should be kept secret.

use camino::Utf8Path;
use color_eyre::eyre::Context;
use color_eyre::{Result, eyre::ContextCompat};

/// Read a secret
///
/// Try to read from multiple sources, in order of precedence:
/// 1. env var specified by `name`
/// 2. file specified by `file_path`
/// 3. `$XDG_CONFIG_HOME`/buildbtw/{name}
///
/// If all fail, return an error.
pub fn get_required(name: &str, file_path: Option<&Utf8Path>) -> Result<redact::Secret<String>> {
    let env_value = std::env::var(name).wrap_err(format!("Could not read env var {name}"));

    let explicit_file_value = file_path
        .map(|path| {
            std::fs::read_to_string(path).wrap_err(format!("Could not read secret at {path}"))
        })
        .wrap_err(format!("Secret file path for {name} not specified"))
        .flatten();

    let xdg_dir_file_value = crate::xdg_dirs::new().and_then(|dirs| {
        let config_dir = dirs.config_dir();
        let path = config_dir.join(name);
        std::fs::read_to_string(&path)
            .wrap_err(format!("Could not read secret at {}", path.display()))
    });

    env_value
        .or(explicit_file_value)
        .or(xdg_dir_file_value)
        .map(redact::Secret::new)
}

/// Create a [`axum_extra::extract::cookie::Key`] from a string
pub fn get_cookie_encryption_key(
    path: Option<&Utf8Path>,
) -> Result<redact::Secret<axum_extra::extract::cookie::Key>> {
    let secret = get_required("BUILDBTW_COOKIE_ENCRYPTION_KEY", path)?;
    let key_string = secret.expose_secret();
    axum_extra::extract::cookie::Key::try_from(key_string.as_bytes())
        .wrap_err("Failed to parse encryption key")
        .map(redact::Secret::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_required_secret_from_env() {
        let var_name = "TEST_SECRET";
        let expected_value = "secret_from_env";

        temp_env::with_var(var_name, Some(expected_value), || {
            let result = get_required(var_name, None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().expose_secret(), expected_value);
        });
    }

    #[test]
    fn test_get_required_secret_from_file() {
        let var_name = "TEST_SECRET";
        let expected_value = "secret_from_file";

        // Create temporary file with secret
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "{expected_value}").unwrap();
        temp_file.flush().unwrap();

        let file_path = Utf8PathBuf::from_path_buf(temp_file.path().to_path_buf()).unwrap();

        temp_env::with_var_unset(var_name, || {
            let result = get_required(var_name, Some(&file_path));
            assert!(result.is_ok());
            // Note: read_to_string includes newline, so we need to trim
            assert_eq!(result.unwrap().expose_secret().trim(), expected_value);
        });
    }

    #[test]
    fn test_get_required_secret_env_priority_over_file() {
        let var_name = "TEST_SECRET_PRIORITY_11111";
        let env_value = "secret_from_env";
        let file_value = "secret_from_file";

        // Create temporary file with different secret
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "{file_value}").unwrap();
        temp_file.flush().unwrap();

        // Set environment variable
        temp_env::with_var(var_name, Some(env_value), || {
            let file_path = Utf8PathBuf::from_path_buf(temp_file.path().to_path_buf()).unwrap();

            let result = get_required(var_name, Some(&file_path));
            assert!(result.is_ok());
            // Environment variable should take priority
            assert_eq!(result.unwrap().expose_secret(), env_value);
        });
    }

    #[test]
    fn test_get_required_secret_nonexistent_file() {
        let var_name = "TEST_SECRET_NONEXISTENT_22222";

        let nonexistent_path = Utf8PathBuf::from("/tmp/this_file_should_not_exist_98765.txt");

        let result = get_required(var_name, Some(&nonexistent_path));
        // Should fail when file doesn't exist and no env var is set
        assert!(result.is_err());
    }

    #[test]
    fn test_get_required_secret_no_sources() {
        let var_name = "TEST_SECRET_NO_SOURCES_33333";

        let result = get_required(var_name, None);
        // Should fail when no sources are available
        assert!(result.is_err());
    }
}
