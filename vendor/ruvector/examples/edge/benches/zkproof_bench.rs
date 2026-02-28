use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use ruvector_edge::plaid::zkproofs_prod::*;

fn bench_proof_generation_by_bits(c: &mut Criterion) {
    let mut group = c.benchmark_group("proof_generation_by_bits");

    for bits in [8, 16, 32, 64] {
        let value = (1u64 << (bits - 1)) - 1; // Max value for bit size
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}bit", bits)),
            &bits,
            |b, _| {
                let mut prover = FinancialProver::new();
                prover.set_income(vec![value; 12]);
                b.iter(|| {
                    black_box(prover.prove_income_above(value / 2).unwrap())
                });
            },
        );
    }
    group.finish();
}

fn bench_income_proof(c: &mut Criterion) {
    c.bench_function("prove_income_above", |b| {
        let mut prover = FinancialProver::new();
        prover.set_income(vec![650000, 650000, 680000, 650000]);
        b.iter(|| {
            black_box(prover.prove_income_above(500000).unwrap())
        })
    });
}

fn bench_affordability_proof(c: &mut Criterion) {
    c.bench_function("prove_affordability", |b| {
        let mut prover = FinancialProver::new();
        prover.set_income(vec![650000, 650000, 680000, 650000]);
        b.iter(|| {
            black_box(prover.prove_affordability(200000, 3).unwrap())
        })
    });
}

fn bench_no_overdraft_proof(c: &mut Criterion) {
    c.bench_function("prove_no_overdrafts", |b| {
        let mut prover = FinancialProver::new();
        prover.set_balances(vec![100000i64; 90]); // 90 days of balance data
        b.iter(|| {
            black_box(prover.prove_no_overdrafts(30).unwrap())
        })
    });
}

fn bench_rental_bundle_creation(c: &mut Criterion) {
    c.bench_function("rental_bundle_create", |b| {
        let mut prover = FinancialProver::new();
        prover.set_income(vec![650000, 650000, 680000, 650000]);
        prover.set_balances(vec![500000i64; 90]);
        b.iter(|| {
            black_box(
                RentalApplicationBundle::create(
                    &mut prover,
                    200000, // $2000 rent
                    3,      // 3x income
                    30,     // 30 days stability
                    Some(2) // 2 months savings
                ).unwrap()
            )
        })
    });
}

fn bench_verification(c: &mut Criterion) {
    let mut prover = FinancialProver::new();
    prover.set_income(vec![650000; 12]);
    let proof = prover.prove_income_above(500000).unwrap();

    c.bench_function("verify_single", |b| {
        b.iter(|| {
            black_box(FinancialVerifier::verify(&proof).unwrap())
        })
    });
}

fn bench_batch_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_verification");

    for n in [1, 3, 10, 50, 100] {
        let mut prover = FinancialProver::new();
        prover.set_income(vec![650000; 12]);
        let proofs: Vec<_> = (0..n)
            .map(|_| prover.prove_income_above(500000).unwrap())
            .collect();

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(n),
            &proofs,
            |b, proofs| {
                b.iter(|| {
                    black_box(FinancialVerifier::verify_batch(proofs))
                })
            },
        );
    }
    group.finish();
}

fn bench_bundle_verification(c: &mut Criterion) {
    let mut prover = FinancialProver::new();
    prover.set_income(vec![650000, 650000, 680000, 650000]);
    prover.set_balances(vec![500000i64; 90]);

    let bundle = RentalApplicationBundle::create(
        &mut prover,
        200000,
        3,
        30,
        Some(2)
    ).unwrap();

    c.bench_function("bundle_verify", |b| {
        b.iter(|| {
            black_box(bundle.verify().unwrap())
        })
    });
}

fn bench_commitment_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("commitment_operations");

    group.bench_function("commit_new", |b| {
        b.iter(|| {
            black_box(PedersenCommitment::commit(650000))
        })
    });

    let (commitment, blinding) = PedersenCommitment::commit(650000);
    group.bench_function("commit_with_blinding", |b| {
        b.iter(|| {
            black_box(PedersenCommitment::commit_with_blinding(650000, &blinding))
        })
    });

    group.bench_function("decompress", |b| {
        b.iter(|| {
            black_box(commitment.decompress())
        })
    });

    group.finish();
}

fn bench_proof_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("proof_sizes");

    for bits in [8, 16, 32, 64] {
        let value = (1u64 << (bits - 1)) - 1;
        let mut prover = FinancialProver::new();
        prover.set_income(vec![value; 12]);
        let proof = prover.prove_income_above(value / 2).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}bit_serialize", bits)),
            &proof,
            |b, proof| {
                b.iter(|| {
                    black_box(serde_json::to_string(proof).unwrap())
                })
            },
        );
    }
    group.finish();
}

fn bench_metadata_hashing(c: &mut Criterion) {
    use sha2::{Digest, Sha512};

    let mut group = c.benchmark_group("metadata_operations");

    let data = vec![0u8; 800]; // Typical proof size

    group.bench_function("sha512_hash", |b| {
        b.iter(|| {
            let mut hasher = Sha512::new();
            hasher.update(&data);
            black_box(hasher.finalize())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_proof_generation_by_bits,
    bench_income_proof,
    bench_affordability_proof,
    bench_no_overdraft_proof,
    bench_rental_bundle_creation,
    bench_verification,
    bench_batch_verification,
    bench_bundle_verification,
    bench_commitment_operations,
    bench_proof_size,
    bench_metadata_hashing,
);

criterion_main!(benches);
