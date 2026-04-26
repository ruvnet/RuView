"""
Mock pose data generator for testing and development.

This module provides synthetic pose estimation data for use in development
and testing environments ONLY. The generated data mimics realistic human
pose detection outputs including keypoints, bounding boxes, and activities.

WARNING: This module uses random number generation intentionally for test data.
Do NOT use this module in production data paths.
"""

import random
import logging
from typing import Dict, List, Any, Optional
from datetime import datetime, timedelta

logger = logging.getLogger(__name__)

# Banner displayed when mock pose mode is active
MOCK_POSE_BANNER = """
================================================================================
  WARNING: MOCK POSE MODE ACTIVE - Using synthetic pose data

  All pose detections are randomly generated and do NOT represent real humans.
  For real pose estimation, provide trained model weights and real CSI data.
  See docs/hardware-setup.md for configuration instructions.
================================================================================
"""

_banner_shown = False


def _show_banner() -> None:
    """Display the mock pose mode warning banner (once per session)."""
    global _banner_shown
    if not _banner_shown:
        logger.warning(MOCK_POSE_BANNER)
        _banner_shown = True


def generate_mock_keypoints() -> List[Dict[str, Any]]:
    """Generate mock keypoints for a single person.

    Returns:
        List of 17 COCO-format keypoint dictionaries with name, x, y, confidence.
    """
    keypoint_names = [
        "nose", "left_eye", "right_eye", "left_ear", "right_ear",
        "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
        "left_wrist", "right_wrist", "left_hip", "right_hip",
        "left_knee", "right_knee", "left_ankle", "right_ankle",
    ]

    keypoints = []
    for name in keypoint_names:
        keypoints.append({
            "name": name,
            "x": random.uniform(0.1, 0.9),
            "y": random.uniform(0.1, 0.9),
            "confidence": random.uniform(0.5, 0.95),
        })

    return keypoints


def generate_mock_bounding_box() -> Dict[str, float]:
    """Generate a mock bounding box for a single person.

    Returns:
        Dictionary with x, y, width, height as normalized coordinates.
    """
    x = random.uniform(0.1, 0.6)
    y = random.uniform(0.1, 0.6)
    width = random.uniform(0.2, 0.4)
    height = random.uniform(0.3, 0.5)

    return {"x": x, "y": y, "width": width, "height": height}


def generate_mock_poses(max_persons: int = 3) -> List[Dict[str, Any]]:
    """Generate mock pose detections for testing.

    Args:
        max_persons: Maximum number of persons to generate (1 to max_persons).

    Returns:
        List of pose detection dictionaries.
    """
    _show_banner()

    num_persons = random.randint(1, min(3, max_persons))
    poses = []

    for i in range(num_persons):
        confidence = random.uniform(0.3, 0.95)

        pose = {
            "person_id": i,
            "confidence": confidence,
            "keypoints": generate_mock_keypoints(),
            "bounding_box": generate_mock_bounding_box(),
            "activity": random.choice(["standing", "sitting", "walking", "lying"]),
            "timestamp": datetime.now().isoformat(),
        }

        poses.append(pose)

    return poses


def generate_mock_zone_occupancy(zone_id: str) -> Dict[str, Any]:
    """Generate mock zone occupancy data.

    Args:
        zone_id: Zone identifier.

    Returns:
        Dictionary with occupancy count and person details.
    """
    _show_banner()

    count = random.randint(0, 5)
    persons = []

    for i in range(count):
        persons.append({
            "person_id": f"person_{i}",
            "confidence": random.uniform(0.7, 0.95),
            "activity": random.choice(["standing", "sitting", "walking"]),
        })

    return {
        "count": count,
        "max_occupancy": 10,
        "persons": persons,
        "timestamp": datetime.now(),
    }


def generate_mock_zones_summary(
    zone_ids: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """Generate mock zones summary data.

    Args:
        zone_ids: List of zone identifiers. Defaults to zone_1 through zone_4.

    Returns:
        Dictionary with per-zone occupancy and aggregate counts.
    """
    _show_banner()

    zones = zone_ids or ["zone_1", "zone_2", "zone_3", "zone_4"]
    zone_data = {}
    total_persons = 0
    active_zones = 0

    for zone_id in zones:
        count = random.randint(0, 3)
        zone_data[zone_id] = {
            "occupancy": count,
            "max_occupancy": 10,
            "status": "active" if count > 0 else "inactive",
        }
        total_persons += count
        if count > 0:
            active_zones += 1

    return {
        "total_persons": total_persons,
        "zones": zone_data,
        "active_zones": active_zones,
    }


def generate_mock_historical_data(
    start_time: datetime,
    end_time: datetime,
    zone_ids: Optional[List[str]] = None,
    aggregation_interval: int = 300,
    include_raw_data: bool = False,
) -> Dict[str, Any]:
    """Generate mock historical pose data.

    Args:
        start_time: Start of the time range.
        end_time: End of the time range.
        zone_ids: Zones to include. Defaults to zone_1, zone_2, zone_3.
        aggregation_interval: Seconds between data points.
        include_raw_data: Whether to include simulated raw detections.

    Returns:
        Dictionary with aggregated_data, optional raw_data, and total_records.
    """
    _show_banner()

    zones = zone_ids or ["zone_1", "zone_2", "zone_3"]
    current_time = start_time
    aggregated_data = []
    raw_data = [] if include_raw_data else None

    while current_time < end_time:
        data_point = {
            "timestamp": current_time,
            "total_persons": random.randint(0, 8),
            "zones": {},
        }

        for zone_id in zones:
            data_point["zones"][zone_id] = {
                "occupancy": random.randint(0, 3),
                "avg_confidence": random.uniform(0.7, 0.95),
            }

        aggregated_data.append(data_point)

        if include_raw_data:
            for _ in range(random.randint(0, 5)):
                raw_data.append({
                    "timestamp": current_time + timedelta(seconds=random.randint(0, aggregation_interval)),
                    "person_id": f"person_{random.randint(1, 10)}",
                    "zone_id": random.choice(zones),
                    "confidence": random.uniform(0.5, 0.95),
                    "activity": random.choice(["standing", "sitting", "walking"]),
                })

        current_time += timedelta(seconds=aggregation_interval)

    return {
        "aggregated_data": aggregated_data,
        "raw_data": raw_data,
        "total_records": len(aggregated_data),
    }


def generate_mock_recent_activities(
    zone_id: Optional[str] = None,
    limit: int = 10,
) -> List[Dict[str, Any]]:
    """Generate mock recent activity data.

    Args:
        zone_id: Optional zone filter. If None, random zones are used.
        limit: Number of activities to generate.

    Returns:
        List of activity dictionaries.
    """
    _show_banner()

    activities = []

    for i in range(limit):
        activity = {
            "activity_id": f"activity_{i}",
            "person_id": f"person_{random.randint(1, 5)}",
            "zone_id": zone_id or random.choice(["zone_1", "zone_2", "zone_3"]),
            "activity": random.choice(["standing", "sitting", "walking", "lying"]),
            "confidence": random.uniform(0.6, 0.95),
            "timestamp": datetime.now() - timedelta(minutes=random.randint(0, 60)),
            "duration_seconds": random.randint(10, 300),
        }
        activities.append(activity)

    return activities


def generate_mock_statistics(
    start_time: datetime,
    end_time: datetime,
) -> Dict[str, Any]:
    """Generate mock pose estimation statistics.

    Args:
        start_time: Start of the statistics period.
        end_time: End of the statistics period.

    Returns:
        Dictionary with detection counts, rates, and distributions.
    """
    _show_banner()

    total_detections = random.randint(100, 1000)
    successful_detections = int(total_detections * random.uniform(0.8, 0.95))

    return {
        "total_detections": total_detections,
        "successful_detections": successful_detections,
        "failed_detections": total_detections - successful_detections,
        "success_rate": successful_detections / total_detections,
        "average_confidence": random.uniform(0.75, 0.90),
        "average_processing_time_ms": random.uniform(50, 200),
        "unique_persons": random.randint(5, 20),
        "most_active_zone": random.choice(["zone_1", "zone_2", "zone_3"]),
        "activity_distribution": {
            "standing": random.uniform(0.3, 0.5),
            "sitting": random.uniform(0.2, 0.4),
            "walking": random.uniform(0.1, 0.3),
            "lying": random.uniform(0.0, 0.1),
        },
    }
