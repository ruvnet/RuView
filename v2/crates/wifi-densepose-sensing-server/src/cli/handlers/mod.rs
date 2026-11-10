//! Handlers for standalone CLI commands and early-exit modes.

pub mod benchmark;
pub mod convert;
pub mod export;
pub mod pretrain;
pub mod train;

pub use benchmark::run_benchmark;
pub use convert::{diagnose_model_load_error, load_or_convert_model, run_convert_model};
pub use export::{export_emits_placeholder_demo, run_export_rvf_placeholder};
pub use pretrain::{run_build_index, run_embed, run_pretrain};
pub use train::run_train;

use super::Args;

/// Check and dispatch early-exit CLI commands.
///
/// Returns `Some(exit_code)` if a standalone command was handled,
/// or `None` if execution should continue to server startup.
pub fn handle_cli_modes(args: &Args) -> Option<i32> {
    if args.benchmark {
        run_benchmark(1000);
        return Some(0);
    }

    if let Some(ref in_path) = args.convert_model {
        let out_path = args
            .convert_out
            .clone()
            .unwrap_or_else(|| in_path.with_extension("rvf"));
        return Some(run_convert_model(in_path, &out_path));
    }

    if export_emits_placeholder_demo(args.export_rvf.is_some(), args.train, args.pretrain) {
        let rvf_path = args.export_rvf.as_ref().expect("export_rvf is set");
        run_export_rvf_placeholder(rvf_path);
        return Some(0);
    } else if args.export_rvf.is_some() {
        eprintln!(
            "Note: --export-rvf is ignored in training mode — the trained model \
             is exported by the training pipeline to the models/ directory."
        );
    }

    if args.pretrain {
        run_pretrain(args);
        return Some(0);
    }

    if args.embed {
        run_embed(args);
        return Some(0);
    }

    if let Some(ref index_type) = args.build_index {
        run_build_index(index_type);
        return Some(0);
    }

    if args.train {
        run_train(args);
        return Some(0);
    }

    None
}

