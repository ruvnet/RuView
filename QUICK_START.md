# ⚡ QUICK START - RuView on Windows 11

## 🚀 Get Running in 60 Seconds

### Step 1: Open PowerShell and Run Setup
```powershell
# Navigate to RuView folder
cd C:\path\to\RuView

# Run complete setup
python SETUP_AND_RUN.py
```

**What this does:**
- ✅ Builds Rust backend (release mode, ~5 minutes first time)
- ✅ Installs Python dependencies
- ✅ Builds web UI
- ✅ Verifies all signal processing works
- ✅ Creates RUN_RUVIEW.bat launcher

### Step 2: Start the App
```powershell
# Double-click:
RUN_RUVIEW.bat

# Or from PowerShell:
.\RUN_RUVIEW.bat
```

**What opens:**
1. ✅ API Server on port 8000
2. ✅ Web Dashboard on port 3000
3. ✅ Browser opens automatically
4. ✅ Real-time charts start streaming

### Step 3: View Data
- **Dashboard:** http://localhost:3000
- **API Docs:** http://localhost:8000/docs
- **WebSocket:** ws://localhost:8000/api/v1/stream

---

## 📊 What You See (Real-Time)

```
╔════════════════════════════════════════════════════╗
║  RUVIEW DASHBOARD                                  ║
╠════════════════════════════════════════════════════╣
║                                                    ║
║  👥 PRESENCE                                       ║
║  ┌──────────────────────────────────────────────┐ ║
║  │ People Detected: 2    Confidence: 98%       │ ║
║  │ Room: Living Room                            │ ║
║  │ Duration: 5m 32s                             │ ║
║  └──────────────────────────────────────────────┘ ║
║                                                    ║
║  🫁 BREATHING RATE                                ║
║  ┌──────────────────────────────────────────────┐ ║
║  │ BPM: 16.3 (±0.5)         [▁▂▃▄▅▄▃▂▁]       │ ║
║  │ Quality: 92%                                 │ ║
║  └──────────────────────────────────────────────┘ ║
║                                                    ║
║  💓 HEART RATE                                    ║
║  ┌──────────────────────────────────────────────┐ ║
║  │ BPM: 72                  [▂▃▄▅▆▅▄▃▂]        │ ║
║  │ Trend: Steady                                │ ║
║  └──────────────────────────────────────────────┘ ║
║                                                    ║
║  🚶 ACTIVITY                                      ║
║  ┌──────────────────────────────────────────────┐ ║
║  │ Type: Sitting (92% confidence)              │ ║
║  │ Last motion: 2s ago                         │ ║
║  └──────────────────────────────────────────────┘ ║
║                                                    ║
║  ⚠️  ALERTS                                       ║
║  ┌──────────────────────────────────────────────┐ ║
║  │ All Clear - No anomalies detected           │ ║
║  └──────────────────────────────────────────────┘ ║
║                                                    ║
╚════════════════════════════════════════════════════╝
```

---

## 🔌 Add Hardware (Optional)

### What You Need
- ESP32-S3 board: **$9** (Amazon, AliExpress)
- USB-C cable: **$2**
- **Total: $11** ← Much cheaper than any other system

### Setup Steps
1. **Flash firmware** (from Dashboard → Hardware → Flash)
2. **Configure WiFi** (Dashboard will prompt)
3. **Sensor auto-discovers** server
4. **Real-time CSI data** streams immediately

### Compatible Boards
- ✅ ESP32-S3 (8MB flash, recommended)
- ✅ ESP32-C6 (WiFi 6 + mmWave radar)
- ✅ ESP32 (original, limited CSI)

---

## 🧪 Test Without Hardware

**Included:** 1,000 real WiFi CSI frames  
**Mode:** Automatic simulation when no hardware detected

```powershell
# Verify everything works
python archive/v1/data/proof/verify.py

# Output:
# ✓ CSI frame parsing: PASS
# ✓ Signal preprocessing: PASS  
# ✓ Neural network inference: PASS
# ✓ Vital signs extraction: PASS
# VERDICT: PASS
```

---

## 🔌 Stop/Restart

### Stop the App
```powershell
# Ctrl+C in each PowerShell window, OR
taskkill /F /IM python.exe
```

### Restart
```powershell
.\RUN_RUVIEW.bat
```

### Check if Running
```powershell
curl http://localhost:8000/api/v1/health
# Response: {"status": "ok", "version": "1.2.0"}
```

---

## 🏠 Integrate with Home Assistant

See: `WINDOWS_APP_GUIDE.md` → "Home Assistant Integration"

Quick version:
```bash
# 1. Home Assistant addon
# 2. Set MQTT broker
# 3. RuView publishes sensor data
# 4. Add automations in HA
```

---

## 🛠️ Troubleshooting

### Port 8000 or 3000 Already in Use
```powershell
# Find what's using it
netstat -ano | findstr :8000

# Kill it
taskkill /PID <PID> /F
```

### "Python not found"
```powershell
# Install from: https://www.python.org/
# Make sure to CHECK "Add Python to PATH"
```

### "Rust not found"
```powershell
# Install from: https://rustup.rs/
# Restart PowerShell after install
```

### API shows 404 errors
```powershell
# Restart app
taskkill /F /IM python.exe
.\RUN_RUVIEW.bat
```

### Browser can't connect
```powershell
# Check server is running
curl http://localhost:3000
curl http://localhost:8000

# Check Windows Firewall
# Allow Python through firewall
```

---

## 📚 Full Documentation

| Document | Contents |
|----------|----------|
| `WINDOWS_APP_GUIDE.md` | Complete user guide (8KB) |
| `docs/user-guide.md` | All features explained |
| `http://localhost:8000/docs` | Interactive API reference |
| `README.md` | Project overview |

---

## 🚀 Performance Tips

### Make It Faster
```powershell
# 1. Close other apps to free RAM
# 2. Move ESP32 close to WiFi router (<5m)
# 3. Reduce chart update interval in dashboard
```

### Reduce Resource Usage
```powershell
# Run in "light" mode (simulated data only)
# Edit: config.json → "simulation_enabled": true
```

### Get Better Signal
```
- Position ESP32 with antenna pointing toward monitored area
- Place between WiFi router and room to monitor
- Avoid metal surfaces and RF interference
```

---

## 💾 Save Your Config

```powershell
# Backup config
Copy-Item config.json config.json.backup

# Edit with Notepad
notepad config.json

# Restart to apply changes
.\RUN_RUVIEW.bat
```

---

## 📊 API Examples

### Get Current Status
```powershell
curl http://localhost:8000/api/v1/health

# Response:
{
  "status": "ok",
  "version": "1.2.0",
  "sensors": 1,
  "uptime_seconds": 1234
}
```

### List Connected Sensors
```powershell
curl http://localhost:8000/api/v1/sensors

# Response:
[
  {
    "id": "ESP32-A1B2C3",
    "room": "living_room",
    "last_frame": "2024-01-15T10:30:45.123Z",
    "signal_quality": 0.92,
    "battery": 85
  }
]
```

### Connect to WebSocket Stream
```javascript
// In browser console:
const ws = new WebSocket('ws://localhost:8000/api/v1/stream');
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log("Breathing:", data.vitals.breathing_bpm);
  console.log("Heart Rate:", data.vitals.heart_rate_bpm);
  console.log("Presence:", data.presence.count);
};
```

---

## 🎯 Next Steps

1. ✅ Run `SETUP_AND_RUN.py` 
2. ✅ Open http://localhost:3000
3. ✅ Watch live charts
4. ✅ (Optional) Buy $9 ESP32 for real hardware
5. ✅ Read `WINDOWS_APP_GUIDE.md` for advanced features

---

## ❓ Still Stuck?

**Check these in order:**
1. Is Python 3.9+ installed? `python --version`
2. Is Rust installed? `cargo --version`
3. Did build finish successfully? Check console output
4. Can you reach servers? `curl http://localhost:8000`
5. Are ports 8000/3000 available? `netstat -ano | findstr :8000`

**GitHub Issues:** https://github.com/ruvnet/RuView/issues

---

**Ready? Run this:**
```powershell
python SETUP_AND_RUN.py
```

**That's it! 🚀**
