"""Tests for HealthCheckService."""

import pytest
from unittest.mock import MagicMock


class TestHealthCheckServiceInit:
    def test_init(self, mock_settings):
        from src.services.health_check import HealthCheckService
        svc = HealthCheckService(mock_settings)
        assert svc._initialized is False
        assert svc._running is False

    @pytest.mark.asyncio
    async def test_initialize(self, mock_settings):
        from src.services.health_check import HealthCheckService
        svc = HealthCheckService(mock_settings)
        await svc.initialize()
        assert svc._initialized is True
        assert "api" in svc._services
        assert "database" in svc._services
        assert "hardware" in svc._services

    @pytest.mark.asyncio
    async def test_double_initialize(self, mock_settings):
        from src.services.health_check import HealthCheckService
        svc = HealthCheckService(mock_settings)
        await svc.initialize()
        await svc.initialize()  # idempotent
        assert svc._initialized is True


class TestHealthCheckAggregation:
    @pytest.mark.asyncio
    async def test_services_registered(self, mock_settings):
        from src.services.health_check import HealthCheckService, HealthStatus
        svc = HealthCheckService(mock_settings)
        await svc.initialize()
        assert len(svc._services) == 6
        for name, sh in svc._services.items():
            assert sh.status == HealthStatus.UNKNOWN

    @pytest.mark.asyncio
    async def test_service_names(self, mock_settings):
        from src.services.health_check import HealthCheckService
        svc = HealthCheckService(mock_settings)
        await svc.initialize()
        expected = {"api", "database", "redis", "hardware", "pose", "stream"}
        assert set(svc._services.keys()) == expected


class TestHealthStatus:
    def test_enum_values(self):
        from src.services.health_check import HealthStatus
        assert HealthStatus.HEALTHY.value == "healthy"
        assert HealthStatus.DEGRADED.value == "degraded"
        assert HealthStatus.UNHEALTHY.value == "unhealthy"
        assert HealthStatus.UNKNOWN.value == "unknown"


class TestHealthCheck:
    def test_health_check_dataclass(self):
        from src.services.health_check import HealthCheck, HealthStatus
        hc = HealthCheck(name="test", status=HealthStatus.HEALTHY, message="ok")
        assert hc.name == "test"
        assert hc.status == HealthStatus.HEALTHY
        assert hc.duration_ms == 0.0
