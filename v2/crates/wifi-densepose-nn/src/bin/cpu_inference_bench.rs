//! Reproducible CPU inference benchmark (`cpu-micro-fp32` / `cpu-micro-int8`).
//!
//! ```text
//! cpu-inference-bench --model cpu-micro-int8.onnx --profile cpu-micro-int8
//!     --threads 1 --iterations 1000 --report int8.json
//! ```
//!
//! Every run prints the machine header (OS, architecture, CPU model,
//! compiler), the exact configuration, and the measured breakdown
//! (model-load time, P50/P95 latency, throughput, model size, peak RSS where
//! the platform exposes it). Batch size is 1: streaming edge inference is
//! the target.
//!
//! Without `--model` the binary runs a pipeline-only smoke check (input
//! validation, no inference) and exits 0 — this is what CI exercises.
//!
//! `--compare a.json b.json` contrasts two reports (typically one FP32, one
//! INT8 run over the same inputs) and prints numerical + latency deltas.
//! Compare only runs whose `input_source` matches; anything else exits
//! non-zero rather than print a misleading table.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use ndarray::Array4;
use wifi_densepose_nn::inference::{Backend, InferenceOptions};
use wifi_densepose_nn::onnx::OnnxBackendBuilder;
use wifi_densepose_nn::profile::{CpuProfile, Precision};
use wifi_densepose_nn::tensor::Tensor;

/// Default synthetic input: `[B, TX*RX, T, SC]`.
///
/// A `[T, tx, rx, SC]` CSI window maps to 4-D model input by folding the
/// antenna pairs into channels (`C = tx * rx`). The exact mapping is
/// model-defined; this default (`[1, 3, 10, 56]`, i.e. a 10-frame MM-Fi
/// window with 1 TX x 3 RX at 56 subcarriers) matches the `cpu-micro-*`
/// reference layout. Real inputs come from `--input-npy`.
const DEFAULT_SHAPE: [usize; 4] = [1, 3, 10, 56];

fn main() {
    if let Err(e) = run() {
        eprintln!("cpu-inference-bench: error: {e}");
        std::process::exit(1);
    }
}

#[derive(Debug, Default)]
struct Args {
    model: Option<PathBuf>,
    input_npy: Option<PathBuf>,
    input_shape: [usize; 4],
    profile: String,
    precision: Option<String>,
    threads: usize,
    deterministic: bool,
    warmup: usize,
    iterations: usize,
    report: Option<PathBuf>,
    compare: Vec<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args = Self {
            profile: "cpu-micro-fp32".to_string(),
            threads: 1,
            warmup: 10,
            iterations: 200,
            input_shape: DEFAULT_SHAPE,
            ..Self::default()
        };
        let mut it = {
            let argv: Vec<String> = std::env::args().skip(1).collect();
            argv.into_iter().peekable()
        };
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--model" => args.model = Some(take_value(&mut it, "--model")?.into()),
                "--input-npy" => args.input_npy = Some(take_value(&mut it, "--input-npy")?.into()),
                "--input-shape" => {
                    args.input_shape = parse_shape(&take_value(&mut it, "--input-shape")?)?;
                }
                "--profile" => args.profile = take_value(&mut it, "--profile")?,
                "--precision" => args.precision = Some(take_value(&mut it, "--precision")?),
                "--threads" => {
                    args.threads = take_value(&mut it, "--threads")?
                        .parse::<usize>()
                        .map_err(|_| "--threads needs a non-negative integer".to_string())?;
                }
                "--deterministic" => args.deterministic = true,
                "--warmup" => {
                    args.warmup = take_value(&mut it, "--warmup")?
                        .parse::<usize>()
                        .map_err(|_| "--warmup needs a non-negative integer".to_string())?;
                }
                "--iterations" => {
                    args.iterations = take_value(&mut it, "--iterations")?
                        .parse::<usize>()
                        .map_err(|_| "--iterations needs a positive integer".to_string())?;
                    if args.iterations == 0 {
                        return Err("--iterations must be > 0".to_string());
                    }
                }
                "--report" => args.report = Some(take_value(&mut it, "--report")?.into()),
                "--compare" => {
                    args.compare.push(take_value(&mut it, "--compare")?.into());
                    args.compare.push(take_value(&mut it, "--compare")?.into());
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument `{other}` (see --help)")),
            }
        }
        Ok(args)
    }
}

fn take_value(
    it: &mut std::iter::Peekable<std::vec::IntoIter<String>>,
    flag: &str,
) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_shape(s: &str) -> Result<[usize; 4], String> {
    let parts: Vec<usize> = s
        .split(',')
        .map(|p| {
            p.trim()
                .parse::<usize>()
                .map_err(|_| format!("bad --input-shape `{s}` (want B,C,H,W)"))
        })
        .collect::<Result<_, _>>()?;
    if parts.len() != 4 || parts.contains(&0) {
        return Err(format!("bad --input-shape `{s}` (want B,C,H,W, all > 0)"));
    }
    Ok([parts[0], parts[1], parts[2], parts[3]])
}

fn print_help() {
    println!(
        "cpu-inference-bench — reproducible CPU inference benchmark\n\
         \n\
         --model PATH        ONNX model file (omit for pipeline-only smoke)\n\
         --input-npy PATH    4-D f32 input tensor (omit for synthetic smoke input)\n\
         --input-shape B,C,H,W   synthetic input shape (default 1,3,10,56)\n\
         --profile NAME      cpu-micro-fp32 | cpu-micro-int8 (default cpu-micro-fp32)\n\
         --precision P       fp32 | int8 | int4 (default: the profile's precision)\n\
         --threads N         intra-op threads, 0 = runtime default (default 1)\n\
         --deterministic     force single-threaded execution\n\
         --warmup N          untimed warmup iterations (default 10)\n\
         --iterations N      timed iterations, batch size 1 (default 200)\n\
         --report PATH       write JSON report\n\
         --compare A B       contrast two reports (FP32 vs INT8 deltas)"
    );
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;

    if args.compare.len() == 2 {
        return compare_reports(&args.compare[0], &args.compare[1]);
    }
    if args.compare.len() == 1 {
        return Err("--compare needs exactly two reports".to_string());
    }

    let profile = CpuProfile::from_name(&args.profile)
        .ok_or_else(|| format!("unknown --profile `{}`", args.profile))?;
    profile.require_supported().map_err(|e| e.to_string())?;
    let precision = match args.precision.as_deref() {
        None => profile.precision(),
        Some("fp32") => Precision::Fp32,
        Some("int8") => Precision::Int8,
        Some("int4") => Precision::Int4,
        Some(other) => return Err(format!("unknown --precision `{other}`")),
    };
    if precision != profile.precision() {
        return Err(format!(
            "profile `{profile}` executes at {} but --precision {precision} was requested",
            profile.precision()
        ));
    }

    let options = InferenceOptions::default()
        .with_profile(profile)
        .with_precision(precision)
        .with_threads(args.threads)
        .with_deterministic(args.deterministic);
    options.validate().map_err(|e| e.to_string())?;

    // ---- machine header (every number below is tied to this machine) ----
    let env = machine_header();
    println!("os            : {}", env["os"]);
    println!("arch          : {}", env["arch"]);
    println!("cpu           : {}", env["cpu"]);
    println!("rustc         : {}", env["rustc"]);
    println!("profile       : {profile} ({precision})");
    println!(
        "threads       : {} (deterministic: {})",
        options.effective_threads(),
        options.deterministic
    );
    println!("warmup/iters  : {}/{}", args.warmup, args.iterations);

    // ---- input ----
    let (input, input_source) = load_input(&args)?;
    let shape = input.shape().to_vec();
    if shape.len() != 4 {
        return Err(format!("input must be 4-D [B,T?,…], got {shape:?}"));
    }
    if shape[0] != 1 {
        eprintln!(
            "warning: batch size is {} (streaming target is 1); results are not batch-1 latencies",
            shape[0]
        );
    }
    println!("input shape   : {shape:?} ({input_source})");

    let mut report = serde_json::json!({
        "tool": "cpu-inference-bench",
        "env": env,
        "config": {
            "profile": profile.name(),
            "precision": precision.to_string(),
            "threads": options.effective_threads(),
            "deterministic": options.deterministic,
            "input_shape": shape,
            "input_source": input_source,
            "warmup": args.warmup,
            "iterations": args.iterations,
            "model": args.model.as_ref().map(|p| p.display().to_string()),
        },
        "notes": [
            "Batch size 1: streaming edge inference is the target.",
            "Numbers below hold for the machine header only.",
        ],
    });

    match args.model.as_ref() {
        None => {
            println!("model         : none (pipeline-only smoke check)");
            println!("result        : SMOKE-OK (input validation only, no inference ran)");
            report["result"] = serde_json::json!("smoke-ok-no-inference");
        }
        Some(model_path) => {
            run_inference(&args, &options, &input, model_path, &mut report)?;
        }
    }

    if let Some(path) = args.report.as_ref() {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap())
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("report        : {}", path.display());
    }
    Ok(())
}

/// Load the input tensor: a 4-D f32 `.npy` file, or a deterministic
/// synthetic pattern (clearly labeled — never a data result).
fn load_input(args: &Args) -> Result<(Array4<f32>, String), String> {
    if let Some(path) = args.input_npy.as_ref() {
        let arr: Array4<f32> = ndarray_npy::read_npy(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        return Ok((arr, format!("npy:{}", path.display())));
    }
    let [b, c, h, w] = args.input_shape;
    let data: Vec<f32> = (0..b * c * h * w).map(|i| (i % 97) as f32 / 97.0).collect();
    let arr = Array4::from_shape_vec((b, c, h, w), data)
        .map_err(|e| format!("synthetic input reshape failed: {e}"))?;
    Ok((arr, "synthetic-io-smoke (NOT data)".to_string()))
}

fn run_inference(
    args: &Args,
    options: &InferenceOptions,
    input: &Array4<f32>,
    model_path: &PathBuf,
    report: &mut serde_json::Value,
) -> Result<(), String> {
    let size = std::fs::metadata(model_path)
        .map_err(|e| format!("cannot stat {}: {e}", model_path.display()))?
        .len();
    println!("model         : {} ({size} bytes)", model_path.display());

    let load_start = Instant::now();
    let backend = OnnxBackendBuilder::new()
        .model_path(model_path.display().to_string())
        .cpu()
        .threads(options.num_threads)
        .optimize(true)
        .profile(options.profile.unwrap_or(CpuProfile::CpuMicroFp32))
        .precision(options.precision)
        .deterministic(options.deterministic)
        .build()
        .map_err(|e| format!("model load failed: {e}"))?;
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    println!("model load    : {load_ms:.1} ms");

    let input_names = backend.input_names();
    if input_names.is_empty() {
        return Err("model reports no inputs".to_string());
    }
    let first = input_names[0].clone();
    let feed: HashMap<String, Tensor> = HashMap::from([(first, Tensor::Float4D(input.clone()))]);

    for _ in 0..args.warmup {
        let _ = backend
            .run(feed.clone())
            .map_err(|e| format!("warmup inference failed: {e}"))?;
    }
    let mut dts: Vec<f64> = Vec::with_capacity(args.iterations);
    let mut last: HashMap<String, Tensor> = HashMap::new();
    for _ in 0..args.iterations {
        let start = Instant::now();
        last = backend
            .run(feed.clone())
            .map_err(|e| format!("inference failed: {e}"))?;
        std::hint::black_box(&last);
        dts.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    dts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = percentile(&dts, 0.50);
    let p95 = percentile(&dts, 0.95);
    let mean = dts.iter().sum::<f64>() / dts.len() as f64;
    let throughput = 1000.0 / mean;
    println!("latency p50   : {p50:.3} ms");
    println!("latency p95   : {p95:.3} ms");
    println!("throughput    : {throughput:.1} infer/s (batch 1)");

    let outputs = summarize_outputs(&last);
    for o in &outputs {
        println!(
            "output {}     : shape={:?} mean={:.6} std={:.6} min={:.6} max={:.6}",
            o["name"].as_str().unwrap_or("?"),
            o["shape"],
            o["mean"].as_f64().unwrap_or(0.0),
            o["std"].as_f64().unwrap_or(0.0),
            o["min"].as_f64().unwrap_or(0.0),
            o["max"].as_f64().unwrap_or(0.0),
        );
    }

    let peak = peak_rss_bytes();
    match peak {
        Some(b) => println!("peak RSS      : {b} bytes"),
        None => println!("peak RSS      : unavailable on this platform"),
    }

    report["timings_ms"] = serde_json::json!({
        "model_load": load_ms,
        "p50": p50, "p95": p95, "mean": mean,
        "min": dts[0], "max": dts[dts.len() - 1],
        "throughput_per_s": throughput,
    });
    report["model_size_bytes"] = serde_json::json!(size);
    report["peak_rss_bytes"] = peak
        .map(serde_json::Value::from)
        .unwrap_or(serde_json::Value::Null);
    report["outputs"] = serde_json::Value::Array(outputs);
    report["result"] = serde_json::json!("measured");
    Ok(())
}

fn summarize_outputs(last: &HashMap<String, Tensor>) -> Vec<serde_json::Value> {
    let mut names: Vec<&String> = last.keys().collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let tensor = &last[name];
            let (shape, values) = tensor_values(tensor);
            let n = values.len().max(1) as f64;
            let mean = values.iter().sum::<f64>() / n;
            let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
            let mut entry = serde_json::json!({
                "name": name,
                "shape": shape,
                "len": values.len(),
                "mean": mean,
                "std": var.sqrt(),
                "min": values.iter().cloned().fold(f64::INFINITY, f64::min),
                "max": values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            });
            // Full values only for small outputs (keypoint-scale heads);
            // large feature maps keep stats only.
            if values.len() <= 65536 {
                entry["values"] = values.into_iter().map(serde_json::Value::from).collect();
            } else {
                entry["values"] = serde_json::Value::Null;
            }
            entry
        })
        .collect()
}

fn tensor_values(tensor: &Tensor) -> (Vec<usize>, Vec<f64>) {
    match tensor {
        Tensor::Float2D(a) => (a.shape().to_vec(), a.iter().map(|&v| v as f64).collect()),
        Tensor::Float4D(a) => (a.shape().to_vec(), a.iter().map(|&v| v as f64).collect()),
        Tensor::FloatND(a) => (a.shape().to_vec(), a.iter().map(|&v| v as f64).collect()),
        _ => (vec![], vec![]),
    }
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Contrast two reports (FP32 baseline first). Refuses mismatched inputs.
fn compare_reports(a_path: &PathBuf, b_path: &PathBuf) -> Result<(), String> {
    let a: serde_json::Value = serde_json::from_slice(
        &std::fs::read(a_path).map_err(|e| format!("cannot read {}: {e}", a_path.display()))?,
    )
    .map_err(|e| format!("cannot parse {}: {e}", a_path.display()))?;
    let b: serde_json::Value = serde_json::from_slice(
        &std::fs::read(b_path).map_err(|e| format!("cannot read {}: {e}", b_path.display()))?,
    )
    .map_err(|e| format!("cannot parse {}: {e}", b_path.display()))?;
    if a["config"]["input_shape"] != b["config"]["input_shape"]
        || a["config"]["input_source"] != b["config"]["input_source"]
    {
        return Err("reports used different inputs; comparison would be misleading".to_string());
    }
    println!("| side | profile | precision | p50 (ms) | p95 (ms) | size (B) |");
    for (side, r) in [("A", &a), ("B", &b)] {
        println!(
            "| {side} | {} | {} | {:.3} | {:.3} | {} |",
            name_str(&r["config"]["profile"]),
            name_str(&r["config"]["precision"]),
            r["timings_ms"]["p50"].as_f64().unwrap_or(0.0),
            r["timings_ms"]["p95"].as_f64().unwrap_or(0.0),
            r["model_size_bytes"].as_u64().unwrap_or(0),
        );
    }
    let a_outs = a["outputs"].as_array().cloned().unwrap_or_default();
    let b_outs = b["outputs"].as_array().cloned().unwrap_or_default();
    println!("| output | max |VoutA - outB| | mean |.| | rel (max/rmsB) |");
    for ao in &a_outs {
        let name = name_str(&ao["name"]);
        let Some(bo) = b_outs.iter().find(|o| name_str(&o["name"]) == name) else {
            println!("| {name} | — (missing in B) | — | — |");
            continue;
        };
        let (Some(va), Some(vb)) = (float_vec(&ao["values"]), float_vec(&bo["values"])) else {
            println!("| {name} | — (values not stored) | — | — |");
            continue;
        };
        if va.len() != vb.len() {
            println!(
                "| {name} | — (length {} vs {}) | — | — |",
                va.len(),
                vb.len()
            );
            continue;
        }
        let max_abs = va
            .iter()
            .zip(&vb)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        let mean_abs =
            va.iter().zip(&vb).map(|(x, y)| (x - y).abs()).sum::<f64>() / va.len().max(1) as f64;
        let rms_b = (vb.iter().map(|v| v * v).sum::<f64>() / vb.len().max(1) as f64).sqrt();
        let rel = max_abs / (rms_b + 1e-12);
        println!("| {name} | {max_abs:.6} | {mean_abs:.6} | {rel:.6} |");
    }
    Ok(())
}

fn name_str(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or("?").to_string()
}

fn float_vec(v: &serde_json::Value) -> Option<Vec<f64>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_f64())
        .collect::<Option<Vec<_>>>()
}

/// Best-effort machine description. Every benchmark number is scoped to this.
fn machine_header() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("os".to_string(), std::env::consts::OS.to_string());
    env.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    env.insert("cpu".to_string(), cpu_model());
    env.insert("rustc".to_string(), rustc_version());
    env
}

fn cpu_model() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in text.lines() {
                if let Some(model) = line.strip_prefix("model name") {
                    return model.trim_start_matches([':', ' ', '\t']).to_string();
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("sysctl")
            .arg("-n")
            .arg("machdep.cpu.brand_string")
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(id) = std::env::var("PROCESSOR_IDENTIFIER") {
            if !id.trim().is_empty() {
                return id;
            }
        }
    }
    "unknown".to_string()
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| "unknown (no rustc on PATH)".to_string())
}

/// Peak resident set size in bytes, Linux only (`VmHWM`).
fn peak_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb * 1024);
            }
        }
        return None;
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_parser_accepts_four_dims() {
        assert_eq!(parse_shape("1,3,10,56").unwrap(), [1, 3, 10, 56]);
    }

    #[test]
    fn shape_parser_rejects_bad_input() {
        assert!(parse_shape("1,3,10").is_err());
        assert!(parse_shape("1,3,10,0").is_err());
        assert!(parse_shape("a,b,c,d").is_err());
    }

    #[test]
    fn percentile_edges() {
        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), 3.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.0), 1.0);
    }
}
