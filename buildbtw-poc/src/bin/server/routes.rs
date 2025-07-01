use axum::{
    Json, debug_handler,
    extract::{Path, Request, State},
    response::Html,
};
use color_eyre::eyre::{OptionExt, Result, WrapErr};
use layout::backends::svg::SVGWriter;
use layout::gv::{GraphBuilder, parser::DotParser};
use minijinja::context;
use petgraph::visit::{EdgeRef, NodeRef};
use reqwest::StatusCode;
use serde::Serialize;
use time::macros::format_description;
use tokio::fs;
use uuid::Uuid;

use buildbtw_poc::pacman_repo::{add_to_repo, repo_dir_path};
use buildbtw_poc::source_info::{
    ConcreteArchitecture, package_file_name, package_for_architecture,
};
use buildbtw_poc::{
    BuildNamespace, BuildSetIteration, CreateBuildNamespace, PackageBuildStatus, Pkgbase, Pkgname,
    SetBuildStatus, UpdateBuildNamespace,
};
use buildbtw_poc::{
    BuildNamespaceStatus,
    build_set_graph::{BuildPackageNode, BuildSetGraph, calculate_packages_to_be_built},
};

use crate::db::iteration::BuildSetIterationUpdate;
use crate::db::namespace::CreateDbBuildNamespace;
use crate::response_error::ResponseError::{self};
use crate::response_error::ResponseResult;
use crate::{AppState, db, stream_to_file::stream_to_file};

#[debug_handler]
pub(crate) async fn create_build_namespace(
    State(state): State<AppState>,
    Json(body): Json<CreateBuildNamespace>,
) -> Result<Json<BuildNamespace>, ResponseError> {
    let name = body.name.unwrap_or(
        body.origin_changesets
            .first()
            .ok_or_eyre("Cannot create a build namespace without origin changesets")?
            .0
            .to_string(),
    );
    let create = CreateDbBuildNamespace {
        name,
        origin_changesets: body.origin_changesets,
    };
    let namespace = db::namespace::create(create, &state.db_pool).await?;

    let base_url = state
        .base_url
        .join(&format!("/namespace/{}", namespace.name))
        .wrap_err("Failed to parse URL")?;
    tracing::info!("Namespace overview available at: {base_url}",);

    Ok(Json(namespace))
}

#[derive(Serialize)]
struct RunningBuildsEntry {
    gitlab_pipeline_url: Option<String>,
    pkgbase: Pkgbase,
    namespace_name: String,
}

pub(crate) async fn home_html(State(state): State<AppState>) -> ResponseResult<Html<String>> {
    let namespaces = db::namespace::list(&state.db_pool).await?;
    let (active_namespaces, cancelled_namespaces): (Vec<_>, Vec<_>) =
        namespaces.into_iter().partition(|ns| match ns.status {
            BuildNamespaceStatus::Active => true,
            BuildNamespaceStatus::Cancelled => false,
        });

    let mut running_builds_table: Vec<RunningBuildsEntry> = Vec::new();
    // Include cancelled namespaces here because they can contain leftover
    // running builds as well
    for namespace in db::namespace::list(&state.db_pool).await? {
        let latest_iteration =
            if let Ok(i) = db::iteration::read_newest(&state.db_pool, namespace.id).await {
                i
            } else {
                continue;
            };

        for (architecture, graph) in latest_iteration.packages_to_be_built {
            for node in graph.node_weights() {
                // Only check nodes that are currently building.
                if node.status != PackageBuildStatus::Building {
                    continue;
                }

                // Check if there's a gitlab pipeline we started
                // If yes, we'll find it in the DB
                let maybe_pipeline =
                    db::gitlab_pipeline::read_by_iteration_and_pkgbase_and_architecture(
                        &state.db_pool,
                        latest_iteration.id,
                        &node.pkgbase,
                        architecture,
                    )
                    .await?
                    .map(|pipeline| pipeline.gitlab_url);

                running_builds_table.push(RunningBuildsEntry {
                    gitlab_pipeline_url: maybe_pipeline,
                    pkgbase: node.pkgbase.clone(),
                    namespace_name: namespace.name.clone(),
                });
            }
        }
    }

    let template = state.jinja_env.get_template("home").unwrap();

    let rendered = template
        .render(context! {
            active_namespaces => active_namespaces,
            cancelled_namespaces => cancelled_namespaces,
            running_builds_table => running_builds_table
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
pub(crate) async fn render_latest_namespace(
    State(state): State<AppState>,
) -> Result<Html<String>, ResponseError> {
    let namespace = db::namespace::read_latest(&state.db_pool).await?;

    show_build_namespace_iteration_architecture_html(
        Path((namespace.name, None, None)),
        State(state),
    )
    .await
}

pub(crate) async fn show_build_namespace_html(
    Path(namespace_name): Path<String>,
    state: State<AppState>,
) -> Result<Html<String>, ResponseError> {
    show_build_namespace_iteration_architecture_html(Path((namespace_name, None, None)), state)
        .await
}

pub(crate) async fn show_build_namespace_json(
    Path(namespace_name): Path<String>,
    state: State<AppState>,
) -> Result<Json<Option<(Uuid, BuildSetGraph)>>, ResponseError> {
    show_build_namespace_iteration_architecture_json(Path((namespace_name, None, None)), state)
        .await
}

pub(crate) async fn show_build_namespace_iteration_html(
    Path((namespace_name, iteration_id)): Path<(String, Option<Uuid>)>,
    state: State<AppState>,
) -> Result<Html<String>, ResponseError> {
    show_build_namespace_iteration_architecture_html(
        Path((namespace_name, iteration_id, None)),
        state,
    )
    .await
}

pub(crate) async fn show_build_namespace_iteration_json(
    Path((namespace_name, iteration_id)): Path<(String, Option<Uuid>)>,
    state: State<AppState>,
) -> Result<Json<Option<(Uuid, BuildSetGraph)>>, ResponseError> {
    show_build_namespace_iteration_architecture_json(
        Path((namespace_name, iteration_id, None)),
        state,
    )
    .await
}

#[derive(Serialize)]
struct PipelineTableEntry {
    status_icon: String,
    status_description: String,
    status: PackageBuildStatus,
    gitlab_url: Option<String>,
    pkgbase: Pkgbase,
}

impl PipelineTableEntry {
    fn from_build_package_node(node: &BuildPackageNode, gitlab_url: Option<String>) -> Self {
        PipelineTableEntry {
            status_icon: node.status.as_icon().to_string(),
            status_description: node.status.as_description(),
            gitlab_url,
            pkgbase: node.pkgbase.clone(),
            status: node.status,
        }
    }
}

#[derive(Serialize)]
struct IterationTableEntry {
    id: Uuid,
    created_at: String,
    create_reason: &'static str,
}

const FORMAT: &[time::format_description::BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]");

impl IterationTableEntry {
    fn from_iteration(iteration: &BuildSetIteration) -> Self {
        IterationTableEntry {
            id: iteration.id,
            created_at: iteration.created_at.format(FORMAT).unwrap(),
            create_reason: iteration.create_reason.short_description(),
        }
    }
}

#[derive(Serialize)]
struct IterationView {
    id: Uuid,
    created_at: String,
    architectures: Vec<ConcreteArchitecture>,
    create_reason: &'static str,
}

impl IterationView {
    fn from_iteration(iteration: &BuildSetIteration) -> Result<Self> {
        Ok(IterationView {
            id: iteration.id,
            created_at: iteration.created_at.format(FORMAT)?,
            architectures: iteration.packages_to_be_built.keys().cloned().collect(),
            create_reason: iteration.create_reason.short_description(),
        })
    }
}

fn default_architecture_for_namespace(
    architecture: Option<ConcreteArchitecture>,
    current_iteration: Option<&BuildSetIteration>,
) -> (Option<ConcreteArchitecture>, Option<&BuildSetGraph>) {
    let current_iteration = if let Some(iteration) = current_iteration {
        iteration
    } else {
        return (architecture, None);
    };

    if let Some(architecture) = architecture {
        return (
            Some(architecture),
            current_iteration.packages_to_be_built.get(&architecture),
        );
    }
    // Try x86_64 as a default
    if let Some(graph) = current_iteration
        .packages_to_be_built
        .get(&ConcreteArchitecture::X86_64)
    {
        (Some(ConcreteArchitecture::X86_64), Some(graph))
    } else {
        // Otherwise, just use the first available architecture.
        current_iteration
            .packages_to_be_built
            .iter()
            .next()
            .map(|(arch, graph)| (Some(*arch), Some(graph)))
            .unwrap_or((architecture, None))
    }
}

#[debug_handler]
pub(crate) async fn show_build_namespace_iteration_architecture_html(
    Path((namespace_name, iteration_id, architecture)): Path<(
        String,
        Option<Uuid>,
        Option<ConcreteArchitecture>,
    )>,
    State(state): State<AppState>,
) -> Result<Html<String>, ResponseError> {
    let namespace = db::namespace::read_by_name(&namespace_name, &state.db_pool).await?;
    let iterations = db::iteration::list_for_namespace(&state.db_pool, namespace.id).await?;

    let mut pipeline_table = None;
    let current_iteration = if let Some(id) = iteration_id {
        Some(db::iteration::read(&state.db_pool, id).await?)
    } else {
        iterations.last().cloned()
    };
    let iteration_table: Vec<_> = iterations
        .iter()
        .map(IterationTableEntry::from_iteration)
        .collect();
    // If no architecture was specified, take a default one from the current iteration.
    let (architecture, build_graph) =
        default_architecture_for_namespace(architecture, current_iteration.as_ref());

    if let (Some(current_iteration), Some(architecture), Some(build_graph)) =
        (&current_iteration, architecture, build_graph)
    {
        let mut table_entries = Vec::new();
        for node in build_graph.node_weights() {
            // Many small queries are efficient in sqlite:
            // https://sqlite.org/np1queryprob.html
            let gitlab_url = db::gitlab_pipeline::read_by_iteration_and_pkgbase_and_architecture(
                &state.db_pool,
                current_iteration.id,
                &node.pkgbase,
                architecture,
            )
            .await?
            .map(|p| p.gitlab_url);
            table_entries.push(PipelineTableEntry::from_build_package_node(
                node, gitlab_url,
            ));
        }

        table_entries.sort_by_key(|entry| match entry.status {
            PackageBuildStatus::Scheduled => 0,
            PackageBuildStatus::Building => 1,
            PackageBuildStatus::Failed => 2,
            PackageBuildStatus::Built => 3,
            PackageBuildStatus::Blocked => 4,
            PackageBuildStatus::Pending => 5,
        });

        pipeline_table = Some(table_entries);
    }

    let template = state
        .jinja_env
        .get_template("show_build_namespace")
        .unwrap();

    let rendered = template
        .render(context! {
            namespace => namespace,
            iteration_table => iteration_table,
            current_iteration => current_iteration.as_ref().map(IterationView::from_iteration).transpose()?,
            pipeline_table => pipeline_table,
            base_url => state.base_url,
            architecture => architecture,
        })
        .unwrap();

    Ok(Html(rendered))
}

pub(crate) async fn show_build_namespace_iteration_architecture_json(
    Path((namespace_name, iteration_id, architecture)): Path<(
        String,
        Option<Uuid>,
        Option<ConcreteArchitecture>,
    )>,
    State(state): State<AppState>,
) -> ResponseResult<Json<Option<(Uuid, BuildSetGraph)>>> {
    let namespace = db::namespace::read_by_name(&namespace_name, &state.db_pool).await?;
    let iterations = db::iteration::list_for_namespace(&state.db_pool, namespace.id).await?;

    let current_iteration = match iteration_id {
        Some(id) => Some(db::iteration::read(&state.db_pool, id).await?),
        None => iterations.last().cloned(),
    };

    let current_iteration = match current_iteration {
        Some(it) => it,
        None => return Ok(Json(None)),
    };

    let (_, build_graph) =
        default_architecture_for_namespace(architecture, Some(&current_iteration));

    let build_graph = build_graph.ok_or(ResponseError::NotFound("architecture"))?;

    Ok(Json(Some((current_iteration.id, build_graph.clone()))))
}

#[debug_handler]
pub(crate) async fn render_build_namespace_graph(
    Path((_namespace_name, iteration_id, architecture)): Path<(String, Uuid, ConcreteArchitecture)>,
    State(state): State<AppState>,
) -> ResponseResult<Html<String>> {
    let iteration = db::iteration::read(&state.db_pool, iteration_id).await?;

    let latest_packages_to_be_built = iteration
        .packages_to_be_built
        .get(&architecture)
        .ok_or(ResponseError::NotFound("Build Graph"))?;

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
    let mut dot_parser = DotParser::new(&format!("{dot_output:?}"));
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
        created_at: time::OffsetDateTime::now_utc(),
        origin_changesets: namespace.current_origin_changesets.clone(),
        packages_to_be_built: calculate_packages_to_be_built(&namespace)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        create_reason: buildbtw_poc::iteration::NewIterationReason::CreatedByUser,
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
    let path = repo_path.join(package_file_name(&package, &node.srcinfo)?);
    if tokio::fs::try_exists(&path).await? {
        // This should only happen if a builder was temporarily unreachable
        // so the build got scheduled elsewhere as well
        // We assume that written files are correct, so we can ignore this
        return Ok(());
    }
    // TODO ensure no package exists for the given build yet
    stream_to_file(&path, request.into_body().into_data_stream()).await?;

    add_to_repo(&repo_path, &package, &node.srcinfo).await?;

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
