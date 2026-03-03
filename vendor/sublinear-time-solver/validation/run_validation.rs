//! Validation runner for temporal neural solver
//!
//! CRITICAL VALIDATION EXECUTION
//! Run this to perform comprehensive validation of all claims.

use std::env;
use std::process;

mod real_world_validation;
mod hardware_timing;
mod comprehensive_validation_report;

use real_world_validation::*;
use hardware_timing::*;
use comprehensive_validation_report::*;

fn main() {
    println!("🔬 TEMPORAL NEURAL SOLVER CRITICAL VALIDATION");
    println!("=" * 50);
    println!("PURPOSE: Rigorous validation of sub-millisecond claims");
    println!();

    let args: Vec<String> = env::args().collect();
    let validation_type = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    let result = match validation_type {
        "real-world" | "rw" => run_real_world_validation(),
        "hardware" | "hw" => run_hardware_validation(),
        "baseline" | "bl" => run_baseline_validation(),
        "comprehensive" | "comp" => run_comprehensive_validation(),
        "all" => run_all_validations(),
        _ => {
            print_usage();
            return;
        }
    };

    match result {
        Ok(_) => {
            println!("\n🎉 VALIDATION COMPLETED SUCCESSFULLY!");
            println!("📄 Check validation/ directory for detailed reports");
        }
        Err(e) => {
            eprintln!("\n❌ VALIDATION FAILED: {}", e);
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!("Usage: cargo run --bin validation [TYPE]");
    println!();
    println!("Validation types:");
    println!("  real-world  (rw)   - Test on real datasets");
    println!("  hardware    (hw)   - Hardware timing validation");
    println!("  baseline    (bl)   - Compare against baselines");
    println!("  comprehensive      - Full validation suite");
    println!("  all               - Run all validations (default)");
    println!();
    println!("Examples:");
    println!("  cargo run --bin validation");
    println!("  cargo run --bin validation hardware");
    println!("  cargo run --bin validation comprehensive");
}

fn run_real_world_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 REAL-WORLD VALIDATION");
    println!("-" * 30);

    // Financial data validation
    println!("Testing on financial time series...");
    let financial_results = RealWorldValidator::validate_financial_data()?;
    println!("✅ Financial validation: {:?}", financial_results.conclusion);

    // Sensor data validation
    println!("Testing on sensor data...");
    let sensor_results = RealWorldValidator::validate_sensor_data()?;
    println!("✅ Sensor validation: {:?}", sensor_results.conclusion);

    // Generate report
    let report = generate_real_world_validation_report()?;
    std::fs::write("validation/real_world_validation_report.md", report)?;

    println!("📄 Report saved: real_world_validation_report.md");
    Ok(())
}

fn run_hardware_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔬 HARDWARE TIMING VALIDATION");
    println!("-" * 30);

    let mut validator = HardwareTimingValidator::new()?;

    println!("Validating System A with CPU cycle counters...");
    let system_a_results = validator.validate_system_a(5000)?;

    println!("Validating System B with CPU cycle counters...");
    let system_b_results = validator.validate_system_b(5000)?;

    // Check for critical red flags
    let total_flags = system_a_results.red_flags.len() + system_b_results.red_flags.len();
    let critical_flags = system_a_results.red_flags.iter()
        .chain(system_b_results.red_flags.iter())
        .filter(|f| matches!(f.severity, RedFlagSeverity::Critical))
        .count();

    println!("Hardware validation results:");
    println!("  System A P99.9: {:.3}ms", system_a_results.wall_clock.p99_9_ns / 1_000_000.0);
    println!("  System B P99.9: {:.3}ms", system_b_results.wall_clock.p99_9_ns / 1_000_000.0);
    println!("  Red flags: {} ({} critical)", total_flags, critical_flags);

    if critical_flags > 0 {
        println!("⚠️  CRITICAL TIMING ISSUES DETECTED!");
    }

    // Generate report
    let report = generate_hardware_timing_report(&system_a_results, &system_b_results);
    std::fs::write("validation/hardware_timing_report.md", report)?;

    println!("📄 Report saved: hardware_timing_report.md");
    Ok(())
}

fn run_baseline_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("📈 BASELINE COMPARISON VALIDATION");
    println!("-" * 30);

    // Run Python baseline comparison script
    println!("Running Python baseline comparisons...");
    let output = std::process::Command::new("python3")
        .arg("validation/baseline_comparison.py")
        .output()?;

    if !output.status.success() {
        eprintln!("Python script error: {}", String::from_utf8_lossy(&output.stderr));
        return Err("Baseline comparison failed".into());
    }

    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("✅ Baseline comparison completed");

    Ok(())
}

fn run_comprehensive_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 COMPREHENSIVE VALIDATION");
    println!("-" * 30);

    let report_content = comprehensive_validation_report::run_comprehensive_validation()?;

    // Extract verdict from report (simplified)
    let verdict = if report_content.contains("BREAKTHROUGH VERIFIED") {
        "VERIFIED"
    } else if report_content.contains("CRITICAL FLAWS") {
        "CRITICAL FLAWS"
    } else if report_content.contains("PARTIAL") {
        "PARTIAL"
    } else {
        "INCONCLUSIVE"
    };

    println!("📊 COMPREHENSIVE VALIDATION VERDICT: {}", verdict);

    match verdict {
        "VERIFIED" => println!("🎉 Claims appear to be validated!"),
        "CRITICAL FLAWS" => println!("🚫 Critical issues detected - claims questionable"),
        "PARTIAL" => println!("⚠️  Some claims validated, others need work"),
        _ => println!("❓ Additional validation required"),
    }

    Ok(())
}

fn run_all_validations() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 RUNNING ALL VALIDATIONS");
    println!("=" * 30);

    // Run each validation step
    println!("\n1️⃣ Real-world validation...");
    run_real_world_validation()?;

    println!("\n2️⃣ Hardware timing validation...");
    run_hardware_validation()?;

    println!("\n3️⃣ Baseline comparison...");
    run_baseline_validation()?;

    println!("\n4️⃣ Comprehensive analysis...");
    run_comprehensive_validation()?;

    println!("\n✅ ALL VALIDATIONS COMPLETED");
    println!("📊 Summary reports available in validation/ directory");

    // Print final summary
    print_final_summary()?;

    Ok(())
}

fn print_final_summary() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n" + "=" * 50);
    println!("📋 FINAL VALIDATION SUMMARY");
    println!("=" * 50);

    // Check if comprehensive report exists and extract verdict
    if let Ok(comprehensive_report) = std::fs::read_to_string("validation/COMPREHENSIVE_VALIDATION_REPORT.md") {
        if comprehensive_report.contains("BREAKTHROUGH VERIFIED") {
            println!("🎉 RESULT: BREAKTHROUGH CLAIMS VALIDATED");
            println!("   The temporal neural solver has passed rigorous validation.");
        } else if comprehensive_report.contains("CRITICAL FLAWS") {
            println!("🚫 RESULT: CRITICAL FLAWS DETECTED");
            println!("   Significant issues prevent validation of claims.");
        } else if comprehensive_report.contains("PARTIAL") {
            println!("⚠️  RESULT: PARTIAL VALIDATION");
            println!("   Some claims validated, others require additional work.");
        } else {
            println!("❓ RESULT: INCONCLUSIVE");
            println!("   Additional validation required for definitive assessment.");
        }
    } else {
        println!("❌ No comprehensive report found");
    }

    println!("\n📄 Generated Reports:");
    println!("   - real_world_validation_report.md");
    println!("   - baseline_comparison_report.md");
    println!("   - hardware_timing_report.md");
    println!("   - COMPREHENSIVE_VALIDATION_REPORT.md");

    println!("\n🔍 Next Steps:");
    println!("   1. Review detailed reports for specific findings");
    println!("   2. Address any critical red flags identified");
    println!("   3. Consider independent third-party validation");
    println!("   4. Document any implementation improvements made");

    Ok(())
}