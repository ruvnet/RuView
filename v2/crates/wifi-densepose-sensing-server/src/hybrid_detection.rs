use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSensingInput {
    pub source: String,
    pub presence: bool,
    pub motion_level: String,
    pub confidence: f64,
    pub estimated_persons: usize,
}

impl Default for HybridSensingInput {
    fn default() -> Self {
        Self {
            source: "unknown".to_string(),
            presence: false,
            motion_level: "absent".to_string(),
            confidence: 0.0,
            estimated_persons: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HybridSnapshot {
    pub scanned_at_ms: u64,
    pub network_devices: Vec<HybridNetworkDevice>,
    pub sensing: HybridSensingSummary,
    pub fusion: HybridFusionSummary,
    pub bluetooth: HybridBluetoothSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HybridSensingSummary {
    pub source: String,
    pub presence: bool,
    pub motion_level: String,
    pub confidence: f64,
    pub estimated_persons: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HybridFusionSummary {
    pub likely_human_presence: bool,
    pub estimated_humans: usize,
    pub known_devices: usize,
    pub likely_smartphones: usize,
    pub likely_smart_tvs: usize,
    pub likely_computers: usize,
    pub likely_iot_devices: usize,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HybridBluetoothSummary {
    pub enabled: bool,
    pub available: bool,
    pub effective: bool,
    pub adapter_present: bool,
    pub service_running: bool,
    pub helper_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_name: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HybridDeviceOverride {
    #[serde(default)]
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default = "default_override_category")]
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridNetworkDevice {
    pub ip: String,
    pub mac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub category: String,
    pub confidence: f64,
    #[serde(default)]
    pub manual_override: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_override_id: Option<String>,
}

#[derive(Debug, Clone)]
struct DeviceFingerprint {
    vendor: Option<String>,
    category: String,
    confidence: f64,
}

pub fn collect_snapshot(
    input: HybridSensingInput,
    overrides: &[HybridDeviceOverride],
) -> HybridSnapshot {
    let devices = apply_device_overrides(scan_local_network_devices(), overrides);
    let smartphones = devices
        .iter()
        .filter(|device| device.category == "smartphone")
        .count();
    let smart_tvs = devices
        .iter()
        .filter(|device| device.category == "smart_tv")
        .count();
    let computers = devices
        .iter()
        .filter(|device| device.category == "computer")
        .count();
    let iot = devices
        .iter()
        .filter(|device| device.category == "iot_or_media")
        .count();
    let known_devices = devices.len();

    let estimated_humans = if input.presence {
        input.estimated_persons.max(1)
    } else {
        0
    };

    let note = if estimated_humans == 0 && !devices.is_empty() {
        "Dispositivos de rede detectados, mas sem evidencia forte de presenca humana no sensing atual.".to_string()
    } else if estimated_humans > 0 && devices.is_empty() {
        "Presenca humana sugerida pelo Wi-Fi, mas nenhum dispositivo local foi identificado na LAN.".to_string()
    } else if estimated_humans > 0 {
        "Camada hibrida ativa: inventario de dispositivos da LAN combinado com presenca/movimento inferidos pelo RuView.".to_string()
    } else {
        "Sem atividade humana clara; snapshot mostra apenas o inventario local de dispositivos que respondeu na rede.".to_string()
    };

    HybridSnapshot {
        scanned_at_ms: now_ms(),
        network_devices: devices,
        sensing: HybridSensingSummary {
            source: input.source,
            presence: input.presence,
            motion_level: input.motion_level,
            confidence: input.confidence,
            estimated_persons: input.estimated_persons,
        },
        fusion: HybridFusionSummary {
            likely_human_presence: input.presence,
            estimated_humans,
            known_devices,
            likely_smartphones: smartphones,
            likely_smart_tvs: smart_tvs,
            likely_computers: computers,
            likely_iot_devices: iot,
            note,
        },
        bluetooth: HybridBluetoothSummary::default(),
    }
}

fn default_override_category() -> String {
    "unknown_device".to_string()
}

pub fn normalize_override(mut override_entry: HybridDeviceOverride) -> HybridDeviceOverride {
    override_entry.match_mac = override_entry
        .match_mac
        .as_deref()
        .map(normalize_mac)
        .filter(|value| !value.is_empty());
    override_entry.match_ip = override_entry
        .match_ip
        .as_deref()
        .map(str::trim)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    override_entry.match_hostname = override_entry
        .match_hostname
        .as_deref()
        .map(normalize_hostname)
        .filter(|value| !value.is_empty());
    override_entry.display_name = override_entry
        .display_name
        .as_deref()
        .map(str::trim)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    override_entry.notes = override_entry
        .notes
        .as_deref()
        .map(str::trim)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    if override_entry.category.trim().is_empty() {
        override_entry.category = default_override_category();
    }
    if override_entry.id.trim().is_empty() {
        override_entry.id = override_entry_identity(&override_entry);
    }
    override_entry
}

fn override_entry_identity(entry: &HybridDeviceOverride) -> String {
    if let Some(mac) = entry.match_mac.as_deref().filter(|value| !value.is_empty()) {
        return format!("mac:{mac}");
    }
    if let Some(ip) = entry.match_ip.as_deref().filter(|value| !value.is_empty()) {
        return format!("ip:{ip}");
    }
    if let Some(hostname) = entry
        .match_hostname
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return format!("host:{hostname}");
    }
    let display_name = entry
        .display_name
        .as_deref()
        .unwrap_or("manual-device")
        .to_ascii_lowercase()
        .replace(' ', "-");
    format!("manual:{display_name}")
}

fn apply_device_overrides(
    mut devices: Vec<HybridNetworkDevice>,
    overrides: &[HybridDeviceOverride],
) -> Vec<HybridNetworkDevice> {
    for device in &mut devices {
        if let Some(override_entry) = overrides
            .iter()
            .find(|entry| override_matches_device(entry, device))
        {
            if let Some(display_name) = override_entry.display_name.clone() {
                device.display_name = Some(display_name);
            }
            device.category = override_entry.category.clone();
            device.confidence = device.confidence.max(0.98);
            device.manual_override = true;
            device.matched_override_id = Some(override_entry.id.clone());
        }
    }
    devices
}

fn override_matches_device(entry: &HybridDeviceOverride, device: &HybridNetworkDevice) -> bool {
    if let Some(mac) = entry.match_mac.as_deref() {
        if mac == device.mac {
            return true;
        }
    }
    if let Some(ip) = entry.match_ip.as_deref() {
        if ip.eq_ignore_ascii_case(&device.ip) {
            return true;
        }
    }
    if let Some(hostname) = entry.match_hostname.as_deref() {
        if device
            .hostname
            .as_deref()
            .map(normalize_hostname)
            .as_deref()
            == Some(hostname)
        {
            return true;
        }
    }
    false
}

fn scan_local_network_devices() -> Vec<HybridNetworkDevice> {
    #[cfg(target_os = "windows")]
    {
        scan_windows_arp()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "windows")]
fn scan_windows_arp() -> Vec<HybridNetworkDevice> {
    let output = Command::new("arp").arg("-a").output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Interface:") {
            continue;
        }

        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let ip = parts[0];
        let Ok(parsed_ip) = ip.parse::<IpAddr>() else {
            continue;
        };
        if !is_private_lan_ip(parsed_ip) {
            continue;
        }
        let mac = normalize_mac(parts[1]);
        if mac.is_empty() || mac == "FF:FF:FF:FF:FF:FF" || mac == "00:00:00:00:00:00" {
            continue;
        }

        let hostname = resolve_windows_hostname(ip);
        let fingerprint = fingerprint_device(&mac, hostname.as_deref());
        devices.push(HybridNetworkDevice {
            ip: ip.to_string(),
            mac,
            hostname,
            vendor: fingerprint.vendor,
            display_name: None,
            category: fingerprint.category,
            confidence: fingerprint.confidence,
            manual_override: false,
            matched_override_id: None,
        });
    }

    devices.sort_by(|left, right| left.ip.cmp(&right.ip));
    devices.dedup_by(|left, right| left.ip == right.ip || left.mac == right.mac);
    devices
}

#[cfg(target_os = "windows")]
fn resolve_windows_hostname(ip: &str) -> Option<String> {
    let output = Command::new("ping")
        .args(["-a", "-n", "1", "-w", "120", ip])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains('[') && line.contains(']') && line.contains(ip) {
            let start = line.find('[')?;
            let hostname = line[..start]
                .split_whitespace()
                .last()
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != ip)?;
            return Some(hostname.to_string());
        }
    }
    None
}

fn fingerprint_device(mac: &str, hostname: Option<&str>) -> DeviceFingerprint {
    let vendor = vendor_from_mac(mac);
    let mut category = "unknown_device".to_string();
    let mut confidence = 0.35;
    let host = hostname.unwrap_or("").to_ascii_lowercase();
    let vendor_name = vendor.unwrap_or("Unknown").to_ascii_lowercase();

    let smartphone_clues = [
        "iphone", "android", "galaxy", "pixel", "redmi", "moto", "oneplus", "mi-", "sm-",
    ];
    let smart_tv_clues = [
        "tv", "webos", "bravia", "roku", "chromecast", "firetv", "shield", "appletv",
    ];
    let computer_clues = ["desktop", "laptop", "notebook", "thinkpad", "macbook", "pc"];

    if smartphone_clues.iter().any(|clue| host.contains(clue)) {
        category = "smartphone".to_string();
        confidence = 0.95;
    } else if smart_tv_clues.iter().any(|clue| host.contains(clue)) {
        category = "smart_tv".to_string();
        confidence = 0.95;
    } else if computer_clues.iter().any(|clue| host.contains(clue)) {
        category = "computer".to_string();
        confidence = 0.9;
    } else if vendor_name.contains("apple") {
        category = "smartphone".to_string();
        confidence = 0.6;
    } else if vendor_name.contains("samsung") {
        category = if host.contains("tv") || host.contains("livingroom") {
            "smart_tv".to_string()
        } else {
            "smartphone".to_string()
        };
        confidence = 0.55;
    } else if [
        "xiaomi", "motorola", "google", "huawei", "oppo", "vivo", "oneplus",
    ]
    .iter()
    .any(|clue| vendor_name.contains(clue))
    {
        category = "smartphone".to_string();
        confidence = 0.55;
    } else if [
        "lg", "hisense", "tcl", "sony", "philips", "roku", "vestel",
    ]
    .iter()
    .any(|clue| vendor_name.contains(clue))
    {
        category = "smart_tv".to_string();
        confidence = 0.55;
    } else if [
        "intel", "dell", "lenovo", "hp", "hewlett", "asus", "acer", "microsoft",
    ]
    .iter()
    .any(|clue| vendor_name.contains(clue))
    {
        category = "computer".to_string();
        confidence = 0.6;
    } else if [
        "amazon", "tp-link", "tuya", "espressif", "google nest", "roku",
    ]
    .iter()
    .any(|clue| vendor_name.contains(clue))
    {
        category = "iot_or_media".to_string();
        confidence = 0.5;
    }

    DeviceFingerprint {
        vendor: vendor.map(str::to_string),
        category,
        confidence,
    }
}

fn vendor_from_mac(mac: &str) -> Option<&'static str> {
    let prefix = mac.chars().filter(|character| *character != ':').take(6).collect::<String>();
    match prefix.as_str() {
        "001C42" | "28CFE9" | "3C0754" | "A4B197" => Some("Apple"),
        "001632" | "2C54CF" | "8C8590" | "FCF136" => Some("Samsung"),
        "64BC0C" | "8CBEBE" | "64CC2E" => Some("Xiaomi"),
        "58CB52" | "78D294" => Some("Motorola"),
        "F4F5DB" | "D8BB2C" => Some("Google Nest"),
        "A4CF12" | "50C7BF" => Some("Hisense"),
        "14AB02" | "6C5A34" => Some("LG"),
        "6C2B59" | "34EA34" => Some("Sony"),
        "B0BE76" | "70F11C" => Some("TCL"),
        "F8E079" | "2CF0A2" => Some("TP-Link"),
        "30C6F7" | "7CD95C" => Some("Amazon"),
        "7C87CE" | "84F3EB" => Some("Intel"),
        "3C5282" | "F80D60" => Some("Dell"),
        "C83A35" | "4851B7" => Some("Lenovo"),
        "9C7BEF" | "3CD92B" => Some("ASUS"),
        _ => None,
    }
}

fn normalize_mac(raw: &str) -> String {
    let filtered = raw
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .map(|character| character.to_ascii_uppercase())
        .collect::<String>();
    if filtered.len() != 12 {
        return String::new();
    }
    let mut normalized = String::with_capacity(17);
    for (index, character) in filtered.chars().enumerate() {
        if index > 0 && index % 2 == 0 {
            normalized.push(':');
        }
        normalized.push(character);
    }
    normalized
}

fn normalize_hostname(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn is_private_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(_) => false,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
