use std::io::{self, Write};
use std::path::PathBuf;

use buildbtw::xdg_dirs;
use color_eyre::{Result, eyre::ContextCompat};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::fs;
use tracing::instrument;
use url::Url;

use crate::args::AuthCommand;
use crate::client;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthToken {
    pub created_at: OffsetDateTime,
    #[serde(serialize_with = "redact::expose_secret")]
    pub secret_token: redact::Secret<String>,
}

/// Return the path to the login token
///
/// It doesn't guarantee that it exists, it's just the path where it would be at.
fn auth_token_path() -> Result<PathBuf> {
    let project_dir = xdg_dirs::new()?;
    Ok(project_dir
        .state_dir()
        .wrap_err("Missing XDG state dir")?
        .join("auth_token")
        .to_path_buf())
}

/// Return an auth token if it exists
#[instrument]
pub async fn auth_token() -> Result<Option<AuthToken>> {
    if auth_token_path()?.exists() {
        let auth_token_str = fs::read_to_string(auth_token_path()?).await?;
        let auth_token: AuthToken = serde_json::from_str(&auth_token_str)?;
        Ok(Some(auth_token))
    } else {
        Ok(None)
    }
}

#[instrument]
pub async fn login(server_url: &Url) -> Result<()> {
    if let Some(auth_token) = auth_token().await? {
        eprintln!(
            "{} You're already logged in as (since {}).",
            "Warning:".yellow().bold(),
            auth_token
                .created_at
                .truncate_to_second()
                .format(&Rfc3339)?
        );
        eprintln!("         By logging in again, you're overwriting the previous session.\n");
    }

    let cli_session_url =
        server_url.join(&buildbtw::web::account::CliSessionLanding {}.to_string())?;

    let _ = webbrowser::open(cli_session_url.as_str());
    println!("Continue the login in your browser. Opening your browser to");
    println!("{}", cli_session_url.to_string().bold());
    println!("If your browser didn't open automatically, manually navigate to the URL above.");
    println!("\nYou'll need to log in first via OIDC if you haven't already.");
    println!("Then click 'Create CLI Session' and copy the generated token.\n");

    let auth_token = loop {
        print!("{}", "Paste your CLI session token here: ".bold());
        io::stdout().flush()?;

        let mut secret_token = String::new();

        io::stdin().read_line(&mut secret_token)?;
        let secret_token = secret_token.trim().to_string();

        if secret_token.is_empty() {
            eprintln!("{} Token cannot be empty", "Error:".bright_red().bold());
            continue;
        }

        let auth_token = AuthToken {
            created_at: OffsetDateTime::now_utc(),
            secret_token: redact::Secret::new(secret_token),
        };

        // Verify whether this is even a valid token.
        let user = client::user::current(server_url, &client::reqwest::new().await?).await?;
        if let Some(user) = user {
            println!("Logged in (as {})", user.username.bold());
            break auth_token;
        }
        eprintln!(
            "{} Couldn't log in using provided token",
            "Error:".bright_red().bold()
        );
    };

    let token_str = serde_json::to_string(&auth_token)?;
    let auth_path = auth_token_path()?;

    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(auth_path, token_str).await?;

    println!("\n{}", "Successfully logged in!".bright_green().bold());

    Ok(())
}

#[instrument]
pub async fn status(server_url: &Url) -> Result<()> {
    if let Some(auth_token) = auth_token().await? {
        // We'll verify that we're actually logged in properly and that the session is valid.
        let user = client::user::current(server_url, &client::reqwest::new().await?).await?;
        if let Some(user) = user {
            println!(
                "Logged in as {} (since {})",
                user.username.bold(),
                auth_token
                    .created_at
                    .truncate_to_second()
                    .format(&Rfc3339)?
            );
        } else {
            eprintln!(
                "{} Auth token invalid or expired",
                "Error:".bright_red().bold()
            );
        }
    } else {
        println!("Not logged in");
    }
    Ok(())
}

pub async fn auth(auth_command: AuthCommand, server_url: &Url) -> Result<()> {
    match auth_command {
        AuthCommand::Login => login(server_url).await,
        AuthCommand::Status => status(server_url).await,
    }
}
