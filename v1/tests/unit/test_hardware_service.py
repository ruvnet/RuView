"""Tests for HardwareService."""

import pytest
from unittest.mock import MagicMock, AsyncMock, patch


class TestHardwareServiceInit:
    def test_init(self, mock_settings, mock_domain_config):
        mock_settings.mock_hardware = True
        with patch("src.services.hardware_service.RouterInterface"):
            from src.services.hardware_service import HardwareService
            svc = HardwareService(mock_settings, mock_domain_config)
            assert svc.is_running is False
            assert svc.stats["total_samples"] == 0
            assert svc.stats["connected_routers"] == 0

    def test_stats_defaults(self, mock_settings, mock_domain_config):
        mock_settings.mock_hardware = True
        with patch("src.services.hardware_service.RouterInterface"):
            from src.services.hardware_service import HardwareService
            svc = HardwareService(mock_settings, mock_domain_config)
            assert svc.stats["successful_samples"] == 0
            assert svc.stats["failed_samples"] == 0
            assert svc.stats["last_sample_time"] is None


class TestHardwareServiceLifecycle:
    @pytest.mark.asyncio
    async def test_start(self, mock_settings, mock_domain_config):
        mock_settings.mock_hardware = True
        with patch("src.services.hardware_service.RouterInterface"):
            from src.services.hardware_service import HardwareService
            svc = HardwareService(mock_settings, mock_domain_config)
            svc._initialize_routers = AsyncMock()
            svc._monitoring_loop = AsyncMock()
            await svc.start()
            assert svc.is_running is True

    @pytest.mark.asyncio
    async def test_double_start_idempotent(self, mock_settings, mock_domain_config):
        mock_settings.mock_hardware = True
        with patch("src.services.hardware_service.RouterInterface"):
            from src.services.hardware_service import HardwareService
            svc = HardwareService(mock_settings, mock_domain_config)
            svc._initialize_routers = AsyncMock()
            svc._monitoring_loop = AsyncMock()
            await svc.start()
            await svc.start()  # idempotent
            assert svc.is_running is True


class TestHardwareServiceRouter:
    def test_no_routers_on_init(self, mock_settings, mock_domain_config):
        mock_settings.mock_hardware = True
        with patch("src.services.hardware_service.RouterInterface"):
            from src.services.hardware_service import HardwareService
            svc = HardwareService(mock_settings, mock_domain_config)
            assert len(svc.router_interfaces) == 0

    def test_max_recent_samples(self, mock_settings, mock_domain_config):
        mock_settings.mock_hardware = True
        with patch("src.services.hardware_service.RouterInterface"):
            from src.services.hardware_service import HardwareService
            svc = HardwareService(mock_settings, mock_domain_config)
            assert svc.max_recent_samples == 1000
