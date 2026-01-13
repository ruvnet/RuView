"""
Centralized configuration management for WiFi-DensePose API
"""

import os
import logging
from pathlib import Path
from typing import Dict, Any, Optional, List
from functools import lru_cache

from src.config.settings import Settings, get_settings
from src.config.domains import DomainConfig, get_domain_config

logger = logging.getLogger(__name__)


class ConfigManager:
    """Centralized configuration manager."""
    
    def __init__(self):
        self._settings: Optional[Settings] = None
        self._domain_config: Optional[DomainConfig] = None
        self._environment_overrides: Dict[str, Any] = {}
    
    @property
    def settings(self) -> Settings:
        """Get application settings."""
        if self._settings is None:
            self._settings = get_settings()
        return self._settings
    
    @property
    def domain_config(self) -> DomainConfig:
        """Get domain configuration."""
        if self._domain_config is None:
            self._domain_config = get_domain_config()
        return self._domain_config
    
    def reload_settings(self) -> Settings:
        """Reload settings from environment."""
        self._settings = None
        return self.settings
    
    def reload_domain_config(self) -> DomainConfig:
        """Reload domain configuration."""
        self._domain_config = None
        return self.domain_config
    
    def set_environment_override(self, key: str, value: Any):
        """Set environment variable override."""
        self._environment_overrides[key] = value
        os.environ[key] = str(value)
    
    def get_environment_override(self, key: str, default: Any = None) -> Any:
        """Get environment variable override."""
        return self._environment_overrides.get(key, os.environ.get(key, default))
    
    def clear_environment_overrides(self):
        """Clear all environment overrides."""
        for key in self._environment_overrides:
            if key in os.environ:
                del os.environ[key]
        self._environment_overrides.clear()
    
    def get_database_config(self) -> Dict[str, Any]:
        """Get database configuration."""
        settings = self.settings
        
        config = {
            "url": settings.get_database_url(),
            "pool_size": settings.database_pool_size,
            "max_overflow": settings.database_max_overflow,
            "echo": settings.is_development and settings.debug,
            "pool_pre_ping": True,
            "pool_recycle": 3600,  # 1 hour
        }
        
        return config
    
    def get_redis_config(self) -> Optional[Dict[str, Any]]:
        """Get Redis configuration."""
        settings = self.settings
        redis_url = settings.get_redis_url()
        
        if not redis_url:
            return None
        
        config = {
            "url": redis_url,
            "password": settings.redis_password,
            "db": settings.redis_db,
            "decode_responses": True,
            "socket_connect_timeout": 5,
            "socket_timeout": 5,
            "retry_on_timeout": True,
            "health_check_interval": 30,
        }
        
        return config
    
    def get_logging_config(self) -> Dict[str, Any]:
        """Get logging configuration."""
        return self.settings.get_logging_config()
    
    def get_cors_config(self) -> Dict[str, Any]:
        """Get CORS configuration."""
        return self.settings.get_cors_config()
    
    def get_security_config(self) -> Dict[str, Any]:
        """Get security configuration."""
        settings = self.settings
        
        config = {
            "secret_key": settings.secret_key,
            "jwt_algorithm": settings.jwt_algorithm,
            "jwt_expire_hours": settings.jwt_expire_hours,
            "allowed_hosts": settings.allowed_hosts,
            "enable_authentication": settings.enable_authentication,
        }
        
        return config
    
    def get_hardware_config(self) -> Dict[str, Any]:
        """Get hardware configuration."""
        settings = self.settings
        domain_config = self.domain_config
        
        config = {
            "wifi_interface": settings.wifi_interface,
            "csi_buffer_size": settings.csi_buffer_size,
            "polling_interval": settings.hardware_polling_interval,
            "mock_hardware": settings.mock_hardware,
            "routers": [router.dict() for router in domain_config.routers],
        }
        
        return config
    
    def get_pose_config(self) -> Dict[str, Any]:
        """Get pose estimation configuration."""
        settings = self.settings
        domain_config = self.domain_config
        
        config = {
            "model_path": settings.pose_model_path,
            "confidence_threshold": settings.pose_confidence_threshold,
            "batch_size": settings.pose_processing_batch_size,
            "max_persons": settings.pose_max_persons,
            "mock_pose_data": settings.mock_pose_data,
            "models": [model.dict() for model in domain_config.pose_models],
        }
        
        return config
    
    def get_streaming_config(self) -> Dict[str, Any]:
        """Get streaming configuration."""
        settings = self.settings
        domain_config = self.domain_config
        
        config = {
            "fps": settings.stream_fps,
            "buffer_size": settings.stream_buffer_size,
            "websocket_ping_interval": settings.websocket_ping_interval,
            "websocket_timeout": settings.websocket_timeout,
            "enable_websockets": settings.enable_websockets,
            "enable_real_time_processing": settings.enable_real_time_processing,
            "max_connections": domain_config.streaming.max_connections,
            "compression": domain_config.streaming.compression,
        }
        
        return config
    
    def get_storage_config(self) -> Dict[str, Any]:
        """Get storage configuration."""
        settings = self.settings
        
        config = {
            "data_path": Path(settings.data_storage_path),
            "model_path": Path(settings.model_storage_path),
            "temp_path": Path(settings.temp_storage_path),
            "max_size_gb": settings.max_storage_size_gb,
            "enable_historical_data": settings.enable_historical_data,
        }
        
        # Ensure directories exist
        for path in [config["data_path"], config["model_path"], config["temp_path"]]:
            path.mkdir(parents=True, exist_ok=True)
        
        return config
    
    def get_monitoring_config(self) -> Dict[str, Any]:
        """Get monitoring configuration."""
        settings = self.settings
        
        config = {
            "metrics_enabled": settings.metrics_enabled,
            "health_check_interval": settings.health_check_interval,
            "performance_monitoring": settings.performance_monitoring,
            "log_level": settings.log_level,
            "log_file": settings.log_file,
        }
        
        return config
    
    def get_rate_limiting_config(self) -> Dict[str, Any]:
        """Get rate limiting configuration."""
        settings = self.settings
        
        config = {
            "enabled": settings.enable_rate_limiting,
            "requests": settings.rate_limit_requests,
            "authenticated_requests": settings.rate_limit_authenticated_requests,
            "window": settings.rate_limit_window,
        }
        
        return config
    
    def validate_configuration(self) -> List[str]:
        """Validate complete configuration and return issues."""
        issues = []
        
        try:
            # Validate settings
            from src.config.settings import validate_settings
            settings_issues = validate_settings(self.settings)
            issues.extend(settings_issues)
            
            # Validate database configuration
            try:
                db_config = self.get_database_config()
                if not db_config["url"]:
                    issues.append("Database URL is not configured")
            except Exception as e:
                issues.append(f"Database configuration error: {e}")
            
            # Validate storage paths
            try:
                storage_config = self.get_storage_config()
                for name, path in storage_config.items():
                    if name.endswith("_path") and not path.exists():
                        issues.append(f"Storage path does not exist: {path}")
            except Exception as e:
                issues.append(f"Storage configuration error: {e}")
            
            # Validate hardware configuration
            try:
                hw_config = self.get_hardware_config()
                if not hw_config["routers"]:
                    issues.append("No routers configured")
            except Exception as e:
                issues.append(f"Hardware configuration error: {e}")
            
            # Validate pose configuration
            try:
                pose_config = self.get_pose_config()
                if not pose_config["models"]:
                    issues.append("No pose models configured")
            except Exception as e:
                issues.append(f"Pose configuration error: {e}")
            
        except Exception as e:
            issues.append(f"Configuration validation error: {e}")
        
        return issues
    
    def get_full_config(self) -> Dict[str, Any]:
        """Get complete configuration dictionary."""
        return {
            "settings": self.settings.dict(),
            "domain_config": self.domain_config.to_dict(),
            "database": self.get_database_config(),
            "redis": self.get_redis_config(),
            "security": self.get_security_config(),
            "hardware": self.get_hardware_config(),
            "pose": self.get_pose_config(),
            "streaming": self.get_streaming_config(),
            "storage": self.get_storage_config(),
            "monitoring": self.get_monitoring_config(),
            "rate_limiting": self.get_rate_limiting_config(),
        }


# Global configuration manager instance
@lru_cache()
def get_config_manager() -> ConfigManager:
    """Get cached configuration manager instance."""
    return ConfigManager()


# Convenience functions
def get_app_settings() -> Settings:
    """Get application settings."""
    return get_config_manager().settings


def get_app_domain_config() -> DomainConfig:
    """Get domain configuration."""
    return get_config_manager().domain_config


def validate_app_configuration() -> List[str]:
    """Validate application configuration."""
    return get_config_manager().validate_configuration()


def reload_configuration():
    """Reload all configuration."""
    config_manager = get_config_manager()
    config_manager.reload_settings()
    config_manager.reload_domain_config()
    logger.info("Configuration reloaded")