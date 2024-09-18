use std::net::{SocketAddr, TcpListener};

use anyhow::{Context, Result};
use axum::extract::Path;
use axum::response::Html;
use axum::{
    debug_handler,
    extract::State,
    routing::{get, patch, post},
    Json, Router,
};
use clap::Parser;
use layout::backends::svg::SVGWriter;
use layout::gv::{parser::DotParser, GraphBuilder};
use listenfd::ListenFd;
use minijinja::context;
use petgraph::visit::EdgeRef;
use petgraph::visit::NodeRef;
use petgraph::visit::{Bfs, Walker};
use reqwest::StatusCode;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::args::{Args, Command};
use buildbtw::git::fetch_all_packaging_repositories;
use buildbtw::SetBuildStatusResult::Success;
use buildbtw::{
    worker, BuildNamespace, BuildNextPendingPackageResponse, CreateBuildNamespace, Pkgbase,
    ScheduleBuildResult, SetBuildStatus, SetBuildStatusResult, DATABASE,
};

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
            .ok_or_else(|| {
                println!("No build namespace for id: {namespace_id}");
                StatusCode::NOT_FOUND
            })?
            .clone()
    };

    let latest_packages_to_be_built = &namespace
        .iterations
        .last()
        .ok_or(StatusCode::PROCESSING)?
        .packages_to_be_built;

    let template = state
        .jinja_env
        .get_template("render_build_namespace")
        .unwrap();

    let dot_output = petgraph::dot::Dot::with_attr_getters(
        latest_packages_to_be_built,
        &[petgraph::dot::Config::EdgeNoLabel],
        &|_graph, _edge| "".to_string(),
        &|_graph, node| format!("label={}", node.weight().pkgbase),
    );
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

#[debug_handler]
async fn schedule_build(
    Path(namespace_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Json<ScheduleBuildResult> {
    // TODO: first build scheduled source, like openimageio, then build the rest

    if let Some(namespace) = DATABASE.lock().await.get_mut(&namespace_id) {
        // TODO: may not be computed yet
        let iteration = namespace.iterations.iter_mut().last().unwrap();
        let graph = &mut iteration.packages_to_be_built;

        // Identify root nodes (nodes with no incoming edges)
        let root_nodes: Vec<_> = graph
            .node_indices()
            .filter(|&node| graph.edges_directed(node, petgraph::Incoming).count() == 0)
            .collect();

        // Traverse the graph from each root node using BFS
        let graph_clone = graph.clone();
        for root in root_nodes {
            let bfs = Bfs::new(&graph_clone, root);
            for node_idx in bfs.iter(&graph_clone) {
                let node = &graph[node_idx];
                match node.status {
                    buildbtw::PackageBuildStatus::Built
                    | buildbtw::PackageBuildStatus::Building
                    | buildbtw::PackageBuildStatus::Failed => {
                        continue;
                    }
                    _ => {}
                }

                let mut blocked = false;
                let edges = graph.edges_directed(node_idx, petgraph::Incoming);
                for edge in edges {
                    let target = graph[edge.source()].clone();
                    match target.status {
                        buildbtw::PackageBuildStatus::Pending
                        | buildbtw::PackageBuildStatus::Building
                        | buildbtw::PackageBuildStatus::Failed => {
                            blocked = true;
                            break;
                        }
                        _ => {}
                    }
                }
                if !blocked {
                    let node = &mut graph[node_idx];
                    node.status = buildbtw::PackageBuildStatus::Building;

                    let response = BuildNextPendingPackageResponse {
                        iteration: iteration.id,
                        pkgbase: node.pkgbase.clone(),
                    };
                    return Json(ScheduleBuildResult::Scheduled(response));
                }
            }
        }
    }

    Json(ScheduleBuildResult::NoPendingPackages)
}

#[debug_handler]
async fn set_build_status(
    Path((namespace_id, iteration_id, pkgbase)): Path<(Uuid, Uuid, Pkgbase)>,
    State(state): State<AppState>,
    Json(body): Json<SetBuildStatus>,
) -> Json<SetBuildStatusResult> {
    if let Some(namespace) = DATABASE.lock().await.get_mut(&namespace_id) {
        println!(
            "set package build: {:?} {:?} {:?}",
            namespace, pkgbase, body.status
        );
        let iteration = namespace
            .iterations
            .iter_mut()
            .filter(|i| i.id == iteration_id)
            .next();
        match iteration {
            None => {
                return Json(SetBuildStatusResult::IterationNotFound);
            }
            Some(iteration) => {
                let graph = &mut iteration.packages_to_be_built;

                for node_idx in graph.node_indices() {
                    let node = &mut graph[node_idx];
                    if node.pkgbase == pkgbase {
                        node.status = body.status;

                        return Json(SetBuildStatusResult::Success);
                    }
                }
            }
        }
    }

    Json( SetBuildStatusResult::IterationNotFound)
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
                "render_build_namespace",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/templates/render_build_namespace.jinja"
                )),
            )?;
            let worker_sender = worker::start(port);
            let app = Router::new()
                .route("/namespace", post(generate_build_namespace))
                .route("/namespace/:namespace_id/graph", get(render_build_namespace))
                .route("/namespace/:namespace_id/build", post(schedule_build))
                .route(
                    "/namespace/:namespace_id/iteration/:iteration/pkgbase/:pkgbase",
                    patch(set_build_status),
                )
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
