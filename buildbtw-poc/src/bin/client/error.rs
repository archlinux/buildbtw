use color_eyre::eyre::eyre;

pub trait MapReqwestError {
    /// If there was an error, replace it with an
    /// error suitable for displaying it to the user.
    async fn map_reqwest_error(self) -> Result<reqwest::Response, color_eyre::Report>;
}

impl MapReqwestError for reqwest::Response {
    async fn map_reqwest_error(self) -> Result<reqwest::Response, color_eyre::Report> {
        match self.error_for_status_ref() {
            Ok(_) => Ok(self),
            // Replace reqwest's developer-oriented error
            // with the body of the response, which should be better suited to show users what went wrong.
            // TODO only do this for errors that are expected, e.g. InvalidInput errors or NotFound errors.
            // TODO find a way to print these without showing stacktraces or spantraces
            Err(_) => Err(eyre!(self.text().await?)),
        }
    }
}
