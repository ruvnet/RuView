"""
SQLAlchemy models for WiFi-DensePose API
"""

import uuid
from datetime import datetime
from typing import Optional, Dict, Any, List
from enum import Enum

from sqlalchemy import (
    Column, String, Integer, Float, Boolean, DateTime, Text, JSON,
    ForeignKey, Index, UniqueConstraint, CheckConstraint
)
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import relationship, validates
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.sql import func

# Import custom array type for compatibility
from src.database.model_types import StringArray, FloatArray

Base = declarative_base()


class TimestampMixin:
    """Mixin for timestamp fields."""
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    updated_at = Column(DateTime(timezone=True), server_default=func.now(), onupdate=func.now(), nullable=False)


class UUIDMixin:
    """Mixin for UUID primary key."""
    id = Column(UUID(as_uuid=True), primary_key=True, default=uuid.uuid4, nullable=False)


class DeviceStatus(str, Enum):
    """Device status enumeration."""
    ACTIVE = "active"
    INACTIVE = "inactive"
    MAINTENANCE = "maintenance"
    ERROR = "error"


class SessionStatus(str, Enum):
    """Session status enumeration."""
    ACTIVE = "active"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class ProcessingStatus(str, Enum):
    """Processing status enumeration."""
    PENDING = "pending"
    PROCESSING = "processing"
    COMPLETED = "completed"
    FAILED = "failed"


class Device(Base, UUIDMixin, TimestampMixin):
    """Device model for WiFi routers and sensors."""
    __tablename__ = "devices"
    
    # Basic device information
    name = Column(String(255), nullable=False)
    device_type = Column(String(50), nullable=False)  # router, sensor, etc.
    mac_address = Column(String(17), unique=True, nullable=False)
    ip_address = Column(String(45), nullable=True)  # IPv4 or IPv6
    
    # Device status and configuration
    status = Column(String(20), default=DeviceStatus.INACTIVE, nullable=False)
    firmware_version = Column(String(50), nullable=True)
    hardware_version = Column(String(50), nullable=True)
    
    # Location information
    location_name = Column(String(255), nullable=True)
    room_id = Column(String(100), nullable=True)
    coordinates_x = Column(Float, nullable=True)
    coordinates_y = Column(Float, nullable=True)
    coordinates_z = Column(Float, nullable=True)
    
    # Configuration
    config = Column(JSON, nullable=True)
    capabilities = Column(StringArray, nullable=True)
    
    # Metadata
    description = Column(Text, nullable=True)
    tags = Column(StringArray, nullable=True)
    
    # Relationships
    sessions = relationship("Session", back_populates="device", cascade="all, delete-orphan")
    csi_data = relationship("CSIData", back_populates="device", cascade="all, delete-orphan")
    
    # Constraints and indexes
    __table_args__ = (
        Index("idx_device_mac_address", "mac_address"),
        Index("idx_device_status", "status"),
        Index("idx_device_type", "device_type"),
        CheckConstraint("status IN ('active', 'inactive', 'maintenance', 'error')", name="check_device_status"),
    )
    
    @validates('mac_address')
    def validate_mac_address(self, key, address):
        """Validate MAC address format."""
        if address and len(address) == 17:
            # Basic MAC address format validation
            parts = address.split(':')
            if len(parts) == 6 and all(len(part) == 2 for part in parts):
                return address.lower()
        raise ValueError("Invalid MAC address format")
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": str(self.id),
            "name": self.name,
            "device_type": self.device_type,
            "mac_address": self.mac_address,
            "ip_address": self.ip_address,
            "status": self.status,
            "firmware_version": self.firmware_version,
            "hardware_version": self.hardware_version,
            "location_name": self.location_name,
            "room_id": self.room_id,
            "coordinates": {
                "x": self.coordinates_x,
                "y": self.coordinates_y,
                "z": self.coordinates_z,
            } if any([self.coordinates_x, self.coordinates_y, self.coordinates_z]) else None,
            "config": self.config,
            "capabilities": self.capabilities,
            "description": self.description,
            "tags": self.tags,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }


class Session(Base, UUIDMixin, TimestampMixin):
    """Session model for tracking data collection sessions."""
    __tablename__ = "sessions"
    
    # Session identification
    name = Column(String(255), nullable=False)
    description = Column(Text, nullable=True)
    
    # Session timing
    started_at = Column(DateTime(timezone=True), nullable=True)
    ended_at = Column(DateTime(timezone=True), nullable=True)
    duration_seconds = Column(Integer, nullable=True)
    
    # Session status and configuration
    status = Column(String(20), default=SessionStatus.ACTIVE, nullable=False)
    config = Column(JSON, nullable=True)
    
    # Device relationship
    device_id = Column(UUID(as_uuid=True), ForeignKey("devices.id"), nullable=False)
    device = relationship("Device", back_populates="sessions")
    
    # Data relationships
    csi_data = relationship("CSIData", back_populates="session", cascade="all, delete-orphan")
    pose_detections = relationship("PoseDetection", back_populates="session", cascade="all, delete-orphan")
    
    # Metadata
    tags = Column(StringArray, nullable=True)
    meta_data = Column(JSON, nullable=True)
    
    # Statistics
    total_frames = Column(Integer, default=0, nullable=False)
    processed_frames = Column(Integer, default=0, nullable=False)
    error_count = Column(Integer, default=0, nullable=False)
    
    # Constraints and indexes
    __table_args__ = (
        Index("idx_session_device_id", "device_id"),
        Index("idx_session_status", "status"),
        Index("idx_session_started_at", "started_at"),
        CheckConstraint("status IN ('active', 'completed', 'failed', 'cancelled')", name="check_session_status"),
        CheckConstraint("total_frames >= 0", name="check_total_frames_positive"),
        CheckConstraint("processed_frames >= 0", name="check_processed_frames_positive"),
        CheckConstraint("error_count >= 0", name="check_error_count_positive"),
    )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": str(self.id),
            "name": self.name,
            "description": self.description,
            "started_at": self.started_at.isoformat() if self.started_at else None,
            "ended_at": self.ended_at.isoformat() if self.ended_at else None,
            "duration_seconds": self.duration_seconds,
            "status": self.status,
            "config": self.config,
            "device_id": str(self.device_id),
            "tags": self.tags,
            "metadata": self.meta_data,
            "total_frames": self.total_frames,
            "processed_frames": self.processed_frames,
            "error_count": self.error_count,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }


class CSIData(Base, UUIDMixin, TimestampMixin):
    """CSI (Channel State Information) data model."""
    __tablename__ = "csi_data"
    
    # Data identification
    sequence_number = Column(Integer, nullable=False)
    timestamp_ns = Column(Integer, nullable=False)  # Nanosecond timestamp
    
    # Device and session relationships
    device_id = Column(UUID(as_uuid=True), ForeignKey("devices.id"), nullable=False)
    session_id = Column(UUID(as_uuid=True), ForeignKey("sessions.id"), nullable=True)
    
    device = relationship("Device", back_populates="csi_data")
    session = relationship("Session", back_populates="csi_data")
    
    # CSI data
    amplitude = Column(FloatArray, nullable=False)
    phase = Column(FloatArray, nullable=False)
    frequency = Column(Float, nullable=False)  # MHz
    bandwidth = Column(Float, nullable=False)  # MHz
    
    # Signal characteristics
    rssi = Column(Float, nullable=True)  # dBm
    snr = Column(Float, nullable=True)   # dB
    noise_floor = Column(Float, nullable=True)  # dBm
    
    # Antenna information
    tx_antenna = Column(Integer, nullable=True)
    rx_antenna = Column(Integer, nullable=True)
    num_subcarriers = Column(Integer, nullable=False)
    
    # Processing status
    processing_status = Column(String(20), default=ProcessingStatus.PENDING, nullable=False)
    processed_at = Column(DateTime(timezone=True), nullable=True)
    
    # Quality metrics
    quality_score = Column(Float, nullable=True)
    is_valid = Column(Boolean, default=True, nullable=False)
    
    # Metadata
    meta_data = Column(JSON, nullable=True)
    
    # Constraints and indexes
    __table_args__ = (
        Index("idx_csi_device_id", "device_id"),
        Index("idx_csi_session_id", "session_id"),
        Index("idx_csi_timestamp", "timestamp_ns"),
        Index("idx_csi_sequence", "sequence_number"),
        Index("idx_csi_processing_status", "processing_status"),
        UniqueConstraint("device_id", "sequence_number", "timestamp_ns", name="uq_csi_device_seq_time"),
        CheckConstraint("frequency > 0", name="check_frequency_positive"),
        CheckConstraint("bandwidth > 0", name="check_bandwidth_positive"),
        CheckConstraint("num_subcarriers > 0", name="check_subcarriers_positive"),
        CheckConstraint("processing_status IN ('pending', 'processing', 'completed', 'failed')", name="check_processing_status"),
    )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": str(self.id),
            "sequence_number": self.sequence_number,
            "timestamp_ns": self.timestamp_ns,
            "device_id": str(self.device_id),
            "session_id": str(self.session_id) if self.session_id else None,
            "amplitude": self.amplitude,
            "phase": self.phase,
            "frequency": self.frequency,
            "bandwidth": self.bandwidth,
            "rssi": self.rssi,
            "snr": self.snr,
            "noise_floor": self.noise_floor,
            "tx_antenna": self.tx_antenna,
            "rx_antenna": self.rx_antenna,
            "num_subcarriers": self.num_subcarriers,
            "processing_status": self.processing_status,
            "processed_at": self.processed_at.isoformat() if self.processed_at else None,
            "quality_score": self.quality_score,
            "is_valid": self.is_valid,
            "metadata": self.meta_data,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }


class PoseDetection(Base, UUIDMixin, TimestampMixin):
    """Pose detection results model."""
    __tablename__ = "pose_detections"
    
    # Detection identification
    frame_number = Column(Integer, nullable=False)
    timestamp_ns = Column(Integer, nullable=False)
    
    # Session relationship
    session_id = Column(UUID(as_uuid=True), ForeignKey("sessions.id"), nullable=False)
    session = relationship("Session", back_populates="pose_detections")
    
    # Detection results
    person_count = Column(Integer, default=0, nullable=False)
    keypoints = Column(JSON, nullable=True)  # Array of person keypoints
    bounding_boxes = Column(JSON, nullable=True)  # Array of bounding boxes
    
    # Confidence scores
    detection_confidence = Column(Float, nullable=True)
    pose_confidence = Column(Float, nullable=True)
    overall_confidence = Column(Float, nullable=True)
    
    # Processing information
    processing_time_ms = Column(Float, nullable=True)
    model_version = Column(String(50), nullable=True)
    algorithm = Column(String(100), nullable=True)
    
    # Quality metrics
    image_quality = Column(Float, nullable=True)
    pose_quality = Column(Float, nullable=True)
    is_valid = Column(Boolean, default=True, nullable=False)
    
    # Metadata
    meta_data = Column(JSON, nullable=True)
    
    # Constraints and indexes
    __table_args__ = (
        Index("idx_pose_session_id", "session_id"),
        Index("idx_pose_timestamp", "timestamp_ns"),
        Index("idx_pose_frame", "frame_number"),
        Index("idx_pose_person_count", "person_count"),
        CheckConstraint("person_count >= 0", name="check_person_count_positive"),
        CheckConstraint("detection_confidence >= 0 AND detection_confidence <= 1", name="check_detection_confidence_range"),
        CheckConstraint("pose_confidence >= 0 AND pose_confidence <= 1", name="check_pose_confidence_range"),
        CheckConstraint("overall_confidence >= 0 AND overall_confidence <= 1", name="check_overall_confidence_range"),
    )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": str(self.id),
            "frame_number": self.frame_number,
            "timestamp_ns": self.timestamp_ns,
            "session_id": str(self.session_id),
            "person_count": self.person_count,
            "keypoints": self.keypoints,
            "bounding_boxes": self.bounding_boxes,
            "detection_confidence": self.detection_confidence,
            "pose_confidence": self.pose_confidence,
            "overall_confidence": self.overall_confidence,
            "processing_time_ms": self.processing_time_ms,
            "model_version": self.model_version,
            "algorithm": self.algorithm,
            "image_quality": self.image_quality,
            "pose_quality": self.pose_quality,
            "is_valid": self.is_valid,
            "metadata": self.meta_data,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }


class SystemMetric(Base, UUIDMixin, TimestampMixin):
    """System metrics model for monitoring."""
    __tablename__ = "system_metrics"
    
    # Metric identification
    metric_name = Column(String(255), nullable=False)
    metric_type = Column(String(50), nullable=False)  # counter, gauge, histogram
    
    # Metric value
    value = Column(Float, nullable=False)
    unit = Column(String(50), nullable=True)
    
    # Labels and tags
    labels = Column(JSON, nullable=True)
    tags = Column(StringArray, nullable=True)
    
    # Source information
    source = Column(String(255), nullable=True)
    component = Column(String(100), nullable=True)
    
    # Metadata
    description = Column(Text, nullable=True)
    meta_data = Column(JSON, nullable=True)
    
    # Constraints and indexes
    __table_args__ = (
        Index("idx_metric_name", "metric_name"),
        Index("idx_metric_type", "metric_type"),
        Index("idx_metric_created_at", "created_at"),
        Index("idx_metric_source", "source"),
        Index("idx_metric_component", "component"),
    )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": str(self.id),
            "metric_name": self.metric_name,
            "metric_type": self.metric_type,
            "value": self.value,
            "unit": self.unit,
            "labels": self.labels,
            "tags": self.tags,
            "source": self.source,
            "component": self.component,
            "description": self.description,
            "metadata": self.meta_data,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }


class AuditLog(Base, UUIDMixin, TimestampMixin):
    """Audit log model for tracking system events."""
    __tablename__ = "audit_logs"
    
    # Event information
    event_type = Column(String(100), nullable=False)
    event_name = Column(String(255), nullable=False)
    description = Column(Text, nullable=True)
    
    # User and session information
    user_id = Column(String(255), nullable=True)
    session_id = Column(String(255), nullable=True)
    ip_address = Column(String(45), nullable=True)
    user_agent = Column(Text, nullable=True)
    
    # Resource information
    resource_type = Column(String(100), nullable=True)
    resource_id = Column(String(255), nullable=True)
    
    # Event details
    before_state = Column(JSON, nullable=True)
    after_state = Column(JSON, nullable=True)
    changes = Column(JSON, nullable=True)
    
    # Result information
    success = Column(Boolean, nullable=False)
    error_message = Column(Text, nullable=True)
    
    # Metadata
    meta_data = Column(JSON, nullable=True)
    tags = Column(StringArray, nullable=True)
    
    # Constraints and indexes
    __table_args__ = (
        Index("idx_audit_event_type", "event_type"),
        Index("idx_audit_user_id", "user_id"),
        Index("idx_audit_resource", "resource_type", "resource_id"),
        Index("idx_audit_created_at", "created_at"),
        Index("idx_audit_success", "success"),
    )
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "id": str(self.id),
            "event_type": self.event_type,
            "event_name": self.event_name,
            "description": self.description,
            "user_id": self.user_id,
            "session_id": self.session_id,
            "ip_address": self.ip_address,
            "user_agent": self.user_agent,
            "resource_type": self.resource_type,
            "resource_id": self.resource_id,
            "before_state": self.before_state,
            "after_state": self.after_state,
            "changes": self.changes,
            "success": self.success,
            "error_message": self.error_message,
            "metadata": self.meta_data,
            "tags": self.tags,
            "created_at": self.created_at.isoformat() if self.created_at else None,
            "updated_at": self.updated_at.isoformat() if self.updated_at else None,
        }


# Model registry for easy access
MODEL_REGISTRY = {
    "Device": Device,
    "Session": Session,
    "CSIData": CSIData,
    "PoseDetection": PoseDetection,
    "SystemMetric": SystemMetric,
    "AuditLog": AuditLog,
}


def get_model_by_name(name: str):
    """Get model class by name."""
    return MODEL_REGISTRY.get(name)


def get_all_models() -> List:
    """Get all model classes."""
    return list(MODEL_REGISTRY.values())