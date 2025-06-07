# Testing Guide

## Overview

This guide provides comprehensive information about testing the WiFi-DensePose system, including test types, frameworks, best practices, and continuous integration setup. Our testing strategy ensures reliability, performance, and maintainability of the codebase.

## Table of Contents

1. [Testing Philosophy](#testing-philosophy)
2. [Test Types and Structure](#test-types-and-structure)
3. [Testing Frameworks and Tools](#testing-frameworks-and-tools)
4. [Unit Testing](#unit-testing)
5. [Integration Testing](#integration-testing)
6. [End-to-End Testing](#end-to-end-testing)
7. [Performance Testing](#performance-testing)
8. [Test Data and Fixtures](#test-data-and-fixtures)
9. [Mocking and Test Doubles](#mocking-and-test-doubles)
10. [Continuous Integration](#continuous-integration)
11. [Test Coverage](#test-coverage)
12. [Testing Best Practices](#testing-best-practices)

## Testing Philosophy

### Test Pyramid

We follow the test pyramid approach:

```
    /\
   /  \     E2E Tests (Few)
  /____\    - Full system integration
 /      \   - User journey validation
/________\  Integration Tests (Some)
           - Component interaction
           - API contract testing
___________
           Unit Tests (Many)
           - Individual function testing
           - Fast feedback loop
```

### Testing Principles

1. **Fast Feedback**: Unit tests provide immediate feedback
2. **Reliability**: Tests should be deterministic and stable
3. **Maintainability**: Tests should be easy to understand and modify
4. **Coverage**: Critical paths must be thoroughly tested
5. **Isolation**: Tests should not depend on external systems
6. **Documentation**: Tests serve as living documentation

## Test Types and Structure

### Directory Structure

```
tests/
├── unit/                           # Unit tests
│   ├── api/
│   │   ├── test_routers.py
│   │   └── test_middleware.py
│   ├── neural_network/
│   │   ├── test_inference.py
│   │   ├── test_models.py
│   │   └── test_training.py
│   ├── hardware/
│   │   ├── test_csi_processor.py
│   │   ├── test_router_interface.py
│   │   └── test_phase_sanitizer.py
│   ├── tracking/
│   │   ├── test_tracker.py
│   │   └── test_kalman_filter.py
│   └── analytics/
│       ├── test_event_detection.py
│       └── test_metrics.py
├── integration/                    # Integration tests
│   ├── test_api_endpoints.py
│   ├── test_database_operations.py
│   ├── test_neural_network_pipeline.py
│   └── test_hardware_integration.py
├── e2e/                           # End-to-end tests
│   ├── test_full_pipeline.py
│   ├── test_user_scenarios.py
│   └── test_domain_workflows.py
├── performance/                   # Performance tests
│   ├── test_throughput.py
│   ├── test_latency.py
│   └── test_memory_usage.py
├── fixtures/                      # Test data and fixtures
│   ├── csi_data/
│   ├── pose_data/
│   ├── config/
│   └── models/
├── conftest.py                    # Pytest configuration
└── utils/                         # Test utilities
    ├── factories.py
    ├── helpers.py
    └── assertions.py
```

### Test Categories

#### Unit Tests
- Test individual functions and classes in isolation
- Fast execution (< 1 second per test)
- No external dependencies
- High coverage of business logic

#### Integration Tests
- Test component interactions
- Database operations
- API contract validation
- External service integration

#### End-to-End Tests
- Test complete user workflows
- Full system integration
- Real-world scenarios
- Acceptance criteria validation

#### Performance Tests
- Throughput and latency measurements
- Memory usage profiling
- Scalability testing
- Resource utilization monitoring

## Testing Frameworks and Tools

### Core Testing Stack

```python
# pytest - Primary testing framework
pytest==7.4.0
pytest-asyncio==0.21.0      # Async test support
pytest-cov==4.1.0           # Coverage reporting
pytest-mock==3.11.1         # Mocking utilities
pytest-xdist==3.3.1         # Parallel test execution

# Testing utilities
factory-boy==3.3.0          # Test data factories
faker==19.3.0               # Fake data generation
freezegun==1.2.2            # Time mocking
responses==0.23.1           # HTTP request mocking

# Performance testing
pytest-benchmark==4.0.0     # Performance benchmarking
memory-profiler==0.60.0     # Memory usage profiling

# API testing
httpx==0.24.1               # HTTP client for testing
pytest-httpx==0.21.3        # HTTP mocking for httpx
```

### Configuration

#### pytest.ini

```ini
[tool:pytest]
testpaths = tests
python_files = test_*.py
python_classes = Test*
python_functions = test_*
addopts = 
    --strict-markers
    --strict-config
    --verbose
    --tb=short
    --cov=src
    --cov-report=term-missing
    --cov-report=html:htmlcov
    --cov-report=xml
    --cov-fail-under=80
markers =
    unit: Unit tests
    integration: Integration tests
    e2e: End-to-end tests
    performance: Performance tests
    slow: Slow running tests
    gpu: Tests requiring GPU
    hardware: Tests requiring hardware
asyncio_mode = auto
```

#### conftest.py

```python
import pytest
import asyncio
from unittest.mock import Mock
from fastapi.testclient import TestClient
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from src.api.main import app
from src.config.settings import get_settings, get_test_settings
from src.database.models import Base
from tests.utils.factories import CSIDataFactory, PoseEstimationFactory

# Test database setup
@pytest.fixture(scope="session")
def test_db():
    """Create test database."""
    engine = create_engine("sqlite:///:memory:")
    Base.metadata.create_all(engine)
    TestingSessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
    
    yield TestingSessionLocal
    
    Base.metadata.drop_all(engine)

@pytest.fixture
def db_session(test_db):
    """Create database session for testing."""
    session = test_db()
    try:
        yield session
    finally:
        session.close()

# API testing setup
@pytest.fixture
def test_client():
    """Create test client with test configuration."""
    app.dependency_overrides[get_settings] = get_test_settings
    return TestClient(app)

@pytest.fixture
def auth_headers(test_client):
    """Get authentication headers for testing."""
    response = test_client.post(
        "/api/v1/auth/token",
        json={"username": "test_user", "password": "test_password"}
    )
    token = response.json()["access_token"]
    return {"Authorization": f"Bearer {token}"}

# Mock hardware components
@pytest.fixture
def mock_csi_processor():
    """Mock CSI processor for testing."""
    processor = Mock()
    processor.process_frame.return_value = CSIDataFactory()
    return processor

@pytest.fixture
def mock_neural_network():
    """Mock neural network for testing."""
    network = Mock()
    network.predict.return_value = [PoseEstimationFactory()]
    return network

# Test data factories
@pytest.fixture
def csi_data():
    """Generate test CSI data."""
    return CSIDataFactory()

@pytest.fixture
def pose_estimation():
    """Generate test pose estimation."""
    return PoseEstimationFactory()
```

## Unit Testing

### Testing Individual Components

#### CSI Processor Tests

```python
import pytest
import numpy as np
from unittest.mock import Mock, patch
from src.hardware.csi_processor import CSIProcessor, CSIConfig
from src.hardware.models import CSIFrame, ProcessedCSIData

class TestCSIProcessor:
    """Test suite for CSI processor."""
    
    @pytest.fixture
    def csi_config(self):
        """Create test CSI configuration."""
        return CSIConfig(
            buffer_size=100,
            sampling_rate=30,
            antenna_count=3,
            subcarrier_count=56
        )
    
    @pytest.fixture
    def csi_processor(self, csi_config):
        """Create CSI processor for testing."""
        return CSIProcessor(csi_config)
    
    def test_process_frame_valid_data(self, csi_processor):
        """Test processing of valid CSI frame."""
        # Arrange
        frame = CSIFrame(
            timestamp=1704686400.0,
            antenna_data=np.random.complex128((3, 56)),
            metadata={"router_id": "router_001"}
        )
        
        # Act
        result = csi_processor.process_frame(frame)
        
        # Assert
        assert isinstance(result, ProcessedCSIData)
        assert result.timestamp == frame.timestamp
        assert result.phase.shape == (3, 56)
        assert result.amplitude.shape == (3, 56)
        assert np.all(np.isfinite(result.phase))
        assert np.all(result.amplitude >= 0)
    
    def test_process_frame_invalid_shape(self, csi_processor):
        """Test processing with invalid data shape."""
        # Arrange
        frame = CSIFrame(
            timestamp=1704686400.0,
            antenna_data=np.random.complex128((2, 30)),  # Wrong shape
            metadata={"router_id": "router_001"}
        )
        
        # Act & Assert
        with pytest.raises(ValueError, match="Invalid antenna data shape"):
            csi_processor.process_frame(frame)
    
    def test_phase_sanitization(self, csi_processor):
        """Test phase unwrapping and sanitization."""
        # Arrange
        # Create data with phase wrapping
        phase_data = np.array([0, np.pi/2, np.pi, -np.pi/2, 0])
        complex_data = np.exp(1j * phase_data)
        frame = CSIFrame(
            timestamp=1704686400.0,
            antenna_data=complex_data.reshape(1, -1),
            metadata={"router_id": "router_001"}
        )
        
        # Act
        result = csi_processor.process_frame(frame)
        
        # Assert
        # Check that phase is properly unwrapped
        phase_diff = np.diff(result.phase[0])
        assert np.all(np.abs(phase_diff) < np.pi), "Phase should be unwrapped"
    
    @pytest.mark.asyncio
    async def test_process_stream(self, csi_processor):
        """Test continuous stream processing."""
        # Arrange
        frames = [
            CSIFrame(
                timestamp=1704686400.0 + i,
                antenna_data=np.random.complex128((3, 56)),
                metadata={"router_id": "router_001"}
            )
            for i in range(5)
        ]
        
        with patch.object(csi_processor, '_receive_frames') as mock_receive:
            mock_receive.return_value = iter(frames)
            
            # Act
            results = []
            async for result in csi_processor.process_stream():
                results.append(result)
                if len(results) >= 5:
                    break
            
            # Assert
            assert len(results) == 5
            for i, result in enumerate(results):
                assert result.timestamp == frames[i].timestamp
```

#### Neural Network Tests

```python
import pytest
import torch
from unittest.mock import Mock, patch
from src.neural_network.inference import PoseEstimationService
from src.neural_network.models import DensePoseNet
from src.config.settings import ModelConfig

class TestPoseEstimationService:
    """Test suite for pose estimation service."""
    
    @pytest.fixture
    def model_config(self):
        """Create test model configuration."""
        return ModelConfig(
            model_path="test_model.pth",
            batch_size=16,
            confidence_threshold=0.5,
            device="cpu"
        )
    
    @pytest.fixture
    def pose_service(self, model_config):
        """Create pose estimation service for testing."""
        with patch('torch.load') as mock_load:
            mock_model = Mock(spec=DensePoseNet)
            mock_load.return_value = mock_model
            
            service = PoseEstimationService(model_config)
            return service
    
    def test_estimate_poses_single_detection(self, pose_service):
        """Test pose estimation with single person detection."""
        # Arrange
        csi_features = torch.randn(1, 256, 32, 32)
        
        # Mock model output
        mock_output = {
            'poses': torch.randn(1, 17, 3),  # 17 keypoints, 3 coords each
            'confidences': torch.tensor([0.8])
        }
        pose_service.model.return_value = mock_output
        
        # Act
        with torch.no_grad():
            result = pose_service.estimate_poses(csi_features)
        
        # Assert
        assert len(result) == 1
        assert result[0].confidence >= 0.5  # Above threshold
        assert len(result[0].keypoints) == 17
        pose_service.model.assert_called_once()
    
    def test_estimate_poses_multiple_detections(self, pose_service):
        """Test pose estimation with multiple persons."""
        # Arrange
        csi_features = torch.randn(1, 256, 32, 32)
        
        # Mock model output for 3 persons
        mock_output = {
            'poses': torch.randn(3, 17, 3),
            'confidences': torch.tensor([0.9, 0.7, 0.3])  # One below threshold
        }
        pose_service.model.return_value = mock_output
        
        # Act
        result = pose_service.estimate_poses(csi_features)
        
        # Assert
        assert len(result) == 2  # Only 2 above confidence threshold
        assert all(pose.confidence >= 0.5 for pose in result)
    
    def test_estimate_poses_empty_input(self, pose_service):
        """Test pose estimation with empty input."""
        # Arrange
        csi_features = torch.empty(0, 256, 32, 32)
        
        # Act & Assert
        with pytest.raises(ValueError, match="Empty input features"):
            pose_service.estimate_poses(csi_features)
    
    @pytest.mark.gpu
    def test_gpu_inference(self, model_config):
        """Test GPU inference if available."""
        if not torch.cuda.is_available():
            pytest.skip("GPU not available")
        
        # Arrange
        model_config.device = "cuda"
        
        with patch('torch.load') as mock_load:
            mock_model = Mock(spec=DensePoseNet)
            mock_load.return_value = mock_model
            
            service = PoseEstimationService(model_config)
            csi_features = torch.randn(1, 256, 32, 32).cuda()
            
            # Act
            result = service.estimate_poses(csi_features)
            
            # Assert
            assert service.device.type == "cuda"
            mock_model.assert_called_once()
```

#### Tracking Tests

```python
import pytest
import numpy as np
from src.tracking.tracker import PersonTracker, TrackingConfig
from src.tracking.models import Detection, Track
from tests.utils.factories import DetectionFactory

class TestPersonTracker:
    """Test suite for person tracker."""
    
    @pytest.fixture
    def tracking_config(self):
        """Create test tracking configuration."""
        return TrackingConfig(
            max_age=30,
            min_hits=3,
            iou_threshold=0.3
        )
    
    @pytest.fixture
    def tracker(self, tracking_config):
        """Create person tracker for testing."""
        return PersonTracker(tracking_config)
    
    def test_create_new_track(self, tracker):
        """Test creation of new track from detection."""
        # Arrange
        detection = DetectionFactory(
            bbox=[100, 100, 50, 100],
            confidence=0.8
        )
        
        # Act
        tracks = tracker.update([detection])
        
        # Assert
        assert len(tracks) == 0  # Track not confirmed yet (min_hits=3)
        assert len(tracker.tracks) == 1
        assert tracker.tracks[0].hits == 1
    
    def test_track_confirmation(self, tracker):
        """Test track confirmation after minimum hits."""
        # Arrange
        detection = DetectionFactory(
            bbox=[100, 100, 50, 100],
            confidence=0.8
        )
        
        # Act - Update tracker multiple times
        for _ in range(3):
            tracks = tracker.update([detection])
        
        # Assert
        assert len(tracks) == 1  # Track should be confirmed
        assert tracks[0].is_confirmed()
        assert tracks[0].track_id is not None
    
    def test_track_association(self, tracker):
        """Test association of detections with existing tracks."""
        # Arrange - Create initial track
        detection1 = DetectionFactory(bbox=[100, 100, 50, 100])
        for _ in range(3):
            tracker.update([detection1])
        
        # Similar detection (should associate)
        detection2 = DetectionFactory(bbox=[105, 105, 50, 100])
        
        # Act
        tracks = tracker.update([detection2])
        
        # Assert
        assert len(tracks) == 1
        assert len(tracker.tracks) == 1  # Same track, not new one
        # Check that track position was updated
        track = tracks[0]
        assert abs(track.bbox[0] - 105) < 10  # Position updated
    
    def test_track_loss_and_deletion(self, tracker):
        """Test track loss and deletion after max age."""
        # Arrange - Create confirmed track
        detection = DetectionFactory(bbox=[100, 100, 50, 100])
        for _ in range(3):
            tracker.update([detection])
        
        # Act - Update without detections (track should be lost)
        for _ in range(35):  # Exceed max_age=30
            tracks = tracker.update([])
        
        # Assert
        assert len(tracks) == 0
        assert len(tracker.tracks) == 0  # Track should be deleted
    
    def test_multiple_tracks(self, tracker):
        """Test tracking multiple persons simultaneously."""
        # Arrange
        detection1 = DetectionFactory(bbox=[100, 100, 50, 100])
        detection2 = DetectionFactory(bbox=[300, 100, 50, 100])
        
        # Act - Create two confirmed tracks
        for _ in range(3):
            tracks = tracker.update([detection1, detection2])
        
        # Assert
        assert len(tracks) == 2
        track_ids = [track.track_id for track in tracks]
        assert len(set(track_ids)) == 2  # Different track IDs
```

## Integration Testing

### API Integration Tests

```python
import pytest
import httpx
from fastapi.testclient import TestClient
from unittest.mock import patch, Mock

class TestPoseAPI:
    """Integration tests for pose API endpoints."""
    
    def test_pose_estimation_workflow(self, test_client, auth_headers):
        """Test complete pose estimation workflow."""
        # Step 1: Start system
        start_response = test_client.post(
            "/api/v1/system/start",
            json={
                "configuration": {
                    "domain": "healthcare",
                    "environment_id": "test_room"
                }
            },
            headers=auth_headers
        )
        assert start_response.status_code == 200
        
        # Step 2: Wait for system to be ready
        import time
        time.sleep(1)  # In real tests, poll status endpoint
        
        # Step 3: Get pose data
        pose_response = test_client.get(
            "/api/v1/pose/latest",
            headers=auth_headers
        )
        assert pose_response.status_code == 200
        
        pose_data = pose_response.json()
        assert "timestamp" in pose_data
        assert "persons" in pose_data
        
        # Step 4: Stop system
        stop_response = test_client.post(
            "/api/v1/system/stop",
            headers=auth_headers
        )
        assert stop_response.status_code == 200
    
    def test_configuration_update_workflow(self, test_client, auth_headers):
        """Test configuration update workflow."""
        # Get current configuration
        get_response = test_client.get("/api/v1/config", headers=auth_headers)
        assert get_response.status_code == 200
        
        original_config = get_response.json()
        
        # Update configuration
        update_data = {
            "detection": {
                "confidence_threshold": 0.8,
                "max_persons": 3
            }
        }
        
        put_response = test_client.put(
            "/api/v1/config",
            json=update_data,
            headers=auth_headers
        )
        assert put_response.status_code == 200
        
        # Verify configuration was updated
        verify_response = test_client.get("/api/v1/config", headers=auth_headers)
        updated_config = verify_response.json()
        
        assert updated_config["detection"]["confidence_threshold"] == 0.8
        assert updated_config["detection"]["max_persons"] == 3
    
    @pytest.mark.asyncio
    async def test_websocket_connection(self, test_client):
        """Test WebSocket connection and data streaming."""
        with test_client.websocket_connect("/ws/pose") as websocket:
            # Send subscription message
            websocket.send_json({
                "type": "subscribe",
                "channel": "pose_updates",
                "filters": {"min_confidence": 0.7}
            })
            
            # Receive confirmation
            confirmation = websocket.receive_json()
            assert confirmation["type"] == "subscription_confirmed"
            
            # Simulate pose data (in real test, trigger actual detection)
            with patch('src.api.websocket.pose_manager.broadcast_pose_update'):
                # Receive pose update
                data = websocket.receive_json()
                assert data["type"] == "pose_update"
                assert "data" in data
```

### Database Integration Tests

```python
import pytest
from sqlalchemy.orm import Session
from src.database.models import PoseData, SystemConfig
from src.database.operations import PoseDataRepository
from datetime import datetime, timedelta

class TestDatabaseOperations:
    """Integration tests for database operations."""
    
    def test_pose_data_crud(self, db_session: Session):
        """Test CRUD operations for pose data."""
        repo = PoseDataRepository(db_session)
        
        # Create
        pose_data = PoseData(
            timestamp=datetime.utcnow(),
            frame_id=12345,
            person_id=1,
            confidence=0.85,
            keypoints=[{"x": 100, "y": 200, "confidence": 0.9}],
            environment_id="test_room"
        )
        
        created_pose = repo.create(pose_data)
        assert created_pose.id is not None
        
        # Read
        retrieved_pose = repo.get_by_id(created_pose.id)
        assert retrieved_pose.frame_id == 12345
        assert retrieved_pose.confidence == 0.85
        
        # Update
        retrieved_pose.confidence = 0.90
        updated_pose = repo.update(retrieved_pose)
        assert updated_pose.confidence == 0.90
        
        # Delete
        repo.delete(updated_pose.id)
        deleted_pose = repo.get_by_id(updated_pose.id)
        assert deleted_pose is None
    
    def test_time_series_queries(self, db_session: Session):
        """Test time-series queries for pose data."""
        repo = PoseDataRepository(db_session)
        
        # Create test data with different timestamps
        base_time = datetime.utcnow()
        test_data = []
        
        for i in range(10):
            pose_data = PoseData(
                timestamp=base_time + timedelta(minutes=i),
                frame_id=i,
                person_id=1,
                confidence=0.8,
                keypoints=[],
                environment_id="test_room"
            )
            test_data.append(repo.create(pose_data))
        
        # Query by time range
        start_time = base_time + timedelta(minutes=2)
        end_time = base_time + timedelta(minutes=7)
        
        results = repo.get_by_time_range(start_time, end_time)
        assert len(results) == 6  # Minutes 2-7 inclusive
        
        # Query latest N records
        latest_results = repo.get_latest(limit=3)
        assert len(latest_results) == 3
        assert latest_results[0].frame_id == 9  # Most recent first
    
    def test_database_performance(self, db_session: Session):
        """Test database performance with large datasets."""
        repo = PoseDataRepository(db_session)
        
        # Insert large batch of data
        import time
        start_time = time.time()
        
        batch_data = []
        for i in range(1000):
            pose_data = PoseData(
                timestamp=datetime.utcnow(),
                frame_id=i,
                person_id=i % 5,  # 5 different persons
                confidence=0.8,
                keypoints=[],
                environment_id="test_room"
            )
            batch_data.append(pose_data)
        
        repo.bulk_create(batch_data)
        insert_time = time.time() - start_time
        
        # Query performance
        start_time = time.time()
        results = repo.get_latest(limit=100)
        query_time = time.time() - start_time
        
        # Assert performance requirements
        assert insert_time < 5.0  # Bulk insert should be fast
        assert query_time < 0.1   # Query should be very fast
        assert len(results) == 100
```

## End-to-End Testing

### Full Pipeline Tests

```python
import pytest
import asyncio
import numpy as np
from unittest.mock import patch, Mock
from src.pipeline.main import WiFiDensePosePipeline
from src.config.settings import get_test_settings

class TestFullPipeline:
    """End-to-end tests for complete system pipeline."""
    
    @pytest.fixture
    def pipeline(self):
        """Create test pipeline with mocked hardware."""
        settings = get_test_settings()
        settings.mock_hardware = True
        return WiFiDensePosePipeline(settings)
    
    @pytest.mark.asyncio
    async def test_complete_pose_estimation_pipeline(self, pipeline):
        """Test complete pipeline from CSI data to pose output."""
        # Arrange
        mock_csi_data = np.random.complex128((3, 56, 100))  # 3 antennas, 56 subcarriers, 100 samples
        
        with patch.object(pipeline.csi_processor, 'get_latest_data') as mock_csi:
            mock_csi.return_value = mock_csi_data
            
            # Act
            await pipeline.start()
            
            # Wait for processing
            await asyncio.sleep(2)
            
            # Get results
            results = await pipeline.get_latest_poses()
            
            # Assert
            assert len(results) > 0
            for pose in results:
                assert pose.confidence > 0
                assert len(pose.keypoints) == 17  # COCO format
                assert pose.timestamp is not None
            
            await pipeline.stop()
    
    @pytest.mark.asyncio
    async def test_healthcare_domain_workflow(self, pipeline):
        """Test healthcare-specific workflow with fall detection."""
        # Configure for healthcare domain
        await pipeline.configure_domain("healthcare")
        
        # Mock fall scenario
        fall_poses = self._create_fall_sequence()
        
        with patch.object(pipeline.pose_estimator, 'estimate_poses') as mock_estimate:
            mock_estimate.side_effect = fall_poses
            
            await pipeline.start()
            
            # Wait for fall detection
            alerts = []
            for _ in range(10):  # Check for 10 iterations
                await asyncio.sleep(0.1)
                new_alerts = await pipeline.get_alerts()
                alerts.extend(new_alerts)
                
                if any(alert.type == "fall_detection" for alert in alerts):
                    break
            
            # Assert fall was detected
            fall_alerts = [a for a in alerts if a.type == "fall_detection"]
            assert len(fall_alerts) > 0
            assert fall_alerts[0].severity in ["medium", "high"]
            
            await pipeline.stop()
    
    def _create_fall_sequence(self):
        """Create sequence of poses simulating a fall."""
        # Standing pose
        standing_pose = Mock()
        standing_pose.keypoints = [
            {"name": "head", "y": 100},
            {"name": "hip", "y": 200},
            {"name": "knee", "y": 300},
            {"name": "ankle", "y": 400}
        ]
        
        # Falling pose (head getting lower)
        falling_pose = Mock()
        falling_pose.keypoints = [
            {"name": "head", "y": 300},
            {"name": "hip", "y": 350},
            {"name": "knee", "y": 380},
            {"name": "ankle", "y": 400}
        ]
        
        # Fallen pose (horizontal)
        fallen_pose = Mock()
        fallen_pose.keypoints = [
            {"name": "head", "y": 380},
            {"name": "hip", "y": 385},
            {"name": "knee", "y": 390},
            {"name": "ankle", "y": 395}
        ]
        
        return [
            [standing_pose] * 5,    # Standing for 5 frames
            [falling_pose] * 3,     # Falling for 3 frames
            [fallen_pose] * 10      # Fallen for 10 frames
        ]
```

### User Scenario Tests

```python
import pytest
from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC

class TestUserScenarios:
    """End-to-end tests for user scenarios."""
    
    @pytest.fixture
    def driver(self):
        """Create web driver for UI testing."""
        options = webdriver.ChromeOptions()
        options.add_argument("--headless")
        driver = webdriver.Chrome(options=options)
        yield driver
        driver.quit()
    
    def test_dashboard_monitoring_workflow(self, driver):
        """Test user monitoring workflow through dashboard."""
        # Navigate to dashboard
        driver.get("http://localhost:8000/dashboard")
        
        # Login
        username_field = driver.find_element(By.ID, "username")
        password_field = driver.find_element(By.ID, "password")
        login_button = driver.find_element(By.ID, "login")
        
        username_field.send_keys("test_user")
        password_field.send_keys("test_password")
        login_button.click()
        
        # Wait for dashboard to load
        WebDriverWait(driver, 10).until(
            EC.presence_of_element_located((By.ID, "pose-visualization"))
        )
        
        # Check that pose data is displayed
        pose_count = driver.find_element(By.ID, "person-count")
        assert pose_count.text.isdigit()
        
        # Check real-time updates
        initial_timestamp = driver.find_element(By.ID, "last-update").text
        
        # Wait for update
        WebDriverWait(driver, 5).until(
            lambda d: d.find_element(By.ID, "last-update").text != initial_timestamp
        )
        
        # Verify update occurred
        updated_timestamp = driver.find_element(By.ID, "last-update").text
        assert updated_timestamp != initial_timestamp
    
    def test_alert_notification_workflow(self, driver):
        """Test alert notification workflow."""
        driver.get("http://localhost:8000/dashboard")
        
        # Login and navigate to alerts page
        self._login(driver)
        
        alerts_tab = driver.find_element(By.ID, "alerts-tab")
        alerts_tab.click()
        
        # Configure alert settings
        fall_detection_toggle = driver.find_element(By.ID, "fall-detection-enabled")
        if not fall_detection_toggle.is_selected():
            fall_detection_toggle.click()
        
        sensitivity_slider = driver.find_element(By.ID, "fall-sensitivity")
        driver.execute_script("arguments[0].value = 0.8", sensitivity_slider)
        
        save_button = driver.find_element(By.ID, "save-settings")
        save_button.click()
        
        # Trigger test alert
        test_alert_button = driver.find_element(By.ID, "test-fall-alert")
        test_alert_button.click()
        
        # Wait for alert notification
        WebDriverWait(driver, 10).until(
            EC.presence_of_element_located((By.CLASS_NAME, "alert-notification"))
        )
        
        # Verify alert details
        alert_notification = driver.find_element(By.CLASS_NAME, "alert-notification")
        assert "Fall detected" in alert_notification.text
    
    def _login(self, driver):
        """Helper method to login."""
        username_field = driver.find_element(By.ID, "username")
        password_field = driver.find_element(By.ID, "password")
        login_button = driver.find_element(By.ID, "login")
        
        username_field.send_keys("test_user")
        password_field.send_keys("test_password")
        login_button.click()
        
        WebDriverWait(driver, 10).until(
            EC.presence_of_element_located((By.ID, "dashboard"))
        )
```

## Performance Testing

### Throughput and Latency Tests

```python
import pytest
import time
import asyncio
import statistics
from concurrent.futures import ThreadPoolExecutor
from src.neural_network.inference import PoseEstimationService

class TestPerformance:
    """Performance tests for critical system components."""
    
    @pytest.mark.performance
    def test_pose_estimation_latency(self, pose_service):
        """Test pose estimation latency requirements."""
        csi_features = torch.randn(1, 256, 32, 32)
        
        # Warm up
        for _ in range(5):
            pose_service.estimate_poses(csi_features)
        
        # Measure latency
        latencies = []
        for _ in range(100):
            start_time = time.perf_counter()
            result = pose_service.estimate_poses(csi_features)
            end_time = time.perf_counter()
            
            latency_ms = (end_time - start_time) * 1000
            latencies.append(latency_ms)
        
        # Assert latency requirements
        avg_latency = statistics.mean(latencies)
        p95_latency = statistics.quantiles(latencies, n=20)[18]  # 95th percentile
        
        assert avg_latency < 50, f"Average latency {avg_latency:.1f}ms exceeds 50ms"
        assert p95_latency < 100, f"P95 latency {p95_latency:.1f}ms exceeds 100ms"
    
    @pytest.mark.performance
    async def test_system_throughput(self, pipeline):
        """Test system throughput requirements."""
        # Generate test data
        test_frames = [
            torch.randn(1, 256, 32, 32) for _ in range(1000)
        ]
        
        start_time = time.perf_counter()
        
        # Process frames concurrently
        tasks = []
        for frame in test_frames:
            task = asyncio.create_task(pipeline.process_frame(frame))
            tasks.append(task)
        
        results = await asyncio.gather(*tasks)
        end_time = time.perf_counter()
        
        # Calculate throughput
        total_time = end_time - start_time
        fps = len(test_frames) / total_time
        
        assert fps >= 30, f"Throughput {fps:.1f} FPS below 30 FPS requirement"
        assert len(results) == len(test_frames)
    
    @pytest.mark.performance
    def test_memory_usage(self, pose_service):
        """Test memory usage during processing."""
        import psutil
        import gc
        
        process = psutil.Process()
        
        # Baseline memory
        gc.collect()
        baseline_memory = process.memory_info().rss / 1024 / 1024  # MB
        
        # Process large batch
        large_batch = torch.randn(64, 256, 32, 32)
        
        for _ in range(10):
            result = pose_service.estimate_poses(large_batch)
            del result
        
        # Measure peak memory
        peak_memory = process.memory_info().rss / 1024 / 1024  # MB
        memory_increase = peak_memory - baseline_memory
        
        # Clean up
        gc.collect()
        final_memory = process.memory_info().rss / 1024 / 1024  # MB
        memory_leak = final_memory - baseline_memory
        
        # Assert memory requirements
        assert memory_increase < 2000, f"Memory usage {memory_increase:.1f}MB exceeds 2GB"
        assert memory_leak < 100, f"Memory leak {memory_leak:.1f}MB detected"
    
    @pytest.mark.performance
    def test_concurrent_requests(self, test_client, auth_headers):
        """Test API performance under concurrent load."""
        def make_request():
            response = test_client.get("/api/v1/pose/latest", headers=auth_headers)
            return response.status_code, response.elapsed.total_seconds()
        
        # Concurrent requests
        with ThreadPoolExecutor(max_workers=50) as executor:
            start_time = time.perf_counter()
            futures = [executor.submit(make_request) for _ in range(200)]
            results = [future.result() for future in futures]
            end_time = time.perf_counter()
        
        # Analyze results
        status_codes = [result[0] for result in results]
        response_times = [result[1] for result in results]
        
        success_rate = sum(1 for code in status_codes if code == 200) / len(status_codes)
        avg_response_time = statistics.mean(response_times)
        total_time = end_time - start_time
        
        # Assert performance requirements
        assert success_rate >= 0.95, f"Success rate {success_rate:.2%} below 95%"
        assert avg_response_time < 1.0, f"Average response time {avg_response_time:.2f}s exceeds 1s"
        assert total_time < 30, f"Total time {total_time:.1f}s exceeds 30s"
```

## Test Data and Fixtures

### Data Factories

```python
import factory
import numpy as np
from datetime import datetime
from src.hardware.models import CSIFrame, CSIData
from src.neural_network.models import PoseEstimation, Keypoint

class CSIFrameFactory(factory.Factory):
    """Factory for generating test CSI frames."""
    
    class Meta:
        model = CSIFrame
    
    timestamp = factory.LazyFunction(lambda: datetime.utcnow().timestamp())
    antenna_data = factory.LazyFunction(
        lambda: np.random.complex128((3, 56))
    )
    metadata = factory.Dict({
        "router_id": factory.Sequence(lambda n: f"router_{n:03d}"),
        "signal_strength": factory.Faker("pyfloat", min_value=-80, max_value=-20),
        "noise_level": factory.Faker("pyfloat", min_value=-100, max_value=-60)
    })

class KeypointFactory(factory.Factory):
    """Factory for generating test keypoints."""
    
    class Meta:
        model = Keypoint
    
    name = factory.Iterator([
        "nose", "left_eye", "right_eye", "left_ear", "right_ear",
        "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
        "left_wrist", "right_wrist", "left_hip", "right_hip",
        "left_knee", "right_knee", "left_ankle", "right_ankle"
    ])
    x = factory.Faker("pyfloat", min_value=0, max_value=640)
    y = factory.Faker("pyfloat", min_value=0, max_value=480)
    confidence = factory.Faker("pyfloat", min_value=0.5, max_value=1.0)
    visible = factory.Faker("pybool")

class PoseEstimationFactory(factory.Factory):
    """Factory for generating test pose estimations."""
    
    class Meta:
        model = PoseEstimation
    
    person_id = factory.Sequence(lambda n: n)
    confidence = factory.Faker("pyfloat", min_value=0.5, max_value=1.0)
    bounding_box = factory.LazyFunction(
        lambda: {
            "x": np.random.randint(0, 400),
            "y": np.random.randint(0, 300),
            "width": np.random.randint(50, 200),
            "height": np.random.randint(100, 300)
        }
    )
    keypoints = factory.SubFactoryList(KeypointFactory, size=17)
    timestamp = factory.LazyFunction(datetime.utcnow)
```

### Test Fixtures

```python
# tests/fixtures/csi_data.py
import numpy as np
import json
from pathlib import Path

def load_test_csi_data():
    """Load pre-recorded CSI data for testing."""
    fixture_path = Path(__file__).parent / "csi_data" / "sample_data.npz"
    
    if fixture_path.exists():
        data = np.load(fixture_path)
        return {
            "amplitude": data["amplitude"],
            "phase": data["phase"],
            "timestamps": data["timestamps"]
        }
    else:
        # Generate synthetic data if fixture doesn't exist
        return generate_synthetic_csi_data()

def generate_synthetic_csi_data():
    """Generate synthetic CSI data for testing."""
    num_samples = 1000
    num_antennas = 3
    num_subcarriers = 56
    
    # Generate realistic CSI patterns
    amplitude = np.random.exponential(scale=10, size=(num_samples, num_antennas, num_subcarriers))
    phase = np.random.uniform(-np.pi, np.pi, size=(num_samples, num_antennas, num_subcarriers))
    timestamps = np.linspace(0, 33.33, num_samples)  # 30 FPS for 33.33 seconds
    
    return {
        "amplitude": amplitude,
        "phase": phase,
        "timestamps": timestamps
    }

# tests/fixtures/pose_data.py
def load_test_pose_sequences():
    """Load test pose sequences for different scenarios."""
    return {
        "walking": load_walking_sequence(),
        "sitting": load_sitting_sequence(),
        "falling": load_falling_sequence(),
        "multiple_persons": load_multiple_persons_sequence()
    }

def load_walking_sequence():
    """Load walking pose sequence."""
    # Simplified walking pattern
    poses = []
    for frame in range(30):  # 1 second at 30 FPS
        pose = {
            "keypoints": generate_walking_keypoints(frame),
            "confidence": 0.8 + 0.1 * np.sin(frame * 0.2),
            "timestamp": frame / 30.0
        }
        poses.append(pose)
    return poses

def generate_walking_keypoints(frame):
    """Generate keypoints for walking motion."""
    # Simplified walking pattern with leg movement
    base_keypoints = {
        "nose": {"x": 320, "y": 100},
        "left_shoulder": {"x": 300, "y": 150},
        "right_shoulder": {"x": 340, "y": 150},
        "left_hip": {"x": 310, "y": 250},
        "right_hip": {"x": 330, "y": 250},
    }
    
    # Add walking motion to legs
    leg_offset = 20 * np.sin(frame * 0.4)  # Walking cycle
    base_keypoints["left_knee"] = {"x": 305 + leg_offset, "y": 350}
    base_keypoints["right_knee"] = {"x": 335 - leg_offset, "y": 350}
    base_keypoints["left_ankle"] = {"x": 300 + leg_offset, "y": 450}
    base_keypoints["right_ankle"] = {"x": 340 - leg_offset, "y": 450}
    
    return base_keypoints
```

## Mocking and Test Doubles

### Hardware Mocking

```python
# tests/mocks/hardware.py
from unittest.mock import Mock, AsyncMock
import numpy as np
import asyncio

class MockCSIProcessor:
    """Mock CSI processor for testing."""
    
    def __init__(self, config=None):
        self.config = config or {}
        self.is_running = False
        self._data_generator = self._generate_mock_data()
    
    async def start(self):
        """Start mock CSI processing."""
        self.is_running = True
    
    async def stop(self):
        """Stop mock CSI processing."""
        self.is_running = False
    
    async def get_latest_frame(self):
        """Get latest mock CSI frame."""
        if not self.is_running:
            raise RuntimeError("CSI processor not running")
        
        return next(self._data_generator)
    
    def _generate_mock_data(self):
        """Generate realistic mock CSI data."""
        frame_id = 0
        while True:
            # Generate data with some patterns
            amplitude = np.random.exponential(scale=10, size=(3, 56))
            phase = np.random.uniform(-np.pi, np.pi, size=(3, 56))
            
            # Add some motion patterns
            if frame_id % 30 < 15:  # Simulate person movement
                amplitude *= 1.2
                phase += 0.1 * np.sin(frame_id * 0.1)
            
            yield {
                "frame_id": frame_id,
                "timestamp": frame_id / 30.0,
                "amplitude": amplitude,
                "phase": phase,
                "metadata": {"router_id": "mock_router"}
            }
            frame_id += 1

class MockNeuralNetwork:
    """Mock neural network for testing."""
    
    def __init__(self, model_config=None):
        self.model_config = model_config or {}
        self.is_loaded = False
    
    def load_model(self, model_path):
        """Mock model loading."""
        self.is_loaded = True
        return True
    
    def predict(self, csi_features):
        """Mock pose prediction."""
        if not self.is_loaded:
            raise RuntimeError("Model not loaded")
        
        batch_size = csi_features.shape[0]
        
        # Generate mock predictions
        predictions = []
        for i in range(batch_size):
            # Simulate 0-2 persons detected
            num_persons = np.random.choice([0, 1, 2], p=[0.1, 0.7, 0.2])
            
            frame_predictions = []
            for person_id in range(num_persons):
                pose = {
                    "person_id": person_id,
                    "confidence": np.random.uniform(0.6, 0.95),
                    "keypoints": self._generate_mock_keypoints(),
                    "bounding_box": self._generate_mock_bbox()
                }
                frame_predictions.append(pose)
            
            predictions.append(frame_predictions)
        
        return predictions
    
    def _generate_mock_keypoints(self):
        """Generate mock keypoints."""
        keypoints = []
        for i in range(17):  # COCO format
            keypoint = {
                "x": np.random.uniform(50, 590),
                "y": np.random.uniform(50, 430),
                "confidence": np.random.uniform(0.5, 1.0),
                "visible": np.random.choice([True, False], p=[0.8, 0.2])
            }
            keypoints.append(keypoint)
        return keypoints
    
    def _generate_mock_bbox(self):
        """Generate mock bounding box."""
        x = np.random.uniform(0, 400)
        y = np.random.uniform(0, 300)
        width = np.random.uniform(50, 200)
        height = np.random.uniform(100, 300)
        
        return {"x": x, "y": y, "width": width, "height": height}
```

### API Mocking

```python
# tests/mocks/external_apis.py
import responses
import json

@responses.activate
def test_external_api_integration():
    """Test integration with external APIs using mocked responses."""
    
    # Mock external pose estimation API
    responses.add(
        responses.POST,
        "https://external-api.com/pose/estimate",
        json={
            "poses": [
                {
                    "id": 1,
                    "confidence": 0.85,
                    "keypoints": [...]
                }
            ]
        },
        status=200
    )
    
    # Mock webhook endpoint
    responses.add(
        responses.POST,
        "https://webhook.example.com/alerts",
        json={"status": "received"},
        status=200
    )
    
    # Test code that makes external API calls
    # ...

class MockWebhookServer:
    """Mock webhook server for testing notifications."""
    
    def __init__(self):
        self.received_webhooks = []
    
    def start(self, port=8080):
        """Start mock webhook server."""
        from flask import Flask, request
        
        app = Flask(__name__)
        
        @app.route('/webhook', methods=['POST'])
        def receive_webhook():
            data = request.get_json()
            self.received_webhooks.append(data)
            return {"status": "received"}, 200
        
        app.run(port=port, debug=False)
    
    def get_received_webhooks(self):
        """Get all received webhooks."""
        return self.received_webhooks.copy()
    
    def clear_webhooks(self):
        """Clear received webhooks."""
        self.received_webhooks.clear()
```

## Continuous Integration

### GitHub Actions Configuration

```yaml
# .github/workflows/test.yml
name: Test Suite

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        python-version: [3.8, 3.9, "3.10", "3.11"]
    
    services:
      postgres:
        image: timescale/timescaledb:latest-pg14
        env:
          POSTGRES_PASSWORD: postgres
          POSTGRES_DB: test_wifi_densepose
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      
      redis:
        image: redis:7-alpine
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6379:6379
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Set up Python ${{ matrix.python-version }}
      uses: actions/setup-python@v4
      with:
        python-version: ${{ matrix.python-version }}
    
    - name: Cache pip dependencies
      uses: actions/cache@v3
      with:
        path: ~/.cache/pip
        key: ${{ runner.os }}-pip-${{ hashFiles('**/requirements*.txt') }}
        restore-keys: |
          ${{ runner.os }}-pip-
    
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libopencv-dev ffmpeg
    
    - name: Install Python dependencies
      run: |
        python -m pip install --upgrade pip
        pip install -r requirements-dev.txt
    
    - name: Lint with flake8
      run: |
        flake8 src/ tests/ --count --select=E9,F63,F7,F82 --show-source --statistics
        flake8 src/ tests/ --count --exit-zero --max-complexity=10 --max-line-length=88 --statistics
    
    - name: Type check with mypy
      run: |
        mypy src/
    
    - name: Test with pytest
      env:
        DATABASE_URL: postgresql://postgres:postgres@localhost:5432/test_wifi_densepose
        REDIS_URL: redis://localhost:6379/0
        SECRET_KEY: test-secret-key
        MOCK_HARDWARE: true
      run: |
        pytest tests/ -v --cov=src --cov-report=xml --cov-report=term-missing
    
    - name: Upload coverage to Codecov
      uses: codecov/codecov-action@v3
      with:
        file: ./coverage.xml
        flags: unittests
        name: codecov-umbrella

  performance-test:
    runs-on: ubuntu-latest
    needs: test
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Set up Python
      uses: actions/setup-python@v4
      with:
        python-version: "3.10"
    
    - name: Install dependencies
      run: |
        python -m pip install --upgrade pip
        pip install -r requirements-dev.txt
    
    - name: Run performance tests
      run: |
        pytest tests/performance/ -v --benchmark-only --benchmark-json=benchmark.json
    
    - name: Store benchmark result
      uses: benchmark-action/github-action-benchmark@v1
      with:
        tool: 'pytest'
        output-file-path: benchmark.json
        github-token: ${{ secrets.GITHUB_TOKEN }}
        auto-push: true

  integration-test:
    runs-on: ubuntu-latest
    needs: test
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Build Docker images
      run: |
        docker-compose -f docker-compose.test.yml build
    
    - name: Run integration tests
      run: |
        docker-compose -f docker-compose.test.yml up --abort-on-container-exit
    
    - name: Cleanup
      run: |
        docker-compose -f docker-compose.test.yml down -v
```

### Pre-commit Configuration

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.4.0
    hooks:
      - id: trailing-whitespace
      - id: end-of-file-fixer
      - id: check-yaml
      - id: check-added-large-files
      - id: check-merge-conflict
  
  - repo: https://github.com/psf/black
    rev: 23.3.0
    hooks:
      - id: black
        language_version: python3
  
  - repo: https://github.com/pycqa/isort
    rev: 5.12.0
    hooks:
      - id: isort
        args: ["--profile", "black"]
  
  - repo: https://github.com/pycqa/flake8
    rev: 6.0.0
    hooks:
      - id: flake8
        additional_dependencies: [flake8-docstrings]
  
  - repo: https://github.com/pre-commit/mirrors-mypy
    rev: v1.3.0
    hooks:
      - id: mypy
        additional_dependencies: [types-all]
  
  - repo: local
    hooks:
      - id: pytest-check
        name: pytest-check
        entry: pytest
        language: system
        pass_filenames: false
        always_run: true
        args: [tests/unit/, --tb=short]
```

## Test Coverage

### Coverage Configuration

```ini
# .coveragerc
[run]
source = src/
omit = 
    src/*/tests/*
    src/*/test_*
    */venv/*
    */virtualenv/*
    */.tox/*
    */migrations/*
    */settings/*

[report]
exclude_lines =
    pragma: no cover
    def __repr__
    if self.debug:
    if settings.DEBUG
    raise AssertionError
    raise NotImplementedError
    if 0:
    if __name__ == .__main__.:
    class .*\bProtocol\):
    @(abc\.)?abstractmethod

[html]
directory = htmlcov
```

### Coverage Targets

- **Overall Coverage**: Minimum 80%
- **Critical Components**: Minimum 90%
  - Neural network inference
  - CSI processing
  - Person tracking
  - API endpoints
- **New Code**: Minimum 95%

### Coverage Reporting

```bash
# Generate coverage report
pytest --cov=src --cov-report=html --cov-report=term-missing

# View HTML report
open htmlcov/index.html

# Check coverage thresholds
pytest --cov=src --cov-fail-under=80
```

## Testing Best Practices

### Test Organization

1. **One Test Class per Component**: Group related tests together
2. **Descriptive Test Names**: Use clear, descriptive test method names
3. **Arrange-Act-Assert**: Structure tests with clear sections
4. **Test Independence**: Each test should be independent and isolated

### Test Data Management

1. **Use Factories**: Generate test data with factories instead of hardcoded values
2. **Realistic Data**: Use realistic test data that represents actual usage
3. **Edge Cases**: Test boundary conditions and edge cases
4. **Error Conditions**: Test error handling and exception cases

### Performance Considerations

1. **Fast Unit Tests**: Keep unit tests fast (< 1 second each)
2. **Parallel Execution**: Use pytest-xdist for parallel test execution
3. **Test Categorization**: Use markers to categorize slow tests
4. **Resource Cleanup**: Properly clean up resources after tests

### Maintenance

1. **Regular Updates**: Keep test dependencies updated
2. **Flaky Test Detection**: Monitor and fix flaky tests
3. **Test Documentation**: Document complex test scenarios
4. **Refactoring**: Refactor tests when production code changes

---

This testing guide provides a comprehensive framework for ensuring the reliability and quality of the WiFi-DensePose system. Regular testing and continuous improvement of the test suite are essential for maintaining a robust and reliable system.

For more information, see:
- [Contributing Guide](contributing.md)
- [Architecture Overview](architecture-overview.md)
- [Deployment Guide](deployment-guide.md)