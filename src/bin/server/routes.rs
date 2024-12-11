use anyhow::{Context, Result};
use axum::extract::Path;
use axum::response::Html;
use axum::{debug_handler, extract::State, Json};
use layout::backends::svg::SVGWriter;
use layout::gv::{parser::DotParser, GraphBuilder};
use minijinja::context;
use petgraph::visit::EdgeRef;
use petgraph::visit::NodeRef;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{db, tasks, AppState};
use buildbtw::{BuildNamespace, CreateBuildNamespace};

#[debug_handler]
pub(crate) async fn generate_build_namespace(
    State(state): State<AppState>,
    Json(body): Json<CreateBuildNamespace>,
) -> Result<Json<BuildNamespace>, StatusCode> {
    let create = CreateBuildNamespace {
        name: body.name,
        origin_changesets: body.origin_changesets,
    };
    let namespace = db::namespace::create(create, &state.db_pool)
        .await
        .map_err(|e| {
            println!("{e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // TODO proper error handling
    state
        .worker_sender
        .send(tasks::Message::BuildNamespaceCreated(namespace.id))
        .context("Failed to dispatch worker job")
        .unwrap();

    Ok(Json(namespace))
}

/// For debugging: Render the newest build namespace, regardless of its ID.
#[debug_handler]
pub(crate) async fn render_latest_namespace(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespace = db::namespace::read_latest(&state.db_pool)
        .await
        .map_err(|e| {
            println!("{e:?}");
            StatusCode::NOT_FOUND
        })?;

    render_build_namespace(Path(namespace.id), State(state)).await
}

#[debug_handler]
pub(crate) async fn render_build_namespace(
    Path(namespace_id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespace = db::namespace::read(namespace_id, &state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let iterations = db::iteration::list(&state.db_pool, namespace_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let latest_packages_to_be_built = &iterations
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
        &|graph, edge| {
            let color = graph[edge.source()].status.as_color();
            format!("color=\"{color}\"")
        },
        &|_graph, node| {
            let color = node.weight().status.as_color();
            let build_status = node.weight().status.as_icon();
            let pkgbase = &node.weight().pkgbase;
            format!("label=\"{pkgbase}\n{build_status}\",color=\"{color}\"")
        },
    );
    let mut dot_parser = DotParser::new(&format!("{:?}", dot_output));
    let tree = dot_parser.process();
    let mut graph_builder = GraphBuilder::new();
    let graph = tree.unwrap();
    graph_builder.visit_graph(&graph);
    let mut visual_graph = graph_builder.get();
    let mut svg = SVGWriter::new();
    visual_graph.do_it(false, false, false, &mut svg);
    let svg_content = svg.finalize();

    let rendered = template
        .render(context! {
            svg => svg_content,
            namespace => namespace,
            iterations => iterations,
        })
        .unwrap();

    Ok(Html(rendered))
}
