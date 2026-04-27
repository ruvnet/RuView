"""Tests for StreamService."""

import pytest
from unittest.mock import MagicMock, AsyncMock, patch


class TestStreamServiceLifecycle:
    def test_init(self, mock_settings, mock_domain_config):
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        assert svc.is_running is False
        assert len(svc.connections) == 0
        assert svc.stats["active_connections"] == 0

    @pytest.mark.asyncio
    async def test_initialize(self, mock_settings, mock_domain_config):
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        await svc.initialize()

    @pytest.mark.asyncio
    async def test_start(self, mock_settings, mock_domain_config):
        mock_settings.enable_real_time_processing = False
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        await svc.start()
        assert svc.is_running is True

    @pytest.mark.asyncio
    async def test_stop(self, mock_settings, mock_domain_config):
        mock_settings.enable_real_time_processing = False
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        await svc.start()
        await svc.stop()
        assert svc.is_running is False

    @pytest.mark.asyncio
    async def test_double_start(self, mock_settings, mock_domain_config):
        mock_settings.enable_real_time_processing = False
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        await svc.start()
        await svc.start()  # should be idempotent
        assert svc.is_running is True


class TestStreamServiceConnections:
    def test_no_connections_on_init(self, mock_settings, mock_domain_config):
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        assert svc.stats["total_connections"] == 0
        assert svc.stats["messages_sent"] == 0

    def test_buffer_sizes(self, mock_settings, mock_domain_config):
        mock_settings.stream_buffer_size = 50
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        assert svc.pose_buffer.maxlen == 50
        assert svc.csi_buffer.maxlen == 50


class TestStreamServiceBroadcast:
    def test_stats_messages_failed_init_zero(self, mock_settings, mock_domain_config):
        from src.services.stream_service import StreamService
        svc = StreamService(mock_settings, mock_domain_config)
        assert svc.stats["messages_failed"] == 0
        assert svc.stats["data_points_streamed"] == 0
