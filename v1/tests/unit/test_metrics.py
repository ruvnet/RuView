"""Tests for MetricsService."""

import pytest
from datetime import timedelta
from unittest.mock import MagicMock, patch


class TestMetricSeries:
    def test_add_point(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        ms.add_point(42.0)
        assert len(ms.points) == 1
        assert ms.points[0].value == 42.0

    def test_get_latest(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        ms.add_point(1.0)
        ms.add_point(2.0)
        latest = ms.get_latest()
        assert latest is not None
        assert latest.value == 2.0

    def test_get_latest_empty(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        assert ms.get_latest() is None

    def test_get_average(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        for v in [10.0, 20.0, 30.0]:
            ms.add_point(v)
        avg = ms.get_average(timedelta(minutes=5))
        assert avg == pytest.approx(20.0)

    def test_get_average_empty(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        assert ms.get_average(timedelta(minutes=5)) is None

    def test_get_max(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        for v in [10.0, 50.0, 30.0]:
            ms.add_point(v)
        mx = ms.get_max(timedelta(minutes=5))
        assert mx == 50.0

    def test_labels(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        ms.add_point(1.0, {"region": "us-east"})
        assert ms.points[0].labels["region"] == "us-east"

    def test_maxlen(self):
        from src.services.metrics import MetricSeries
        ms = MetricSeries(name="test", description="desc", unit="ms")
        for i in range(1100):
            ms.add_point(float(i))
        assert len(ms.points) == 1000


class TestMetricsService:
    def test_init(self, mock_settings):
        with patch("src.services.metrics.psutil"):
            from src.services.metrics import MetricsService
            svc = MetricsService(mock_settings)
            assert svc._metrics is not None
