import Foundation
import CoreWLAN

// Output format: JSON lines for easy parsing by Python
// {"timestamp": 1234567.89, "rssi": -50, "noise": -90, "tx_rate": 866.0}

func main() {
    guard let interface = CWWiFiClient.shared().interface() else {
        fputs("{\"error\": \"No WiFi interface found\"}\n", stderr)
        exit(1)
    }

    // One connected-link RSSI sample for the Rust adapter. Keep the legacy
    // no-argument 10 Hz stream below unchanged for Python callers.
    if CommandLine.arguments.contains("--scan-once") {
        guard interface.powerOn(),
              let channel = interface.wlanChannel(),
              interface.rssiValue() < 0 else {
            fputs("WiFi is not connected\n", stderr)
            exit(1)
        }
        let sample: [String: Any] = [
            "ssid": interface.ssid() ?? "",
            "bssid": interface.bssid() ?? "00:00:00:00:00:00",
            "channel": channel.channelNumber,
            "rssi": interface.rssiValue(),
            "noise": interface.noiseMeasurement(),
            "timestamp": Date().timeIntervalSince1970,
            "tx_rate": interface.transmitRate()
        ]
        do {
            let data = try JSONSerialization.data(withJSONObject: sample, options: [.sortedKeys])
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data([0x0a]))
        } catch {
            fputs("Could not encode WiFi sample: \(error)\n", stderr)
            exit(1)
        }
        return
    }

    // Flush stdout automatically to prevent buffering issues with Python subprocess
    setbuf(stdout, nil)

    // Run at ~10Hz
    let interval: TimeInterval = 0.1

    while true {
        let timestamp = Date().timeIntervalSince1970
        let rssi = interface.rssiValue()
        let noise = interface.noiseMeasurement()
        let txRate = interface.transmitRate()

        let json = """
        {"timestamp": \(timestamp), "rssi": \(rssi), "noise": \(noise), "tx_rate": \(txRate)}
        """
        print(json)

        Thread.sleep(forTimeInterval: interval)
    }
}

main()
