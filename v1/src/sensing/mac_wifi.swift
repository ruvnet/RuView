import Foundation
import CoreWLAN

// Output format: JSON lines for easy parsing by Python
// {"timestamp": 1234567.89, "rssi": -50, "noise": -90, "tx_rate": 866.0}

func main() {
    guard let interface = CWWiFiClient.shared().interface() else {
        fputs("{\"error\": \"No WiFi interface found\"}\n", stderr)
        exit(1)
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
