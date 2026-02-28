"""
Testing utilities for WiFi-DensePose.

This module contains mock data generators and testing helpers that are
ONLY intended for use in development/testing environments. These generators
produce synthetic data that mimics real CSI and pose data patterns.

WARNING: Code in this module uses random number generation intentionally
for mock/test data. Do NOT import from this module in production code paths
unless behind an explicit mock_mode flag with appropriate logging.
"""

from .mock_csi_generator import MockCSIGenerator
from .mock_pose_generator import generate_mock_poses, generate_mock_keypoints, generate_mock_bounding_box

__all__ = [
    "MockCSIGenerator",
    "generate_mock_poses",
    "generate_mock_keypoints",
    "generate_mock_bounding_box",
]
