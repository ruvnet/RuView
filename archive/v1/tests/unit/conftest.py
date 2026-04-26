"""Shared fixtures for unit tests."""

import os
import pytest
from unittest.mock import MagicMock, AsyncMock, patch

# Set SECRET_KEY before any settings import
os.environ.setdefault("SECRET_KEY", "test-secret-key-for-unit-tests-only")
os.environ.setdefault("JWT_SECRET_KEY", "test-secret-key-for-unit-tests-only")


@pytest.fixture
def mock_settings():
    """Create a mock Settings object."""
    settings = MagicMock()
    settings.secret_key = "test-secret-key-for-unit-tests-only"
    settings.jwt_algorithm = "HS256"
    settings.jwt_expire_hours = 24
    settings.app_name = "test-app"
    settings.version = "0.1.0"
    settings.is_production = False
    settings.enable_rate_limiting = False
    settings.enable_authentication = False
    settings.rate_limit_requests = 100
    settings.rate_limit_window = 60
    settings.rate_limit_authenticated_requests = 1000
    settings.allowed_hosts = ["*"]
    settings.csi_buffer_size = 100
    settings.stream_buffer_size = 100
    settings.mock_hardware = True
    settings.mock_pose_data = True
    settings.enable_real_time_processing = False
    settings.trusted_proxies = ["127.0.0.1"]
    return settings


@pytest.fixture
def mock_domain_config():
    """Create a mock DomainConfig object."""
    config = MagicMock()
    config.pose_estimation = MagicMock()
    config.streaming = MagicMock()
    config.hardware = MagicMock()
    return config


@pytest.fixture
def mock_redis():
    """Provide a mock Redis client."""
    with patch("redis.Redis") as mock:
        client = MagicMock()
        client.ping.return_value = True
        client.get.return_value = None
        client.set.return_value = True
        mock.return_value = client
        yield client
