//! `veil` — a custom, dependency-free terminal harness for the VEIL privacy
//! shield (ADR-288). It is the in-repo, native counterpart to the npm
//! metaharness (`harness/wifi-densepose-privshield/`, ADR-289): where that one
//! assists *development*, this one *drives the model* — an interactive TUI plus
//! scriptable subcommands over the same crate API the tests use.
//!
//! Std-only on purpose: no `crossterm`/`ratatui`, no external deps. The TUI is
//! a command-driven ANSI dashboard (line input, redraw on change), which keeps
//! the crate a pure leaf and lets the harness run in any pipe or CI.
//!
//! ```text
//! veil                 # TUI if attached to a terminal, else a one-shot report
//! veil tui             # force the interactive dashboard
//! veil report          # print the dashboard once (plain, pipe-friendly)
//! veil sweep           # re-ID vs passes and throughput vs bits tables
//! veil optimize        # run the hyper-optimizer, print the recommendation
//! veil adaptive <N>    # derive the shield for a room of N candidate identities
//! veil proof           # verify the deterministic witness
//! veil doctor          # self-check (exit 0 = healthy)
//! ```
//!
//! All numbers are **SYNTHETIC / L0** — reproduced by `cargo test`, describing
//! the model, not real hardware.

use std::io::{self, BufRead, IsTerminal, Write};

use veil::optimize;
use veil::{run, ExperimentConfig, ExperimentReport, Metric, Proof};
use wifi_densepose_privshield as veil;

// ---- ANSI palette (matches the VEIL Console: teal shield, amber threat) ----
const TEAL: &str = "\x1b[38;2;32;211;192m";
const AMBER: &str = "\x1b[38;2;245;158;75m";
const GOOD: &str = "\x1b[38;2;62;207;142m";
const CRIT: &str = "\x1b[38;2;242;107;111m";
const MUTE: &str = "\x1b[38;2;139;160;159m";
const BOLD: &str = "\x1b[1m";
const RST: &str = "\x1b[0m";
const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Candidate-identity domain the model supports. A room must name at least two
/// candidates to be a re-identification problem at all, and
/// [`veil::adaptive_shield`] runs one full experiment per candidate while it
/// searches the pass budget, so the upper bound keeps `adaptive` bounded.
const MIN_IDENTITIES: usize = 2;
const MAX_IDENTITIES: usize = 128;

/// Parse the `N` of `veil adaptive <N>`.
///
/// The count is user input, so it is checked against the supported domain
/// before it reaches the model: below [`MIN_IDENTITIES`] the scene has no
/// identity to protect, and above [`MAX_IDENTITIES`] the shield search is no
/// longer bounded. A value that does not parse is an error rather than a
/// silent fallback to the default, so the answer always describes the `N` the
/// operator actually asked for.
fn parse_identity_count(raw: &str) -> Result<usize, String> {
    match raw.parse::<usize>() {
        Ok(n) if (MIN_IDENTITIES..=MAX_IDENTITIES).contains(&n) => Ok(n),
        Ok(n) => Err(format!(
            "N={n} is outside {MIN_IDENTITIES}..={MAX_IDENTITIES} candidate identities"
        )),
        Err(_) => Err(format!(
            "'{raw}' is not a candidate-identity count ({MIN_IDENTITIES}..={MAX_IDENTITIES})"
        )),
    }
}

/// Emit color for a real terminal; never when `NO_COLOR` is set; always when
/// `CLICOLOR_FORCE` is set (so piped captures keep their color).
fn color_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var_os("CLICOLOR_FORCE").is_some() {
        return true;
    }
    io::stdout().is_terminal()
}

/// Wrap `s` in `code` when color is on.
fn c(s: &str, code: &str, on: bool) -> String {
    if on {
        format!("{code}{s}{RST}")
    } else {
        s.to_string()
    }
}

/// A raw color code, or "" when color is off — for inline `format!` colouring.
fn k(code: &'static str, on: bool) -> &'static str {
    if on {
        code
    } else {
        ""
    }
}

/// Shield-on re-ID at a given mixing budget, holding the rest of `cfg`.
fn reid_at(cfg: &ExperimentConfig, passes: usize) -> f32 {
    let mut c = cfg.clone();
    c.shield.givens_passes = passes;
    run(&c).accuracy_shield_on
}

/// A block-sparkline character for a value in `[0, 1]`.
fn spark(v: f32) -> char {
    let i = (v.clamp(0.0, 1.0) * 7.0).round() as usize;
    BLOCKS[i.min(7)]
}

/// Render the full dashboard as colored lines (left-bar panel; no right border,
/// so ANSI escape width never has to be counted).
fn dashboard(cfg: &ExperimentConfig, on: bool) -> Vec<String> {
    let rep: ExperimentReport = run(cfg);
    let chance = rep.chance_level * 100.0;
    let off = rep.accuracy_shield_off * 100.0;
    let onp = rep.accuracy_shield_on * 100.0;
    let tp = rep.throughput_ratio * 100.0;

    let (state, scode) = if !cfg.shield.enabled {
        ("EXPOSED", CRIT)
    } else if rep.passed() {
        ("PROTECTED", GOOD)
    } else {
        ("AT RISK", AMBER)
    };
    let on_code = if onp <= rep.chance_band * 100.0 {
        GOOD
    } else {
        AMBER
    };
    let tp_code = if tp >= 95.0 { GOOD } else { CRIT };
    let bar = c("│", MUTE, on);

    let mut out = Vec::new();
    out.push(c(
        "┌──────────────────────────────────────────────────────────",
        MUTE,
        on,
    ));
    out.push(format!(
        "{}  {}{}VEIL{} {}· wifi-sensing privacy shield{}        {}● {}{}",
        bar,
        k(BOLD, on),
        k(TEAL, on),
        k(RST, on),
        k(MUTE, on),
        k(RST, on),
        k(scode, on),
        state,
        k(RST, on),
    ));
    out.push(bar.clone());
    out.push(format!(
        "{}  re-ID off  {}{:>6.1}%{}     re-ID on  {}{:>5.1}%{}   {}(chance {:.2}%){}",
        bar,
        k(AMBER, on),
        off,
        k(RST, on),
        k(on_code, on),
        onp,
        k(RST, on),
        k(MUTE, on),
        chance,
        k(RST, on),
    ));
    out.push(format!(
        "{}  throughput {}{:>6.1}%{}     emission  {}{:.3}×{}   {}· not jamming{}",
        bar,
        k(tp_code, on),
        tp,
        k(RST, on),
        k(GOOD, on),
        rep.compliance.energy_ratio,
        k(RST, on),
        k(MUTE, on),
        k(RST, on),
    ));
    out.push(bar.clone());

    let cand = optimize::PASS_CANDIDATES;
    let line: String = cand.iter().map(|&p| spark(reid_at(cfg, p))).collect();
    out.push(format!(
        "{}  {}collapse{}  {}{}{}   {}passes {}→{} · op {}{}",
        bar,
        k(MUTE, on),
        k(RST, on),
        k(TEAL, on),
        line,
        k(RST, on),
        k(MUTE, on),
        cand[0],
        cand[cand.len() - 1],
        cfg.shield.givens_passes,
        k(RST, on),
    ));
    out.push(bar.clone());

    let metric = match cfg.attacker_metric {
        Metric::Euclidean => "euclid",
        Metric::Cosine => "cosine",
    };
    out.push(format!(
        "{}  {}config{}   passes {} · bits {} · N {} · snr {:.0}dB · {}",
        bar,
        k(MUTE, on),
        k(RST, on),
        cfg.shield.givens_passes,
        cfg.shield.feedback_bits,
        cfg.scene.identities,
        cfg.link.snr_db,
        metric,
    ));
    let (vlabel, vcode) = if !cfg.shield.enabled {
        ("SHIELD OFF — room exposed", CRIT)
    } else if rep.passed() {
        (
            "✓ PASS — re-ID at chance · throughput ≥95% · compliant",
            GOOD,
        )
    } else {
        (
            "△ OUT OF SPEC — raise passes/bits to re-enter the chance band",
            AMBER,
        )
    };
    out.push(format!(
        "{}  {}verdict{}  {}{}{}",
        bar,
        k(MUTE, on),
        k(RST, on),
        k(vcode, on),
        vlabel,
        k(RST, on)
    ));
    out.push(c(
        "└──────────────────────────────────────────────────────────",
        MUTE,
        on,
    ));
    out
}

/// Deployment presets (mirror `optimize::adaptive_shield` results per room).
fn preset(name: &str, cfg: &mut ExperimentConfig) -> bool {
    let (n, passes, bits, snr) = match name {
        "scif" => (64, 96, 5, 20.0),
        "board" => (16, 96, 5, 25.0),
        "ward" => (32, 96, 5, 15.0),
        "hotel" => (48, 64, 5, 20.0),
        _ => return false,
    };
    cfg.scene.identities = n;
    cfg.shield.givens_passes = passes;
    cfg.shield.feedback_bits = bits;
    cfg.link.snr_db = snr;
    true
}

fn print_dashboard(cfg: &ExperimentConfig, on: bool) {
    for l in dashboard(cfg, on) {
        println!("{l}");
    }
}

fn cmd_sweep(cfg: &ExperimentConfig, on: bool) {
    println!(
        "{}re-ID (shield on) vs Givens passes — N={}{}",
        k(MUTE, on),
        cfg.scene.identities,
        k(RST, on)
    );
    for &p in &optimize::PASS_CANDIDATES {
        let robust =
            optimize::passes_collapse_at_n(cfg, p, cfg.shield.feedback_bits, cfg.scene.identities);
        println!(
            "  passes {:>3}  re-ID {:>5.1}%  {}",
            p,
            reid_at(cfg, p) * 100.0,
            if robust {
                c("collapses", GOOD, on)
            } else {
                c("above chance", AMBER, on)
            }
        );
    }
    println!(
        "\n{}throughput vs feedback bits — snr={:.0}dB{}",
        k(MUTE, on),
        cfg.link.snr_db,
        k(RST, on)
    );
    for bits in 1..=12u32 {
        let mut s = cfg.shield.clone();
        s.feedback_bits = bits;
        let tp = cfg.link.throughput_ratio(&s) * 100.0;
        let barlen = ((tp - 90.0).clamp(0.0, 10.0) / 10.0 * 24.0) as usize;
        println!(
            "  {:>2} bit  {:>6.3}%  {}{}{}",
            bits,
            tp,
            k(TEAL, on),
            "█".repeat(barlen),
            k(RST, on)
        );
    }
    let (sb, _) = optimize::spec_optimal_feedback_bits(cfg);
    println!("  {}spec-optimal: {} bit{}", k(MUTE, on), sb, k(RST, on));
}

fn cmd_optimize(cfg: &ExperimentConfig, on: bool) {
    let opt = veil::hyper_optimize(cfg);
    let r = &opt.report;
    println!("{}hyper-optimizer{}", k(BOLD, on), k(RST, on));
    println!("  min robust passes : {}", opt.min_passes);
    println!(
        "  shipped passes    : {}  {}(min × 2 margin, throughput-free){}",
        opt.shipped_passes,
        k(MUTE, on),
        k(RST, on)
    );
    println!(
        "  spec-optimal bits : {}  {}(model optimum {}){}",
        opt.spec_optimal_bits,
        k(MUTE, on),
        opt.model_optimal_bits,
        k(RST, on)
    );
    println!(
        "  result            : re-ID {}{:.1}%{} · throughput {}{:.1}%{} · {}",
        k(GOOD, on),
        r.accuracy_shield_on * 100.0,
        k(RST, on),
        k(GOOD, on),
        r.throughput_ratio * 100.0,
        k(RST, on),
        if r.passed() {
            c("PASS", GOOD, on)
        } else {
            c("FAIL", CRIT, on)
        }
    );
    println!(
        "  {}SNR → model-optimal bits: {:?}{}",
        k(MUTE, on),
        optimize::optimal_bits_across_snr(cfg),
        k(RST, on)
    );
}

fn cmd_adaptive(cfg: &ExperimentConfig, n: usize, on: bool) {
    let sh = veil::adaptive_shield(cfg, n);
    println!(
        "adaptive shield for N={}: passes {} · bits {}  {}(mixing budget is N-independent in this model){}",
        n, sh.givens_passes, sh.feedback_bits, k(MUTE, on), k(RST, on)
    );
}

fn cmd_proof(on: bool) -> i32 {
    let w = Proof::witness(&Proof::run_reference());
    let ok = w == Proof::EXPECTED_WITNESS;
    println!(
        "witness {:#018x}  expected {:#018x}  {}",
        w,
        Proof::EXPECTED_WITNESS,
        if ok {
            c("MATCH", GOOD, on)
        } else {
            c("DRIFT", CRIT, on)
        }
    );
    i32::from(!ok)
}

fn cmd_doctor(on: bool) -> i32 {
    let rep = run(&ExperimentConfig::default());
    let checks = [
        ("reference experiment passes", rep.passed()),
        (
            "attack is real without shield",
            rep.attack_is_effective_without_shield(),
        ),
        ("collapse drives to chance", rep.drives_to_chance()),
        ("throughput ≥ 95%", rep.preserves_throughput()),
        ("emission is compliant", rep.compliance.is_compliant()),
        (
            "deterministic witness matches",
            Proof::witness(&Proof::run_reference()) == Proof::EXPECTED_WITNESS,
        ),
    ];
    let mut ok = true;
    for (label, pass) in checks {
        ok &= pass;
        println!(
            "{} {label}",
            if pass {
                c("PASS", GOOD, on)
            } else {
                c("FAIL", CRIT, on)
            }
        );
    }
    println!(
        "\nveil doctor: {}",
        if ok {
            c("all checks passed", GOOD, on)
        } else {
            c("problems found", CRIT, on)
        }
    );
    i32::from(!ok)
}

fn help() {
    println!(
        "veil — VEIL privacy-shield harness (SYNTHETIC / L0)\n\n\
         USAGE\n  veil [command]\n\n\
         COMMANDS\n\
         \x20 tui              interactive dashboard (default on a terminal)\n\
         \x20 report           print the dashboard once\n\
         \x20 sweep            re-ID vs passes + throughput vs bits\n\
         \x20 optimize         run the hyper-optimizer\n\
         \x20 adaptive <N>     derive the shield for N candidate identities (2..=128)\n\
         \x20 proof            verify the deterministic witness\n\
         \x20 doctor           self-check (exit 0 = healthy)\n\
         \x20 help             this text\n\n\
         TUI COMMANDS (type + Enter)\n\
         \x20 on | off         toggle the shield\n\
         \x20 passes <n> · bits <n> · n <k> · snr <db>\n\
         \x20 metric euclid|cosine\n\
         \x20 preset scif|board|ward|hotel\n\
         \x20 run | optimize | proof | help | quit"
    );
}

fn tui(mut cfg: ExperimentConfig, on: bool) {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    let redraw = |cfg: &ExperimentConfig, msg: &str| {
        if interactive {
            print!("\x1b[2J\x1b[H");
        }
        print_dashboard(cfg, on);
        if !msg.is_empty() {
            println!("  {}{}{}", k(MUTE, on), msg, k(RST, on));
        }
        print!("{}veil›{} ", k(TEAL, on), k(RST, on));
        let _ = io::stdout().flush();
    };
    redraw(&cfg, "type `help` for commands");
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let mut it = line.split_whitespace();
        let cmd = it.next().unwrap_or("");
        let arg = it.next().unwrap_or("");
        let mut msg = String::new();
        match cmd {
            "" => {}
            "quit" | "q" | "exit" => break,
            "help" | "h" => {
                if interactive {
                    print!("\x1b[2J\x1b[H");
                }
                help();
                continue;
            }
            "on" => cfg.shield.enabled = true,
            "off" => cfg.shield.enabled = false,
            "passes" => match arg.parse::<usize>() {
                Ok(v) => cfg.shield.givens_passes = v.clamp(1, 512),
                Err(_) => msg = "passes: need a number".into(),
            },
            "bits" => match arg.parse::<u32>() {
                Ok(v) => cfg.shield.feedback_bits = v.clamp(1, 12),
                Err(_) => msg = "bits: need 1..12".into(),
            },
            "n" => match arg.parse::<usize>() {
                Ok(v) => cfg.scene.identities = v.clamp(MIN_IDENTITIES, MAX_IDENTITIES),
                Err(_) => msg = format!("n: need {MIN_IDENTITIES}..={MAX_IDENTITIES}"),
            },
            "snr" => match arg.parse::<f64>() {
                Ok(v) => cfg.link.snr_db = v.clamp(0.0, 60.0),
                Err(_) => msg = "snr: need a number (dB)".into(),
            },
            "metric" => match arg {
                "euclid" | "euclidean" => cfg.attacker_metric = Metric::Euclidean,
                "cosine" | "cos" => cfg.attacker_metric = Metric::Cosine,
                _ => msg = "metric: euclid | cosine".into(),
            },
            "preset" => {
                if !preset(arg, &mut cfg) {
                    msg = "preset: scif | board | ward | hotel".into();
                }
            }
            "run" => msg = "ran — numbers above reflect current settings".into(),
            "optimize" | "opt" => {
                cfg.shield = veil::hyper_optimize(&cfg).shield;
                msg = format!(
                    "optimized → passes {} · bits {}",
                    cfg.shield.givens_passes, cfg.shield.feedback_bits
                );
            }
            "proof" => {
                let w = Proof::witness(&Proof::run_reference());
                msg = format!(
                    "witness {:#018x} ({})",
                    w,
                    if w == Proof::EXPECTED_WITNESS {
                        "match"
                    } else {
                        "drift"
                    }
                );
            }
            other => msg = format!("unknown: {other} (try `help`)"),
        }
        redraw(&cfg, &msg);
    }
    if interactive {
        println!();
    }
}

fn main() {
    let on = color_enabled();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = ExperimentConfig::default();
    let code = match args.first().map(String::as_str).unwrap_or("") {
        "" => {
            if io::stdout().is_terminal() {
                tui(cfg, on);
            } else {
                print_dashboard(&cfg, on);
            }
            0
        }
        "tui" => {
            tui(cfg, on);
            0
        }
        "report" => {
            print_dashboard(&cfg, on);
            0
        }
        "sweep" => {
            cmd_sweep(&cfg, on);
            0
        }
        "optimize" | "opt" => {
            cmd_optimize(&cfg, on);
            0
        }
        "adaptive" => match args.get(1).map(String::as_str) {
            Some(raw) => match parse_identity_count(raw) {
                Ok(n) => {
                    cmd_adaptive(&cfg, n, on);
                    0
                }
                Err(message) => {
                    eprintln!("veil adaptive: {message}");
                    2
                }
            },
            // No count at all: fall back to the configured scene, which is
            // inside the domain by construction (SceneConfig::default).
            None => {
                cmd_adaptive(&cfg, cfg.scene.identities, on);
                0
            }
        },
        "proof" => cmd_proof(on),
        "doctor" => cmd_doctor(on),
        "help" | "-h" | "--help" => {
            help();
            0
        }
        other => {
            eprintln!("unknown command: {other}. Try `veil help`.");
            2
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_count_accepts_the_supported_domain() {
        for n in [MIN_IDENTITIES, 3, 16, 32, 64, MAX_IDENTITIES] {
            assert_eq!(parse_identity_count(&n.to_string()), Ok(n));
        }
    }

    /// `veil adaptive 0` used to reach `Channel::observe`, which indexes a
    /// signature vector that `SceneConfig::identities == 0` never fills — an
    /// index-out-of-bounds panic on a documented subcommand. The count is
    /// rejected at the boundary instead.
    #[test]
    fn identity_count_rejects_a_scene_with_no_candidates() {
        assert!(parse_identity_count("0").is_err());
        assert!(parse_identity_count("1").is_err());
    }

    /// The pass search runs one experiment per candidate, so an unbounded `N`
    /// is unbounded work rather than a rejected argument.
    #[test]
    fn identity_count_rejects_counts_above_the_model_bound() {
        let too_many = (MAX_IDENTITIES + 1).to_string();
        assert!(parse_identity_count(&too_many).is_err());
        assert!(parse_identity_count("4000").is_err());
    }

    #[test]
    fn identity_count_rejects_unparseable_input_instead_of_defaulting() {
        // A silent fallback would report a shield for a different room.
        assert!(parse_identity_count("abc").is_err());
        assert!(parse_identity_count("").is_err());
        assert!(parse_identity_count("-4").is_err());
    }

    /// The TUI and `adaptive` share one domain, so the two surfaces cannot
    /// drift apart on what `N` means.
    #[test]
    fn tui_clamp_matches_the_parsed_domain() {
        let clamp = |v: usize| v.clamp(MIN_IDENTITIES, MAX_IDENTITIES);
        assert_eq!(clamp(0), MIN_IDENTITIES);
        assert_eq!(clamp(1), MIN_IDENTITIES);
        assert_eq!(clamp(4000), MAX_IDENTITIES);
        assert_eq!(clamp(16), 16);
        assert_eq!(
            parse_identity_count(&MAX_IDENTITIES.to_string()),
            Ok(MAX_IDENTITIES)
        );
    }
}
