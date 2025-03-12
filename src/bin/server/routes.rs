use anyhow::Result;
use axum::extract::Path;
use axum::response::Html;
use axum::{debug_handler, extract::State, Json};
use buildbtw::build_set_graph::calculate_packages_to_be_built;
use layout::backends::svg::SVGWriter;
use layout::gv::{parser::DotParser, GraphBuilder};
use minijinja::context;
use petgraph::visit::EdgeRef;
use petgraph::visit::NodeRef;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::db::iteration::BuildSetIterationUpdate;
use crate::{db, AppState};
use buildbtw::{
    build_set_graph, BuildNamespace, BuildSetIteration, CreateBuildNamespace, Pkgbase,
    SetBuildStatus, SetBuildStatusResult, UpdateBuildNamespace,
};

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
            tracing::info!("{e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let base_url = state.base_url;
    tracing::info!(
        "Namespace overview available at: {base_url}/namespace/{}/graph",
        namespace.name
    );

    Ok(Json(namespace))
}

/// For debugging: List all existing namespaces.
pub(crate) async fn list_namespaces_html(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespaces = db::namespace::list(&state.db_pool).await.map_err(|e| {
        tracing::info!("{e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let template = state
        .jinja_env
        .get_template("list_build_namespaces")
        .unwrap();

    let rendered = template
        .render(context! {
            namespaces => namespaces,
        })
        .unwrap();

    Ok(Html(rendered))
}

pub(crate) async fn list_namespaces_json(
    State(state): State<AppState>,
) -> Result<Json<Vec<BuildNamespace>>, StatusCode> {
    let namespaces = db::namespace::list(&state.db_pool).await.map_err(|e| {
        tracing::info!("{e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(namespaces))
}

/// For debugging: Render the newest build namespace, regardless of its ID.
#[debug_handler]
pub(crate) async fn render_latest_namespace(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespace = db::namespace::read_latest(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::info!("{e:?}");
            StatusCode::NOT_FOUND
        })?;

    show_build_namespace(Path(namespace.name), State(state)).await
}

#[debug_handler]
pub(crate) async fn show_build_namespace(
    Path(namespace_name): Path<String>,
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespace = db::namespace::read_by_name(&namespace_name, &state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let iterations = db::iteration::list(&state.db_pool, namespace.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let template = state
        .jinja_env
        .get_template("show_build_namespace")
        .unwrap();

    let rendered = template
        .render(context! {
            namespace => namespace,
            iterations => iterations,
        })
        .unwrap();

    Ok(Html(rendered))
}

#[debug_handler]
pub(crate) async fn render_build_namespace_graph(
    Path(namespace_name): Path<String>,
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let namespace = db::namespace::read_by_name(&namespace_name, &state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let iterations = db::iteration::list(&state.db_pool, namespace.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let latest_packages_to_be_built = &iterations
        .last()
        .ok_or(StatusCode::PROCESSING)?
        .packages_to_be_built;

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

    let template = state
        .jinja_env
        .get_template("render_build_namespace_graph")
        .unwrap();

    let rendered = template
        .render(context! {
            svg => svg_content,
        })
        .unwrap();

    Ok(Html(rendered))
}

pub async fn update_namespace(
    Path(namespace_name): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<UpdateBuildNamespace>,
) -> Result<(), StatusCode> {
    db::namespace::update(&state.db_pool, &namespace_name, body.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::debug!(r#"Updated build namespace "{namespace_name}": {body:?}"#);

    Ok(())
}

pub async fn create_namespace_iteration(
    Path(namespace_name): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<()>,
) -> Result<Json<BuildSetIteration>, StatusCode> {
    let namespace = db::namespace::read_by_name(&namespace_name, &state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let new_iteration = BuildSetIteration {
        id: Uuid::new_v4(),
        origin_changesets: namespace.current_origin_changesets.clone(),
        packages_to_be_built: calculate_packages_to_be_built(&namespace)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        create_reason: buildbtw::iteration::NewIterationReason::CreatedByUser,
    };

    db::iteration::create(&state.db_pool, namespace.id, new_iteration.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::debug!(r#"Updated build namespace "{namespace_name}": {body:?}"#);

    Ok(Json(new_iteration))
}

#[debug_handler]
pub async fn set_build_status(
    Path((namespace_id, iteration_id, pkgbase)): Path<(Uuid, Uuid, Pkgbase)>,
    State(state): State<AppState>,
    Json(body): Json<SetBuildStatus>,
) -> Json<SetBuildStatusResult> {
    tracing::info!(
        "setting build status: namespace: {:?} iteration: {:?} pkgbase: {:?} status: {:?}",
        namespace_id,
        iteration_id,
        pkgbase,
        body.status
    );

    // TODO proper error handling
    if let Ok(iterations) = db::iteration::list(&state.db_pool, namespace_id).await {
        let iteration = iterations.into_iter().find(|i| i.id == iteration_id);
        match iteration {
            None => {
                return Json(SetBuildStatusResult::IterationNotFound);
            }
            Some(iteration) => {
                let new_graph = build_set_graph::set_build_status(
                    iteration.packages_to_be_built,
                    &pkgbase,
                    body.status,
                );
                let update = BuildSetIterationUpdate {
                    id: iteration.id,
                    packages_to_be_built: new_graph,
                };

                if let Err(e) = db::iteration::update(&state.db_pool, update).await {
                    tracing::info!("{e:?}");
                    return Json(SetBuildStatusResult::InternalError);
                };

                return Json(SetBuildStatusResult::Success);
            }
        }
    }

    Json(SetBuildStatusResult::IterationNotFound)
}
