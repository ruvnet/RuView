"""
Simple script to check ESP32 connection status.
Run this to see if your ESP32 is connected to WiFi.
"""
import serial
import time
import sys

def check_esp32(port='COM5', baud=115200):
    print(f"Checking ESP32 on {port} at {baud} baud...")
    print("Press Ctrl+C to exit\n")

    try:
        ser = serial.Serial(port, baud, timeout=1)
        time.sleep(2)  # Wait for connection

        # Read some output
        for _ in range(20):
            if ser.in_waiting:
                line = ser.readline().decode('utf-8', errors='ignore').strip()
                if line:
                    print(line)
            time.sleep(0.5)

    except serial.SerialException as e:
        print(f"Error: {e}")
        print("\nMake sure:")
        print("1. ESP32 is connected to your PC via USB")
        print("2. No other program is using the COM port")
        print("3. You're using the correct COM port (e.g., COM5)")

if __name__ == "__main__":
    port = sys.argv[1] if len(sys.argv) > 1 else 'COM5'
    check_esp32(port)