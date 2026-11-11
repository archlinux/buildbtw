#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Benchmark for `BuildGraphs::calculate` and its impact on unrelated Tokio tasks.
//!
//! The benchmark has two parts:
//!
//! 1. A Criterion benchmark for the warm, single-threaded `BuildGraphs::calculate`
//!    wall time. Use `--save-baseline` and `--baseline` for before/after comparisons.
//! 2. A heartbeat latency measurement that runs periodic timer wakeups concurrently
//!    with graph calculations on a multi-thread Tokio runtime. This exposes scheduler
//!    starvation of unrelated async tasks. The loaded percentiles are printed and
//!    saved to `target/criterion/heartbeat-<label>.txt`, together with an idle control.

use std::{
    hint::black_box,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use buildbtw::{
    dependency_graph::{BuildGraph, BuildGraphs, SourceRepoCache},
    git, package,
};
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Report, Result,
    eyre::{Context, bail, eyre},
};
use criterion::{Criterion, Throughput};
use futures::future::try_join_all;
use petgraph::visit::EdgeRef;
use tokio::{runtime::Runtime, sync::Barrier};

const DEFAULT_PACKAGE_COUNT: usize = 20000;
const DEFAULT_WORKER_THREADS: usize = 4;
const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 1;
const DEFAULT_HEARTBEAT_DURATION_MS: u64 = 5000;

fn main() -> Result<()> {
    let config = BenchConfig::from_env();
    let mut criterion = criterion();

    let BenchConfig {
        packages,
        worker_threads,
        concurrency,
        heartbeat_interval,
        heartbeat_duration,
    } = config;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
        .wrap_err("failed to build Tokio runtime")?;

    let fixture = Fixture::build(packages)?;
    let mut caches = runtime.block_on(create_caches(&fixture.source_repo_dir, concurrency))?;
    let fingerprint = warm_caches(
        &runtime,
        &mut caches,
        &fixture.changesets,
        fixture.package_count,
    )?;

    eprintln!(
        "build graph benchmark: packages={packages}, worker_threads={worker_threads}, \
         concurrency={concurrency}, heartbeat_interval={heartbeat_interval:?}, \
         heartbeat_window={heartbeat_duration:?}, fingerprint=0x{fingerprint:016x}"
    );

    bench_calculate(&mut criterion, &runtime, &mut caches[0], &fixture);

    if benchmark_mode_requested() {
        let idle = heartbeat_idle(&runtime, config)?;
        let loaded = heartbeat_loaded(&runtime, caches, &fixture.changesets, config)?;
        report_heartbeat(config, &fixture, fingerprint, &loaded, &idle)?;
    }

    criterion.final_summary();
    Ok(())
}

fn criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(20))
        .warm_up_time(Duration::from_secs(3))
        .configure_from_args()
}

fn benchmark_mode_requested() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|argument| argument == flag);

    has("--bench") && !has("--test") && !has("--list") && !has("--profile-time")
}

#[derive(Debug, Clone, Copy)]
struct BenchConfig {
    packages: usize,
    worker_threads: usize,
    concurrency: usize,
    heartbeat_interval: Duration,
    heartbeat_duration: Duration,
}

impl BenchConfig {
    fn from_env() -> Self {
        let worker_threads = env_or("BUILDBTW_BENCH_WORKERS", DEFAULT_WORKER_THREADS).max(1);

        Self {
            packages: env_or("BUILDBTW_BENCH_PACKAGES", DEFAULT_PACKAGE_COUNT).max(1),
            worker_threads,
            concurrency: env_or("BUILDBTW_BENCH_CONCURRENCY", worker_threads).max(1),
            heartbeat_interval: Duration::from_millis(
                env_or(
                    "BUILDBTW_BENCH_HEARTBEAT_INTERVAL_MS",
                    DEFAULT_HEARTBEAT_INTERVAL_MS,
                )
                .max(1),
            ),
            heartbeat_duration: Duration::from_millis(
                env_or("BUILDBTW_BENCH_HEARTBEAT_MS", DEFAULT_HEARTBEAT_DURATION_MS).max(10),
            ),
        }
    }
}

fn env_or<T>(name: &str, default: T) -> T
where
    T: FromStr,
{
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

struct Fixture {
    _tempdir: camino_tempfile::Utf8TempDir,
    source_repo_dir: Utf8PathBuf,
    changesets: git::Changesets,
    package_count: usize,
}

impl Fixture {
    fn build(package_count: usize) -> Result<Self> {
        let tempdir = camino_tempfile::Builder::new()
            .prefix("buildbtw-bench-build-graph-")
            .tempdir()?;
        let source_repo_dir = tempdir.path().join("source-repos");
        std::fs::create_dir(&source_repo_dir)?;

        for index in 0..package_count {
            write_repo(&source_repo_dir, index)?;
        }

        let changesets = git::Changesets::from(vec![git::Changeset {
            pkgbase: package_name(0).parse()?,
            branch_name: "main".try_into()?,
        }]);

        Ok(Self {
            _tempdir: tempdir,
            source_repo_dir,
            changesets,
            package_count,
        })
    }
}

fn write_repo(source_repo_dir: &Utf8Path, index: usize) -> Result<()> {
    let name = package_name(index);
    let repo_dir = source_repo_dir.join(&name);
    std::fs::create_dir(&repo_dir)?;

    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = git2::Repository::init_opts(&repo_dir, &options)?;

    std::fs::write(repo_dir.join(".SRCINFO"), srcinfo(index))?;

    let mut git_index = repo.index()?;
    git_index.add_path(Path::new(".SRCINFO"))?;
    git_index.write()?;
    let tree_id = git_index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let signature = git2::Signature::now("buildbtw benchmark", "bench@buildbtw.localhost")?;
    let commit = repo.commit(Some("HEAD"), &signature, &signature, "init", &tree, &[])?;

    repo.remote("origin", "unused-url")?;
    repo.reference(
        "refs/remotes/origin/main",
        commit,
        false,
        "set up origin/main",
    )?;

    Ok(())
}

fn package_name(index: usize) -> String {
    format!("bench-pkg-{index:04}")
}

fn srcinfo(index: usize) -> String {
    let name = package_name(index);
    // Build a deterministic dependency DAG: every package depends on its
    // predecessor and, from the third package on, also on the package before
    // that. One changeset for the first package therefore reaches all packages.
    let dependencies = match index {
        0 => String::new(),
        1 => format!("\tdepends = {}\n", package_name(0)),
        _ => format!(
            "\tdepends = {}\n\tdepends = {}\n",
            package_name(index - 1),
            package_name(index - 2)
        ),
    };

    format!(
        "pkgbase = {name}\n\
         \tpkgver = 1.0\n\
         \tpkgrel = 1\n\
         \tarch = any\n\
         \turl = https://www.archlinux.org\n\
         {dependencies}\n\
         pkgname = {name}\n"
    )
}

async fn create_caches(source_repo_dir: &Utf8Path, count: usize) -> Result<Vec<SourceRepoCache>> {
    let caches = try_join_all((0..count).map(|_| SourceRepoCache::new(source_repo_dir))).await?;
    Ok(caches)
}

fn warm_caches(
    runtime: &Runtime,
    caches: &mut [SourceRepoCache],
    changesets: &git::Changesets,
    package_count: usize,
) -> Result<u64> {
    let mut fingerprint = 0;

    for cache in caches.iter_mut() {
        let graphs = runtime.block_on(BuildGraphs::calculate(changesets, cache))?;
        fingerprint = verify_graphs(&graphs, package_count)?;
    }

    Ok(fingerprint)
}

fn verify_graphs(graphs: &BuildGraphs, expected: usize) -> Result<u64> {
    for architecture in [
        package::BuildArchitecture::X86_64,
        package::BuildArchitecture::X86_64V3,
    ] {
        let graph = graphs
            .get(&architecture)
            .ok_or_else(|| eyre!("missing build graph for {architecture}"))?;

        if graph.node_count() != expected {
            bail!(
                "expected {expected} nodes for {architecture}, found {}",
                graph.node_count()
            );
        }
    }

    Ok(fingerprint(graphs))
}

fn fingerprint(graphs: &BuildGraphs) -> u64 {
    let mut architectures: Vec<(String, &BuildGraph)> = graphs
        .iter()
        .map(|(architecture, graph)| (architecture.to_string(), graph))
        .collect();
    architectures.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for (architecture, graph) in architectures {
        hash = fnv_bytes(hash, architecture.as_bytes());
        hash = fnv_bytes(hash, &[0xff]);

        let mut nodes: Vec<String> = graph
            .node_weights()
            .map(|node| node.pkgbase.to_string())
            .collect();
        nodes.sort();
        for node in nodes {
            hash = fnv_bytes(hash, node.as_bytes());
            hash = fnv_bytes(hash, &[0xff]);
        }

        let mut edges: Vec<(String, String)> = graph
            .edge_references()
            .map(|edge| {
                (
                    graph[edge.source()].pkgbase.to_string(),
                    graph[edge.target()].pkgbase.to_string(),
                )
            })
            .collect();
        edges.sort();
        for (source, target) in edges {
            hash = fnv_bytes(hash, source.as_bytes());
            hash = fnv_bytes(hash, &[0xff]);
            hash = fnv_bytes(hash, target.as_bytes());
            hash = fnv_bytes(hash, &[0xff]);
        }
    }

    hash
}

fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn bench_calculate(
    criterion: &mut Criterion,
    runtime: &Runtime,
    cache: &mut SourceRepoCache,
    fixture: &Fixture,
) {
    let changesets = &fixture.changesets;
    let elements = u64::try_from(fixture.package_count * 2).expect("package count fits in u64");

    let mut group = criterion.benchmark_group("build_graph");
    group.throughput(Throughput::Elements(elements));
    group.bench_function("warm_build_graphs_calculate", move |bencher| {
        bencher.iter_custom(|iterations| {
            let start = Instant::now();
            for _ in 0..iterations {
                let graphs = runtime
                    .block_on(BuildGraphs::calculate(changesets, cache))
                    .expect("build graph calculation failed");
                // Keep the graph observable so the optimizer cannot discard it.
                black_box(graph_shape(&graphs));
            }
            start.elapsed()
        });
    });
    group.finish();
}

fn graph_shape(graphs: &BuildGraphs) -> (usize, usize) {
    let mut nodes = 0;
    let mut edges = 0;
    for graph in graphs.values() {
        nodes += graph.node_count();
        edges += graph.edge_count();
    }
    (nodes, edges)
}

#[derive(Debug)]
struct HeartbeatStats {
    samples: usize,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
    calculations: usize,
}

fn heartbeat_loaded(
    runtime: &Runtime,
    caches: Vec<SourceRepoCache>,
    changesets: &git::Changesets,
    config: BenchConfig,
) -> Result<HeartbeatStats> {
    let barrier = Arc::new(Barrier::new(caches.len() + 1));
    let changesets = changesets.clone();
    let interval = config.heartbeat_interval;
    let duration = config.heartbeat_duration;

    let (calculations, latencies) = runtime.block_on(async move {
        let mut handles = Vec::with_capacity(caches.len());

        for mut cache in caches {
            let barrier = Arc::clone(&barrier);
            let changesets = changesets.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                let deadline = Instant::now() + duration;
                let mut calculations = 0_usize;

                while Instant::now() < deadline {
                    let graphs = BuildGraphs::calculate(&changesets, &mut cache).await?;
                    black_box(graph_shape(&graphs));
                    calculations += 1;

                    // Let the scheduler poll other tasks between calculations.
                    // The calculation itself still runs to completion in a single
                    // poll and blocks its worker for its full duration.
                    tokio::task::yield_now().await;
                }

                Ok::<usize, Report>(calculations)
            }));
        }

        let heartbeat = tokio::spawn(run_heartbeat(Arc::clone(&barrier), interval, duration));

        let mut calculations = 0;
        for handle in handles {
            calculations += handle.await??;
        }

        let latencies = heartbeat.await?;
        Ok::<(usize, Vec<Duration>), Report>((calculations, latencies))
    })?;

    let stats = heartbeat_stats(latencies, calculations);

    if stats.calculations == 0 {
        bail!("loaded heartbeat measurement did not run any graph calculations");
    }
    if stats.samples == 0 {
        bail!("loaded heartbeat measurement did not collect any samples");
    }

    Ok(stats)
}

fn heartbeat_idle(runtime: &Runtime, config: BenchConfig) -> Result<HeartbeatStats> {
    let barrier = Arc::new(Barrier::new(1));
    let latencies = runtime.block_on(run_heartbeat(
        barrier,
        config.heartbeat_interval,
        config.heartbeat_duration,
    ));
    let stats = heartbeat_stats(latencies, 0);

    if stats.samples == 0 {
        bail!("idle heartbeat measurement did not collect any samples");
    }

    Ok(stats)
}

async fn run_heartbeat(
    barrier: Arc<Barrier>,
    interval: Duration,
    duration: Duration,
) -> Vec<Duration> {
    barrier.wait().await;

    let start = Instant::now();
    let deadline = start + duration;
    let mut scheduled = start + interval;
    let mut latencies = Vec::new();

    while Instant::now() < deadline {
        tokio::time::sleep_until(tokio::time::Instant::from_std(scheduled)).await;
        let woke_at = Instant::now();
        latencies.push(woke_at.saturating_duration_since(scheduled));

        scheduled += interval;
        if scheduled <= woke_at {
            scheduled = woke_at + interval;
        }
    }

    latencies
}

fn heartbeat_stats(mut latencies: Vec<Duration>, calculations: usize) -> HeartbeatStats {
    latencies.sort_unstable();

    HeartbeatStats {
        samples: latencies.len(),
        p50: percentile(&latencies, 50),
        p95: percentile(&latencies, 95),
        p99: percentile(&latencies, 99),
        max: latencies.last().copied().unwrap_or(Duration::ZERO),
        calculations,
    }
}

fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }

    let rank = sorted.len().saturating_mul(percent).div_ceil(100);
    let index = rank.saturating_sub(1).min(sorted.len() - 1);

    sorted[index]
}

fn report_heartbeat(
    config: BenchConfig,
    fixture: &Fixture,
    fingerprint: u64,
    loaded: &HeartbeatStats,
    idle: &HeartbeatStats,
) -> Result<()> {
    let label = std::env::var("BUILDBTW_BENCH_LABEL").unwrap_or_else(|_| "latest".to_string());

    let summary = format!(
        "build graph heartbeat summary\n\
         label: {label}\n\
         packages: {packages}\n\
         worker_threads: {worker_threads}\n\
         concurrency: {concurrency}\n\
         heartbeat_interval: {heartbeat_interval:?}\n\
         heartbeat_window: {heartbeat_duration:?}\n\
         fingerprint: 0x{fingerprint:016x}\n\
         loaded: samples={loaded_samples}, calculations={loaded_calculations}, \
         p50={loaded_p50:?}, p95={loaded_p95:?}, p99={loaded_p99:?}, max={loaded_max:?}\n\
         idle: samples={idle_samples}, \
         p50={idle_p50:?}, p95={idle_p95:?}, p99={idle_p99:?}, max={idle_max:?}\n",
        packages = fixture.package_count,
        worker_threads = config.worker_threads,
        concurrency = config.concurrency,
        heartbeat_interval = config.heartbeat_interval,
        heartbeat_duration = config.heartbeat_duration,
        loaded_samples = loaded.samples,
        loaded_calculations = loaded.calculations,
        loaded_p50 = loaded.p50,
        loaded_p95 = loaded.p95,
        loaded_p99 = loaded.p99,
        loaded_max = loaded.max,
        idle_samples = idle.samples,
        idle_p50 = idle.p50,
        idle_p95 = idle.p95,
        idle_p99 = idle.p99,
        idle_max = idle.max,
    );

    println!("{summary}");

    let target_dir =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| PathBuf::from("target"), PathBuf::from);
    let output_dir = target_dir.join("criterion");
    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join(format!("heartbeat-{}.txt", sanitize_label(&label)));
    std::fs::write(&output_path, &summary)?;
    println!("heartbeat summary saved to {}", output_path.display());

    Ok(())
}

fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}
