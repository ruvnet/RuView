//! Local DNS-SD advertisement for RuView installation discovery.
//!
//! The TXT record deliberately contains no SSID, node identifiers, room data,
//! sensor values, credentials, or personal data. Mobile clients treat the
//! installation id as a routing hint only and still health-check the service.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mdns_sd::{DaemonEvent, ServiceDaemon, ServiceInfo};

pub const SERVICE_TYPE: &str = "_ruview._tcp.local.";
pub const SCHEMA: &str = "ruview.installation.v1";

pub struct DiscoveryAdvertiser {
    daemon: ServiceDaemon,
    fullname: String,
    recovery: RecoveryWorker,
}

impl Drop for DiscoveryAdvertiser {
    fn drop(&mut self) {
        // Join BEFORE unregister: any already-dispatched refresh precedes the
        // goodbye, and no worker can enqueue a registration after shutdown.
        self.recovery.stop();
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Reduce an operator/host label to a deterministic RFC 6762-safe hostname.
pub fn discovery_hostname(raw: &str) -> String {
    let mut label = String::with_capacity(48);
    for character in raw.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            label.push(character);
        } else if (character == '-' || character == '_' || character == ' ')
            && !label.ends_with('-')
        {
            label.push('-');
        }
        if label.len() >= 40 {
            break;
        }
    }
    let label = label.trim_matches('-');
    let safe = if label.is_empty() {
        "installation"
    } else {
        label
    };
    format!("ruview-{safe}.local.")
}

pub fn build_service(
    instance_name: &str,
    installation_id: &str,
    hostname: &str,
    http_port: u16,
    tls: bool,
) -> Result<ServiceInfo, mdns_sd::Error> {
    let mut properties = HashMap::with_capacity(3);
    properties.insert("schema".to_string(), SCHEMA.to_string());
    properties.insert("tls".to_string(), if tls { "1" } else { "0" }.to_string());
    properties.insert(
        "installation".to_string(),
        installation_id.chars().take(128).collect(),
    );
    ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name.chars().take(96).collect::<String>(),
        hostname,
        (),
        http_port,
        Some(properties),
    )
    .map(ServiceInfo::enable_addr_auto)
}

/// Register a live local advertisement. Failure is recoverable: the sensing
/// server remains available through its explicitly configured origin.
pub fn start_advertiser(
    instance_name: &str,
    installation_id: &str,
    hostname: &str,
    http_port: u16,
) -> Result<DiscoveryAdvertiser, mdns_sd::Error> {
    start_service(build_service(
        instance_name,
        installation_id,
        hostname,
        http_port,
        false,
    )?)
}

/// Advertise the QR-pinned companion TLS endpoint independently of legacy HTTP.
pub fn start_tls_advertiser(
    instance_name: &str,
    installation_id: &str,
    hostname: &str,
    port: u16,
) -> Result<DiscoveryAdvertiser, mdns_sd::Error> {
    start_service(build_service(
        instance_name,
        installation_id,
        hostname,
        port,
        true,
    )?)
}

// IpAdd is only a wakeup, never an address/owner/health authority. Re-registering
// this pristine, empty-address template lets mdns-sd perform its EXISTING live
// interface enumeration; no event address is copied into a service record.
fn start_service(template: ServiceInfo) -> Result<DiscoveryAdvertiser, mdns_sd::Error> {
    let daemon = ServiceDaemon::new()?;
    let fullname = template.get_fullname().to_owned();
    let result = (|| {
        let events = daemon.monitor()?;
        daemon.register(template.clone())?;
        let recovery = RecoveryWorker::start(daemon.clone(), events, template)?;
        Ok(DiscoveryAdvertiser {
            daemon: daemon.clone(),
            fullname: fullname.clone(),
            recovery,
        })
    })();
    if result.is_err() {
        let _ = daemon.unregister(&fullname);
        let _ = daemon.shutdown();
    }
    result
}

const COALESCE_MS: u64 = 250;
const MIN_REFRESH_MS: u64 = 1_000;
const MAX_RECOVERED_IPS: usize = 8;
const MAX_RETRIES: u8 = 3;

/// Per-advertiser bounded event reducer. It does not retain packets, names,
/// credentials, or an additional service/address authority.
#[derive(Default)]
struct RecoveryState {
    // Pending wakeups only, not a persistent interface cache.
    recovered: HashSet<Ipv4Addr>,
    due: Option<u64>,
    last_attempt: Option<u64>,
    retries: u8,
    stopped: bool,
}
impl RecoveryState {
    fn event(&mut self, event: DaemonEvent, now: u64) {
        if self.stopped {
            return;
        }
        match event {
            DaemonEvent::IpAdd(IpAddr::V4(ip)) if ip.is_private() => {
                if !self.recovered.contains(&ip) && self.recovered.len() >= MAX_RECOVERED_IPS {
                    return;
                }
                self.recovered.insert(ip);
                // Duplicate notifications coalesce while pending. Once settled,
                // a fresh IpAdd is allowed to wake us even if its IpDel was
                // dropped by the bounded daemon monitor. Rate limiting remains.
                if self.due.is_none() {
                    self.retries = 0;
                    self.due = Some(
                        now.saturating_add(COALESCE_MS).max(
                            self.last_attempt
                                .map_or(0, |at| at.saturating_add(MIN_REFRESH_MS)),
                        ),
                    );
                }
            }
            DaemonEvent::IpDel(IpAddr::V4(ip)) => {
                self.recovered.remove(&ip);
                if self.recovered.is_empty() {
                    self.due = None;
                }
            }
            // Announce is not a successful wire-send receipt in mdns-sd0.11.5.
            _ => {}
        }
    }
    fn stop(&mut self) {
        self.stopped = true;
        self.due = None;
        self.recovered.clear();
    }
    fn poll(&mut self, now: u64, register: impl FnOnce() -> Result<(), mdns_sd::Error>) {
        if self.stopped || self.due.is_none_or(|due| now < due) {
            return;
        }
        self.due = None;
        self.last_attempt = Some(now);
        match register() {
            Ok(()) => {
                self.retries = 0;
                self.recovered.clear();
            }
            Err(mdns_sd::Error::Again) if self.retries < MAX_RETRIES => {
                self.retries += 1;
                self.due = Some(now.saturating_add(MIN_REFRESH_MS << (self.retries - 1)));
            }
            Err(error) => {
                self.recovered.clear();
                // No busyloop, implicit publisher replacement or health claim.
                tracing::warn!(%error, "Discovery interface refresh was not accepted");
            }
        }
    }
}

enum EventRead {
    Event(DaemonEvent),
    Idle,
    Closed,
}
trait RecoveryEvents: Send + 'static {
    fn read(&self, timeout: Duration) -> EventRead;
}
impl RecoveryEvents for mdns_sd::Receiver<DaemonEvent> {
    fn read(&self, timeout: Duration) -> EventRead {
        match self.recv_timeout(timeout) {
            Ok(event) => EventRead::Event(event),
            Err(_) if self.is_disconnected() => EventRead::Closed,
            Err(_) => EventRead::Idle,
        }
    }
}

struct RecoveryWorker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl RecoveryWorker {
    fn start(
        daemon: ServiceDaemon,
        events: mdns_sd::Receiver<DaemonEvent>,
        template: ServiceInfo,
    ) -> Result<Self, mdns_sd::Error> {
        Self::start_with(events, move || daemon.register(template.clone()))
    }
    fn start_with(
        events: impl RecoveryEvents,
        mut register: impl FnMut() -> Result<(), mdns_sd::Error> + Send + 'static,
    ) -> Result<Self, mdns_sd::Error> {
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let thread = thread::Builder::new()
            .name("ruview-mdns-recovery".into())
            .spawn(move || {
                let origin = Instant::now();
                let now = || origin.elapsed().as_millis().min(u64::MAX as u128) as u64;
                let mut state = RecoveryState::default();
                while !cancelled.load(Ordering::Acquire) {
                    match events.read(Duration::from_millis(100)) {
                        EventRead::Event(event) => state.event(event, now()),
                        EventRead::Closed => break,
                        EventRead::Idle => {}
                    }
                    let mut closed = false;
                    // Drain one bounded monitor batch before acting, including any
                    // queued IpDel. A continuous event producer cannot starve stop.
                    for _ in 0..100 {
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        match events.read(Duration::ZERO) {
                            EventRead::Event(event) => state.event(event, now()),
                            EventRead::Closed => {
                                closed = true;
                                break;
                            }
                            EventRead::Idle => break,
                        }
                    }
                    if cancelled.load(Ordering::Acquire) || closed {
                        break;
                    }
                    state.poll(now(), || {
                        if cancelled.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        register()
                    });
                }
                state.stop();
            })
            .map_err(|error| {
                mdns_sd::Error::Msg(format!(
                    "Discovery recovery worker could not start: {error}"
                ))
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for RecoveryWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
#[path = "discovery_recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertisement_matches_mobile_contract_without_sensor_metadata() {
        let service = build_service(
            "RuView Living Room",
            "home-a",
            "ruview-home-a.local.",
            3000,
            false,
        )
        .unwrap();
        assert_eq!(service.get_type(), SERVICE_TYPE);
        assert_eq!(service.get_port(), 3000);
        assert!(service.is_addr_auto());
        assert_eq!(service.get_property_val_str("schema"), Some(SCHEMA));
        assert_eq!(service.get_property_val_str("tls"), Some("0"));
        assert_eq!(service.get_property_val_str("installation"), Some("home-a"));
        for forbidden in ["ssid", "node", "room", "csi", "pose", "token"] {
            assert!(service.get_property(forbidden).is_none());
        }
    }

    #[test]
    fn hostname_is_bounded_and_safe() {
        assert_eq!(
            discovery_hostname("Cohen's Mac Mini"),
            "ruview-cohens-mac-mini.local."
        );
        let value = discovery_hostname("🚫 ///");
        assert_eq!(value, "ruview-installation.local.");
        assert!(discovery_hostname(&"A".repeat(200)).len() <= 54);
    }
}
