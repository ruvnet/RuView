"""Command line interface for the Hyprstream client."""

import time
import click
import pandas as pd
from typing import List, Optional

from .client import MetricsClient, MetricRecord

@click.group()
def cli():
    """Hyprstream metrics client CLI."""
    pass

@cli.command()
@click.option('--metric-id', required=True, help='ID of the metric to set')
@click.option('--value', required=True, type=float, help='Value to set')
@click.option('--window-size', default=10, type=int, help='Size of the running window')
@click.option('--host', default='localhost', help='Hyprstream server host')
@click.option('--port', default=50051, type=int, help='Hyprstream server port')
def set_metric(metric_id: str, value: float, window_size: int, host: str, port: int):
    """Set a single metric value."""
    connection_string = f"grpc://{host}:{port}"
    with MetricsClient(connection_string) as client:
        metric = MetricRecord(
            metric_id=metric_id,
            timestamp=int(time.time() * 1e9),
            value_running_window_sum=value * window_size,
            value_running_window_avg=value,
            value_running_window_count=window_size
        )
        client.set_metric(metric)
        click.echo(f"Set metric {metric_id} to {value}")

@cli.command()
@click.option('--metric-id', multiple=True, help='Filter by metric ID')
@click.option('--window', default=60, type=int, help='Time window in seconds')
@click.option('--host', default='localhost', help='Hyprstream server host')
@click.option('--port', default=50051, type=int, help='Hyprstream server port')
def query_metrics(metric_id: Optional[List[str]], window: int, host: str, port: int):
    """Query metrics within a time window."""
    connection_string = f"grpc://{host}:{port}"
    with MetricsClient(connection_string) as client:
        if metric_id:
            df = client.query_metrics(
                metric_ids=list(metric_id),
                from_timestamp=int((time.time() - window) * 1e9)
            )
        else:
            df = client.get_metrics_window(window)
        
        if df.empty:
            click.echo("No metrics found")
        else:
            # Format timestamp for better readability
            df['timestamp'] = pd.to_datetime(df['timestamp'], unit='ns')
            click.echo(df.to_string())

if __name__ == '__main__':
    cli() 