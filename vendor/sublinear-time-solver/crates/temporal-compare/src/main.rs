mod data;
mod metrics;
mod baseline;
mod mlp;
mod mlp_optimized;
mod mlp_ultra;
mod mlp_classifier;
mod ensemble;
mod attention;
mod reservoir;
mod fourier;
mod sparse;
mod mlp_avx512;
mod quantization;
mod mlp_quantized;
mod ruv_fann_adapter;
mod ruv_fann_impl;

use clap::{Parser, ValueEnum};
use data::{make_synthetic, to_class};
use metrics::{mse, acc};
use baseline::Baseline;
use mlp::Mlp;
use mlp_optimized::OptimizedMlp;
use mlp_ultra::UltraMlp;
use mlp_classifier::ClassifierMlp;
use ensemble::{EnsembleModel, BoostedEnsemble};
use reservoir::{ReservoirComputer, QuantumReservoir};
use fourier::{FourierFeatures, AdaptiveFourierFeatures};
use sparse::{SparseNetwork, LotteryTicketNetwork};
use mlp_avx512::DynamicAvx512Mlp;
use mlp_quantized::QuantizedMlpBackend;
#[cfg(feature = "ruv-fann")]
use ruv_fann_impl::RuvFannModel;
#[cfg(not(feature = "ruv-fann"))]
use ruv_fann_adapter::ruv_fann_backend::RuvFannModel;

#[derive(Copy, Clone, ValueEnum)]
enum Backend { Mlp, MlpOpt, MlpUltra, MlpAvx512, MlpQuantized, MlpClassifier, Ensemble, Boosted, Reservoir, QuantumReservoir, Fourier, AdaptiveFourier, Sparse, LotteryTicket, RuvFann, Baseline }

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t=32)]
    window: usize,
    #[arg(long, default_value_t=5000)]
    n: usize,
    #[arg(long, default_value_t=42)]
    seed: u64,
    #[arg(long, value_enum, default_value_t=Backend::Mlp)]
    backend: Backend,
    #[arg(long, default_value_t=64)]
    hidden: usize,
    #[arg(long, default_value_t=8)]
    epochs: usize,
    #[arg(long, default_value_t=0.01)]
    lr: f32,
    #[arg(long, default_value_t=false)]
    classify: bool
}

fn main() {
    let args = Args::parse();
    let ds = make_synthetic(args.window, args.n, args.seed);

    // prepare tensors
    let to_xy = |v: &Vec<data::Sample>| {
        let x: Vec<Vec<f32>> = v.iter().map(|s| s.x.clone()).collect();
        let y_reg: Vec<f32> = v.iter().map(|s| s.y).collect();
        let y_cls: Vec<usize> = v.iter().map(|s| to_class(s.y)).collect();
        (x, y_reg, y_cls)
    };
    let (xtr, ytr, ytrc) = to_xy(&ds.train);
    let (xva, yva, yvac) = to_xy(&ds.val);
    let (xte, yte, ytec) = to_xy(&ds.test);

    match args.backend {
        Backend::Baseline => {
            let yhat = Baseline::predict_reg(&ds.test, args.window);
            let yhatc = Baseline::predict_cls(&ds.test, args.window);
            println!("baseline_mse_test={:.6}", mse(&yte, &yhat));
            println!("baseline_acc_test={:.4}", acc(&ytec, &yhatc));
        }
        Backend::Mlp => {
            let mut model = if args.classify { Mlp::new(xtr[0].len(), args.hidden, 3) }
                            else { Mlp::new(xtr[0].len(), args.hidden, 1) };
            if args.classify {
                // train via regression to continuous y, then map to buckets
                model.train_regression(&xtr, &ytr, args.epochs, args.lr);
                let yhat = model.predict_cls3(&xte);
                println!("mlp_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train_regression(&xtr, &ytr, args.epochs, args.lr);
                let yhat = model.predict_reg(&xte);
                println!("mlp_mse_val={:.6}", mse(&yva, &model.predict_reg(&xva)));
                println!("mlp_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::MlpOpt => {
            let mut model = if args.classify { OptimizedMlp::new(xtr[0].len(), args.hidden, 3) }
                            else { OptimizedMlp::new(xtr[0].len(), args.hidden, 1) };
            if args.classify {
                model.train_batch(&xtr, &ytr, args.epochs, args.lr, 32);
                let yhat = model.predict_cls3(&xte);
                println!("mlp_opt_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train_batch(&xtr, &ytr, args.epochs, args.lr, 32);
                let yhat = model.predict_reg(&xte);
                println!("mlp_opt_mse_val={:.6}", mse(&yva, &model.predict_reg(&xva)));
                println!("mlp_opt_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::MlpUltra => {
            let mut model = if args.classify { UltraMlp::new(xtr[0].len(), args.hidden, 3) }
                            else { UltraMlp::new(xtr[0].len(), args.hidden, 1) };
            if args.classify {
                model.train_batch_parallel(&xtr, &ytr, args.epochs, args.lr, 32);
                let yhat = model.predict_cls3_parallel(&xte);
                println!("mlp_ultra_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train_batch_parallel(&xtr, &ytr, args.epochs, args.lr, 32);
                let yhat = model.predict_parallel(&xte);
                println!("mlp_ultra_mse_val={:.6}", mse(&yva, &model.predict_parallel(&xva)));
                println!("mlp_ultra_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::MlpAvx512 => {
            let model = DynamicAvx512Mlp::new(xtr[0].len(), args.hidden, if args.classify { 3 } else { 1 });
            if args.classify {
                // Note: AVX512 model doesn't have train method yet, using predict only
                let yhat = model.predict_class(&xte);
                println!("mlp_avx512_acc_test={:.4} (inference only)", acc(&ytec, &yhat));
            } else {
                let yhat = model.predict(&xte);
                println!("mlp_avx512_mse_test={:.6} (inference only)", mse(&yte, &yhat));
            }
        }
        Backend::MlpQuantized => {
            let mut model = QuantizedMlpBackend::new(xtr[0].len(), args.hidden, if args.classify { 3 } else { 1 });

            // Train in FP32
            model.train(&xtr, &ytr, args.epochs, args.lr);

            // Benchmark INT8 vs FP32
            println!("\n=== INT8 Quantization Performance ===");
            model.benchmark_inference(&xte[..100.min(xte.len())], 100);

            if args.classify {
                let yhat = model.predict_class(&xte);
                println!("\nmlp_quantized_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                // Compare FP32 vs INT8 accuracy
                let yhat_fp32 = model.predict_fp32(&xte);
                let yhat_int8 = model.predict(&xte);

                println!("\nmlp_quantized_mse_test_fp32={:.6}", mse(&yte, &yhat_fp32));
                println!("mlp_quantized_mse_test_int8={:.6}", mse(&yte, &yhat_int8));

                // Calculate accuracy loss
                let mse_fp32 = mse(&yte, &yhat_fp32);
                let mse_int8 = mse(&yte, &yhat_int8);
                let accuracy_loss = ((mse_int8 - mse_fp32).abs() / mse_fp32) * 100.0;
                println!("Quantization accuracy loss: {:.2}%", accuracy_loss);
            }

            let (orig, quant, ratio) = model.get_compression_stats();
            println!("\nModel compression: {} -> {} bytes ({:.2}x)", orig, quant, ratio);
        }
        Backend::MlpClassifier => {
            let mut model = ClassifierMlp::new(xtr[0].len(), 3);
            if args.classify {
                model.train_classification(&xtr, &ytr, args.epochs, 32);
                let yhat = model.predict_cls3(&xte);
                let yhat_val = model.predict_cls3(&xva);
                println!("mlp_classifier_acc_val={:.4}", acc(&yvac, &yhat_val));
                println!("mlp_classifier_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                println!("MlpClassifier is for classification only, use --classify flag");
            }
        }
        Backend::Ensemble => {
            if args.classify {
                let mut ensemble = EnsembleModel::new();
                let input_dim = xtr[0].len();

                // Add diverse models
                ensemble.add_model_simple(input_dim, args.hidden, 3);
                ensemble.add_model_optimized(input_dim, args.hidden * 2, 3);
                ensemble.add_model_ultra(input_dim, args.hidden, 3);
                ensemble.add_model_classifier(input_dim, 3);

                ensemble.train_ensemble(&xtr, &ytr, args.epochs, args.lr, &xva, &yvac);
                let yhat = ensemble.predict_ensemble(&xte);
                println!("ensemble_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                println!("Ensemble is for classification only, use --classify flag");
            }
        }
        Backend::Boosted => {
            if args.classify {
                let mut boosted = BoostedEnsemble::new(xtr[0].len(), args.hidden);
                boosted.train_boosted(&xtr, &ytrc, 10, args.epochs / 2);
                let yhat = boosted.predict_boosted(&xte);
                println!("boosted_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                println!("Boosted is for classification only, use --classify flag");
            }
        }
        Backend::Reservoir => {
            let mut model = ReservoirComputer::new(xtr[0].len(), 100, 1);
            if args.classify {
                model.train_ridge(&xtr, &ytr, 0.001);
                let yhat = model.predict_class(&xte);
                println!("reservoir_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train_ridge(&xtr, &ytr, 0.001);
                let yhat = model.predict(&xte);
                println!("reservoir_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::QuantumReservoir => {
            let mut model = QuantumReservoir::new(xtr[0].len(), 100, 1);
            if args.classify {
                model.train(&xtr, &ytr);
                let yhat = model.predict_quantum(&xte);
                println!("quantum_reservoir_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train(&xtr, &ytr);
                // For regression, use classical prediction
                let yhat = model.classical_reservoir.predict(&xte);
                println!("quantum_reservoir_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::Fourier => {
            let mut model = FourierFeatures::new(xtr[0].len(), 500, 1.0);
            if args.classify {
                model.train(&xtr, &ytr, 0.001);
                let yhat = model.predict_class(&xte);
                println!("fourier_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train(&xtr, &ytr, 0.001);
                let yhat = model.predict(&xte);
                println!("fourier_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::AdaptiveFourier => {
            let mut model = AdaptiveFourierFeatures::new(xtr[0].len(), 500, 1.0);
            if args.classify {
                model.train_adaptive(&xtr, &ytr, 50);
                let yhat = model.predict_class(&xte);
                println!("adaptive_fourier_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.train_adaptive(&xtr, &ytr, 50);
                let yhat = model.predict(&xte);
                println!("adaptive_fourier_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::Sparse => {
            let mut model = SparseNetwork::new(xtr[0].len(), args.hidden, if args.classify { 3 } else { 1 }, 0.1);
            if args.classify {
                model.train(&xtr, &ytr, args.epochs * 10, 0.01);
                let yhat = model.predict_class(&xte);
                let (active, pruned, sparsity) = model.get_sparsity_stats();
                println!("sparse_acc_test={:.4} (active={}, pruned={}, sparsity={:.1}%)",
                         acc(&ytec, &yhat), active, pruned, sparsity * 100.0);
            } else {
                model.train(&xtr, &ytr, args.epochs * 10, 0.01);
                let yhat = model.predict(&xte);
                let (active, pruned, sparsity) = model.get_sparsity_stats();
                println!("sparse_mse_test={:.6} (active={}, pruned={}, sparsity={:.1}%)",
                         mse(&yte, &yhat), active, pruned, sparsity * 100.0);
            }
        }
        Backend::LotteryTicket => {
            let mut model = LotteryTicketNetwork::new(xtr[0].len(), args.hidden, if args.classify { 3 } else { 1 });
            if args.classify {
                model.find_winning_ticket(&xtr, &ytr, 0.2, 5);
                let yhat = model.predict_class(&xte);
                println!("lottery_ticket_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                model.find_winning_ticket(&xtr, &ytr, 0.2, 5);
                let yhat = model.predict(&xte);
                println!("lottery_ticket_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
        Backend::RuvFann => {
            let mut model = if args.classify { RuvFannModel::new(xtr[0].len(), args.hidden, 3) }
                            else { RuvFannModel::new(xtr[0].len(), args.hidden, 1) };
            model.train_regression(&xtr, &ytr, args.epochs, args.lr);
            if args.classify {
                let yhat = model.predict_cls3(&xte);
                println!("ruv_fann_acc_test={:.4}", acc(&ytec, &yhat));
            } else {
                let yhat = model.predict_reg(&xte);
                println!("ruv_fann_mse_test={:.6}", mse(&yte, &yhat));
            }
        }
    }
}
