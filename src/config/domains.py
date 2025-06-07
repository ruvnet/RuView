"""
Domain-specific configuration for WiFi-DensePose
"""

from typing import Dict, List, Optional, Any
from dataclasses import dataclass, field
from enum import Enum
from functools import lru_cache

from pydantic import BaseModel, Field, validator


class ZoneType(str, Enum):
    """Zone types for pose detection."""
    ROOM = "room"
    HALLWAY = "hallway"
    ENTRANCE = "entrance"
    OUTDOOR = "outdoor"
    OFFICE = "office"
    MEETING_ROOM = "meeting_room"
    KITCHEN = "kitchen"
    BATHROOM = "bathroom"
    BEDROOM = "bedroom"
    LIVING_ROOM = "living_room"


class ActivityType(str, Enum):
    """Activity types for pose classification."""
    STANDING = "standing"
    SITTING = "sitting"
    WALKING = "walking"
    LYING = "lying"
    RUNNING = "running"
    JUMPING = "jumping"
    FALLING = "falling"
    UNKNOWN = "unknown"


class HardwareType(str, Enum):
    """Hardware types for WiFi devices."""
    ROUTER = "router"
    ACCESS_POINT = "access_point"
    REPEATER = "repeater"
    MESH_NODE = "mesh_node"
    CUSTOM = "custom"


@dataclass
class ZoneConfig:
    """Configuration for a detection zone."""
    
    zone_id: str
    name: str
    zone_type: ZoneType
    description: Optional[str] = None
    
    # Physical boundaries (in meters)
    x_min: float = 0.0
    x_max: float = 10.0
    y_min: float = 0.0
    y_max: float = 10.0
    z_min: float = 0.0
    z_max: float = 3.0
    
    # Detection settings
    enabled: bool = True
    confidence_threshold: float = 0.5
    max_persons: int = 5
    activity_detection: bool = True
    
    # Hardware assignments
    primary_router: Optional[str] = None
    secondary_routers: List[str] = field(default_factory=list)
    
    # Processing settings
    processing_interval: float = 0.1  # seconds
    data_retention_hours: int = 24
    
    # Alert settings
    enable_alerts: bool = False
    alert_threshold: float = 0.8
    alert_activities: List[ActivityType] = field(default_factory=list)


@dataclass
class RouterConfig:
    """Configuration for a WiFi router/device."""
    
    router_id: str
    name: str
    hardware_type: HardwareType
    
    # Network settings
    ip_address: str
    mac_address: str
    interface: str = "wlan0"
    channel: int = 6
    frequency: float = 2.4  # GHz
    
    # CSI settings
    csi_enabled: bool = True
    csi_rate: int = 100  # Hz
    csi_subcarriers: int = 56
    antenna_count: int = 3
    
    # Position (in meters)
    x_position: float = 0.0
    y_position: float = 0.0
    z_position: float = 2.5  # typical ceiling mount
    
    # Calibration
    calibrated: bool = False
    calibration_data: Optional[Dict[str, Any]] = None
    
    # Status
    enabled: bool = True
    last_seen: Optional[str] = None
    
    # Performance settings
    max_connections: int = 50
    power_level: int = 20  # dBm
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return {
            "router_id": self.router_id,
            "name": self.name,
            "hardware_type": self.hardware_type.value,
            "ip_address": self.ip_address,
            "mac_address": self.mac_address,
            "interface": self.interface,
            "channel": self.channel,
            "frequency": self.frequency,
            "csi_enabled": self.csi_enabled,
            "csi_rate": self.csi_rate,
            "csi_subcarriers": self.csi_subcarriers,
            "antenna_count": self.antenna_count,
            "position": {
                "x": self.x_position,
                "y": self.y_position,
                "z": self.z_position
            },
            "calibrated": self.calibrated,
            "calibration_data": self.calibration_data,
            "enabled": self.enabled,
            "last_seen": self.last_seen,
            "max_connections": self.max_connections,
            "power_level": self.power_level
        }


class PoseModelConfig(BaseModel):
    """Configuration for pose estimation models."""
    
    model_name: str = Field(..., description="Model name")
    model_path: str = Field(..., description="Path to model file")
    model_type: str = Field(default="densepose", description="Model type")
    
    # Input settings
    input_width: int = Field(default=256, description="Input image width")
    input_height: int = Field(default=256, description="Input image height")
    input_channels: int = Field(default=3, description="Input channels")
    
    # Processing settings
    batch_size: int = Field(default=1, description="Batch size for inference")
    confidence_threshold: float = Field(default=0.5, description="Confidence threshold")
    nms_threshold: float = Field(default=0.4, description="NMS threshold")
    
    # Output settings
    max_detections: int = Field(default=10, description="Maximum detections per frame")
    keypoint_count: int = Field(default=17, description="Number of keypoints")
    
    # Performance settings
    use_gpu: bool = Field(default=True, description="Use GPU acceleration")
    gpu_memory_fraction: float = Field(default=0.5, description="GPU memory fraction")
    num_threads: int = Field(default=4, description="Number of CPU threads")
    
    @validator("confidence_threshold", "nms_threshold", "gpu_memory_fraction")
    def validate_thresholds(cls, v):
        """Validate threshold values."""
        if not 0.0 <= v <= 1.0:
            raise ValueError("Threshold must be between 0.0 and 1.0")
        return v


class StreamingConfig(BaseModel):
    """Configuration for real-time streaming."""
    
    # Stream settings
    fps: int = Field(default=30, description="Frames per second")
    resolution: str = Field(default="720p", description="Stream resolution")
    quality: str = Field(default="medium", description="Stream quality")
    
    # Buffer settings
    buffer_size: int = Field(default=100, description="Buffer size")
    max_latency_ms: int = Field(default=100, description="Maximum latency in milliseconds")
    
    # Compression settings
    compression_enabled: bool = Field(default=True, description="Enable compression")
    compression_level: int = Field(default=5, description="Compression level (1-9)")
    
    # WebSocket settings
    ping_interval: int = Field(default=60, description="Ping interval in seconds")
    timeout: int = Field(default=300, description="Connection timeout in seconds")
    max_connections: int = Field(default=100, description="Maximum concurrent connections")
    
    # Data filtering
    min_confidence: float = Field(default=0.5, description="Minimum confidence for streaming")
    include_metadata: bool = Field(default=True, description="Include metadata in stream")
    
    @validator("fps")
    def validate_fps(cls, v):
        """Validate FPS value."""
        if not 1 <= v <= 60:
            raise ValueError("FPS must be between 1 and 60")
        return v
    
    @validator("compression_level")
    def validate_compression_level(cls, v):
        """Validate compression level."""
        if not 1 <= v <= 9:
            raise ValueError("Compression level must be between 1 and 9")
        return v


class AlertConfig(BaseModel):
    """Configuration for alerts and notifications."""
    
    # Alert types
    enable_pose_alerts: bool = Field(default=False, description="Enable pose-based alerts")
    enable_activity_alerts: bool = Field(default=False, description="Enable activity-based alerts")
    enable_zone_alerts: bool = Field(default=False, description="Enable zone-based alerts")
    enable_system_alerts: bool = Field(default=True, description="Enable system alerts")
    
    # Thresholds
    confidence_threshold: float = Field(default=0.8, description="Alert confidence threshold")
    duration_threshold: int = Field(default=5, description="Alert duration threshold in seconds")
    
    # Activities that trigger alerts
    alert_activities: List[ActivityType] = Field(
        default=[ActivityType.FALLING],
        description="Activities that trigger alerts"
    )
    
    # Notification settings
    email_enabled: bool = Field(default=False, description="Enable email notifications")
    webhook_enabled: bool = Field(default=False, description="Enable webhook notifications")
    sms_enabled: bool = Field(default=False, description="Enable SMS notifications")
    
    # Rate limiting
    max_alerts_per_hour: int = Field(default=10, description="Maximum alerts per hour")
    cooldown_minutes: int = Field(default=5, description="Cooldown between similar alerts")


class DomainConfig:
    """Main domain configuration container."""
    
    def __init__(self):
        self.zones: Dict[str, ZoneConfig] = {}
        self.routers: Dict[str, RouterConfig] = {}
        self.pose_models: Dict[str, PoseModelConfig] = {}
        self.streaming = StreamingConfig()
        self.alerts = AlertConfig()
        
        # Load default configurations
        self._load_defaults()
    
    def _load_defaults(self):
        """Load default configurations."""
        # Default pose model
        self.pose_models["default"] = PoseModelConfig(
            model_name="densepose_rcnn_R_50_FPN_s1x",
            model_path="./models/densepose_rcnn_R_50_FPN_s1x.pkl",
            model_type="densepose"
        )
        
        # Example zone
        self.zones["living_room"] = ZoneConfig(
            zone_id="living_room",
            name="Living Room",
            zone_type=ZoneType.LIVING_ROOM,
            description="Main living area",
            x_max=5.0,
            y_max=4.0,
            z_max=3.0
        )
        
        # Example router
        self.routers["main_router"] = RouterConfig(
            router_id="main_router",
            name="Main Router",
            hardware_type=HardwareType.ROUTER,
            ip_address="192.168.1.1",
            mac_address="00:11:22:33:44:55",
            x_position=2.5,
            y_position=2.0,
            z_position=2.5
        )
    
    def add_zone(self, zone: ZoneConfig):
        """Add a zone configuration."""
        self.zones[zone.zone_id] = zone
    
    def add_router(self, router: RouterConfig):
        """Add a router configuration."""
        self.routers[router.router_id] = router
    
    def add_pose_model(self, model: PoseModelConfig):
        """Add a pose model configuration."""
        self.pose_models[model.model_name] = model
    
    def get_zone(self, zone_id: str) -> Optional[ZoneConfig]:
        """Get zone configuration by ID."""
        return self.zones.get(zone_id)
    
    def get_router(self, router_id: str) -> Optional[RouterConfig]:
        """Get router configuration by ID."""
        return self.routers.get(router_id)
    
    def get_pose_model(self, model_name: str) -> Optional[PoseModelConfig]:
        """Get pose model configuration by name."""
        return self.pose_models.get(model_name)
    
    def get_zones_for_router(self, router_id: str) -> List[ZoneConfig]:
        """Get zones that use a specific router."""
        zones = []
        for zone in self.zones.values():
            if (zone.primary_router == router_id or 
                router_id in zone.secondary_routers):
                zones.append(zone)
        return zones
    
    def get_routers_for_zone(self, zone_id: str) -> List[RouterConfig]:
        """Get routers assigned to a specific zone."""
        zone = self.get_zone(zone_id)
        if not zone:
            return []
        
        routers = []
        
        # Add primary router
        if zone.primary_router and zone.primary_router in self.routers:
            routers.append(self.routers[zone.primary_router])
        
        # Add secondary routers
        for router_id in zone.secondary_routers:
            if router_id in self.routers:
                routers.append(self.routers[router_id])
        
        return routers
    
    def get_all_routers(self) -> List[RouterConfig]:
        """Get all router configurations."""
        return list(self.routers.values())
    
    def validate_configuration(self) -> List[str]:
        """Validate the entire configuration."""
        issues = []
        
        # Validate zones
        for zone_id, zone in self.zones.items():
            if zone.primary_router and zone.primary_router not in self.routers:
                issues.append(f"Zone {zone_id} references unknown primary router: {zone.primary_router}")
            
            for router_id in zone.secondary_routers:
                if router_id not in self.routers:
                    issues.append(f"Zone {zone_id} references unknown secondary router: {router_id}")
        
        # Validate routers
        for router_id, router in self.routers.items():
            if not router.ip_address:
                issues.append(f"Router {router_id} missing IP address")
            
            if not router.mac_address:
                issues.append(f"Router {router_id} missing MAC address")
        
        # Validate pose models
        for model_name, model in self.pose_models.items():
            import os
            if not os.path.exists(model.model_path):
                issues.append(f"Pose model {model_name} file not found: {model.model_path}")
        
        return issues
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert configuration to dictionary."""
        return {
            "zones": {
                zone_id: {
                    "zone_id": zone.zone_id,
                    "name": zone.name,
                    "zone_type": zone.zone_type.value,
                    "description": zone.description,
                    "boundaries": {
                        "x_min": zone.x_min,
                        "x_max": zone.x_max,
                        "y_min": zone.y_min,
                        "y_max": zone.y_max,
                        "z_min": zone.z_min,
                        "z_max": zone.z_max
                    },
                    "settings": {
                        "enabled": zone.enabled,
                        "confidence_threshold": zone.confidence_threshold,
                        "max_persons": zone.max_persons,
                        "activity_detection": zone.activity_detection
                    },
                    "hardware": {
                        "primary_router": zone.primary_router,
                        "secondary_routers": zone.secondary_routers
                    }
                }
                for zone_id, zone in self.zones.items()
            },
            "routers": {
                router_id: router.to_dict()
                for router_id, router in self.routers.items()
            },
            "pose_models": {
                model_name: model.dict()
                for model_name, model in self.pose_models.items()
            },
            "streaming": self.streaming.dict(),
            "alerts": self.alerts.dict()
        }


@lru_cache()
def get_domain_config() -> DomainConfig:
    """Get cached domain configuration instance."""
    return DomainConfig()


def load_domain_config_from_file(file_path: str) -> DomainConfig:
    """Load domain configuration from file."""
    import json
    
    config = DomainConfig()
    
    try:
        with open(file_path, 'r') as f:
            data = json.load(f)
        
        # Load zones
        for zone_data in data.get("zones", []):
            zone = ZoneConfig(**zone_data)
            config.add_zone(zone)
        
        # Load routers
        for router_data in data.get("routers", []):
            router = RouterConfig(**router_data)
            config.add_router(router)
        
        # Load pose models
        for model_data in data.get("pose_models", []):
            model = PoseModelConfig(**model_data)
            config.add_pose_model(model)
        
        # Load streaming config
        if "streaming" in data:
            config.streaming = StreamingConfig(**data["streaming"])
        
        # Load alerts config
        if "alerts" in data:
            config.alerts = AlertConfig(**data["alerts"])
    
    except Exception as e:
        raise ValueError(f"Failed to load domain configuration: {e}")
    
    return config


def save_domain_config_to_file(config: DomainConfig, file_path: str):
    """Save domain configuration to file."""
    import json
    
    try:
        with open(file_path, 'w') as f:
            json.dump(config.to_dict(), f, indent=2)
    except Exception as e:
        raise ValueError(f"Failed to save domain configuration: {e}")