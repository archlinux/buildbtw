use std::io::{self, Write};

use buildbtw::bbtw;
use camino::Utf8PathBuf;
use color_eyre::Result;
use colored::Colorize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::instrument;
use url::Url;

use crate::api;
use crate::args::AuthCommand;

#[instrument]
pub async fn login(server_url: Url, override_state_dir: Option<Utf8PathBuf>) -> Result<()> {
    if let Some(auth_token) = bbtw::auth::Token::read(override_state_dir.clone()).await? {
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

    let client = api::Client::new(server_url, override_state_dir.clone()).await?;

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

        let auth_token = bbtw::auth::Token {
            created_at: OffsetDateTime::now_utc(),
            secret_token: redact::Secret::new(secret_token),
        };

        // Verify whether this is even a valid token.
        let user = api::user::current(&client).await?;
        if let Some(user) = user {
            println!("Logged in (as {})", user.username.bold());
            break auth_token;
        }
        eprintln!(
            "{} Couldn't log in using provided token",
            "Error:".bright_red().bold()
        );
    };

    auth_token
        .persist(&bbtw::auth::token_path(override_state_dir)?)
        .await?;

    println!("\n{}", "Successfully logged in!".bright_green().bold());

    Ok(())
}

#[instrument(skip_all)]
pub async fn status(server_url: Url, override_state_dir: Option<Utf8PathBuf>) -> Result<()> {
    if let Some(auth_token) = bbtw::auth::Token::read(override_state_dir.clone()).await? {
        // We'll verify that we're actually logged in properly and that the session is valid.
        let user =
            api::user::current(&api::Client::new(server_url, override_state_dir).await?).await?;
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

pub async fn auth(
    auth_command: &AuthCommand,
    server_url: Url,
    override_state_dir: Option<Utf8PathBuf>,
) -> Result<()> {
    match auth_command {
        AuthCommand::Login => login(server_url, override_state_dir).await,
        AuthCommand::Status => status(server_url, override_state_dir).await,
    }
}
