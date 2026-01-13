# Contributing Guide

## Overview

Welcome to the WiFi-DensePose project! This guide provides comprehensive information for developers who want to contribute to the project, including setup instructions, coding standards, development workflow, and submission guidelines.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Development Environment Setup](#development-environment-setup)
3. [Project Structure](#project-structure)
4. [Coding Standards](#coding-standards)
5. [Development Workflow](#development-workflow)
6. [Testing Guidelines](#testing-guidelines)
7. [Documentation Standards](#documentation-standards)
8. [Pull Request Process](#pull-request-process)
9. [Code Review Guidelines](#code-review-guidelines)
10. [Release Process](#release-process)

## Getting Started

### Prerequisites

Before contributing, ensure you have:

- **Git**: Version control system
- **Python 3.8+**: Primary development language
- **Docker**: For containerized development
- **Node.js 16+**: For frontend development (if applicable)
- **CUDA Toolkit**: For GPU development (optional)

### Initial Setup

1. **Fork the Repository**:
   ```bash
   # Fork on GitHub, then clone your fork
   git clone https://github.com/YOUR_USERNAME/wifi-densepose.git
   cd wifi-densepose
   
   # Add upstream remote
   git remote add upstream https://github.com/original-org/wifi-densepose.git
   ```

2. **Set Up Development Environment**:
   ```bash
   # Create virtual environment
   python -m venv venv
   source venv/bin/activate  # On Windows: venv\Scripts\activate
   
   # Install development dependencies
   pip install -r requirements-dev.txt
   
   # Install pre-commit hooks
   pre-commit install
   ```

3. **Configure Environment**:
   ```bash
   # Copy development configuration
   cp .env.example .env.dev
   
   # Edit configuration for development
   nano .env.dev
   ```

## Development Environment Setup

### Local Development

#### Option 1: Native Development

```bash
# Install system dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install -y python3-dev build-essential cmake
sudo apt install -y libopencv-dev ffmpeg

# Install Python dependencies
pip install -r requirements-dev.txt

# Install the package in development mode
pip install -e .

# Run tests to verify setup
pytest tests/
```

#### Option 2: Docker Development

```bash
# Build development container
docker-compose -f docker-compose.dev.yml build

# Start development services
docker-compose -f docker-compose.dev.yml up -d

# Access development container
docker-compose -f docker-compose.dev.yml exec wifi-densepose-dev bash
```

### IDE Configuration

#### VS Code Setup

Create `.vscode/settings.json`:

```json
{
    "python.defaultInterpreterPath": "./venv/bin/python",
    "python.linting.enabled": true,
    "python.linting.pylintEnabled": true,
    "python.linting.flake8Enabled": true,
    "python.linting.mypyEnabled": true,
    "python.formatting.provider": "black",
    "python.formatting.blackArgs": ["--line-length", "88"],
    "python.sortImports.args": ["--profile", "black"],
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
        "source.organizeImports": true
    },
    "files.exclude": {
        "**/__pycache__": true,
        "**/*.pyc": true,
        ".pytest_cache": true,
        ".coverage": true
    }
}
```

#### PyCharm Setup

1. Configure Python interpreter to use virtual environment
2. Enable code inspections for Python
3. Set up code style to match Black formatting
4. Configure test runner to use pytest

### Development Tools

#### Required Tools

```bash
# Code formatting
pip install black isort

# Linting
pip install flake8 pylint mypy

# Testing
pip install pytest pytest-cov pytest-asyncio

# Documentation
pip install sphinx sphinx-rtd-theme

# Pre-commit hooks
pip install pre-commit
```

#### Optional Tools

```bash
# Performance profiling
pip install py-spy memory-profiler

# Debugging
pip install ipdb pdbpp

# API testing
pip install httpx pytest-httpx

# Database tools
pip install alembic sqlalchemy-utils
```

## Project Structure

### Directory Layout

```
wifi-densepose/
├── src/                          # Source code
│   ├── api/                      # API layer
│   │   ├── routers/             # API route handlers
│   │   ├── middleware/          # Custom middleware
│   │   └── dependencies.py     # Dependency injection
│   ├── neural_network/          # Neural network components
│   │   ├── models/              # Model definitions
│   │   ├── training/            # Training scripts
│   │   └── inference.py         # Inference engine
│   ├── hardware/                # Hardware interface
│   │   ├── csi_processor.py     # CSI data processing
│   │   └── router_interface.py  # Router communication
│   ├── tracking/                # Person tracking
│   ├── analytics/               # Analytics engine
│   ├── config/                  # Configuration management
│   └── utils/                   # Utility functions
├── tests/                       # Test suite
│   ├── unit/                    # Unit tests
│   ├── integration/             # Integration tests
│   ├── e2e/                     # End-to-end tests
│   └── fixtures/                # Test fixtures
├── docs/                        # Documentation
├── scripts/                     # Development scripts
├── docker/                      # Docker configurations
├── k8s/                         # Kubernetes manifests
└── tools/                       # Development tools
```

### Module Organization

#### Core Modules

- **`src/api/`**: FastAPI application and route handlers
- **`src/neural_network/`**: Deep learning models and inference
- **`src/hardware/`**: Hardware abstraction and CSI processing
- **`src/tracking/`**: Multi-object tracking algorithms
- **`src/analytics/`**: Event detection and analytics
- **`src/config/`**: Configuration management and validation

#### Supporting Modules

- **`src/utils/`**: Common utilities and helper functions
- **`src/database/`**: Database models and migrations
- **`src/monitoring/`**: Metrics collection and health checks
- **`src/security/`**: Authentication and authorization

## Coding Standards

### Python Style Guide

We follow [PEP 8](https://pep8.org/) with some modifications:

#### Code Formatting

```python
# Use Black for automatic formatting
# Line length: 88 characters
# String quotes: Double quotes preferred

class ExampleClass:
    """Example class demonstrating coding standards."""
    
    def __init__(self, config: Config) -> None:
        """Initialize the class with configuration."""
        self.config = config
        self._private_var = None
    
    async def process_data(
        self, 
        input_data: List[CSIData], 
        batch_size: int = 32
    ) -> List[PoseEstimation]:
        """Process CSI data and return pose estimations.
        
        Args:
            input_data: List of CSI data to process
            batch_size: Batch size for processing
            
        Returns:
            List of pose estimations
            
        Raises:
            ProcessingError: If processing fails
        """
        try:
            results = []
            for batch in self._create_batches(input_data, batch_size):
                batch_results = await self._process_batch(batch)
                results.extend(batch_results)
            return results
        except Exception as e:
            raise ProcessingError(f"Failed to process data: {e}") from e
```

#### Type Hints

```python
from typing import List, Dict, Optional, Union, Any, Callable
from dataclasses import dataclass
from pydantic import BaseModel

# Use type hints for all function signatures
def calculate_confidence(
    predictions: torch.Tensor,
    thresholds: Dict[str, float]
) -> List[float]:
    """Calculate confidence scores."""
    pass

# Use dataclasses for simple data structures
@dataclass
class PoseKeypoint:
    """Represents a pose keypoint."""
    x: float
    y: float
    confidence: float
    visible: bool = True

# Use Pydantic for API models and validation
class PoseEstimationRequest(BaseModel):
    """Request model for pose estimation."""
    csi_data: List[float]
    confidence_threshold: float = 0.5
    max_persons: int = 10
```

#### Error Handling

```python
# Define custom exceptions
class WiFiDensePoseError(Exception):
    """Base exception for WiFi-DensePose errors."""
    pass

class CSIProcessingError(WiFiDensePoseError):
    """Error in CSI data processing."""
    pass

class ModelInferenceError(WiFiDensePoseError):
    """Error in neural network inference."""
    pass

# Use specific exception handling
async def process_csi_data(csi_data: CSIData) -> ProcessedCSIData:
    """Process CSI data with proper error handling."""
    try:
        validated_data = validate_csi_data(csi_data)
        processed_data = await preprocess_csi(validated_data)
        return processed_data
    except ValidationError as e:
        logger.error(f"CSI data validation failed: {e}")
        raise CSIProcessingError(f"Invalid CSI data: {e}") from e
    except Exception as e:
        logger.exception("Unexpected error in CSI processing")
        raise CSIProcessingError(f"Processing failed: {e}") from e
```

#### Logging

```python
import logging
from src.utils.logging import get_logger

# Use structured logging
logger = get_logger(__name__)

class CSIProcessor:
    """CSI data processor with proper logging."""
    
    def __init__(self, config: CSIConfig):
        self.config = config
        logger.info(
            "Initializing CSI processor",
            extra={
                "buffer_size": config.buffer_size,
                "sampling_rate": config.sampling_rate
            }
        )
    
    async def process_frame(self, frame_data: CSIFrame) -> ProcessedFrame:
        """Process a single CSI frame."""
        start_time = time.time()
        
        try:
            result = await self._process_frame_internal(frame_data)
            
            processing_time = time.time() - start_time
            logger.debug(
                "Frame processed successfully",
                extra={
                    "frame_id": frame_data.id,
                    "processing_time_ms": processing_time * 1000,
                    "data_quality": result.quality_score
                }
            )
            
            return result
            
        except Exception as e:
            logger.error(
                "Frame processing failed",
                extra={
                    "frame_id": frame_data.id,
                    "error": str(e),
                    "processing_time_ms": (time.time() - start_time) * 1000
                },
                exc_info=True
            )
            raise
```

### Documentation Standards

#### Docstring Format

Use Google-style docstrings:

```python
def estimate_pose(
    csi_features: torch.Tensor,
    model: torch.nn.Module,
    confidence_threshold: float = 0.5
) -> List[PoseEstimation]:
    """Estimate human poses from CSI features.
    
    This function takes preprocessed CSI features and uses a neural network
    model to estimate human poses. The results are filtered by confidence
    threshold to ensure quality.
    
    Args:
        csi_features: Preprocessed CSI feature tensor of shape (batch_size, features)
        model: Trained neural network model for pose estimation
        confidence_threshold: Minimum confidence score for pose detection
        
    Returns:
        List of pose estimations with confidence scores above threshold
        
    Raises:
        ModelInferenceError: If model inference fails
        ValueError: If input features have invalid shape
        
    Example:
        >>> features = preprocess_csi_data(raw_csi)
        >>> model = load_pose_model("densepose_v1.pth")
        >>> poses = estimate_pose(features, model, confidence_threshold=0.7)
        >>> print(f"Detected {len(poses)} persons")
    """
    pass
```

#### Code Comments

```python
class PersonTracker:
    """Multi-object tracker for maintaining person identities."""
    
    def __init__(self, config: TrackingConfig):
        # Initialize Kalman filters for motion prediction
        self.kalman_filters = {}
        
        # Track management parameters
        self.max_age = config.max_age  # Frames to keep lost tracks
        self.min_hits = config.min_hits  # Minimum detections to confirm track
        
        # Association parameters
        self.iou_threshold = config.iou_threshold  # IoU threshold for matching
        
    def update(self, detections: List[Detection]) -> List[Track]:
        """Update tracks with new detections."""
        # Step 1: Predict new locations for existing tracks
        for track in self.tracks:
            track.predict()
        
        # Step 2: Associate detections with existing tracks
        matched_pairs, unmatched_dets, unmatched_trks = self._associate(
            detections, self.tracks
        )
        
        # Step 3: Update matched tracks
        for detection_idx, track_idx in matched_pairs:
            self.tracks[track_idx].update(detections[detection_idx])
        
        # Step 4: Create new tracks for unmatched detections
        for detection_idx in unmatched_dets:
            self._create_new_track(detections[detection_idx])
        
        # Step 5: Mark unmatched tracks as lost
        for track_idx in unmatched_trks:
            self.tracks[track_idx].mark_lost()
        
        # Step 6: Remove old tracks
        self.tracks = [t for t in self.tracks if t.age < self.max_age]
        
        return [t for t in self.tracks if t.is_confirmed()]
```

## Development Workflow

### Git Workflow

We use a modified Git Flow workflow:

#### Branch Types

- **`main`**: Production-ready code
- **`develop`**: Integration branch for features
- **`feature/*`**: Feature development branches
- **`hotfix/*`**: Critical bug fixes
- **`release/*`**: Release preparation branches

#### Workflow Steps

1. **Create Feature Branch**:
   ```bash
   # Update develop branch
   git checkout develop
   git pull upstream develop
   
   # Create feature branch
   git checkout -b feature/pose-estimation-improvements
   ```

2. **Development**:
   ```bash
   # Make changes and commit frequently
   git add .
   git commit -m "feat: improve pose estimation accuracy
   
   - Add temporal smoothing to keypoint detection
   - Implement confidence-based filtering
   - Update unit tests for new functionality
   
   Closes #123"
   ```

3. **Keep Branch Updated**:
   ```bash
   # Regularly sync with develop
   git fetch upstream
   git rebase upstream/develop
   ```

4. **Push and Create PR**:
   ```bash
   # Push feature branch
   git push origin feature/pose-estimation-improvements
   
   # Create pull request on GitHub
   ```

### Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

#### Types

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **style**: Code style changes (formatting, etc.)
- **refactor**: Code refactoring
- **test**: Adding or updating tests
- **chore**: Maintenance tasks

#### Examples

```bash
# Feature addition
git commit -m "feat(tracking): add Kalman filter for motion prediction

Implement Kalman filter to improve tracking accuracy by predicting
person motion between frames. This reduces ID switching and improves
overall tracking performance.

Closes #456"

# Bug fix
git commit -m "fix(api): handle empty pose data in WebSocket stream

Fix issue where empty pose data caused WebSocket disconnections.
Add proper validation and error handling for edge cases.

Fixes #789"

# Documentation
git commit -m "docs(api): update authentication examples

Add comprehensive examples for JWT token usage and API key
authentication in multiple programming languages."
```

## Testing Guidelines

### Test Structure

```
tests/
├── unit/                    # Unit tests
│   ├── test_csi_processor.py
│   ├── test_pose_estimation.py
│   └── test_tracking.py
├── integration/             # Integration tests
│   ├── test_api_endpoints.py
│   ├── test_database.py
│   └── test_neural_network.py
├── e2e/                     # End-to-end tests
│   ├── test_full_pipeline.py
│   └── test_user_scenarios.py
├── performance/             # Performance tests
│   ├── test_throughput.py
│   └── test_latency.py
└── fixtures/                # Test data and fixtures
    ├── csi_data/
    ├── pose_data/
    └── config/
```

### Writing Tests

#### Unit Tests

```python
import pytest
import torch
from unittest.mock import Mock, patch
from src.neural_network.inference import PoseEstimationService
from src.config.settings import ModelConfig

class TestPoseEstimationService:
    """Test suite for pose estimation service."""
    
    @pytest.fixture
    def model_config(self):
        """Create test model configuration."""
        return ModelConfig(
            model_path="test_model.pth",
            batch_size=16,
            confidence_threshold=0.5
        )
    
    @pytest.fixture
    def pose_service(self, model_config):
        """Create pose estimation service for testing."""
        with patch('src.neural_network.inference.torch.load'):
            service = PoseEstimationService(model_config)
            service.model = Mock()
            return service
    
    def test_estimate_poses_single_person(self, pose_service):
        """Test pose estimation for single person."""
        # Arrange
        csi_features = torch.randn(1, 256)
        expected_poses = [Mock(confidence=0.8)]
        pose_service.model.return_value = Mock()
        
        with patch.object(pose_service, '_postprocess_predictions') as mock_postprocess:
            mock_postprocess.return_value = expected_poses
            
            # Act
            result = pose_service.estimate_poses(csi_features)
            
            # Assert
            assert len(result) == 1
            assert result[0].confidence == 0.8
            pose_service.model.assert_called_once()
    
    def test_estimate_poses_empty_input(self, pose_service):
        """Test pose estimation with empty input."""
        # Arrange
        csi_features = torch.empty(0, 256)
        
        # Act & Assert
        with pytest.raises(ValueError, match="Empty input features"):
            pose_service.estimate_poses(csi_features)
    
    @pytest.mark.asyncio
    async def test_batch_processing(self, pose_service):
        """Test batch processing of multiple frames."""
        # Arrange
        batch_data = [torch.randn(1, 256) for _ in range(5)]
        
        # Act
        results = await pose_service.process_batch(batch_data)
        
        # Assert
        assert len(results) == 5
        for result in results:
            assert isinstance(result, list)  # List of poses
```

#### Integration Tests

```python
import pytest
import httpx
from fastapi.testclient import TestClient
from src.api.main import app
from src.config.settings import get_test_settings

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

class TestPoseAPI:
    """Integration tests for pose API endpoints."""
    
    def test_get_latest_pose_success(self, test_client, auth_headers):
        """Test successful retrieval of latest pose data."""
        # Act
        response = test_client.get("/api/v1/pose/latest", headers=auth_headers)
        
        # Assert
        assert response.status_code == 200
        data = response.json()
        assert "timestamp" in data
        assert "persons" in data
        assert isinstance(data["persons"], list)
    
    def test_get_latest_pose_unauthorized(self, test_client):
        """Test unauthorized access to pose data."""
        # Act
        response = test_client.get("/api/v1/pose/latest")
        
        # Assert
        assert response.status_code == 401
    
    def test_start_system_success(self, test_client, auth_headers):
        """Test successful system startup."""
        # Arrange
        config = {
            "configuration": {
                "domain": "healthcare",
                "environment_id": "test_room"
            }
        }
        
        # Act
        response = test_client.post(
            "/api/v1/system/start",
            json=config,
            headers=auth_headers
        )
        
        # Assert
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "starting"
```

#### Performance Tests

```python
import pytest
import time
import asyncio
from src.neural_network.inference import PoseEstimationService

class TestPerformance:
    """Performance tests for critical components."""
    
    @pytest.mark.performance
    def test_pose_estimation_latency(self, pose_service):
        """Test pose estimation latency requirements."""
        # Arrange
        csi_features = torch.randn(1, 256)
        
        # Act
        start_time = time.time()
        result = pose_service.estimate_poses(csi_features)
        end_time = time.time()
        
        # Assert
        latency_ms = (end_time - start_time) * 1000
        assert latency_ms < 50, f"Latency {latency_ms}ms exceeds 50ms requirement"
    
    @pytest.mark.performance
    async def test_throughput_requirements(self, pose_service):
        """Test system throughput requirements."""
        # Arrange
        batch_size = 32
        num_batches = 10
        csi_batches = [torch.randn(batch_size, 256) for _ in range(num_batches)]
        
        # Act
        start_time = time.time()
        tasks = [pose_service.process_batch(batch) for batch in csi_batches]
        results = await asyncio.gather(*tasks)
        end_time = time.time()
        
        # Assert
        total_frames = batch_size * num_batches
        fps = total_frames / (end_time - start_time)
        assert fps >= 30, f"Throughput {fps:.1f} FPS below 30 FPS requirement"
```

### Running Tests

```bash
# Run all tests
pytest

# Run specific test categories
pytest tests/unit/
pytest tests/integration/
pytest -m performance

# Run with coverage
pytest --cov=src --cov-report=html

# Run tests in parallel
pytest -n auto

# Run specific test file
pytest tests/unit/test_csi_processor.py

# Run specific test method
pytest tests/unit/test_csi_processor.py::TestCSIProcessor::test_process_frame
```

## Documentation Standards

### API Documentation

Use OpenAPI/Swagger specifications:

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
from typing import List, Optional

app = FastAPI(
    title="WiFi-DensePose API",
    description="Privacy-preserving human pose estimation using WiFi signals",
    version="1.0.0",
    docs_url="/docs",
    redoc_url="/redoc"
)

class PoseEstimationResponse(BaseModel):
    """Response model for pose estimation."""
    
    timestamp: str = Field(..., description="ISO 8601 timestamp of estimation")
    frame_id: int = Field(..., description="Unique frame identifier")
    persons: List[PersonPose] = Field(..., description="List of detected persons")
    
    class Config:
        schema_extra = {
            "example": {
                "timestamp": "2025-01-07T10:30:00Z",
                "frame_id": 12345,
                "persons": [
                    {
                        "id": 1,
                        "confidence": 0.87,
                        "keypoints": [...]
                    }
                ]
            }
        }

@app.get(
    "/api/v1/pose/latest",
    response_model=PoseEstimationResponse,
    summary="Get latest pose data",
    description="Retrieve the most recent pose estimation results",
    responses={
        200: {"description": "Latest pose data retrieved successfully"},
        404: {"description": "No pose data available"},
        401: {"description": "Authentication required"}
    }
)
async def get_latest_pose():
    """Get the latest pose estimation data."""
    pass
```

### Code Documentation

Generate documentation with Sphinx:

```bash
# Install Sphinx
pip install sphinx sphinx-rtd-theme

# Initialize documentation
sphinx-quickstart docs

# Generate API documentation
sphinx-apidoc -o docs/api src/

# Build documentation
cd docs
make html
```

## Pull Request Process

### Before Submitting

1. **Run Tests**:
   ```bash
   # Run full test suite
   pytest
   
   # Check code coverage
   pytest --cov=src --cov-report=term-missing
   
   # Run linting
   flake8 src/
   pylint src/
   mypy src/
   ```

2. **Format Code**:
   ```bash
   # Format with Black
   black src/ tests/
   
   # Sort imports
   isort src/ tests/
   
   # Run pre-commit hooks
   pre-commit run --all-files
   ```

3. **Update Documentation**:
   ```bash
   # Update API documentation if needed
   # Update README if adding new features
   # Add docstrings to new functions/classes
   ```

### PR Template

```markdown
## Description
Brief description of changes and motivation.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Performance tests pass (if applicable)
- [ ] Manual testing completed

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Code is commented, particularly in hard-to-understand areas
- [ ] Documentation updated
- [ ] No new warnings introduced
- [ ] Tests added for new functionality

## Related Issues
Closes #123
Related to #456
```

### Review Process

1. **Automated Checks**: CI/CD pipeline runs tests and linting
2. **Code Review**: At least one maintainer reviews the code
3. **Testing**: Reviewer tests the changes locally if needed
4. **Approval**: Maintainer approves and merges the PR

## Code Review Guidelines

### For Authors

- Keep PRs focused and reasonably sized
- Provide clear descriptions and context
- Respond promptly to review feedback
- Test your changes thoroughly

### For Reviewers

- Review for correctness, performance, and maintainability
- Provide constructive feedback
- Test complex changes locally
- Approve only when confident in the changes

### Review Checklist

- [ ] Code is correct and handles edge cases
- [ ] Performance implications considered
- [ ] Security implications reviewed
- [ ] Error handling is appropriate
- [ ] Tests are comprehensive
- [ ] Documentation is updated
- [ ] Code style is consistent

## Release Process

### Version Numbering

We use [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Steps

1. **Prepare Release**:
   ```bash
   # Create release branch
   git checkout -b release/v1.2.0
   
   # Update version numbers
   # Update CHANGELOG.md
   # Update documentation
   ```

2. **Test Release**:
   ```bash
   # Run full test suite
   pytest
   
   # Run performance tests
   pytest -m performance
   
   # Test deployment
   docker-compose up --build
   ```

3. **Create Release**:
   ```bash
   # Merge to main
   git checkout main
   git merge release/v1.2.0
   
   # Tag release
   git tag -a v1.2.0 -m "Release version 1.2.0"
   git push origin v1.2.0
   ```

4. **Deploy**:
   ```bash
   # Deploy to staging
   # Run smoke tests
   # Deploy to production
   ```

---

Thank you for contributing to WiFi-DensePose! Your contributions help make privacy-preserving human sensing technology accessible to everyone.

For questions or help, please:
- Check the [documentation](../README.md)
- Open an issue on GitHub
- Join our community discussions
- Contact the maintainers directly