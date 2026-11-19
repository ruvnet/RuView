"""ADR-117 P2 tests — Keypoint + KeypointType binding round-trips.

Run with: cd python && .venv/Scripts/python -m pytest tests/test_keypoint.py -v
"""

from __future__ import annotations

import pytest

from wifi_densepose import Keypoint, KeypointType

# ─── KeypointType ────────────────────────────────────────────────────


def test_keypoint_type_all_returns_17() -> None:
    """COCO standard defines exactly 17 keypoints."""
    assert len(KeypointType.all()) == 17


def test_keypoint_type_index_matches_coco_ordering() -> None:
    """Indexes 0..16 match the COCO canonical ordering."""
    expected = [
        (KeypointType.Nose, 0),
        (KeypointType.LeftEye, 1),
        (KeypointType.RightEye, 2),
        (KeypointType.LeftEar, 3),
        (KeypointType.RightEar, 4),
        (KeypointType.LeftShoulder, 5),
        (KeypointType.RightShoulder, 6),
        (KeypointType.LeftElbow, 7),
        (KeypointType.RightElbow, 8),
        (KeypointType.LeftWrist, 9),
        (KeypointType.RightWrist, 10),
        (KeypointType.LeftHip, 11),
        (KeypointType.RightHip, 12),
        (KeypointType.LeftKnee, 13),
        (KeypointType.RightKnee, 14),
        (KeypointType.LeftAnkle, 15),
        (KeypointType.RightAnkle, 16),
    ]
    for kp, idx in expected:
        assert kp.index == idx, f"{kp} expected index {idx} got {kp.index}"


def test_keypoint_type_snake_name() -> None:
    """snake_name follows COCO convention."""
    assert KeypointType.Nose.snake_name == "nose"
    assert KeypointType.LeftShoulder.snake_name == "left_shoulder"
    assert KeypointType.RightAnkle.snake_name == "right_ankle"


def test_keypoint_type_is_face() -> None:
    """is_face() matches the 5 facial keypoints."""
    face = {
        KeypointType.Nose,
        KeypointType.LeftEye,
        KeypointType.RightEye,
        KeypointType.LeftEar,
        KeypointType.RightEar,
    }
    for kp in KeypointType.all():
        assert kp.is_face() == (kp in face)


def test_keypoint_type_is_upper_body() -> None:
    """is_upper_body() catches shoulders, elbows, wrists."""
    assert KeypointType.LeftShoulder.is_upper_body()
    assert KeypointType.RightShoulder.is_upper_body()
    assert KeypointType.LeftElbow.is_upper_body()
    assert KeypointType.LeftWrist.is_upper_body()
    assert not KeypointType.LeftHip.is_upper_body()


def test_keypoint_type_eq() -> None:
    """Equality + identity work across calls."""
    assert KeypointType.Nose == KeypointType.Nose
    assert KeypointType.Nose != KeypointType.LeftEye


def test_keypoint_type_repr() -> None:
    """repr is a useful Python expression."""
    assert repr(KeypointType.Nose) == "KeypointType.Nose"
    assert repr(KeypointType.LeftWrist) == "KeypointType.LeftWrist"


# ─── Keypoint ────────────────────────────────────────────────────────


def test_keypoint_2d_construct() -> None:
    """Default 2D keypoint."""
    kp = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    assert kp.x == pytest.approx(0.5)
    assert kp.y == pytest.approx(0.3)
    assert kp.z is None
    assert kp.confidence == pytest.approx(0.95)
    assert kp.keypoint_type == KeypointType.Nose
    assert kp.is_visible


def test_keypoint_3d_construct() -> None:
    """3D keypoint with kwarg z."""
    kp = Keypoint(KeypointType.LeftWrist, 0.2, 0.4, 0.8, z=0.1)
    assert kp.position_3d == pytest.approx((0.2, 0.4, 0.1))
    assert kp.z == pytest.approx(0.1)


def test_keypoint_position_2d_tuple() -> None:
    kp = Keypoint(KeypointType.RightHip, 0.6, 0.7, 0.99)
    assert kp.position_2d == pytest.approx((0.6, 0.7))


def test_keypoint_position_3d_none_for_2d() -> None:
    """2D keypoints return None for position_3d, not a default z."""
    kp = Keypoint(KeypointType.Nose, 0.5, 0.5, 0.99)
    assert kp.position_3d is None


def test_keypoint_is_visible_below_threshold() -> None:
    """Confidence under 0.5 is NOT visible (default threshold)."""
    kp_low = Keypoint(KeypointType.Nose, 0.0, 0.0, 0.3)
    kp_high = Keypoint(KeypointType.Nose, 0.0, 0.0, 0.7)
    assert not kp_low.is_visible
    assert kp_high.is_visible


def test_keypoint_confidence_validation_too_high() -> None:
    """Confidence > 1.0 rejected."""
    with pytest.raises(ValueError, match="Confidence must be in"):
        Keypoint(KeypointType.Nose, 0.0, 0.0, 1.5)


def test_keypoint_confidence_validation_negative() -> None:
    """Negative confidence rejected."""
    with pytest.raises(ValueError, match="Confidence must be in"):
        Keypoint(KeypointType.Nose, 0.0, 0.0, -0.1)


def test_keypoint_distance_2d() -> None:
    """Euclidean distance in 2D."""
    a = Keypoint(KeypointType.Nose, 0.0, 0.0, 1.0)
    b = Keypoint(KeypointType.LeftEye, 3.0, 4.0, 1.0)
    assert a.distance_to(b) == pytest.approx(5.0)


def test_keypoint_distance_3d() -> None:
    """Euclidean distance in 3D when both have z."""
    a = Keypoint(KeypointType.Nose, 0.0, 0.0, 1.0, z=0.0)
    b = Keypoint(KeypointType.LeftEye, 1.0, 2.0, 1.0, z=2.0)
    # sqrt(1 + 4 + 4) = 3.0
    assert a.distance_to(b) == pytest.approx(3.0)


def test_keypoint_distance_falls_back_to_2d_if_mixed() -> None:
    """Mixing 2D and 3D keypoints uses 2D distance only."""
    a = Keypoint(KeypointType.Nose, 0.0, 0.0, 1.0)  # 2D
    b = Keypoint(KeypointType.LeftEye, 3.0, 4.0, 1.0, z=99.0)  # 3D
    # Should be 5.0 (2D distance), not include the z=99 term
    assert a.distance_to(b) == pytest.approx(5.0)


def test_keypoint_repr_2d() -> None:
    kp = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    r = repr(kp)
    assert "KeypointType.Nose" in r
    assert "x=0.5" in r
    assert "y=0.3" in r
    assert "z" not in r  # no z field for 2D


def test_keypoint_repr_3d() -> None:
    kp = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95, z=0.1)
    r = repr(kp)
    assert "z=0.1" in r


def test_keypoint_eq() -> None:
    """Two keypoints with same fields compare equal."""
    a = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    b = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    assert a == b


def test_keypoint_neq_different_type() -> None:
    a = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    b = Keypoint(KeypointType.LeftEye, 0.5, 0.3, 0.95)
    assert a != b


def test_keypoint_neq_different_position() -> None:
    a = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
    b = Keypoint(KeypointType.Nose, 0.6, 0.3, 0.95)
    assert a != b


def test_build_features_marks_p2() -> None:
    """The P2 marker is now in the wheel's feature list."""
    import wifi_densepose

    assert "p2-keypoint-bindings" in wifi_densepose.__build_features__
