import Foundation
import CoreWLAN
import CryptoKit

private enum HelperError: Error, CustomStringConvertible {
    case usage(String)
    case noInterface
    case wifiPoweredOff(String)
    case notConnected(String)
    case scanFailed(String)

    var description: String {
        switch self {
        case let .usage(message):
            return message
        case .noInterface:
            return "No active Wi-Fi interface found"
        case let .wifiPoweredOff(name):
            return "Wi-Fi interface \(name) is powered off"
        case let .notConnected(name):
            return "Wi-Fi interface \(name) is not connected to an access point"
        case let .scanFailed(message):
            return "CoreWLAN scan failed: \(message)"
        }
    }
}

private enum Mode {
    case probe
    case scanOnce
    case connected
    case stream(intervalMs: UInt64)
}

private struct Observation: Encodable {
    let timestamp: Double
    let interface: String
    let ssid: String
    let bssid: String
    let bssidSynthetic: Bool
    let rssi: Int
    let noise: Int
    let channel: Int
    let band: String
    let txRateMbps: Double
    let isConnected: Bool

    enum CodingKeys: String, CodingKey {
        case timestamp
        case interface
        case ssid
        case bssid
        case bssidSynthetic = "bssid_synthetic"
        case rssi
        case noise
        case channel
        case band
        case txRateMbps = "tx_rate_mbps"
        case isConnected = "is_connected"
    }
}

private struct ProbeStatus: Encodable {
    let ok: Bool
    let interface: String
    let message: String?
}

private struct Arguments {
    let mode: Mode

    static func parse(_ argv: [String]) throws -> Arguments {
        var mode: Mode?
        var intervalMs: UInt64 = 200

        var index = 1
        while index < argv.count {
            switch argv[index] {
            case "--probe":
                mode = try assign(mode, value: .probe, flag: "--probe")
            case "--scan-once":
                mode = try assign(mode, value: .scanOnce, flag: "--scan-once")
            case "--connected":
                mode = try assign(mode, value: .connected, flag: "--connected")
            case "--stream":
                mode = try assign(mode, value: .stream(intervalMs: intervalMs), flag: "--stream")
            case "--interval-ms":
                index += 1
                guard index < argv.count, let parsed = UInt64(argv[index]), parsed > 0 else {
                    throw HelperError.usage("Expected a positive integer after --interval-ms")
                }
                intervalMs = parsed
            case "--help", "-h":
                throw HelperError.usage("""
                Usage: macos-wifi-scan [--probe|--scan-once|--connected|--stream] [--interval-ms N]
                  --probe       Verify CoreWLAN access and emit one status JSON line.
                  --scan-once   Scan visible networks and emit one JSON line per BSSID.
                  --connected   Emit one JSON line for the currently associated network.
                  --stream      Emit repeated connected-network observations.
                  --interval-ms Stream interval in milliseconds (default: 200).
                """)
            default:
                throw HelperError.usage("Unknown argument: \(argv[index])")
            }
            index += 1
        }

        switch mode {
        case .stream?:
            return Arguments(mode: .stream(intervalMs: intervalMs))
        case let selected?:
            return Arguments(mode: selected)
        case nil:
            throw HelperError.usage("Expected one of --probe, --scan-once, --connected, or --stream")
        }
    }

    private static func assign(_ current: Mode?, value: Mode, flag: String) throws -> Mode {
        guard current == nil else {
            throw HelperError.usage("Specify only one mode flag; duplicate or conflicting flag: \(flag)")
        }
        return value
    }
}

private final class WifiHelper {
    private let encoder: JSONEncoder

    init() {
        encoder = JSONEncoder()
    }

    func run(_ mode: Mode) throws {
        switch mode {
        case .probe:
            try emitProbeStatus()
        case .scanOnce:
            let observations = try scanObservations()
            guard !observations.isEmpty else {
                throw HelperError.scanFailed("no visible networks returned by CoreWLAN")
            }
            try observations.forEach(emit)
        case .connected:
            let observation = try connectedObservation()
            try emit(observation)
        case let .stream(intervalMs):
            try streamObservations(intervalMs: intervalMs)
        }
    }

    private func emitProbeStatus() throws {
        let interface = try requireInterface()
        let interfaceName = interface.interfaceName ?? "unknown"
        let message: String?

        if let ssid = interface.ssid(), !ssid.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            message = "connected:\(ssid)"
        } else {
            message = "ready"
        }

        let payload = ProbeStatus(ok: true, interface: interfaceName, message: message)
        let data = try encoder.encode(payload)
        guard let line = String(data: data, encoding: .utf8) else {
            throw HelperError.scanFailed("failed to encode probe JSON output")
        }
        print(line)
    }

    private func streamObservations(intervalMs: UInt64) throws {
        setbuf(stdout, nil)

        while true {
            do {
                let observation = try connectedObservation()
                try emit(observation)
            } catch HelperError.notConnected {
                fputs("macos-wifi-scan: waiting for Wi-Fi association\n", stderr)
            } catch {
                throw error
            }

            Thread.sleep(forTimeInterval: TimeInterval(intervalMs) / 1000.0)
        }
    }

    private func scanObservations() throws -> [Observation] {
        let interface = try requireInterface()
        let interfaceName = interface.interfaceName ?? "unknown"

        let networks: Set<CWNetwork>
        do {
            networks = try interface.scanForNetworks(withName: nil)
        } catch {
            throw HelperError.scanFailed(error.localizedDescription)
        }

        let connectedBssid = normalizedRealBssid(interface.bssid())
        let connectedSsid = interface.ssid() ?? ""
        let txRate = interface.transmitRate()

        let observations = networks
            .map { network in
                makeObservation(
                    interfaceName: interfaceName,
                    ssid: network.ssid ?? "",
                    rawBssid: network.bssid,
                    rssi: network.rssiValue,
                    noise: network.noiseMeasurement,
                    channelNumber: Int(network.wlanChannel?.channelNumber ?? 0),
                    channelBand: network.wlanChannel?.channelBand,
                    txRateMbps: txRate,
                    isConnected: connectedBssid != nil && normalizedRealBssid(network.bssid) == connectedBssid
                        || (!connectedSsid.isEmpty && connectedSsid == (network.ssid ?? ""))
                )
            }
            .sorted {
                if $0.isConnected != $1.isConnected {
                    return $0.isConnected && !$1.isConnected
                }
                if $0.rssi != $1.rssi {
                    return $0.rssi > $1.rssi
                }
                return $0.ssid.localizedCaseInsensitiveCompare($1.ssid) == .orderedAscending
            }

        return observations
    }

    private func connectedObservation() throws -> Observation {
        let interface = try requireInterface()
        let interfaceName = interface.interfaceName ?? "unknown"

        guard let ssid = interface.ssid(), !ssid.isEmpty else {
            throw HelperError.notConnected(interfaceName)
        }

        let channel = interface.wlanChannel()
        return makeObservation(
            interfaceName: interfaceName,
            ssid: ssid,
            rawBssid: interface.bssid(),
            rssi: interface.rssiValue(),
            noise: interface.noiseMeasurement(),
            channelNumber: Int(channel?.channelNumber ?? 0),
            channelBand: channel?.channelBand,
            txRateMbps: interface.transmitRate(),
            isConnected: true
        )
    }

    private func requireInterface() throws -> CWInterface {
        guard let interface = CWWiFiClient.shared().interface() else {
            throw HelperError.noInterface
        }
        let interfaceName = interface.interfaceName ?? "unknown"
        if !interface.powerOn() {
            throw HelperError.wifiPoweredOff(interfaceName)
        }
        return interface
    }

    private func emit(_ observation: Observation) throws {
        let data = try encoder.encode(observation)
        guard let line = String(data: data, encoding: .utf8) else {
            throw HelperError.scanFailed("failed to encode JSON output")
        }
        print(line)
    }

    private func makeObservation(
        interfaceName: String,
        ssid: String,
        rawBssid: String?,
        rssi: Int,
        noise: Int,
        channelNumber: Int,
        channelBand: CWChannelBand?,
        txRateMbps: Double,
        isConnected: Bool
    ) -> Observation {
        let normalizedSsid = ssid.trimmingCharacters(in: .whitespacesAndNewlines)
        let realBssid = normalizedRealBssid(rawBssid)
        let resolvedBssid: String
        let synthetic: Bool

        if let realBssid {
            resolvedBssid = realBssid
            synthetic = false
        } else {
            resolvedBssid = syntheticBssid(
                interfaceName: interfaceName,
                ssid: normalizedSsid,
                channel: channelNumber
            )
            synthetic = true
        }

        return Observation(
            timestamp: Date().timeIntervalSince1970,
            interface: interfaceName,
            ssid: normalizedSsid,
            bssid: resolvedBssid,
            bssidSynthetic: synthetic,
            rssi: rssi,
            noise: noise,
            channel: channelNumber,
            band: stringifyBand(channelBand),
            txRateMbps: txRateMbps,
            isConnected: isConnected
        )
    }

    private func normalizedRealBssid(_ rawValue: String?) -> String? {
        guard let rawValue else {
            return nil
        }
        let normalized = rawValue
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        guard normalized.count == 17 else {
            return nil
        }
        guard normalized != "00:00:00:00:00:00" else {
            return nil
        }
        let parts = normalized.split(separator: ":")
        guard parts.count == 6 else {
            return nil
        }
        for part in parts where part.count != 2 || UInt8(part, radix: 16) == nil {
            return nil
        }
        return normalized
    }

    private func syntheticBssid(interfaceName: String, ssid: String, channel: Int) -> String {
        let material = "\(interfaceName)|\(ssid.isEmpty ? "<hidden>" : ssid)|\(channel)"
        let digest = SHA256.hash(data: Data(material.utf8))
        var bytes = Array(digest.prefix(6))
        bytes[0] = (bytes[0] | 0x02) & 0xFE
        return bytes.map { String(format: "%02x", $0) }.joined(separator: ":")
    }

    private func stringifyBand(_ band: CWChannelBand?) -> String {
        switch band {
        case .band2GHz:
            return "2.4ghz"
        case .band5GHz:
            return "5ghz"
        case .band6GHz:
            return "6ghz"
        default:
            return ""
        }
    }
}

private func main() -> Int32 {
    do {
        let args = try Arguments.parse(CommandLine.arguments)
        try WifiHelper().run(args.mode)
        return EXIT_SUCCESS
    } catch let error as HelperError {
        fputs("macos-wifi-scan: \(error.description)\n", stderr)
        return EXIT_FAILURE
    } catch {
        fputs("macos-wifi-scan: \(error.localizedDescription)\n", stderr)
        return EXIT_FAILURE
    }
}

exit(main())
