//! Configuration for running the buildbtw server. Made by validating and transforming [super::args::RunArgs].

use axum_server::tls_rustls::RustlsConfig;
use buildbtw::{external_secrets, gitlab_api, oidc, schedule_builds};
use camino::Utf8PathBuf;
use color_eyre::{Result, eyre::Context};
use url::Url;

use crate::args::{self, TcpSocketOrUnixSocket};

pub struct Config {
    pub oidc_state: Option<oidc::State>,
    pub gitlab: Option<gitlab_api::Config>,
    pub rustls: Option<RustlsConfig>,
    pub dispatch_builds_to: Option<schedule_builds::DispatchBuildsTo>,
    pub data_dir: Option<Utf8PathBuf>,
    pub cookie_encryption_key: redact::Secret<axum_extra::extract::cookie::Key>,
    pub listen: TcpSocketOrUnixSocket,
    pub update_source_repos: bool,
    pub auto_create_iterations: bool,
    pub server_url: Url,
    pub web_root: Utf8PathBuf,
}

impl Config {
    pub async fn try_from(args: args::RunArgs) -> Result<Self> {
        let oidc_state = if let Some(oidc) = args.oidc {
            let oidc_init_config = oidc::InitConfig::try_from(oidc)?;
            let oidc_state = oidc::State::initialize(&args.server_url, oidc_init_config)
                .await
                .wrap_err("OIDC configuration failed")?;
            Some(oidc_state)
        } else {
            None
        };

        let gitlab = args.gitlab.map(gitlab_api::Config::try_from).transpose()?;

        let rustls = if let Some(args::Tls { tls_cert, tls_key }) = args.tls {
            Some(
                RustlsConfig::from_pem_file(tls_cert, tls_key)
                    .await
                    .wrap_err("Failed to load TLS certificate and key")?,
            )
        } else {
            None
        };

        let dispatch_builds_to = args
            .dispatch_builds_to
            .map(schedule_builds::DispatchBuildsTo::from);

        let cookie_encryption_key = external_secrets::get_cookie_encryption_key(
            args.cookie_encryption_key_path.as_deref(),
        )?;

        Ok(Config {
            oidc_state,
            gitlab,
            rustls,
            dispatch_builds_to,
            data_dir: args.data_dir,
            cookie_encryption_key,
            listen: args.listen,
            update_source_repos: args.update_source_repos,
            auto_create_iterations: args.auto_create_iterations,
            server_url: args.server_url,
            web_root: args.web_root,
        })
    }
}
