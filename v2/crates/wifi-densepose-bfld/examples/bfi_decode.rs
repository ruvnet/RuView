//! End-to-end BFI decode tool: read a radiotap pcap, decode every VHT
//! Compressed Beamforming Report, and print a report-rate dashboard plus the
//! motion-energy time series (ADR-365).
//!
//! This is the runnable front-end for the BFLD BFI path. It works on any
//! `LINKTYPE_IEEE802_11_RADIOTAP` pcap; the values it prints are only as real
//! as the capture — a synthetic pcap yields synthetic numbers.
//!
//! Run with:
//! ```sh
//! cargo run -p wifi-densepose-bfld --example bfi_decode -- capture.pcap
//! ```

use std::process::ExitCode;

use wifi_densepose_bfld::capture::decode_pcap;
use wifi_densepose_bfld::features::motion_series;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: bfi_decode <capture.pcap>");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let (reports, summary) = match decode_pcap(&bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("not a usable capture: {e}");
            return ExitCode::from(1);
        }
    };

    println!("== BFI report-rate dashboard ({path}) ==");
    println!("frames examined:     {}", summary.frames);
    println!("action frames:       {}", summary.action_frames);
    println!("beamforming decoded: {}", summary.decoded);
    println!("decode failures:     {}", summary.decode_failures);
    println!("sequence gaps:       {}", summary.sequence_gaps);
    println!("sequence dupes:      {}", summary.sequence_dupes);
    println!("reports/sec:         {:.2}", summary.reports_per_sec());

    if let Some(first) = reports.first() {
        let r = &first.report;
        let (nr, nc) = r.dims();
        println!(
            "first report:        {} Nr{nr}xNc{nc} subcarriers={} bits(phi,psi)=({},{}) avg_snr={:?}",
            r.format(),
            r.num_subcarriers(),
            r.phi_bits(),
            r.psi_bits(),
            r.avg_snr(),
        );
    }
    let he = reports.iter().filter(|c| c.report.format() == "HE").count();
    println!("formats:             VHT={} HE={he}", reports.len() - he);

    let series = motion_series(&reports);
    if series.is_empty() {
        println!("motion series:       (need >=2 same-dimension reports)");
    } else {
        let peak = series.iter().map(|&(_, e)| e).fold(0.0_f64, f64::max);
        let mean = series.iter().map(|&(_, e)| e).sum::<f64>() / series.len() as f64;
        println!("motion samples:      {} (mean {mean:.4}, peak {peak:.4})", series.len());
    }

    if summary.decoded == 0 {
        eprintln!(
            "note: zero beamforming reports. On 2.4 GHz there are none (VHT/HE is 5/6 GHz); \
             confirm the client is on a 5 GHz SSID and the monitor is on its exact channel+width."
        );
    }
    ExitCode::SUCCESS
}
