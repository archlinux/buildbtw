use buildbtw_poc::{
    BuildNamespace, BuildNamespaceStatus,
    build_set_graph::{build_global_dependency_graphs, gather_packages_metadata},
};
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("benches");
    group.sample_size(10);
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let pkgname_to_srcinfo_map = rt.block_on(async {
        let namespace = BuildNamespace {
            id: Uuid::new_v4(),
            name: "test namespace".to_string(),
            current_origin_changesets: Vec::new(),
            created_at: time::OffsetDateTime::now_utc(),
            status: BuildNamespaceStatus::Active,
        };

        gather_packages_metadata(namespace.current_origin_changesets.clone())
            .await
            .unwrap()
    });
    group.bench_function("global_dependency_graph", |b| {
        b.iter(|| {
            build_global_dependency_graphs(&pkgname_to_srcinfo_map).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
