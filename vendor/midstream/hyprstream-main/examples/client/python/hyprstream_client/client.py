"""Client implementation for the Hyprstream metrics service."""

import time
from typing import List, Optional, Dict, Any, Union

import pyarrow as pa
import pyarrow.flight as flight
import adbc_driver_flightsql
import adbc_driver_flightsql.dbapi
import pandas as pd

from .types import MetricRecord

class MetricsClient:
    """Client for interacting with the Hyprstream metrics service."""
    
    def __init__(self, connection_string: str = "grpc://localhost:50051"):
        """Initialize the metrics client with a connection string."""
        self.connection_string = connection_string
        self.conn = None
        
    def connect(self):
        """Establish connection to the Flight SQL server."""
        print("Connecting to Flight SQL server")
        self.conn = adbc_driver_flightsql.dbapi.connect(self.connection_string)
        
    def disconnect(self):
        """Close the connection to the Flight SQL server."""
        if self.conn:
            self.conn.close()
            self.conn = None
            
    def __enter__(self):
        """Context manager entry."""
        self.connect()
        return self
        
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.disconnect()

    def set_metric(self, metric: Union[MetricRecord, Dict[str, Any]]) -> None:
        """Insert or update a single metric."""
        if not self.conn:
            raise ConnectionError("Not connected to server")

        if isinstance(metric, dict):
            metric = MetricRecord.from_dict(metric)

        query = """
        INSERT INTO metrics (
            metric_id, timestamp, value_running_window_sum,
            value_running_window_avg, value_running_window_count
        ) VALUES (?, ?, ?, ?, ?)
        """
        
        cursor = self.conn.cursor()
        try:
            cursor.execute(query, [
                metric.metric_id,
                metric.timestamp,
                metric.value_running_window_sum,
                metric.value_running_window_avg,
                metric.value_running_window_count
            ])
        finally:
            cursor.close()

    def set_metrics_batch(self, metrics: List[Union[MetricRecord, Dict[str, Any]]]) -> None:
        """Insert multiple metrics using Arrow's native batching."""
        if not self.conn:
            raise ConnectionError("Not connected to server")

        # Convert all metrics to MetricRecord objects
        records = [
            m if isinstance(m, MetricRecord) else MetricRecord.from_dict(m)
            for m in metrics
        ]

        # Create Arrow arrays
        metric_ids = pa.array([r.metric_id for r in records], type=pa.string())
        timestamps = pa.array([r.timestamp for r in records], type=pa.int64())
        sums = pa.array([r.value_running_window_sum for r in records], type=pa.float64())
        avgs = pa.array([r.value_running_window_avg for r in records], type=pa.float64())
        counts = pa.array([r.value_running_window_count for r in records], type=pa.int64())

        # Create Arrow table
        table = pa.Table.from_arrays(
            [metric_ids, timestamps, sums, avgs, counts],
            names=[
                'metric_id', 'timestamp', 'value_running_window_sum',
                'value_running_window_avg', 'value_running_window_count'
            ]
        )

        cursor = self.conn.cursor()
        try:
            cursor.adbc_statement.set_sql_query("""
                INSERT INTO metrics (
                    metric_id, timestamp, value_running_window_sum,
                    value_running_window_avg, value_running_window_count
                ) VALUES (?, ?, ?, ?, ?)
            """)
            cursor.adbc_statement.bind(table)
            cursor.adbc_statement.execute_update()
        finally:
            cursor.close()

        print(f"Inserted {len(records)} metrics in batch")

    def query_metrics(self, 
                     from_timestamp: Optional[int] = None,
                     to_timestamp: Optional[int] = None,
                     metric_ids: Optional[List[str]] = None) -> pd.DataFrame:
        """Query metrics with flexible filtering."""
        if not self.conn:
            raise ConnectionError("Not connected to server")

        conditions = []
        params = []

        if from_timestamp is not None:
            conditions.append("timestamp >= ?")
            params.append(from_timestamp)
        
        if to_timestamp is not None:
            conditions.append("timestamp <= ?")
            params.append(to_timestamp)

        if metric_ids:
            placeholders = ','.join(['?' for _ in metric_ids])
            conditions.append(f"metric_id IN ({placeholders})")
            params.extend(metric_ids)

        where_clause = " AND ".join(conditions) if conditions else "1=1"
        
        query = f"""
        SELECT * FROM metrics 
        WHERE {where_clause}
        ORDER BY timestamp ASC
        """
        
        cursor = self.conn.cursor()
        try:
            cursor.execute(query, params)
            results = cursor.fetch_arrow_table()
            
            if results.num_rows > 0:
                return results.to_pandas()
            return pd.DataFrame()
        finally:
            cursor.close()

    def get_metrics_window(self, window_seconds: int = 60) -> pd.DataFrame:
        """Get metrics within a time window from now."""
        current_time = int(time.time() * 1e9)
        from_timestamp = current_time - (window_seconds * 1_000_000_000)
        return self.query_metrics(from_timestamp=from_timestamp) 