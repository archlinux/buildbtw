use buildbtw_poc::{
    BuildNamespace, BuildNamespaceStatus,
    build_set_graph::{build_global_dependency_graphs, gather_packages_metadata},
    source_repos::SourceRepos,
};
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("benches");
    group.sample_size(10);
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let namespace = BuildNamespace {
        id: Uuid::new_v4(),
        name: "test namespace".to_string(),
        current_origin_changesets: Vec::new(),
        created_at: time::OffsetDateTime::now_utc(),
        status: BuildNamespaceStatus::Active,
    };

    group.bench_function("global_dependency_graph", |b| {
        b.to_async(&rt).iter(async || {
            let mut source_repos = SourceRepos::new().await.unwrap();

            let packages_metadata = gather_packages_metadata(
                namespace.current_origin_changesets.clone(),
                &mut source_repos,
            )
            .await
            .unwrap();
            build_global_dependency_graphs(&packages_metadata).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
