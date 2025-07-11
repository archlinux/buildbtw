use std::net::{SocketAddr, TcpListener};

use axum::{
    Router,
    response::Redirect,
    routing::{get, patch, post},
};
use axum_extra::handler::HandlerCallWithExtractors;
use clap::Parser;
use color_eyre::Result;
use listenfd::ListenFd;
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tower_http::{services::ServeDir, trace::TraceLayer};
use url::Url;
use with_content_type::{ApplicationJson, with_content_type};

use crate::routes::{
    create_build_namespace, create_namespace_iteration, home_html, list_namespaces_json,
    render_build_namespace_graph, render_latest_namespace, set_build_status,
    show_build_namespace_html, show_build_namespace_iteration_architecture_json,
    show_build_namespace_iteration_json, show_build_namespace_json, update_namespace,
    upload_package,
};
use crate::{
    args::{Args, Command},
    routes::{
        show_build_namespace_iteration_architecture_html, show_build_namespace_iteration_html,
    },
};
use buildbtw_poc::pacman_repo::REPO_DIR;

mod args;
pub mod assets;
mod db;
pub mod response_error;
mod routes;
pub mod stream_to_file;
mod tasks;
pub mod templates;
pub mod with_content_type;

#[derive(Clone)]
struct AppState {
    #[allow(dead_code)]
    worker_sender: UnboundedSender<tasks::Message>,
    jinja_env: minijinja::Environment<'static>,
    db_pool: SqlitePool,
    base_url: Url,
    gitlab_args: Option<args::Gitlab>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // log warnings by default
    buildbtw_poc::tracing::init(args.verbose + 1, true);
    color_eyre::install()?;

    tracing::debug!("{args:#?}");

    match args.command {
        Command::Run {
            interface,
            port,
            base_url,
        } => {
            let mut jinja_env = minijinja::Environment::new();
            templates::add_to_jinja_env(&mut jinja_env)?;
            let db_pool: sqlx::Pool<sqlx::Sqlite> =
                db::create_and_connect_db(&args.database_url).await?;

            sqlx::migrate!("./migrations").run(&db_pool).await?;

            let worker_sender = tasks::start(db_pool.clone(), args.gitlab.clone(), port).await?;
            let app = Router::new()
                .route("/", get(|| async {Redirect::to("/namespace")}))
                .route(
                    "/namespace",
                    post(create_build_namespace).get(
                        with_content_type::<ApplicationJson, _>(list_namespaces_json)
                            .or(home_html),
                    ),
                )
                .route(
                    "/namespace/{name}/iteration",
                    post(create_namespace_iteration),
                )
                .route("/namespace/{name}", get(with_content_type::<ApplicationJson, _>(show_build_namespace_json).or(show_build_namespace_html)))
                .route("/namespace/{name}/{iteration}", get(with_content_type::<ApplicationJson, _>(show_build_namespace_iteration_json).or(show_build_namespace_iteration_html)))
                .route("/namespace/{name}/{iteration}/{architecture}", get(with_content_type::<ApplicationJson, _>(show_build_namespace_iteration_architecture_json).or(show_build_namespace_iteration_architecture_html)))
                .route(
                    "/namespace/{name}/{iteration_id}/{architecture}/graph",
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
                    gitlab_args: args.gitlab
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
