use color_eyre::Result;

pub async fn login() -> Result<()> {
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // Use OpenID Connect Discovery to fetch the provider metadata.
    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new("https://accounts.example.com".to_string())?,
        &http_client,
    )
    .await?;
    Ok(())
}

pub async fn status() -> Result<()> {
    Ok(())
}
