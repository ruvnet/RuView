"""Tests for the Hyprstream client."""

import time
import pytest
import pandas as pd
from hyprstream_client import MetricsClient, MetricRecord

@pytest.fixture
def client():
    """Create a client fixture for tests."""
    client = MetricsClient()
    client.connect()
    yield client
    client.disconnect()

def test_set_single_metric(client):
    """Test setting a single metric."""
    metric = MetricRecord(
        metric_id="test_metric_1",
        timestamp=int(time.time() * 1e9),
        value_running_window_sum=10.0,
        value_running_window_avg=2.0,
        value_running_window_count=5
    )
    client.set_metric(metric)
    
    # Query back the metric
    df = client.query_metrics(metric_ids=["test_metric_1"])
    assert not df.empty
    assert len(df) == 1
    assert df.iloc[0]["metric_id"] == "test_metric_1"
    assert df.iloc[0]["value_running_window_avg"] == 2.0

def test_set_metrics_batch(client):
    """Test setting multiple metrics in a batch."""
    batch_metrics = [
        MetricRecord(
            metric_id=f"test_metric_{i}",
            timestamp=int(time.time() * 1e9),
            value_running_window_sum=float(i * 10),
            value_running_window_avg=float(i),
            value_running_window_count=10
        )
        for i in range(2, 5)
    ]
    client.set_metrics_batch(batch_metrics)
    
    # Query back the metrics
    df = client.query_metrics(metric_ids=["test_metric_2", "test_metric_3", "test_metric_4"])
    assert not df.empty
    assert len(df) == 3
    assert set(df["metric_id"]) == {"test_metric_2", "test_metric_3", "test_metric_4"}

def test_query_time_window(client):
    """Test querying metrics within a time window."""
    # Insert a metric
    current_time = int(time.time() * 1e9)
    metric = MetricRecord(
        metric_id="test_window_metric",
        timestamp=current_time,
        value_running_window_sum=10.0,
        value_running_window_avg=2.0,
        value_running_window_count=5
    )
    client.set_metric(metric)
    
    # Query with different windows
    df = client.get_metrics_window(60)  # Last minute
    assert not df.empty
    assert "test_window_metric" in df["metric_id"].values
    
    df = client.query_metrics(
        from_timestamp=current_time - (3600 * 1e9),  # Last hour
        to_timestamp=current_time
    )
    assert not df.empty
    assert "test_window_metric" in df["metric_id"].values 