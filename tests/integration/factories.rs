use alpm_types::FullVersion;
use buildbtw::{
    buildspace,
    db_fields::TxtUuid,
    dependency_graph::{self, BuildNode},
    entities, input, package, queries,
};
use camino_tempfile::Utf8TempDir;
use color_eyre::Result;
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
};
use uuid::Uuid;

/// Create a user without an OIDC identity
pub async fn user(db: &impl ConnectionTrait, username: &str) -> Result<entities::users::Model> {
    let user = entities::users::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        username: Set(username.to_string()),
    };
    let user = entities::users::Entity::insert(user)
        .exec_with_returning(db)
        .await?;

    Ok(user)
}

/// Create a user with an OIDC identity
///
/// Pretend this user has logged in via OIDC.
pub async fn oidc_user(db: &DatabaseConnection, username: &str) -> Result<entities::users::Model> {
    let create = input::users::ValidatedCreate::try_new(input::users::Create {
        oidc_id: format!("{username}-oidc-id"),
        username: username.to_string(),
    })?;
    let user = queries::users::upsert_with_oidc(db, create, None).await?;

    Ok(user)
}

pub async fn buildspace_with_iteration(
    tx: &DatabaseTransaction,
    name: &str,
) -> Result<(entities::buildspaces::Model, entities::iterations::Model)> {
    let buildspace_slug = buildspace::Slug::try_from(name)?;
    let buildspace = queries::buildspaces::insert(buildspace_slug)
        .exec_with_returning(tx)
        .await?;
    let iteration = queries::iterations::insert(
        buildspace.id.0,
        1,
        Vec::new().into(),
        entities::iterations::NewIterationReason::FirstIteration,
    )
    .exec_with_returning(tx)
    .await?;

    Ok((buildspace, iteration))
}

pub async fn build(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    pkgbase: &str,
) -> Result<entities::builds::Model> {
    let pkgver = "2.1-1".parse()?;
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(
            pkgbase.parse()?,
            format!("{pkgbase}-{pkgver}-any.pkg.tar.zst").parse()?,
        )]
        .iter()
        .cloned()
        .collect(),
        version: pkgver,
    };

    build_from_node(tx, iteration_id, build_node).await
}

/// More flexible, but less convenient way to create a build.
pub async fn build_from_node(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    build_node: BuildNode,
) -> Result<entities::builds::Model> {
    let mut graph = dependency_graph::BuildGraph::new();
    graph.add_node(build_node);

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration_id.into(),
            package::KnownArchitecture::X86_64,
            &graph,
        )?;
    update_iteration.exec(tx).await?;
    let builds = insert_builds.exec_with_returning(tx).await?;
    insert_deps.exec(tx).await?;

    Ok(builds.into_iter().next().unwrap())
}

pub async fn build_with_split_package(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    pkgbase: &str,
) -> Result<entities::builds::Model> {
    let pkgver = "2.1-1".parse()?;
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [
            (
                format!("{pkgbase}-foo").parse()?,
                format!("{pkgbase}-foo-{pkgver}-any.pkg.tar.zst").parse()?,
            ),
            (
                format!("{pkgbase}-bar").parse()?,
                format!("{pkgbase}-bar-{pkgver}-any.pkg.tar.zst").parse()?,
            ),
        ]
        .iter()
        .cloned()
        .collect(),
        version: pkgver,
    };

    let mut graph = dependency_graph::BuildGraph::new();
    graph.add_node(build_node);

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration_id.into(),
            package::KnownArchitecture::X86_64,
            &graph,
        )?;
    update_iteration.exec(tx).await?;
    let builds = insert_builds.exec_with_returning(tx).await?;
    insert_deps.exec(tx).await?;

    Ok(builds.into_iter().next().unwrap())
}

pub async fn package(
    data_dir: &Utf8TempDir,
    pkgbase: &str,
    pkgname: &str,
    pkgver: &FullVersion,
) -> Result<alpm_package::Package> {
    let tmp_storage = buildbtw::storage::data_tmp_dir(&Some(data_dir.path().to_path_buf()))?;
    tokio::fs::create_dir_all(&tmp_storage).await?;

    let input_dir = tmp_storage.join("input");
    tokio::fs::create_dir_all(&input_dir).await?;
    let input_dir = alpm_package::InputDir::new(input_dir.into())?;

    let output_dir = tmp_storage.join("output");
    tokio::fs::create_dir_all(&output_dir).await?;
    let output_dir = alpm_package::OutputDir::new(output_dir.into())?;

    // Create a valid, but minimal BUILDINFOv2 file.
    tokio::fs::write(
        input_dir.join(alpm_types::MetadataFileName::BuildInfo.as_ref()),
        format!(
            r"
format = 2
builddate = 1
builddir = /build
startdir = /startdir/
buildtool = devtools
buildtoolver = 1:1.2.1-1-any
packager = John Doe <john@example.org>
pkgarch = any
pkgbase = {pkgbase}
pkgbuild_sha256sum = b5bb9d8014a0f9b1d61e21e796d78dccdf1352f23cd32812f4850b878ae4944c
pkgname = {pkgname}
pkgver = {pkgver}
",
        ),
    )
    .await?;

    // Create a valid, but minimal PKGINFOv2 file.
    tokio::fs::write(
        input_dir.join(alpm_types::MetadataFileName::PackageInfo.as_ref()),
        format!(
            r"
pkgname = {pkgname}
pkgbase = {pkgbase}
xdata = pkgtype=pkg
pkgver = {pkgver}
pkgdesc = A project that returns true
url = https://example.org/
builddate = 1
packager = John Doe <john@example.org>
size = 181849963
arch = any
license = GPL-3.0-or-later
",
        ),
    )
    .await?;

    // Create a valid ALPM-MTREEv2 file from the input directory.
    alpm_mtree::create_mtree_v2_from_input_dir(&input_dir)?;

    // Create PackageInput and PackageCreationConfig.
    let package_input: alpm_package::PackageInput = input_dir.try_into()?;
    let config = alpm_package::PackageCreationConfig::new(
        package_input,
        output_dir,
        alpm_compress::compression::CompressionSettings::default(),
    )?;

    // Create package file.
    let package = alpm_package::Package::try_from(&config)?;
    Ok(package)
}
