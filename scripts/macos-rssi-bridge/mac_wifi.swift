// mac_wifi — multi-BSSID WiFi scanner using CoreWLAN.
//
// Replaces the v1 single-AP helper at archive/v1/src/sensing/mac_wifi.swift.
// Emits one JSON object per visible BSSID, in the format expected by
// v2/crates/wifi-densepose-wifiscan/src/adapter/macos_scanner.rs:
//
//   {"ssid":"<name>","bssid":"<aa:bb:cc:dd:ee:ff>","rssi":<int>,
//    "noise":<int>,"channel":<int>,"band":"<2.4GHz|5GHz|6GHz>"}
//
// Modes:
//   --scan-once    : run a single active scan, print each BSSID as JSON, exit
//   --watch        : repeat --scan-once forever at --interval seconds (default 1.0)
//   --interval N   : seconds between scans in --watch mode
//
// Notes:
// - macOS Sonoma 14.4+ redacts BSSIDs to "00:00:00:00:00:00" unless the calling
//   process holds the com.apple.wifi.scan entitlement OR Location Services is
//   authorized for it. The Rust adapter handles the redacted case via a
//   deterministic synthetic MAC; we just pass redacted MACs through.
// - Active scans take ~1–3 seconds. Don't run --watch faster than ~1 Hz.

import Foundation
import CoreWLAN

@inline(__always) func putLine(_ s: String) {
    FileHandle.standardOutput.write(Data((s + "\n").utf8))
}

@inline(__always) func putErr(_ s: String) {
    FileHandle.standardError.write(Data((s + "\n").utf8))
}

func bandLabel(_ band: CWChannelBand) -> String {
    switch band {
    case .band2GHz:  return "2.4GHz"
    case .band5GHz:  return "5GHz"
    case .band6GHz:  return "6GHz"
    case .bandUnknown: fallthrough
    @unknown default: return "unknown"
    }
}

func jsonEscape(_ s: String) -> String {
    var out = ""
    out.reserveCapacity(s.count + 2)
    for ch in s.unicodeScalars {
        switch ch {
        case "\"": out += "\\\""
        case "\\": out += "\\\\"
        case "\n": out += "\\n"
        case "\r": out += "\\r"
        case "\t": out += "\\t"
        default:
            if ch.value < 0x20 {
                out += String(format: "\\u%04x", ch.value)
            } else {
                out += String(ch)
            }
        }
    }
    return out
}

struct ScanRecord {
    let ssid: String
    let bssid: String
    let rssi: Int
    let noise: Int
    let channel: Int
    let band: String
}

// Disambiguate redacted entries (empty SSID, zero BSSID) by computing an ordinal
// within each (channel, ~3 dBm RSSI bucket). The downstream Rust adapter hashes
// the SSID to derive a synthetic stable BSSID; making the SSID unique-per-AP
// recovers per-AP tracking even without Location Services authorization.
func disambiguate(_ raw: [ScanRecord]) -> [ScanRecord] {
    var groups: [String: [Int]] = [:]
    for (i, r) in raw.enumerated() where r.ssid.isEmpty && r.bssid.allSatisfy({ $0 == "0" || $0 == ":" }) {
        let bucket = r.rssi / 3
        groups["\(r.channel):\(bucket)", default: []].append(i)
    }
    var out = raw
    for (key, idxs) in groups {
        for (ord, i) in idxs.enumerated() {
            let synthetic = "__redacted_\(key)_n\(ord)"
            out[i] = ScanRecord(ssid: synthetic, bssid: out[i].bssid, rssi: out[i].rssi,
                                noise: out[i].noise, channel: out[i].channel, band: out[i].band)
        }
    }
    return out
}

func emit(_ r: ScanRecord) {
    let json = """
    {"ssid":"\(jsonEscape(r.ssid))","bssid":"\(r.bssid)","rssi":\(r.rssi),"noise":\(r.noise),"channel":\(r.channel),"band":"\(r.band)"}
    """
    putLine(json)
}

func scanOnce(_ iface: CWInterface) {
    do {
        let networks = try iface.scanForNetworks(withName: nil)
        var raw: [ScanRecord] = []
        raw.reserveCapacity(networks.count)
        for n in networks {
            raw.append(ScanRecord(
                ssid: n.ssid ?? "",
                bssid: n.bssid ?? "00:00:00:00:00:00",
                rssi: n.rssiValue,
                noise: n.noiseMeasurement,
                channel: n.wlanChannel?.channelNumber ?? 0,
                band: n.wlanChannel.map { bandLabel($0.channelBand) } ?? "unknown"
            ))
        }
        for r in disambiguate(raw) {
            emit(r)
        }
    } catch let err as NSError {
        putErr("{\"error\":\"scan_failed\",\"code\":\(err.code),\"domain\":\"\(jsonEscape(err.domain))\",\"message\":\"\(jsonEscape(err.localizedDescription))\"}")
        exit(2)
    }
}

func parseInterval(_ args: [String]) -> Double {
    if let i = args.firstIndex(of: "--interval"), i + 1 < args.count,
       let v = Double(args[i + 1]) {
        return max(0.5, v)
    }
    return 1.0
}

let args = Array(CommandLine.arguments.dropFirst())

guard let iface = CWWiFiClient.shared().interface() else {
    putErr("{\"error\":\"no_wifi_interface\"}")
    exit(1)
}

setbuf(stdout, nil)

if args.contains("--watch") {
    let interval = parseInterval(args)
    while true {
        scanOnce(iface)
        Thread.sleep(forTimeInterval: interval)
    }
} else {
    scanOnce(iface)
}
