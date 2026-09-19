//! Importing a subscription-sized batch of share links: the first import into
//! an empty database, and the steady state of an update, where every parsed
//! node matches a stored one. Run with `pnpm bench:rust`.

#![allow(
    clippy::panic,
    reason = "a benchmark that cannot set up has nothing to measure"
)]

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use tokio::runtime::Runtime;
use voya_app::subscriptions::SubscriptionManager;
use voya_core::AppConfig;
use voya_db::Database;

/// `check:rust:test` runs every bench once in the unoptimized test profile to
/// keep them compiling; the smallest size is enough there.
const NODE_COUNTS: &[usize] = if cfg!(debug_assertions) {
    &[100]
} else {
    &[500, 3_000]
};

/// One WebSocket + TLS VLESS link per node, each with its own server.
fn share_links(count: usize) -> String {
    (0..count)
        .map(|index| {
            format!(
                "vless://d7b8b5a5-3c5e-4f7a-9d3b-2f0e8c1a6b4d@n{index}.example.com:443\
                 ?encryption=none&security=tls&sni=cdn.example.com&type=ws\
                 &host=cdn.example.com&path=%2Fws#Node%20{index}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn runtime() -> Runtime {
    match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => panic!("benchmark runtime: {error}"),
    }
}

async fn database_with(links: Option<(&str, usize)>) -> Database {
    let database = match Database::connect_in_memory().await {
        Ok(database) => database,
        Err(error) => panic!("in-memory database: {error}"),
    };
    if let Some((links, count)) = links {
        // A link format the parser stopped accepting would otherwise leave
        // every sample timing an empty import.
        let imported = import(&database, links).await;
        assert_eq!(imported, count, "every generated link imports");
    }
    database
}

async fn import(database: &Database, links: &str) -> usize {
    let mut config = AppConfig::default();
    match SubscriptionManager::new(database)
        .import_profiles_from_text(&mut config, links, None)
        .await
    {
        Ok(result) => result.imported as usize,
        Err(error) => panic!("import: {error}"),
    }
}

fn first_import(criterion: &mut Criterion) {
    let runtime = runtime();
    let mut group = criterion.benchmark_group("subscription_first_import");
    group.sample_size(10);
    for &count in NODE_COUNTS {
        let links = share_links(count);
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &links,
            |bench, links| {
                bench.iter_batched(
                    || runtime.block_on(database_with(None)),
                    |database| {
                        runtime.block_on(async { import(black_box(&database), links).await })
                    },
                    BatchSize::PerIteration,
                );
            },
        );
    }
    group.finish();
}

fn repeat_import(criterion: &mut Criterion) {
    let runtime = runtime();
    let mut group = criterion.benchmark_group("subscription_update_all_matched");
    group.sample_size(10);
    for &count in NODE_COUNTS {
        let links = share_links(count);
        let database = runtime.block_on(database_with(Some((&links, count))));
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &links,
            |bench, links| {
                bench
                    .iter(|| runtime.block_on(async { import(black_box(&database), links).await }));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, first_import, repeat_import);
criterion_main!(benches);
