use std::net::{SocketAddr, TcpListener};

use anyhow::{Context, Result};
use axum::extract::Path;
use axum::response::Html;
use axum::{
    debug_handler,
    extract::State,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use layout::backends::svg::SVGWriter;
use layout::gv::{parser::DotParser, GraphBuilder};
use listenfd::ListenFd;
use minijinja::context;
use reqwest::StatusCode;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::args::{Args, Command};
use buildbtw::git::fetch_all_packaging_repositories;
use buildbtw::worker::{build_global_dependent_graph, build_pkgname_to_srcinfo_map};
use buildbtw::{worker, BuildNamespace, CreateBuildNamespace, DATABASE};

mod args;

#[debug_handler]
async fn generate_build_namespace(
    State(state): State<AppState>,
    Json(body): Json<CreateBuildNamespace>,
) -> Json<BuildNamespace> {
    let namespace = BuildNamespace {
        id: Uuid::new_v4(),
        name: body.name,
        iterations: Vec::new(),
        current_origin_changesets: body.origin_changesets,
    };
    DATABASE
        .lock()
        .await
        .insert(namespace.id, namespace.clone());

    // TODO proper error handling
    state
        .worker_sender
        .send(worker::Message::CalculateBuildNamespace(namespace.id))
        .context("Failed to dispatch worker job")
        .unwrap();

    Json(namespace)
}

#[debug_handler]
async fn render_build_namespace(
    Path(namespace_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespace = {
        let db = DATABASE.lock().await;
        db.get(&namespace_id)
            .expect(&format!("No build namespace for id: {namespace_id}"))
            .clone()
    };

    let pkgname_to_srcinfo_map = build_pkgname_to_srcinfo_map(namespace.clone())
        .await
        .unwrap();
    let global_graph = build_global_dependent_graph(pkgname_to_srcinfo_map)
        .await
        .unwrap();

    let template = state
        .jinja_env
        .get_template("render_build_namespace")
        .unwrap();

    let dot_output = petgraph::dot::Dot::new(&global_graph);
    let mut dot_parser = DotParser::new(&format!("{:?}", dot_output));
    let tree = dot_parser.process();
    let mut gb = GraphBuilder::new();
    let g = tree.unwrap();
    gb.visit_graph(&g);
    let mut vg = gb.get();
    let mut svg = SVGWriter::new();
    vg.do_it(false, false, false, &mut svg);
    let svg_content = svg.finalize();

    let rendered = template
        .render(context! {
            svg => svg_content,
        })
        .unwrap();

    Ok(Html(rendered))
}

#[derive(Clone)]
struct AppState {
    worker_sender: UnboundedSender<worker::Message>,
    jinja_env: minijinja::Environment<'static>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    dbg!(&args);

    match args.command {
        Command::Run { interface, port } => {
            let mut jinja_env = minijinja::Environment::new();
            jinja_env.add_template(
                "layout",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/layout.jinja"
                )),
            )?;
            jinja_env.add_template(
                "build_namespace",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/render_build_namespace.jinja"
                )),
            )?;
            let worker_sender = worker::start();
            let app = Router::new()
                .route("/", post(generate_build_namespace))
                .route("/", get(render_build_namespace))
                .with_state(AppState {
                    worker_sender,
                    jinja_env,
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
        Command::Warmup {} => {
            fetch_all_packaging_repositories().await?;
        }
    }
    Ok(())
}
