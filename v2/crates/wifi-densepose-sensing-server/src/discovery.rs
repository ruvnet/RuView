//! Local DNS-SD advertisement for RuView installation discovery.
//!
//! The TXT record deliberately contains no SSID, node identifiers, room data,
//! sensor values, credentials, or personal data. Mobile clients treat the
//! installation id as a routing hint only and still health-check the service.

use std::collections::HashMap;

use mdns_sd::{ServiceDaemon, ServiceInfo};

pub const SERVICE_TYPE: &str = "_ruview._tcp.local.";
pub const SCHEMA: &str = "ruview.installation.v1";

pub struct DiscoveryAdvertiser {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for DiscoveryAdvertiser {
    fn drop(&mut self) {
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
    let daemon = ServiceDaemon::new()?;
    let service = build_service(instance_name, installation_id, hostname, http_port, false)?;
    let fullname = service.get_fullname().to_owned();
    daemon.register(service)?;
    Ok(DiscoveryAdvertiser { daemon, fullname })
}

/// Advertise the QR-pinned companion TLS endpoint independently of legacy HTTP.
pub fn start_tls_advertiser(instance_name: &str, installation_id: &str, hostname: &str, port: u16) -> Result<DiscoveryAdvertiser, mdns_sd::Error> {
    let daemon = ServiceDaemon::new()?;
    let service = build_service(instance_name, installation_id, hostname, port, true)?;
    let fullname = service.get_fullname().to_owned();
    daemon.register(service)?;
    Ok(DiscoveryAdvertiser { daemon, fullname })
}

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
