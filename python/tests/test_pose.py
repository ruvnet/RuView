"""ADR-117 P2 tests — BoundingBox + PersonPose + PoseEstimate bindings.

Run with: cd python && .venv/Scripts/python -m pytest tests/test_pose.py -v
"""

from __future__ import annotations

import pytest

from wifi_densepose import (
    BoundingBox,
    Keypoint,
    KeypointType,
    PersonPose,
    PoseEstimate,
)

# ─── BoundingBox ─────────────────────────────────────────────────────


def test_bounding_box_construct() -> None:
    bb = BoundingBox(0.1, 0.2, 0.5, 0.7)
    assert bb.x_min == pytest.approx(0.1)
    assert bb.y_min == pytest.approx(0.2)
    assert bb.x_max == pytest.approx(0.5)
    assert bb.y_max == pytest.approx(0.7)


def test_bounding_box_dimensions() -> None:
    bb = BoundingBox(0.0, 0.0, 4.0, 3.0)
    assert bb.width == pytest.approx(4.0)
    assert bb.height == pytest.approx(3.0)
    assert bb.area == pytest.approx(12.0)
    assert bb.center == pytest.approx((2.0, 1.5))


def test_bounding_box_from_center() -> None:
    bb = BoundingBox.from_center(2.0, 3.0, 4.0, 6.0)
    assert bb.x_min == pytest.approx(0.0)
    assert bb.y_min == pytest.approx(0.0)
    assert bb.x_max == pytest.approx(4.0)
    assert bb.y_max == pytest.approx(6.0)


def test_bounding_box_iou_no_overlap() -> None:
    a = BoundingBox(0.0, 0.0, 1.0, 1.0)
    b = BoundingBox(2.0, 2.0, 3.0, 3.0)
    assert a.iou(b) == pytest.approx(0.0)


def test_bounding_box_iou_full_overlap() -> None:
    a = BoundingBox(0.0, 0.0, 1.0, 1.0)
    b = BoundingBox(0.0, 0.0, 1.0, 1.0)
    assert a.iou(b) == pytest.approx(1.0)


def test_bounding_box_iou_partial() -> None:
    a = BoundingBox(0.0, 0.0, 10.0, 10.0)
    b = BoundingBox(5.0, 5.0, 15.0, 15.0)
    # intersection 25, union 175 → 1/7
    assert a.iou(b) == pytest.approx(25.0 / 175.0)


def test_bounding_box_eq() -> None:
    assert BoundingBox(1, 2, 3, 4) == BoundingBox(1, 2, 3, 4)
    assert BoundingBox(1, 2, 3, 4) != BoundingBox(1, 2, 3, 5)


def test_bounding_box_repr() -> None:
    bb = BoundingBox(0.1, 0.2, 0.5, 0.7)
    assert "BoundingBox" in repr(bb)
    assert "x_min=0.1" in repr(bb)


# ─── PersonPose ──────────────────────────────────────────────────────


def test_person_pose_empty() -> None:
    p = PersonPose()
    assert p.id is None
    assert p.visible_keypoint_count == 0
    assert p.bounding_box is None
    assert p.confidence == 0.0


def test_person_pose_set_get_keypoint() -> None:
    p = PersonPose()
    kp = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    p.set_keypoint(kp)
    got = p.get_keypoint(KeypointType.Nose)
    assert got is not None
    assert got.x == pytest.approx(0.5)
    assert got.confidence == pytest.approx(0.95)


def test_person_pose_get_missing_returns_none() -> None:
    p = PersonPose()
    p.set_keypoint(Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95))
    assert p.get_keypoint(KeypointType.LeftWrist) is None


def test_person_pose_visible_count() -> None:
    p = PersonPose()
    p.set_keypoint(Keypoint(KeypointType.Nose, 0.0, 0.0, 0.9))  # visible
    p.set_keypoint(Keypoint(KeypointType.LeftEar, 0.0, 0.0, 0.2))  # invisible
    p.set_keypoint(Keypoint(KeypointType.RightEar, 0.0, 0.0, 0.8))  # visible
    assert p.visible_keypoint_count == 2


def test_person_pose_visible_keypoints_list() -> None:
    p = PersonPose()
    p.set_keypoint(Keypoint(KeypointType.Nose, 0.0, 0.0, 0.9))
    p.set_keypoint(Keypoint(KeypointType.LeftEar, 0.0, 0.0, 0.2))
    vis = p.visible_keypoints()
    assert len(vis) == 1
    assert vis[0].keypoint_type == KeypointType.Nose


def test_person_pose_keypoints_dict_excludes_missing() -> None:
    p = PersonPose()
    p.set_keypoint(Keypoint(KeypointType.Nose, 0.0, 0.0, 0.9))
    p.set_keypoint(Keypoint(KeypointType.LeftWrist, 0.5, 0.5, 0.6))
    d = p.keypoints()
    assert KeypointType.Nose in d
    assert KeypointType.LeftWrist in d
    assert KeypointType.RightAnkle not in d
    assert len(d) == 2


def test_person_pose_set_id() -> None:
    p = PersonPose()
    p.set_id(7)
    assert p.id == 7


def test_person_pose_set_bounding_box() -> None:
    p = PersonPose()
    bb = BoundingBox(0.1, 0.1, 0.5, 0.9)
    p.set_bounding_box(bb)
    assert p.bounding_box == bb


def test_person_pose_compute_bbox_returns_none_when_empty() -> None:
    p = PersonPose()
    assert p.compute_bounding_box() is None


def test_person_pose_compute_bbox_from_keypoints() -> None:
    p = PersonPose()
    p.set_keypoint(Keypoint(KeypointType.Nose, 0.0, 0.0, 0.95))
    p.set_keypoint(Keypoint(KeypointType.RightAnkle, 1.0, 2.0, 0.95))
    bb = p.compute_bounding_box()
    assert bb is not None
    # bbox should span both keypoints
    assert bb.x_min <= 0.0
    assert bb.y_min <= 0.0
    assert bb.x_max >= 1.0
    assert bb.y_max >= 2.0
    # also stored
    assert p.bounding_box is not None


def test_person_pose_set_confidence_validation() -> None:
    p = PersonPose()
    p.set_confidence(0.85)
    assert p.confidence == pytest.approx(0.85)
    with pytest.raises(ValueError):
        p.set_confidence(1.5)


def test_person_pose_repr() -> None:
    p = PersonPose()
    p.set_id(3)
    p.set_keypoint(Keypoint(KeypointType.Nose, 0.0, 0.0, 0.9))
    r = repr(p)
    assert "PersonPose" in r
    assert "id=Some(3)" in r or "id=3" in r


# ─── PoseEstimate ────────────────────────────────────────────────────


def test_pose_estimate_construct_empty() -> None:
    e = PoseEstimate([], 0.5, 1.0, "test-v0")
    assert e.person_count == 0
    assert not e.has_detections
    assert e.confidence == pytest.approx(0.5)
    assert e.latency_ms == pytest.approx(1.0)
    assert e.model_version == "test-v0"


def test_pose_estimate_construct_with_persons() -> None:
    p1 = PersonPose()
    p1.set_id(1)
    p1.set_confidence(0.8)
    p2 = PersonPose()
    p2.set_id(2)
    p2.set_confidence(0.9)
    e = PoseEstimate([p1, p2], 0.85, 5.2, "v0.7.0")
    assert e.person_count == 2
    assert e.has_detections
    assert e.confidence == pytest.approx(0.85)


def test_pose_estimate_highest_confidence_person() -> None:
    p1 = PersonPose()
    p1.set_confidence(0.5)
    p2 = PersonPose()
    p2.set_confidence(0.95)
    p3 = PersonPose()
    p3.set_confidence(0.7)
    e = PoseEstimate([p1, p2, p3], 0.85, 5.2, "v0.7.0")
    best = e.highest_confidence_person()
    assert best is not None
    assert best.confidence == pytest.approx(0.95)


def test_pose_estimate_highest_confidence_returns_none_when_empty() -> None:
    e = PoseEstimate([], 0.5, 1.0, "test")
    assert e.highest_confidence_person() is None


def test_pose_estimate_metadata_strings_nonempty() -> None:
    e = PoseEstimate([], 0.5, 1.0, "test")
    assert isinstance(e.id, str)
    assert isinstance(e.timestamp, str)
    assert e.id  # non-empty
    assert e.timestamp  # non-empty


def test_pose_estimate_confidence_validation() -> None:
    with pytest.raises(ValueError):
        PoseEstimate([], 1.5, 0.0, "test")


def test_pose_estimate_repr_contains_counts() -> None:
    e = PoseEstimate([], 0.5, 2.3, "v0.7.0")
    r = repr(e)
    assert "PoseEstimate" in r
    assert "v0.7.0" in r


def test_build_features_marks_p2_complete() -> None:
    import wifi_densepose

    assert "p2-keypoint-bindings" in wifi_densepose.__build_features__
    assert "p2-pose-bindings" in wifi_densepose.__build_features__
