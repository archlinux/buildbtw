use anyhow::Result;
use axum::extract::{Path, Request};
use axum::response::Html;
use axum::{debug_handler, extract::State, Json};
use buildbtw::build_set_graph::calculate_packages_to_be_built;
use buildbtw::pacman_repo::{add_to_repo, repo_dir_path};
use buildbtw::source_info::{package_file_name, package_for_architecture, ConcreteArchitecture};
use layout::backends::svg::SVGWriter;
use layout::gv::{parser::DotParser, GraphBuilder};
use minijinja::context;
use petgraph::visit::EdgeRef;
use petgraph::visit::NodeRef;
use reqwest::StatusCode;
use tokio::fs;
use uuid::Uuid;

use crate::db::iteration::BuildSetIterationUpdate;
use crate::response_error::ResponseError::{self};
use crate::response_error::ResponseResult;
use crate::{db, stream_to_file::stream_to_file, AppState};
use buildbtw::{
    BuildNamespace, BuildSetIteration, CreateBuildNamespace, Pkgbase, Pkgname, SetBuildStatus,
    UpdateBuildNamespace,
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
        "Namespace overview available at: {base_url}/namespace/{}",
        namespace.name,
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
            base_url => state.base_url
        })
        .unwrap();

    Ok(Html(rendered))
}

#[debug_handler]
pub(crate) async fn render_build_namespace_graph(
    Path((namespace_name, architecture)): Path<(String, ConcreteArchitecture)>,
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
        .packages_to_be_built
        .get(&architecture)
        .ok_or(StatusCode::NOT_FOUND)?;

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
        namespace_id: namespace.id,
    };

    db::iteration::create(&state.db_pool, new_iteration.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::debug!(r#"Updated build namespace "{namespace_name}": {body:?}"#);

    Ok(Json(new_iteration))
}

pub async fn upload_package(
    Path((iteration_id, pkgbase, pkgname, architecture)): Path<(
        Uuid,
        Pkgbase,
        Pkgname,
        ConcreteArchitecture,
    )>,
    State(state): State<AppState>,
    request: Request,
) -> ResponseResult<()> {
    // Read version info from the database
    // And verify that pkgbase, pkgname and architecture actually exist
    // in the given iteration
    let iteration = db::iteration::read(&state.db_pool, iteration_id).await?;
    let namespace = db::namespace::read(iteration.namespace_id, &state.db_pool).await?;

    let graph = iteration
        .packages_to_be_built
        .get(&architecture)
        .ok_or(ResponseError::NotFound("architecture"))?;

    let node = &graph
        .raw_nodes()
        .iter()
        .find(|node| node.weight.pkgbase == pkgbase)
        .ok_or(ResponseError::NotFound("pkgbase"))?
        .weight;

    let package = package_for_architecture(&node.srcinfo, architecture, &pkgname)
        .ok_or(ResponseError::NotFound("pkgname"))?;

    // Calculate path for writing the file
    // This should only use safe inputs such as those read from the DB,
    // or enums like `ConcreteArchitecture`
    let repo_path = repo_dir_path(&namespace.name, iteration.id, architecture);
    fs::create_dir_all(&repo_path).await?;

    // TODO this is probably paranoid, but I think a version like `../../../../../etc/passwd` might actually be valid
    // An attack like that would require a malicious .SRCINFO, though
    let path = repo_path.join(package_file_name(&package));
    if tokio::fs::try_exists(&path).await? {
        // This should only happen if a builder was temporarily unreachable
        // so the build got scheduled elsewhere as well
        // We assume that written files are correct, so we can ignore this
        return Ok(());
    }
    // TODO ensure no package exists for the given build yet
    stream_to_file(&path, request.into_body().into_data_stream()).await?;

    add_to_repo(&namespace.name, iteration.id, architecture, &package).await?;

    Ok(())
}

pub async fn set_build_status(
    Path((iteration_id, pkgbase, architecture)): Path<(Uuid, Pkgbase, ConcreteArchitecture)>,
    State(state): State<AppState>,
    Json(body): Json<SetBuildStatus>,
) -> ResponseResult<()> {
    tracing::info!(
        "setting build status: iteration: {:?} pkgbase: {:?} status: {:?}",
        iteration_id,
        pkgbase,
        body.status
    );
    let iteration = db::iteration::read(&state.db_pool, iteration_id).await?;

    let iteration = iteration.set_build_status(architecture, pkgbase, body.status)?;
    let update = BuildSetIterationUpdate {
        id: iteration.id,
        packages_to_be_built: iteration.packages_to_be_built,
    };

    db::iteration::update(&state.db_pool, update).await?;

    Ok(())
}
