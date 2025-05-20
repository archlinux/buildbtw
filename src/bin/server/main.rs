use std::net::{SocketAddr, TcpListener};

use anyhow::Result;
use axum::{
    response::Redirect,
    routing::{get, patch, post},
    Router,
};
use axum_extra::handler::HandlerCallWithExtractors;
use clap::Parser;
use listenfd::ListenFd;
use routes::{
    create_build_namespace, create_namespace_iteration, list_namespaces_html, list_namespaces_json,
    render_build_namespace_graph, render_latest_namespace, set_build_status, show_build_namespace,
    update_namespace, upload_package,
};
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tower_http::{services::ServeDir, trace::TraceLayer};
use url::Url;
use with_content_type::{with_content_type, ApplictionJson};

use crate::args::{Args, Command};
use buildbtw::pacman_repo::REPO_DIR;

mod args;
pub mod assets;
mod db;
pub mod response_error;
mod routes;
pub mod stream_to_file;
mod tasks;
pub mod with_content_type;

#[derive(Clone)]
struct AppState {
    #[allow(dead_code)]
    worker_sender: UnboundedSender<tasks::Message>,
    jinja_env: minijinja::Environment<'static>,
    db_pool: SqlitePool,
    base_url: Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    buildbtw::tracing::init(args.verbose, true);

    tracing::debug!("{args:#?}");

    match args.command {
        Command::Run {
            interface,
            port,
            base_url,
        } => {
            let mut jinja_env = minijinja::Environment::new();
            jinja_env.add_template(
                "layout",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/layout.jinja"
                )),
            )?;
            jinja_env.add_template(
                "show_build_namespace",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/show_build_namespace.jinja"
                )),
            )?;
            jinja_env.add_template(
                "render_build_namespace_graph",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/render_build_namespace_graph.jinja"
                )),
            )?;
            jinja_env.add_template(
                "list_build_namespaces",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/list_build_namespaces.jinja"
                )),
            )?;
            let db_pool: sqlx::Pool<sqlx::Sqlite> =
                db::create_and_connect_db(&args.database_url).await?;

            let worker_sender = tasks::start(db_pool.clone(), args.gitlab).await?;
            let app = Router::new()
                .route("/", get(|| async {Redirect::to("/namespace")}))
                .route(
                    "/namespace",
                    post(create_build_namespace).get(
                        with_content_type::<ApplictionJson, _>(list_namespaces_json)
                            .or(list_namespaces_html),
                    ),
                )
                .route(
                    "/namespace/{name}/iteration",
                    post(create_namespace_iteration),
                )
                .route("/namespace/{name}", get(show_build_namespace))
                .route(
                    "/namespace/{name}/{architecture}/graph",
                    get(render_build_namespace_graph),
                )
                .route("/latest_namespace", get(render_latest_namespace))
                .route("/namespace/{name}", patch(update_namespace))
                .route(
                    "/iteration/{iteration_id}/pkgbase/{pkgbase}/architecture/{architecture}/status",
                    patch(set_build_status),
                )
                .route(
                    "/iteration/{iteration_id}/pkgbase/{pkgbase}/pkgname/{pkgname}/architecture/{architecture}/package",
                    post(upload_package),
                )
                .route("/assets/{*path}", get(assets::static_handler))
                .nest_service("/repo", ServeDir::new(REPO_DIR.as_path()))
                .layer(TraceLayer::new_for_http())
                .with_state(AppState {
                    worker_sender,
                    jinja_env,
                    db_pool: db_pool.clone(),
                    base_url,
                });

            let mut listenfd = ListenFd::from_env();
            // if listenfd doesn't take a TcpListener (i.e. we're not running via
            // the command above), we fall back to explicitly binding to a given
            // host:port.
            let tcp_listener = if let Some(listener) = listenfd.take_tcp_listener(0).unwrap() {
                listener
            } else {
                let addr = SocketAddr::from((interface, port));
                TcpListener::bind(addr).unwrap()
            };

            axum_server::from_tcp(tcp_listener)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        }
    }
    Ok(())
}
