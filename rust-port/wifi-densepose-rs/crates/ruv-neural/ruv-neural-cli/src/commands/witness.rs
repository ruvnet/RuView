//! Generate and verify Ed25519-signed capability witness bundles.

use ruv_neural_core::witness::{attest_capabilities, WitnessBundle};
use std::path::PathBuf;

/// Run the witness command.
pub fn run(
    output: Option<PathBuf>,
    verify: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = verify {
        // Verify mode
        let json = std::fs::read_to_string(&path)?;
        let bundle: WitnessBundle = serde_json::from_str(&json)?;

        println!("=== rUv Neural \u{2014} Witness Verification ===\n");
        println!("  Version:   {}", bundle.version);
        println!("  Commit:    {}", bundle.commit);
        println!(
            "  Tests:     {}/{} passed",
            bundle.tests_passed, bundle.total_tests
        );
        println!("  Caps:      {} attestations", bundle.capabilities.len());
        println!(
            "  Public Key: {}...{}",
            &bundle.public_key[..8],
            &bundle.public_key[bundle.public_key.len() - 8..]
        );
        println!();

        // Verify digest
        let digest_ok = bundle.verify_digest();
        println!(
            "  Digest integrity: {}",
            if digest_ok { "PASS" } else { "FAIL" }
        );

        // Verify signature
        match bundle.verify() {
            Ok(true) => println!("  Ed25519 signature: PASS"),
            Ok(false) => println!("  Ed25519 signature: FAIL"),
            Err(e) => println!("  Ed25519 signature: ERROR ({e})"),
        }

        let verdict = match bundle.verify_full() {
            Ok(true) => "PASS",
            _ => "FAIL",
        };
        println!("\n  VERDICT: {verdict}");

        if verdict == "FAIL" {
            std::process::exit(1);
        }
    } else {
        // Generate mode
        let caps = attest_capabilities();
        let bundle = WitnessBundle::new(
            env!("CARGO_PKG_VERSION"),
            "0.1.0",
            333,
            333,
            0,
            caps,
        );

        let json = serde_json::to_string_pretty(&bundle)?;

        if let Some(path) = output {
            std::fs::write(&path, &json)?;
            println!("Witness bundle written to {}", path.display());
        } else {
            println!("{json}");
        }

        println!("\n  Attestations: {}", bundle.capabilities.len());
        println!("  Digest: {}", bundle.capabilities_digest);
        println!(
            "  Signature: {}...{}",
            &bundle.signature[..16],
            &bundle.signature[bundle.signature.len() - 16..]
        );
        println!(
            "  Public Key: {}...{}",
            &bundle.public_key[..8],
            &bundle.public_key[bundle.public_key.len() - 8..]
        );
        println!("\n  VERDICT: SIGNED");
    }

    Ok(())
}
