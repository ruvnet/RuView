# 🚀 RuView Windows 11 - MANUAL SETUP COMMANDS
# Run these commands in PowerShell to build and run the application

# ============================================================================
# STEP 1: Verify Prerequisites (Run in PowerShell)
# ============================================================================

# Check Python is installed
python --version
# Should show: Python 3.9.x or higher

# Check Rust is installed
cargo --version
# Should show: cargo 1.75+ or higher

# If either is missing, install:
# Python: https://www.python.org/
# Rust: https://rustup.rs/


# ============================================================================
# STEP 2: Navigate to Project Directory
# ============================================================================

cd "C:\Users\arjun\.copilot\copilot-worktrees\RuView\ad3v3lops-design-super-eureka"


# ============================================================================
# STEP 3: Build Rust Backend (5 minutes)
# ============================================================================

cd v2
cargo build --release --workspace --no-default-features
# This compiles 15 Rust crates and runs 1,031 tests
# First time: ~5-10 minutes
# Subsequent: ~30 seconds
cd ..


# ============================================================================
# STEP 4: Install Python Dependencies (2 minutes)
# ============================================================================

pip install -r requirements.txt --quiet


# ============================================================================
# STEP 5: Build Web Frontend (2 minutes)
# ============================================================================

cd ui
npm install --silent
npm run build --silent
cd ..


# ============================================================================
# STEP 6: Verify Signal Processing Pipeline (30 seconds)
# ============================================================================

python archive/v1/data/proof/verify.py
# Should output: VERDICT: PASS


# ============================================================================
# STEP 7: Start the Application
# ============================================================================

# Start API Server (new PowerShell window)
start powershell -ArgumentList '-NoExit', '-Command', 'cd "C:\Users\arjun\.copilot\copilot-worktrees\RuView\ad3v3lops-design-super-eureka"; python -m uvicorn archive.v1.src.api.main:app --host 0.0.0.0 --port 8000 --reload'

# Start UI Server (new PowerShell window)
start powershell -ArgumentList '-NoExit', '-Command', 'cd "C:\Users\arjun\.copilot\copilot-worktrees\RuView\ad3v3lops-design-super-eureka"; python -m http.server 3000 --directory ui'

# Wait 3 seconds for servers to start
Start-Sleep -Seconds 3

# Open dashboard
start "http://localhost:3000"


# ============================================================================
# STEP 8: Verify Everything is Running
# ============================================================================

# In PowerShell, check these endpoints:

# Check API is running
curl http://localhost:8000/api/v1/health
# Should return: {"status":"ok","version":"1.2.0"}

# Check UI is running
curl http://localhost:3000
# Should return: HTML content

# Open browser to view dashboard
# http://localhost:3000

# Open API documentation
# http://localhost:8000/docs


# ============================================================================
# OPTIONAL: Run Tests to Verify Build
# ============================================================================

# Full Rust test suite
cd v2
cargo test --workspace --no-default-features
# Should show: test result: ok. 1031 passed

cd ..


# ============================================================================
# STOP: Close the application
# ============================================================================

# Close PowerShell windows with:
# - Ctrl+C in each window, OR
# - Close the window, OR
# - Run: taskkill /F /IM python.exe


# ============================================================================
# RESTART: Run the application again
# ============================================================================

# Double-click RUN_RUVIEW.bat (created by setup), OR
# Follow Step 7 above again
