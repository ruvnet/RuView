// RuView macOS Wi-Fi helper, app-bundle build (ADR-025 Amendment 2).
//
// Same output contract as archive/v1/src/sensing/mac_wifi.swift `--scan-once`,
// plus Location Services support. macOS redacts the SSID/BSSID unless the
// *responsible* app holds location authorization. A bare CLI binary run from a
// terminal can never get it, so this helper ships as MacWifi.app and is launched
// through LaunchServices (`open`), which makes the bundle its own responsible
// process.
//
//   --authorize   request Location Services (shows the macOS prompt once) and
//                 print the resulting status as JSON
//   --scan-once   print one JSON sample of the connected link
//   --status      print the current authorization status as JSON
import CoreLocation
import CoreWLAN
import Foundation

func statusName(_ s: CLAuthorizationStatus) -> String {
    switch s {
    case .notDetermined: return "not_determined"
    case .restricted: return "restricted"
    case .denied: return "denied"
    case .authorizedAlways: return "authorized"
    @unknown default: return "unknown"
    }
}

func emit(_ obj: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]) else {
        fputs("Could not encode JSON\n", stderr)
        exit(1)
    }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0a]))
}

final class Authorizer: NSObject, CLLocationManagerDelegate {
    let manager = CLLocationManager()
    var done = false

    func run(timeout: TimeInterval) -> CLAuthorizationStatus {
        manager.delegate = self
        if manager.authorizationStatus == .notDetermined {
            manager.requestWhenInUseAuthorization()
        } else {
            done = true
        }
        let deadline = Date().addingTimeInterval(timeout)
        while !done && Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.2))
        }
        return manager.authorizationStatus
    }

    func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        if manager.authorizationStatus != .notDetermined { done = true }
    }
}

let args = CommandLine.arguments

if args.contains("--authorize") || args.contains("--status") {
    let status: CLAuthorizationStatus
    if args.contains("--authorize") {
        status = Authorizer().run(timeout: 120)
    } else {
        status = CLLocationManager().authorizationStatus
    }
    emit(["location_authorization": statusName(status),
          "location_services_enabled": CLLocationManager.locationServicesEnabled()])
    exit(status == .authorizedAlways ? 0 : 2)
}

guard let interface = CWWiFiClient.shared().interface() else {
    fputs("No WiFi interface found\n", stderr)
    exit(1)
}
guard interface.powerOn(), let channel = interface.wlanChannel(), interface.rssiValue() < 0 else {
    fputs("WiFi is not connected\n", stderr)
    exit(1)
}
emit([
    "connected": true,
    "ssid": interface.ssid() ?? "",
    "bssid": interface.bssid() ?? "00:00:00:00:00:00",
    "channel": channel.channelNumber,
    "rssi": interface.rssiValue(),
    "noise": interface.noiseMeasurement(),
    "timestamp": Date().timeIntervalSince1970,
    "tx_rate": interface.transmitRate(),
])
