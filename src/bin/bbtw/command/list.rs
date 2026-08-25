use buildbtw::{api_client::ApiClient, buildspace, package};
use color_eyre::Result;

pub async fn list(
    api_client: ApiClient,
    all: bool,
    stopped: bool,
    repo_slug: Option<package::RepositorySlug>,
) -> Result<()> {
    let status_filter = if all {
        None
    } else if stopped {
        Some(buildspace::Status::Stopped)
    } else {
        Some(buildspace::Status::Started)
    };

    let response =
        buildbtw::api_client::buildspaces::list(&api_client, status_filter, repo_slug).await?;

    for buildspace in response.buildspaces {
        let active_indicator = if status_filter.is_none() {
            format!(
                " ({})",
                buildspace.status.to_string().to_lowercase().as_str()
            )
        } else {
            String::new()
        };

        println!("{name}{active_indicator}", name = buildspace.name);
    }

    Ok(())
}
