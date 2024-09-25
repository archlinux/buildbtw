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
use buildbtw::{
    BuildNamespace, BuildNextPendingPackageResponse, CreateBuildNamespace, Pkgbase,
    ScheduleBuildResult, SetBuildStatus, SetBuildStatusResult, DATABASE,
};

mod args;
mod tasks;

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
        .send(tasks::Message::CreateBuildNamespace(namespace.id))
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
        &|_graph, node| {
            let color = match node.weight().status {
                buildbtw::PackageBuildStatus::Pending => "black",
                buildbtw::PackageBuildStatus::Building => "orange",
                buildbtw::PackageBuildStatus::Built => "green",
                buildbtw::PackageBuildStatus::Failed => "red",
            };
            let build_status = match node.weight().status {
                buildbtw::PackageBuildStatus::Pending => "pending",
                buildbtw::PackageBuildStatus::Building => "building",
                buildbtw::PackageBuildStatus::Built => "built",
                buildbtw::PackageBuildStatus::Failed => "failed",
            };
            let pkgbase = &node.weight().pkgbase;
            format!("label=\"{pkgbase}\n({build_status})\",color={color}")
        },
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

async fn schedule_next_build_in_graph(namespace_id: Uuid) -> ScheduleBuildResult {
    // TODO: first build scheduled source, like openimageio, then build the rest

    // if we only meet built nodes, we are finished
    let mut status = ScheduleBuildResult::Finished;

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
        // TODO build things in parallel where possible
        let graph_clone = graph.clone();
        for root in root_nodes {
            let bfs = Bfs::new(&graph_clone, root);
            for node_idx in bfs.iter(&graph_clone) {
                let node = &graph[node_idx];
                match node.status {
                    buildbtw::PackageBuildStatus::Built => {
                        continue;
                    }
                    buildbtw::PackageBuildStatus::Building
                    | buildbtw::PackageBuildStatus::Failed => {
                        // We are blocked by a package that is currently building or has failed
                        status = ScheduleBuildResult::NoPendingPackages;
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
                            // Break out of the incoming edge loop representing dependencies and
                            // continue with the next node
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
                        gitref: node.commit_hash.clone(),
                    };
                    return ScheduleBuildResult::Scheduled(response);
                }
            }
        }
    }

    status
}

#[debug_handler]
async fn set_build_status(
    Path((namespace_id, iteration_id, pkgbase)): Path<(Uuid, Uuid, Pkgbase)>,
    Json(body): Json<SetBuildStatus>,
) -> Json<SetBuildStatusResult> {
    println!(
        "set package build: namespace: {:?} iteration: {:?} pkgbase: {:?} status: {:?}",
        namespace_id, iteration_id, pkgbase, body.status
    );

    if let Some(namespace) = DATABASE.lock().await.get_mut(&namespace_id) {
        let iteration = namespace
            .iterations
            .iter_mut()
            .find(|i| i.id == iteration_id);
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

    Json(SetBuildStatusResult::IterationNotFound)
}

#[derive(Clone)]
struct AppState {
    worker_sender: UnboundedSender<tasks::Message>,
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
            let worker_sender = tasks::start(port);
            let app = Router::new()
                .route("/namespace", post(generate_build_namespace))
                .route(
                    "/namespace/:namespace_id/graph",
                    get(render_build_namespace),
                )
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
