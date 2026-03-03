# Hyprstream Python Client

A Python client library for interacting with the Hyprstream metrics service. This client provides a high-level interface for storing and querying metrics using Apache Arrow Flight SQL.

## Installation

```bash
# Using poetry (recommended)
poetry install

# Using pip
pip install .
```

## Quick Start

```python
from hyprstream_client import MetricsClient, MetricRecord

# Connect to Hyprstream server
with MetricsClient("grpc://localhost:50051") as client:
    # Insert a single metric
    metric = MetricRecord(
        metric_id="test_metric_1",
        timestamp=None,  # Will use current time
        value_running_window_sum=10.0,
        value_running_window_avg=2.0,
        value_running_window_count=5
    )
    client.set_metric(metric)
    
    # Query metrics from the last hour
    df = client.get_metrics_window(window_seconds=3600)
    print(df)
```

## Features

- ✅ Native Arrow Flight SQL integration
- ✅ Efficient batch operations
- ✅ Type-safe metric records
- ✅ Prepared statement caching
- ✅ Flexible query filtering
- ✅ Time window support

## Development

```bash
# Install development dependencies
poetry install --with dev

# Run tests
poetry run pytest

# Format code
poetry run black .
poetry run isort .

# Type checking
poetry run mypy .

# Linting
poetry run ruff .
```

## License

This project is licensed under the same terms as the main Hyprstream project. 