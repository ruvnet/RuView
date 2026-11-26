//! Bench check: reads console lines (e.g. captured from the C5 over USB serial) and validates every
//! `C5L8-SELFTEST-SYNTHETIC ... hex=<..>` frame with the real server validator. Exit 1 if any fail
//! or none were found.
use std::io::BufRead;
fn main() {
    let (mut ok, mut bad) = (0u32, 0u32);
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Some(pos) = line.find("C5L8-SELFTEST-SYNTHETIC") else {
            continue;
        };
        let line = &line[pos..];
        let Some(hex) = line.split_whitespace().find_map(|t| t.strip_prefix("hex=")) else {
            continue;
        };
        match ruview_c5l8v1_check::hex_decode(hex)
            .and_then(|w| ruview_c5l8v1_check::validate_c5l8(&w).map(|p| (w, p)))
        {
            Ok((w, p)) => {
                ok += 1;
                println!(
                    "PASS {} bytes flags=0x{:02x} node={} freq={} grid={}",
                    w.len(),
                    w[19],
                    p.source_id,
                    p.frequency_hz,
                    p.grid_id
                );
            }
            Err(e) => {
                bad += 1;
                println!("FAIL {e}: {line}");
            }
        }
    }
    println!("c5l8-check: {ok} passed, {bad} failed");
    std::process::exit(if bad == 0 && ok > 0 { 0 } else { 1 });
}
