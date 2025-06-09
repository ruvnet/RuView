"""
Pydantic settings for WiFi-DensePose API
"""

import os
from typing import List, Optional, Dict, Any
from functools import lru_cache

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application settings with environment variable support."""
    
    # Application settings
    app_name: str = Field(default="WiFi-DensePose API", description="Application name")
    version: str = Field(default="1.0.0", description="Application version")
    environment: str = Field(default="development", description="Environment (development, staging, production)")
    debug: bool = Field(default=False, description="Debug mode")
    
    # Server settings
    host: str = Field(default="0.0.0.0", description="Server host")
    port: int = Field(default=8000, description="Server port")
    reload: bool = Field(default=False, description="Auto-reload on code changes")
    workers: int = Field(default=1, description="Number of worker processes")
    
    # Security settings
    secret_key: str = Field(..., description="Secret key for JWT tokens")
    jwt_algorithm: str = Field(default="HS256", description="JWT algorithm")
    jwt_expire_hours: int = Field(default=24, description="JWT token expiration in hours")
    allowed_hosts: List[str] = Field(default=["*"], description="Allowed hosts")
    cors_origins: List[str] = Field(default=["*"], description="CORS allowed origins")
    
    # Rate limiting settings
    rate_limit_requests: int = Field(default=100, description="Rate limit requests per window")
    rate_limit_authenticated_requests: int = Field(default=1000, description="Rate limit for authenticated users")
    rate_limit_window: int = Field(default=3600, description="Rate limit window in seconds")
    
    # Database settings
    database_url: Optional[str] = Field(default=None, description="Database connection URL")
    database_pool_size: int = Field(default=10, description="Database connection pool size")
    database_max_overflow: int = Field(default=20, description="Database max overflow connections")
    
    # Database connection pool settings (alternative naming for compatibility)
    db_pool_size: int = Field(default=10, description="Database connection pool size")
    db_max_overflow: int = Field(default=20, description="Database max overflow connections")
    db_pool_timeout: int = Field(default=30, description="Database pool timeout in seconds")
    db_pool_recycle: int = Field(default=3600, description="Database pool recycle time in seconds")
    
    # Database connection settings
    db_host: Optional[str] = Field(default=None, description="Database host")
    db_port: int = Field(default=5432, description="Database port")
    db_name: Optional[str] = Field(default=None, description="Database name")
    db_user: Optional[str] = Field(default=None, description="Database user")
    db_password: Optional[str] = Field(default=None, description="Database password")
    db_echo: bool = Field(default=False, description="Enable database query logging")
    
    # Redis settings (for caching and rate limiting)
    redis_url: Optional[str] = Field(default=None, description="Redis connection URL")
    redis_password: Optional[str] = Field(default=None, description="Redis password")
    redis_db: int = Field(default=0, description="Redis database number")
    redis_enabled: bool = Field(default=True, description="Enable Redis")
    redis_host: str = Field(default="localhost", description="Redis host")
    redis_port: int = Field(default=6379, description="Redis port")
    redis_required: bool = Field(default=False, description="Require Redis connection (fail if unavailable)")
    redis_max_connections: int = Field(default=10, description="Maximum Redis connections")
    redis_socket_timeout: int = Field(default=5, description="Redis socket timeout in seconds")
    redis_connect_timeout: int = Field(default=5, description="Redis connection timeout in seconds")
    
    # Failsafe settings
    enable_database_failsafe: bool = Field(default=True, description="Enable automatic SQLite failsafe when PostgreSQL unavailable")
    enable_redis_failsafe: bool = Field(default=True, description="Enable automatic Redis failsafe (disable when unavailable)")
    sqlite_fallback_path: str = Field(default="./data/wifi_densepose_fallback.db", description="SQLite fallback database path")
    
    # Hardware settings
    wifi_interface: str = Field(default="wlan0", description="WiFi interface name")
    csi_buffer_size: int = Field(default=1000, description="CSI data buffer size")
    hardware_polling_interval: float = Field(default=0.1, description="Hardware polling interval in seconds")
    
    # CSI Processing settings
    csi_sampling_rate: int = Field(default=1000, description="CSI sampling rate")
    csi_window_size: int = Field(default=512, description="CSI window size")
    csi_overlap: float = Field(default=0.5, description="CSI window overlap")
    csi_noise_threshold: float = Field(default=0.1, description="CSI noise threshold")
    csi_human_detection_threshold: float = Field(default=0.8, description="CSI human detection threshold")
    csi_smoothing_factor: float = Field(default=0.9, description="CSI smoothing factor")
    csi_max_history_size: int = Field(default=500, description="CSI max history size")
    
    # Pose estimation settings
    pose_model_path: Optional[str] = Field(default=None, description="Path to pose estimation model")
    pose_confidence_threshold: float = Field(default=0.5, description="Minimum confidence threshold")
    pose_processing_batch_size: int = Field(default=32, description="Batch size for pose processing")
    pose_max_persons: int = Field(default=10, description="Maximum persons to detect per frame")
    
    # Streaming settings
    stream_fps: int = Field(default=30, description="Streaming frames per second")
    stream_buffer_size: int = Field(default=100, description="Stream buffer size")
    websocket_ping_interval: int = Field(default=60, description="WebSocket ping interval in seconds")
    websocket_timeout: int = Field(default=300, description="WebSocket timeout in seconds")
    
    # Logging settings
    log_level: str = Field(default="INFO", description="Logging level")
    log_format: str = Field(
        default="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        description="Log format"
    )
    log_file: Optional[str] = Field(default=None, description="Log file path")
    log_directory: str = Field(default="./logs", description="Log directory path")
    log_max_size: int = Field(default=10485760, description="Max log file size in bytes (10MB)")
    log_backup_count: int = Field(default=5, description="Number of log backup files")
    
    # Monitoring settings
    metrics_enabled: bool = Field(default=True, description="Enable metrics collection")
    health_check_interval: int = Field(default=30, description="Health check interval in seconds")
    performance_monitoring: bool = Field(default=True, description="Enable performance monitoring")
    monitoring_interval_seconds: int = Field(default=60, description="Monitoring task interval in seconds")
    cleanup_interval_seconds: int = Field(default=3600, description="Cleanup task interval in seconds")
    backup_interval_seconds: int = Field(default=86400, description="Backup task interval in seconds")
    
    # Storage settings
    data_storage_path: str = Field(default="./data", description="Data storage directory")
    model_storage_path: str = Field(default="./models", description="Model storage directory")
    temp_storage_path: str = Field(default="./temp", description="Temporary storage directory")
    backup_directory: str = Field(default="./backups", description="Backup storage directory")
    max_storage_size_gb: int = Field(default=100, description="Maximum storage size in GB")
    
    # API settings
    api_prefix: str = Field(default="/api/v1", description="API prefix")
    docs_url: str = Field(default="/docs", description="API documentation URL")
    redoc_url: str = Field(default="/redoc", description="ReDoc documentation URL")
    openapi_url: str = Field(default="/openapi.json", description="OpenAPI schema URL")
    
    # Feature flags
    enable_authentication: bool = Field(default=True, description="Enable authentication")
    enable_rate_limiting: bool = Field(default=True, description="Enable rate limiting")
    enable_websockets: bool = Field(default=True, description="Enable WebSocket support")
    enable_historical_data: bool = Field(default=True, description="Enable historical data storage")
    enable_real_time_processing: bool = Field(default=True, description="Enable real-time processing")
    cors_enabled: bool = Field(default=True, description="Enable CORS middleware")
    cors_allow_credentials: bool = Field(default=True, description="Allow credentials in CORS")
    
    # Development settings
    mock_hardware: bool = Field(default=False, description="Use mock hardware for development")
    mock_pose_data: bool = Field(default=False, description="Use mock pose data for development")
    enable_test_endpoints: bool = Field(default=False, description="Enable test endpoints")
    
    # Cleanup settings
    csi_data_retention_days: int = Field(default=30, description="CSI data retention in days")
    pose_detection_retention_days: int = Field(default=30, description="Pose detection retention in days")
    metrics_retention_days: int = Field(default=7, description="Metrics retention in days")
    audit_log_retention_days: int = Field(default=90, description="Audit log retention in days")
    orphaned_session_threshold_days: int = Field(default=7, description="Orphaned session threshold in days")
    cleanup_batch_size: int = Field(default=1000, description="Cleanup batch size")
    
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        case_sensitive=False
    )
    
    @field_validator("environment")
    @classmethod
    def validate_environment(cls, v):
        """Validate environment setting."""
        allowed_environments = ["development", "staging", "production"]
        if v not in allowed_environments:
            raise ValueError(f"Environment must be one of: {allowed_environments}")
        return v
    
    @field_validator("log_level")
    @classmethod
    def validate_log_level(cls, v):
        """Validate log level setting."""
        allowed_levels = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]
        if v.upper() not in allowed_levels:
            raise ValueError(f"Log level must be one of: {allowed_levels}")
        return v.upper()
    
    @field_validator("pose_confidence_threshold")
    @classmethod
    def validate_confidence_threshold(cls, v):
        """Validate confidence threshold."""
        if not 0.0 <= v <= 1.0:
            raise ValueError("Confidence threshold must be between 0.0 and 1.0")
        return v
    
    @field_validator("stream_fps")
    @classmethod
    def validate_stream_fps(cls, v):
        """Validate streaming FPS."""
        if not 1 <= v <= 60:
            raise ValueError("Stream FPS must be between 1 and 60")
        return v
    
    @field_validator("port")
    @classmethod
    def validate_port(cls, v):
        """Validate port number."""
        if not 1 <= v <= 65535:
            raise ValueError("Port must be between 1 and 65535")
        return v
    
    @field_validator("workers")
    @classmethod
    def validate_workers(cls, v):
        """Validate worker count."""
        if v < 1:
            raise ValueError("Workers must be at least 1")
        return v
    
    @field_validator("db_port")
    @classmethod
    def validate_db_port(cls, v):
        """Validate database port."""
        if not 1 <= v <= 65535:
            raise ValueError("Database port must be between 1 and 65535")
        return v
    
    @field_validator("redis_port")
    @classmethod
    def validate_redis_port(cls, v):
        """Validate Redis port."""
        if not 1 <= v <= 65535:
            raise ValueError("Redis port must be between 1 and 65535")
        return v
    
    @field_validator("db_pool_size")
    @classmethod
    def validate_db_pool_size(cls, v):
        """Validate database pool size."""
        if v < 1:
            raise ValueError("Database pool size must be at least 1")
        return v
    
    @field_validator("monitoring_interval_seconds", "cleanup_interval_seconds", "backup_interval_seconds")
    @classmethod
    def validate_interval_seconds(cls, v):
        """Validate interval settings."""
        if v < 0:
            raise ValueError("Interval seconds must be non-negative")
        return v
    @property
    def is_development(self) -> bool:
        """Check if running in development environment."""
        return self.environment == "development"
    
    @property
    def is_production(self) -> bool:
        """Check if running in production environment."""
        return self.environment == "production"
    
    @property
    def is_testing(self) -> bool:
        """Check if running in testing environment."""
        return self.environment == "testing"
    
    def get_database_url(self) -> str:
        """Get database URL with fallback."""
        if self.database_url:
            return self.database_url
        
        # Build URL from individual components if available
        if self.db_host and self.db_name and self.db_user:
            password_part = f":{self.db_password}" if self.db_password else ""
            return f"postgresql://{self.db_user}{password_part}@{self.db_host}:{self.db_port}/{self.db_name}"
        
        # Default SQLite database for development
        if self.is_development:
            return f"sqlite:///{self.data_storage_path}/wifi_densepose.db"
        
        # SQLite failsafe for production if enabled
        if self.enable_database_failsafe:
            return f"sqlite:///{self.sqlite_fallback_path}"
        
        raise ValueError("Database URL must be configured for non-development environments")
    
    def get_sqlite_fallback_url(self) -> str:
        """Get SQLite fallback database URL."""
        return f"sqlite:///{self.sqlite_fallback_path}"
    
    def get_redis_url(self) -> Optional[str]:
        """Get Redis URL with fallback."""
        if not self.redis_enabled:
            return None
            
        if self.redis_url:
            return self.redis_url
        
        # Build URL from individual components
        password_part = f":{self.redis_password}@" if self.redis_password else ""
        return f"redis://{password_part}{self.redis_host}:{self.redis_port}/{self.redis_db}"
    
    def get_cors_config(self) -> Dict[str, Any]:
        """Get CORS configuration."""
        if self.is_development:
            return {
                "allow_origins": ["*"],
                "allow_credentials": True,
                "allow_methods": ["*"],
                "allow_headers": ["*"],
            }
        
        return {
            "allow_origins": self.cors_origins,
            "allow_credentials": True,
            "allow_methods": ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
            "allow_headers": ["Authorization", "Content-Type"],
        }
    
    def get_logging_config(self) -> Dict[str, Any]:
        """Get logging configuration."""
        config = {
            "version": 1,
            "disable_existing_loggers": False,
            "formatters": {
                "default": {
                    "format": self.log_format,
                },
                "detailed": {
                    "format": "%(asctime)s - %(name)s - %(levelname)s - %(module)s:%(lineno)d - %(message)s",
                },
            },
            "handlers": {
                "console": {
                    "class": "logging.StreamHandler",
                    "level": self.log_level,
                    "formatter": "default",
                    "stream": "ext://sys.stdout",
                },
            },
            "loggers": {
                "": {
                    "level": self.log_level,
                    "handlers": ["console"],
                },
                "uvicorn": {
                    "level": "INFO",
                    "handlers": ["console"],
                    "propagate": False,
                },
                "fastapi": {
                    "level": "INFO",
                    "handlers": ["console"],
                    "propagate": False,
                },
            },
        }
        
        # Add file handler if log file is specified
        if self.log_file:
            config["handlers"]["file"] = {
                "class": "logging.handlers.RotatingFileHandler",
                "level": self.log_level,
                "formatter": "detailed",
                "filename": self.log_file,
                "maxBytes": self.log_max_size,
                "backupCount": self.log_backup_count,
            }
            
            # Add file handler to all loggers
            for logger_config in config["loggers"].values():
                logger_config["handlers"].append("file")
        
        return config
    
    def create_directories(self):
        """Create necessary directories."""
        directories = [
            self.data_storage_path,
            self.model_storage_path,
            self.temp_storage_path,
            self.log_directory,
            self.backup_directory,
        ]
        
        for directory in directories:
            os.makedirs(directory, exist_ok=True)


@lru_cache()
def get_settings() -> Settings:
    """Get cached settings instance."""
    settings = Settings()
    settings.create_directories()
    return settings


def get_test_settings() -> Settings:
    """Get settings for testing."""
    return Settings(
        environment="testing",
        debug=True,
        secret_key="test-secret-key",
        database_url="sqlite:///:memory:",
        mock_hardware=True,
        mock_pose_data=True,
        enable_test_endpoints=True,
        log_level="DEBUG"
    )


def load_settings_from_file(file_path: str) -> Settings:
    """Load settings from a specific file."""
    return Settings(_env_file=file_path)


def validate_settings(settings: Settings) -> List[str]:
    """Validate settings and return list of issues."""
    issues = []
    
    # Check required settings for production
    if settings.is_production:
        if not settings.secret_key or settings.secret_key == "change-me":
            issues.append("Secret key must be set for production")
        
        if not settings.database_url and not (settings.db_host and settings.db_name and settings.db_user):
            issues.append("Database URL or database connection parameters must be set for production")
        
        if settings.debug:
            issues.append("Debug mode should be disabled in production")
        
        if "*" in settings.allowed_hosts:
            issues.append("Allowed hosts should be restricted in production")
        
        if "*" in settings.cors_origins:
            issues.append("CORS origins should be restricted in production")
    
    # Check storage paths exist
    try:
        settings.create_directories()
    except Exception as e:
        issues.append(f"Cannot create storage directories: {e}")
    
    return issues