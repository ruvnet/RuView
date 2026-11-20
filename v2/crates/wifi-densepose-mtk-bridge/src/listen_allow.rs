//! Source allowlist for the live UDP path (the ADR-296 pattern).
//!
//! The live path is the only route to `mediatek:physical-unvalidated`, and it
//! trusts whatever reaches its socket. This narrows that to sources the operator
//! named, mirroring `UdpSourceAllowlist` in
//! `wifi-densepose-sensing-server/src/udp_bind.rs:101-215` — same entry syntax,
//! same masking, same family-mismatch-never-matches rule.
//!
//! Two deliberate differences from the sensing server's version:
//!
//! 1. **An empty allowlist is not "inactive", it is refusal.** The server can
//!    fall back on its bind-scope decision; this bridge has no such second gate,
//!    so `--listen` without `--listen-allow` exits rather than listening to
//!    everything.
//! 2. **Loopback is not implicitly allowed.** Since an allowlist is mandatory
//!    here the operator always states intent, and an implicit hole would be both
//!    a silent exception and untestable. Write `--listen-allow 127.0.0.1` to
//!    accept loopback.

use std::net::IpAddr;

use crate::BridgeError;

/// Upper bound on entries, so a pathological flag cannot grow the scan.
pub const MAX_ENTRIES: usize = 64;

/// One parsed `IP` or `IP/prefix` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CidrEntry {
    network: IpAddr,
    prefix_len: u8,
}

impl CidrEntry {
    fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(net), IpAddr::V4(addr)) => {
                let mask = v4_mask(self.prefix_len);
                (u32::from(net) & mask) == (u32::from(addr) & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(addr)) => {
                let mask = v6_mask(self.prefix_len);
                (u128::from(net) & mask) == (u128::from(addr) & mask)
            }
            _ => false,
        }
    }
}

fn v4_mask(prefix: u8) -> u32 {
    match prefix {
        0 => 0,
        p if p >= 32 => u32::MAX,
        p => u32::MAX << (32 - p),
    }
}

fn v6_mask(prefix: u8) -> u128 {
    match prefix {
        0 => 0,
        p if p >= 128 => u128::MAX,
        p => u128::MAX << (128 - p),
    }
}

fn parse_entry(raw: &str) -> Result<CidrEntry, BridgeError> {
    let s = raw.trim();
    let bad = || BridgeError::BadListenAllow(s.to_string());
    match s.split_once('/') {
        Some((ip_str, pfx_str)) => {
            let ip: IpAddr = ip_str.trim().parse().map_err(|_| bad())?;
            let max = if ip.is_ipv4() { 32u8 } else { 128u8 };
            let prefix_len: u8 = pfx_str.trim().parse().map_err(|_| bad())?;
            if prefix_len > max {
                return Err(bad());
            }
            Ok(CidrEntry {
                network: ip,
                prefix_len,
            })
        }
        None => {
            let ip: IpAddr = s.parse().map_err(|_| bad())?;
            Ok(CidrEntry {
                network: ip,
                prefix_len: if ip.is_ipv4() { 32 } else { 128 },
            })
        }
    }
}

/// Which sources may feed the live UDP path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceAllowlist {
    entries: Vec<CidrEntry>,
}

impl SourceAllowlist {
    /// Parse `--listen-allow` values. Each may itself be comma-separated;
    /// whitespace and empty items are ignored. The first malformed entry is an
    /// error rather than being silently dropped.
    pub fn parse<I, S>(specs: I) -> Result<Self, BridgeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut entries: Vec<CidrEntry> = Vec::new();
        for spec in specs {
            for part in spec.as_ref().split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if entries.len() >= MAX_ENTRIES {
                    return Err(BridgeError::TooManyListenAllowEntries(MAX_ENTRIES));
                }
                entries.push(parse_entry(part)?);
            }
        }
        if entries.is_empty() {
            return Err(BridgeError::ListenNeedsAllowlist);
        }
        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn allows(&self, ip: IpAddr) -> bool {
        self.entries.iter().any(|e| e.contains(ip))
    }
}

/// Applies a [`SourceAllowlist`] and counts what it turned away.
#[derive(Debug, Default)]
pub struct SourceFilter {
    allow: SourceAllowlist,
    rejected_source: u64,
}

impl SourceFilter {
    pub fn new(allow: SourceAllowlist) -> Self {
        Self {
            allow,
            rejected_source: 0,
        }
    }

    /// True when this datagram may be decoded. A rejected source is counted and
    /// never reaches the decoder or the assembler.
    pub fn admit(&mut self, ip: IpAddr) -> bool {
        if self.allow.allows(ip) {
            return true;
        }
        self.rejected_source += 1;
        false
    }

    pub fn rejected_source(&self) -> u64 {
        self.rejected_source
    }

    /// True the first time a source is turned away, for a one-line log.
    pub fn is_first_rejection(&self) -> bool {
        self.rejected_source == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn an_empty_allowlist_is_refusal_not_permission() {
        assert!(matches!(
            SourceAllowlist::parse(Vec::<String>::new()),
            Err(BridgeError::ListenNeedsAllowlist)
        ));
        assert!(matches!(
            SourceAllowlist::parse([" , ,  "]),
            Err(BridgeError::ListenNeedsAllowlist)
        ));
    }

    #[test]
    fn a_v4_prefix_matches_only_inside_the_network() {
        let a = SourceAllowlist::parse(["192.168.1.0/24"]).unwrap();
        assert!(a.allows(ip("192.168.1.1")));
        assert!(a.allows(ip("192.168.1.255")));
        assert!(!a.allows(ip("192.168.2.1")));
        assert!(!a.allows(ip("127.0.0.1")));
    }

    /// Loopback is deliberately not implicit here; see the module docs.
    #[test]
    fn loopback_is_not_allowed_unless_named() {
        let a = SourceAllowlist::parse(["10.0.0.0/8"]).unwrap();
        assert!(!a.allows(ip("127.0.0.1")));
        let b = SourceAllowlist::parse(["127.0.0.1"]).unwrap();
        assert!(b.allows(ip("127.0.0.1")));
        assert!(!b.allows(ip("127.0.0.2")));
    }

    #[test]
    fn a_bare_address_is_a_host_route() {
        let a = SourceAllowlist::parse(["10.0.0.5"]).unwrap();
        assert!(a.allows(ip("10.0.0.5")));
        assert!(!a.allows(ip("10.0.0.6")));
    }

    #[test]
    fn entries_may_be_comma_separated_or_repeated() {
        let a = SourceAllowlist::parse(["192.168.1.0/24, 10.0.0.5", "172.16.0.0/12"]).unwrap();
        assert!(a.allows(ip("192.168.1.9")));
        assert!(a.allows(ip("10.0.0.5")));
        assert!(a.allows(ip("172.16.4.4")));
        assert!(!a.allows(ip("8.8.8.8")));
    }

    #[test]
    fn address_families_never_cross() {
        let v6 = SourceAllowlist::parse(["2001:db8::/32"]).unwrap();
        assert!(v6.allows(ip("2001:db8::1")));
        assert!(!v6.allows(ip("192.168.1.1")));
        let v4 = SourceAllowlist::parse(["0.0.0.0/0"]).unwrap();
        assert!(v4.allows(ip("8.8.8.8")));
        assert!(!v4.allows(ip("2001:db8::1")));
    }

    #[test]
    fn malformed_entries_are_rejected_not_ignored() {
        for bad in ["not-an-ip", "192.168.1.0/33", "192.168.1.0/x", "/24"] {
            assert!(
                matches!(
                    SourceAllowlist::parse([bad]),
                    Err(BridgeError::BadListenAllow(_))
                ),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn too_many_entries_is_an_error() {
        let many: Vec<String> = (0..MAX_ENTRIES + 1)
            .map(|n| format!("10.0.{n}.1"))
            .collect();
        assert!(matches!(
            SourceAllowlist::parse(many),
            Err(BridgeError::TooManyListenAllowEntries(MAX_ENTRIES))
        ));
    }

    /// The load-bearing one: an outside source is counted and never admitted.
    #[test]
    fn a_source_outside_the_allowlist_is_dropped_and_counted() {
        let mut filter = SourceFilter::new(SourceAllowlist::parse(["192.168.1.0/24"]).unwrap());
        assert!(filter.admit(ip("192.168.1.10")));
        assert_eq!(filter.rejected_source(), 0);

        assert!(!filter.admit(ip("192.168.2.10")));
        assert!(filter.is_first_rejection());
        assert!(!filter.admit(ip("127.0.0.1")));
        assert!(!filter.admit(ip("8.8.8.8")));
        assert_eq!(filter.rejected_source(), 3);

        // Admitting a legitimate source afterwards does not reset the count.
        assert!(filter.admit(ip("192.168.1.99")));
        assert_eq!(filter.rejected_source(), 3);
    }
}
