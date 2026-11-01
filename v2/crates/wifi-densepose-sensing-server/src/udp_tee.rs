//! ADR-380: raw-datagram tee for the UDP data plane.
//!
//! The sensing server owns the CSI UDP port, and its WebSocket/REST outputs
//! carry amplitude only. A second consumer (a fusion host, a Cognitum cog, a
//! recorder) that needs full I/Q, sequence numbers and the `0xC511A110` sync
//! packets gets them from this tee: every datagram the source allowlist admits
//! is copied unchanged to each configured target.
//!
//! Targets are loopback-only, so the tee can never send raw CSI off the host,
//! and may not use the listener's own port, which would loop datagrams back
//! into the receiver (loopback is always admitted by the allowlist). Sends are
//! non-blocking; a datagram that cannot be queued is dropped and counted rather
//! than stalling the receive loop.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};

/// Upper bound on tee targets; each admitted datagram costs one send per target.
pub const MAX_TEE_TARGETS: usize = 4;

/// Parse tee targets from CLI/env specs. Each item may be a comma-separated
/// list of `ip:port`. Returns an error on the first unusable entry rather than
/// silently dropping it.
pub fn parse_targets<I, S>(specs: I, listen_port: u16) -> Result<Vec<SocketAddr>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut targets: Vec<SocketAddr> = Vec::new();
    for spec in specs {
        for part in spec.as_ref().split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let addr: SocketAddr = part
                .parse()
                .map_err(|_| format!("tee target '{part}' is not ip:port"))?;
            if !addr.ip().is_loopback() {
                return Err(format!("tee target '{part}' must be a loopback address"));
            }
            if addr.port() == 0 || addr.port() == listen_port {
                return Err(format!(
                    "tee target '{part}' may not use port 0 or the listener port {listen_port}"
                ));
            }
            if targets.contains(&addr) {
                continue;
            }
            if targets.len() >= MAX_TEE_TARGETS {
                return Err(format!("too many tee targets (limit {MAX_TEE_TARGETS})"));
            }
            targets.push(addr);
        }
    }
    Ok(targets)
}

/// Non-blocking forwarder that copies admitted datagrams to loopback targets.
#[derive(Debug)]
pub struct UdpTee {
    v4: Option<UdpSocket>,
    v6: Option<UdpSocket>,
    targets: Vec<SocketAddr>,
    forwarded: AtomicU64,
    dropped: AtomicU64,
}

impl UdpTee {
    /// Bind one ephemeral loopback socket per address family in use.
    pub fn bind(targets: Vec<SocketAddr>) -> std::io::Result<Self> {
        let open = |ip: std::net::IpAddr| -> std::io::Result<UdpSocket> {
            let socket = UdpSocket::bind(SocketAddr::new(ip, 0))?;
            socket.set_nonblocking(true)?;
            Ok(socket)
        };
        let v4 = if targets.iter().any(SocketAddr::is_ipv4) {
            Some(open(Ipv4Addr::LOCALHOST.into())?)
        } else {
            None
        };
        let v6 = if targets.iter().any(SocketAddr::is_ipv6) {
            Some(open(Ipv6Addr::LOCALHOST.into())?)
        } else {
            None
        };
        Ok(Self {
            v4,
            v6,
            targets,
            forwarded: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        })
    }

    /// Copy one datagram to every target. Never blocks.
    pub fn forward(&self, datagram: &[u8]) {
        for target in &self.targets {
            let socket = if target.is_ipv4() { &self.v4 } else { &self.v6 };
            let sent = socket
                .as_ref()
                .is_some_and(|s| s.send_to(datagram, target).is_ok());
            let counter = if sent { &self.forwarded } else { &self.dropped };
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Configured targets, for the startup summary.
    pub fn targets(&self) -> &[SocketAddr] {
        &self.targets
    }

    /// Datagram copies successfully handed to the OS.
    pub fn forwarded(&self) -> u64 {
        self.forwarded.load(Ordering::Relaxed)
    }

    /// Datagram copies that could not be sent (buffer full, no listener error).
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parses_loopback_targets_and_ignores_blanks_and_duplicates() {
        let t = parse_targets(["127.0.0.1:5007, ,127.0.0.1:5007", "[::1]:5008"], 5006).unwrap();
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn rejects_non_loopback_targets() {
        let err = parse_targets(["192.0.2.10:5007"], 5006).unwrap_err();
        assert!(err.contains("loopback"), "{err}");
    }

    #[test]
    fn rejects_the_listener_port_to_prevent_a_loop() {
        assert!(parse_targets(["127.0.0.1:5006"], 5006).is_err());
        assert!(parse_targets(["127.0.0.1:0"], 5006).is_err());
    }

    #[test]
    fn rejects_malformed_and_excess_targets() {
        assert!(parse_targets(["localhost:5007"], 5006).is_err());
        let many: Vec<String> = (0..=MAX_TEE_TARGETS)
            .map(|i| format!("127.0.0.1:{}", 6000 + i))
            .collect();
        assert!(parse_targets(many, 5006).is_err());
    }

    #[test]
    fn empty_spec_means_no_tee() {
        assert!(parse_targets(Vec::<String>::new(), 5006)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn forwards_datagrams_byte_for_byte() {
        let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
        sink.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let tee = UdpTee::bind(vec![sink.local_addr().unwrap()]).unwrap();

        let datagram = [0x01, 0x00, 0x11, 0xC5, 7, 1, 64, 0, 0xFF];
        tee.forward(&datagram);

        let mut buf = [0u8; 64];
        let (len, _) = sink.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..len], &datagram);
        assert_eq!(tee.forwarded(), 1);
        assert_eq!(tee.dropped(), 0);
    }
}
