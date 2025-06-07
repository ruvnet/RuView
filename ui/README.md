# WiFi DensePose UI

A modular, modern web interface for the WiFi DensePose human tracking system. This UI provides real-time monitoring, configuration, and visualization of WiFi-based pose estimation.

## 🏗️ Architecture

The UI follows a modular architecture with clear separation of concerns:

```
ui/
├── app.js                 # Main application entry point
├── index.html            # Updated HTML with modular structure
├── style.css             # Complete CSS with additional styles
├── config/               # Configuration modules
│   └── api.config.js     # API endpoints and configuration
├── services/             # Service layer for API communication
│   ├── api.service.js    # HTTP API client
│   ├── websocket.service.js # WebSocket client
│   ├── pose.service.js   # Pose estimation API wrapper
│   ├── health.service.js # Health monitoring API wrapper
│   └── stream.service.js # Streaming API wrapper
├── components/           # UI components
│   ├── TabManager.js     # Tab navigation component
│   ├── DashboardTab.js   # Dashboard component with live data
│   ├── HardwareTab.js    # Hardware configuration component
│   └── LiveDemoTab.js    # Live demo with streaming
├── utils/               # Utility functions and helpers
│   └── mock-server.js   # Mock server for testing
└── tests/               # Comprehensive test suite
    ├── test-runner.html  # Test runner UI
    ├── test-runner.js    # Test framework and cases
    └── integration-test.html # Integration testing page
```

## 🚀 Features

### Smart Backend Detection
- **Automatic Detection**: Automatically detects if your FastAPI backend is running
- **Real Backend Priority**: Always uses the real backend when available
- **Mock Fallback**: Falls back to mock server only when backend is unavailable
- **Testing Mode**: Can force mock mode for testing and development

### Real-time Dashboard
- Live system health monitoring
- Real-time pose detection statistics
- Zone occupancy tracking
- System metrics (CPU, memory, disk)
- API status indicators

### Hardware Configuration
- Interactive antenna array visualization
- Real-time CSI data display
- Configuration panels
- Hardware status monitoring

### Live Demo
- WebSocket-based real-time streaming
- Signal visualization
- Pose detection visualization
- Interactive controls

### API Integration
- Complete REST API coverage
- WebSocket streaming support
- Authentication handling
- Error management
- Request/response interceptors

## 📋 API Coverage

The UI integrates with all WiFi DensePose API endpoints:

### Health Endpoints
- `GET /health/health` - System health check
- `GET /health/ready` - Readiness check
- `GET /health/live` - Liveness check
- `GET /health/metrics` - System metrics
- `GET /health/version` - Version information

### Pose Estimation
- `GET /api/v1/pose/current` - Current pose data
- `POST /api/v1/pose/analyze` - Trigger analysis
- `GET /api/v1/pose/zones/{zone_id}/occupancy` - Zone occupancy
- `GET /api/v1/pose/zones/summary` - All zones summary
- `POST /api/v1/pose/historical` - Historical data
- `GET /api/v1/pose/activities` - Recent activities
- `POST /api/v1/pose/calibrate` - System calibration
- `GET /api/v1/pose/stats` - Statistics

### Streaming
- `WS /api/v1/stream/pose` - Real-time pose stream
- `WS /api/v1/stream/events` - Event stream
- `GET /api/v1/stream/status` - Stream status
- `POST /api/v1/stream/start` - Start streaming
- `POST /api/v1/stream/stop` - Stop streaming
- `GET /api/v1/stream/clients` - Connected clients
- `DELETE /api/v1/stream/clients/{client_id}` - Disconnect client

## 🧪 Testing

### Test Runner
Open `tests/test-runner.html` to run the complete test suite:

```bash
# Serve the UI directory on port 3000 (to avoid conflicts with FastAPI on 8000)
cd /workspaces/wifi-densepose/ui
python -m http.server 3000
# Open http://localhost:3000/tests/test-runner.html
```

### Test Categories
- **API Configuration Tests** - Configuration and URL building
- **API Service Tests** - HTTP client functionality
- **WebSocket Service Tests** - WebSocket connection management
- **Pose Service Tests** - Pose estimation API wrapper
- **Health Service Tests** - Health monitoring functionality
- **UI Component Tests** - Component behavior and interaction
- **Integration Tests** - End-to-end functionality

### Integration Testing
Use `tests/integration-test.html` for visual integration testing:

```bash
# Open http://localhost:3000/tests/integration-test.html
```

Features:
- Mock server with realistic API responses
- Visual testing of all components
- Real-time data simulation
- Error scenario testing
- WebSocket stream testing

## 🛠️ Usage

### Basic Setup
```html
<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <div class="container">
        <!-- Your content -->
    </div>
    <script type="module" src="app.js"></script>
</body>
</html>
```

### Using Services
```javascript
import { poseService } from './services/pose.service.js';
import { healthService } from './services/health.service.js';

// Get current pose data
const poseData = await poseService.getCurrentPose();

// Subscribe to health updates
healthService.subscribeToHealth(health => {
    console.log('Health status:', health.status);
});

// Start pose streaming
poseService.startPoseStream({
    minConfidence: 0.7,
    maxFps: 30
});

poseService.subscribeToPoseUpdates(update => {
    if (update.type === 'pose_update') {
        console.log('New pose data:', update.data);
    }
});
```

### Using Components
```javascript
import { TabManager } from './components/TabManager.js';
import { DashboardTab } from './components/DashboardTab.js';

// Initialize tab manager
const container = document.querySelector('.container');
const tabManager = new TabManager(container);
tabManager.init();

// Initialize dashboard
const dashboardContainer = document.getElementById('dashboard');
const dashboard = new DashboardTab(dashboardContainer);
await dashboard.init();
```

## 🔧 Configuration

### API Configuration
Edit `config/api.config.js` to modify API settings:

```javascript
export const API_CONFIG = {
  BASE_URL: window.location.origin,
  API_VERSION: '/api/v1',
  
  // Rate limiting
  RATE_LIMITS: {
    REQUESTS_PER_MINUTE: 60,
    BURST_LIMIT: 10
  },
  
  // WebSocket configuration
  WS_CONFIG: {
    RECONNECT_DELAY: 5000,
    MAX_RECONNECT_ATTEMPTS: 5,
    PING_INTERVAL: 30000
  }
};
```

### Authentication
```javascript
import { apiService } from './services/api.service.js';

// Set authentication token
apiService.setAuthToken('your-jwt-token');

// Add request interceptor for auth
apiService.addRequestInterceptor((url, options) => {
    // Modify request before sending
    return { url, options };
});
```

## 🎨 Styling

The UI uses a comprehensive CSS design system with:

- CSS Custom Properties for theming
- Dark/light mode support
- Responsive design
- Component-based styling
- Smooth animations and transitions

### Key CSS Variables
```css
:root {
  --color-primary: rgba(33, 128, 141, 1);
  --color-background: rgba(252, 252, 249, 1);
  --color-surface: rgba(255, 255, 253, 1);
  --color-text: rgba(19, 52, 59, 1);
  --space-16: 16px;
  --radius-lg: 12px;
}
```

## 🔍 Monitoring & Debugging

### Health Monitoring
```javascript
import { healthService } from './services/health.service.js';

// Start automatic health checks
healthService.startHealthMonitoring(30000); // Every 30 seconds

// Check if system is healthy
const isHealthy = healthService.isSystemHealthy();

// Get specific component status
const apiStatus = healthService.getComponentStatus('api');
```

### Error Handling
```javascript
// Global error handling
window.addEventListener('error', (event) => {
    console.error('Global error:', event.error);
});

// API error handling
apiService.addResponseInterceptor(async (response, url) => {
    if (!response.ok) {
        console.error(`API error: ${response.status} for ${url}`);
    }
    return response;
});
```

## 🚀 Deployment

### Development

**Option 1: Use the startup script**
```bash
cd /workspaces/wifi-densepose/ui
./start-ui.sh
```

**Option 2: Manual setup**
```bash
# First, start your FastAPI backend (runs on port 8000)
wifi-densepose start
# or from the main project directory:
python -m wifi_densepose.main

# Then, start the UI server on a different port to avoid conflicts
cd /workspaces/wifi-densepose/ui
python -m http.server 3000
# or
npx http-server . -p 3000

# Open the UI at http://localhost:3000
# The UI will automatically detect and connect to your backend
```

### Backend Detection Behavior
- **Real Backend Available**: UI connects to `http://localhost:8000` and shows ✅ "Connected to real backend"
- **Backend Unavailable**: UI automatically uses mock server and shows ⚠️ "Mock server active - testing mode"
- **Force Mock Mode**: Set `API_CONFIG.MOCK_SERVER.ENABLED = true` for testing

### Production
1. Configure `API_CONFIG.BASE_URL` for your backend
2. Set up HTTPS for WebSocket connections
3. Configure authentication if required
4. Optimize assets (minify CSS/JS)
5. Set up monitoring and logging

## 🤝 Contributing

1. Follow the modular architecture
2. Add tests for new functionality
3. Update documentation
4. Ensure TypeScript compatibility
5. Test with mock server

## 📄 License

This project is part of the WiFi-DensePose system. See the main project LICENSE file for details.