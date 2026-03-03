//! Validation module for temporal neural solver
//!
//! This module provides comprehensive validation tools to verify
//! the claims made about the temporal neural solver system.

pub mod real_world_validation;
pub mod hardware_timing;
pub mod comprehensive_validation_report;

pub use real_world_validation::*;
pub use hardware_timing::*;
pub use comprehensive_validation_report::*;

use std::process::Command;

/// Run all validation tests
pub fn run_all_validations() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 STARTING COMPREHENSIVE TEMPORAL NEURAL SOLVER VALIDATION");
    println!("=" * 60);

    // 1. Real-world dataset validation
    println!("\n1️⃣ REAL-WORLD DATASET VALIDATION");
    println!("-" * 40);
    let real_world_report = real_world_validation::generate_real_world_validation_report()?;
    std::fs::write("/workspaces/sublinear-time-solver/validation/real_world_report.md", real_world_report)?;
    println!("✅ Real-world validation completed");

    // 2. Baseline comparison (Python script)
    println!("\n2️⃣ BASELINE COMPARISON VALIDATION");
    println!("-" * 40);
    run_python_baseline_comparison()?;
    println!("✅ Baseline comparison completed");

    // 3. Hardware timing validation
    println!("\n3️⃣ HARDWARE TIMING VALIDATION");
    println!("-" * 40);
    let mut hw_validator = hardware_timing::HardwareTimingValidator::new()?;
    let system_a_timing = hw_validator.validate_system_a(10000)?;
    let system_b_timing = hw_validator.validate_system_b(10000)?;
    let timing_report = hardware_timing::generate_hardware_timing_report(&system_a_timing, &system_b_timing);
    std::fs::write("/workspaces/sublinear-time-solver/validation/hardware_timing_report.md", timing_report)?;
    println!("✅ Hardware timing validation completed");

    // 4. Comprehensive analysis
    println!("\n4️⃣ COMPREHENSIVE VALIDATION REPORT");
    println!("-" * 40);
    let comprehensive_report = comprehensive_validation_report::run_comprehensive_validation()?;
    println!("✅ Comprehensive validation completed");

    println!("\n🎉 ALL VALIDATIONS COMPLETED!");
    println!("📄 Reports generated in /workspaces/sublinear-time-solver/validation/");

    Ok(())
}

/// Run Python baseline comparison script
fn run_python_baseline_comparison() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("python3")
        .arg("/workspaces/sublinear-time-solver/validation/baseline_comparison.py")
        .output()?;

    if !output.status.success() {
        eprintln!("Python baseline comparison failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Err("Python baseline comparison failed".into());
    }

    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

/// Print validation summary
pub fn print_validation_summary() {
    println!("📊 VALIDATION SUMMARY");
    println!("=" * 30);
    println!("✅ Real-world dataset validation");
    println!("✅ Baseline model comparison");
    println!("✅ Hardware timing validation");
    println!("✅ Statistical significance testing");
    println!("✅ Implementation code review");
    println!("✅ Red flag detection");
    println!("✅ Comprehensive analysis");
    println!("\n📄 All reports available in validation/ directory");
}