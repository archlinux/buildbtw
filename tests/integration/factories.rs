use buildbtw::{
    buildspace::BuildspaceSlug, db_fields::TxtUuid, dependency_graph::BuildNode, entities, package,
    queries,
};
use color_eyre::Result;
use sea_orm::DatabaseTransaction;

pub async fn buildspace_with_iteration(
    tx: &DatabaseTransaction,
    name: &str,
) -> Result<(entities::buildspaces::Model, entities::iterations::Model)> {
    let buildspace_slug = BuildspaceSlug::try_from(name)?;
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
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(pkgbase.parse()?, "dummy.tar.gz".parse()?)]
            .iter()
            .cloned()
            .collect(),
        version: "2.1-0".parse()?,
    };

    Ok(queries::builds::insert(
        build_node,
        package::KnownArchitecture::X86_64,
        iteration_id.into(),
    )
    .exec_with_returning(tx)
    .await?)
}
