# ✅ DEPLOYMENT CHECKLIST - RuView Windows 11

## Pre-Flight Checklist

- [ ] Windows 11 (or Windows 10 20H2+)
- [ ] Internet connection (for downloads)
- [ ] 50 GB free disk space (for Rust toolchain + builds)
- [ ] 4 GB RAM minimum (8 GB recommended)

## Prerequisites Installation

### Python 3.9+
- [ ] Download from https://www.python.org/downloads/
- [ ] Run installer
- [ ] **IMPORTANT:** Check ✓ "Add Python to PATH"
- [ ] Click "Install Now"
- [ ] Wait for completion (~2 minutes)
- [ ] Verify: Open PowerShell, type `python --version`
- [ ] Should show: Python 3.9.x or higher

### Rust & Cargo
- [ ] Download from https://rustup.rs/
- [ ] Run installer: rustup-init.exe
- [ ] Press 1 (default installation)
- [ ] Wait for completion (~5 minutes)
- [ ] Close and reopen PowerShell
- [ ] Verify: Type `cargo --version`
- [ ] Should show: cargo 1.75.0 or higher

### Git (Optional, but recommended)
- [ ] Download from https://git-scm.com/
- [ ] Run installer with default settings
- [ ] Verify: Type `git --version`

## Build Setup

- [ ] Navigate to project directory:
  ```powershell
  cd C:\path\to\RuView
  ```

- [ ] Run setup (this is one command):
  ```powershell
  python SETUP_AND_RUN.py
  ```

- [ ] Setup completes with no errors ✓

- [ ] New files created:
  - [ ] RUN_RUVIEW.bat exists
  - [ ] config.json exists
  - [ ] v2/target/release/ folder exists

## Verification

After setup completes:

- [ ] Trust Kill Switch test passed
  ```
  ✓ CSI frame parsing: PASS
  ✓ Signal preprocessing: PASS
  ✓ Neural network inference: PASS
  ✓ Vital signs extraction: PASS
  VERDICT: PASS
  ```

- [ ] No errors in setup output
- [ ] No missing dependencies reported

## Run Application

- [ ] Double-click: RUN_RUVIEW.bat
  OR
- [ ] From PowerShell: `.\RUN_RUVIEW.bat`

- [ ] Wait 5 seconds for startup

- [ ] Two new PowerShell windows open:
  - [ ] "RuView API Server" window
  - [ ] "RuView UI Server" window

- [ ] Browser opens automatically to http://localhost:3000

- [ ] Dashboard loads with:
  - [ ] Presence detection chart
  - [ ] Breathing rate chart
  - [ ] Heart rate chart
  - [ ] Activity recognition display
  - [ ] Alert notifications

## Verify Servers

Check both servers are running:

- [ ] API Server (port 8000)
  ```powershell
  curl http://localhost:8000/api/v1/health
  ```
  Should return: `{"status":"ok","version":"1.2.0"}`

- [ ] UI Server (port 3000)
  ```powershell
  curl http://localhost:3000
  ```
  Should return HTML content

- [ ] WebSocket stream working
  ```powershell
  # Visit http://localhost:8000/docs
  # Try "GET /api/v1/sensors" endpoint
  ```

## Dashboard Verification

On http://localhost:3000:

- [ ] Page loads without errors
- [ ] Dashboard title visible
- [ ] Presence chart displays
- [ ] Breathing rate chart displays
- [ ] Heart rate chart displays
- [ ] Activity display shows status
- [ ] Charts update every 100ms (watch for movement)

## Optional: Hardware Setup

Only if you want to connect ESP32:

- [ ] Buy ESP32-S3 or ESP32-C6
- [ ] Connect via USB-C
- [ ] Dashboard detects device (shows in Hardware panel)
- [ ] Click "Flash Firmware"
- [ ] Configure WiFi SSID/password
- [ ] Device auto-discovers server
- [ ] Real-time CSI data starts streaming

## Troubleshooting Checklist

If something fails:

### Setup Failed
- [ ] Python installed correctly? `python --version`
- [ ] Rust installed correctly? `cargo --version`
- [ ] Internet connection active?
- [ ] 50 GB disk space available?
- [ ] No firewall blocking downloads?
- [ ] Try again: `python SETUP_AND_RUN.py`

### Can't Start App
- [ ] RUN_RUVIEW.bat exists?
- [ ] Ports 8000 and 3000 available?
  ```powershell
  netstat -ano | findstr :8000
  netstat -ano | findstr :3000
  ```
- [ ] Python running? Check Windows Task Manager
- [ ] Try: `taskkill /F /IM python.exe` then restart

### Dashboard Won't Load
- [ ] Both PowerShell windows still open?
- [ ] Try: `curl http://localhost:3000`
- [ ] Check Windows Firewall (allow python.exe)
- [ ] Try different browser (Chrome, Firefox, Edge)
- [ ] Clear browser cache: Ctrl+Shift+Delete

### Port Already in Use
```powershell
# Find what's using port 8000
netstat -ano | findstr :8000

# Kill it (replace PID)
taskkill /PID 12345 /F

# Change port in config.json and restart
```

## Performance Verification

Check performance is good:

- [ ] CPU usage < 10% (Task Manager)
- [ ] Memory usage < 500 MB
- [ ] Charts update smoothly (60 FPS)
- [ ] No lag when moving charts
- [ ] API responds quickly (< 100ms)

## Documentation Review

Before using, read:

- [ ] QUICK_START.md (5 min read)
- [ ] WINDOWS_APP_GUIDE.md (15 min read)
- [ ] http://localhost:8000/docs (API reference)

## Integration Tests

Try these to verify everything works:

### REST API
```powershell
# Get health status
curl http://localhost:8000/api/v1/health

# List sensors
curl http://localhost:8000/api/v1/sensors

# Open API docs
start http://localhost:8000/docs
```

### WebSocket Streaming
```javascript
// Open browser console (F12)
// Paste this:
const ws = new WebSocket('ws://localhost:8000/api/v1/stream');
ws.onmessage = (e) => {
  const data = JSON.parse(e.data);
  console.log('Breathing:', data.vitals.breathing_bpm);
  console.log('Heart Rate:', data.vitals.heart_rate_bpm);
};
```

### Configuration
- [ ] Open config.json in Notepad
- [ ] Verify settings make sense
- [ ] Restart app to apply changes

## Production Readiness

Final checks before regular use:

- [ ] Application starts reliably
- [ ] No errors in PowerShell windows
- [ ] Dashboard responsive (no freezes)
- [ ] Real-time data updating smoothly
- [ ] Can stop/restart without issues
- [ ] Configuration persists after restart

## Next Steps

After verification passes:

### With Hardware
- [ ] Purchase ESP32-S3 ($9)
- [ ] Flash firmware from dashboard
- [ ] Configure WiFi
- [ ] Real-time CSI data streams

### Without Hardware
- [ ] Use built-in simulation
- [ ] 1,000 real CSI frames included
- [ ] Full feature parity with hardware
- [ ] Upgrade to hardware anytime

### Advanced
- [ ] Read WINDOWS_APP_GUIDE.md for all features
- [ ] Integrate with Home Assistant
- [ ] Configure custom automations
- [ ] Scale to multiple sensors

## Maintenance

Ongoing:

- [ ] Keep Windows Updated
- [ ] Keep Python Updated (yearly)
- [ ] Keep Rust Updated: `rustup update`
- [ ] Backup config.json if customized
- [ ] Monitor logs for errors

## Support

If issues persist:

1. [ ] Check troubleshooting section above
2. [ ] Read WINDOWS_APP_GUIDE.md
3. [ ] Check http://localhost:8000/docs
4. [ ] Visit GitHub issues: https://github.com/ruvnet/RuView/issues
5. [ ] Email support: ruv@ruv.net

---

## ✅ READY!

All checks passed? You're ready to use RuView!

Run: `.\RUN_RUVIEW.bat`

Enjoy real-time WiFi sensing! 🚀

---

## Quick Reference

| Component | Check Command | Expected Result |
|-----------|---------------|-----------------|
| Python | `python --version` | 3.9+ |
| Rust | `cargo --version` | 1.75+ |
| API Server | `curl http://localhost:8000/api/v1/health` | {"status":"ok"} |
| UI Server | `curl http://localhost:3000` | HTML content |
| Dashboard | http://localhost:3000 | Charts update |

---

## Files Reference

| File | Purpose |
|------|---------|
| SETUP_AND_RUN.py | Main setup script |
| RUN_RUVIEW.bat | App launcher |
| config.json | Configuration |
| QUICK_START.md | 60-second guide |
| WINDOWS_APP_GUIDE.md | Complete manual |
| README_WINDOWS11.md | Overview |

---

Last updated: January 2026
Platform: Windows 11
Status: Production Ready ✅
