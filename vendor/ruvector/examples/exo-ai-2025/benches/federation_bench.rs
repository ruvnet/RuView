use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use exo_federation::{FederatedMesh, SubstrateInstance, FederationScope, StateUpdate, PeerAddress};
use tokio::runtime::Runtime;

fn create_test_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap()
}

fn create_test_mesh() -> FederatedMesh {
    let substrate = SubstrateInstance {};
    FederatedMesh::new(substrate).unwrap()
}

fn benchmark_local_query(c: &mut Criterion) {
    let rt = create_test_runtime();

    c.bench_function("federation_local_query", |b| {
        let mesh = create_test_mesh();
        let query = vec![1, 2, 3, 4, 5];

        b.iter(|| {
            rt.block_on(async {
                mesh.federated_query(
                    black_box(query.clone()),
                    black_box(FederationScope::Local),
                ).await
            })
        });
    });
}

fn benchmark_consensus(c: &mut Criterion) {
    let mut group = c.benchmark_group("federation_consensus");
    let rt = create_test_runtime();

    for num_peers in [3, 5, 7, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_peers),
            num_peers,
            |b, &_peers| {
                let mesh = create_test_mesh();

                b.iter(|| {
                    rt.block_on(async {
                        let update = StateUpdate {
                            update_id: "test_update".to_string(),
                            data: vec![1, 2, 3, 4, 5],
                            timestamp: 12345,
                        };
                        mesh.byzantine_commit(black_box(update)).await
                    })
                });
            },
        );
    }

    group.finish();
}

fn benchmark_mesh_creation(c: &mut Criterion) {
    c.bench_function("federation_mesh_creation", |b| {
        b.iter(|| {
            let substrate = SubstrateInstance {};
            FederatedMesh::new(black_box(substrate))
        });
    });
}

criterion_group!(
    benches,
    benchmark_local_query,
    benchmark_consensus,
    benchmark_mesh_creation
);
criterion_main!(benches);
