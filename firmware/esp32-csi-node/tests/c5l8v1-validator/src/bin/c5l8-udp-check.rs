//! Bench check for live UDP captures: reads `... len=N magic=XXXXXXXX hex=<..>` lines (one per
//! datagram received on port 5005) and validates every C5L8/v1-magic datagram with the real server
//! validator. Other datagrams are counted by magic and size, never decoded. Exit 1 if any C5L8/v1
//! datagram fails or none were found.
use std::collections::BTreeMap;
use std::io::BufRead;

/// ADR-018 C5L8/v1 magic as printed big-endian by the capture script.
const C5L8_MAGIC_HEX: &str = "c5110001";

fn main() {
    let (mut ok, mut bad) = (0u32, 0u32);
    let mut other: BTreeMap<(String, usize), u32> = BTreeMap::new();
    let mut flags: BTreeMap<u8, u32> = BTreeMap::new();
    let mut freqs: BTreeMap<u64, u32> = BTreeMap::new();
    let mut seqs: Vec<u32> = Vec::new();
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Some(hex) = line.split_whitespace().find_map(|t| t.strip_prefix("hex=")) else {
            continue;
        };
        let magic = line
            .split_whitespace()
            .find_map(|t| t.strip_prefix("magic="))
            .unwrap_or("?");
        if magic != C5L8_MAGIC_HEX {
            *other.entry((magic.to_owned(), hex.len() / 2)).or_default() += 1;
            continue;
        }
        let checked = ruview_c5l8v1_check::hex_decode(hex)
            .and_then(|w| ruview_c5l8v1_check::validate_c5l8(&w).map(|p| (w, p)));
        match checked {
            Ok((w, p)) => {
                ok += 1;
                *flags.entry(w[19]).or_default() += 1;
                *freqs.entry(p.frequency_hz).or_default() += 1;
                seqs.push(u32::from_le_bytes([w[12], w[13], w[14], w[15]]));
                if ok <= 3 {
                    println!(
                        "PASS {} bytes flags=0x{:02x} node={} freq={} grid={}",
                        w.len(),
                        w[19],
                        p.source_id,
                        p.frequency_hz,
                        p.grid_id
                    );
                }
            }
            Err(e) => {
                bad += 1;
                if bad <= 10 {
                    println!("FAIL {e}: {}", &hex[..hex.len().min(80)]);
                }
            }
        }
    }
    let gaps: u64 = seqs
        .windows(2)
        .map(|w| u64::from(w[1].wrapping_sub(w[0]).saturating_sub(1)))
        .sum();
    println!("flags {flags:?} freq_hz {freqs:?} seq_gaps {gaps}");
    for ((magic, len), n) in &other {
        println!("other magic={magic} len={len} count={n}");
    }
    println!("c5l8-udp-check: {ok} passed, {bad} failed");
    std::process::exit(if bad == 0 && ok > 0 { 0 } else { 1 });
}
