use buildbtw_poc::{
    BuildSetIteration, build_set_graph::calculate_packages_to_be_built, source_repos::SourceRepos,
};
use color_eyre::{Result, eyre::eyre};
use uuid::Uuid;

use crate::{
    db::{self, namespace::CreateDbBuildNamespace},
    response_error::ResponseError,
};

const PACKAGES: &[&str] = &[
    "llvm",
    "eza",
    "zenity",
    "nsxiv",
    "sxiv",
    "miniserve",
    "openimageio",
    "poppler",
    "boost",
    "clang",
    "linux",
    "thunderbird",
    "apt",
    "abseil-cpp",
];

pub async fn seed(pool: &sqlx::SqlitePool) -> Result<()> {
    match db::namespace::read_latest(pool).await {
        Ok(_) => {
            return Err(eyre!(
                "Database contains namespaces. Please wipe your database and re-run this script."
            ));
        }
        Err(ResponseError::NotFound(_)) => {}
        Err(e) => return Err(e.into()),
    }
    let mut source_repos = SourceRepos::new().await?;

    for pkgbase in PACKAGES {
        let namespace = db::namespace::create(
            CreateDbBuildNamespace {
                name: pkgbase.to_string(),
                origin_changesets: vec![(pkgbase.to_string().into(), "main".to_string())],
            },
            pool,
        )
        .await?;
        let packages_to_be_built =
            calculate_packages_to_be_built(&namespace, &mut source_repos).await?;

        for _ in 0..50 {
            let new_iteration = BuildSetIteration {
                id: Uuid::new_v4(),
                created_at: time::OffsetDateTime::now_utc(),
                origin_changesets: namespace.current_origin_changesets.clone(),
                packages_to_be_built: packages_to_be_built.clone(),
                create_reason: buildbtw_poc::iteration::NewIterationReason::CreatedByUser,
                namespace_id: namespace.id,
            };

            db::iteration::create(pool, new_iteration).await?;
        }
    }
    Ok(())
}
