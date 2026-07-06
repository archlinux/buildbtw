use buildbtw::api::users::User;
use color_eyre::{Result, eyre::Context};
use reqwest::StatusCode;
use tracing::instrument;

/// Get the currently logged in user
///
/// If successful, will return the logged-in [`User`].
/// If we receive an UNAUTHORIZED from the server, return `None`.
#[instrument(skip(client))]
pub async fn current(client: &super::Client) -> Result<Option<User>> {
    let resp = client
        .reqwest_client
        .get(
            client
                .buildbtw_server_url
                .join(&buildbtw::api::users::AuthenticatedUser {}.to_string())?,
        )
        .send()
        .await
        .wrap_err("Couldn't get login status")?;

    match resp.error_for_status_ref() {
        Ok(_) => {
            let user = resp
                .json::<User>()
                .await
                .wrap_err("Couldn't deserialize JSON to User")?;
            Ok(Some(user))
        }
        Err(e) if e.status() == Some(StatusCode::UNAUTHORIZED) => {
            // If we get UNAUTHORIZED we probably entered an invalid secret token. That is, a secret token
            // that can't be associated with a Session.
            return Ok(None);
        }
        Err(e) => Err(e).wrap_err("Unexpected HTTP error while getting user from auth token"),
    }
}
