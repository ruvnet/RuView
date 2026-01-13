"""
Full system integration tests for WiFi-DensePose API
Tests the complete integration of all components working together.
"""

import asyncio
import pytest
import httpx
import json
import time
from pathlib import Path
from typing import Dict, Any
from unittest.mock import AsyncMock, MagicMock, patch

from src.config.settings import get_settings
from src.app import app
from src.database.connection import get_database_manager
from src.services.orchestrator import get_service_orchestrator
from src.tasks.cleanup import get_cleanup_manager
from src.tasks.monitoring import get_monitoring_manager
from src.tasks.backup import get_backup_manager


class TestFullSystemIntegration:
    """Test complete system integration."""
    
    @pytest.fixture
    async def settings(self):
        """Get test settings."""
        settings = get_settings()
        settings.environment = "test"
        settings.debug = True
        settings.database_url = "sqlite+aiosqlite:///test_integration.db"
        settings.redis_enabled = False
        return settings
    
    @pytest.fixture
    async def db_manager(self, settings):
        """Get database manager for testing."""
        manager = get_database_manager(settings)
        await manager.initialize()
        yield manager
        await manager.close_all_connections()
    
    @pytest.fixture
    async def client(self, settings):
        """Get test HTTP client."""
        async with httpx.AsyncClient(app=app, base_url="http://test") as client:
            yield client
    
    @pytest.fixture
    async def orchestrator(self, settings, db_manager):
        """Get service orchestrator for testing."""
        orchestrator = get_service_orchestrator(settings)
        await orchestrator.initialize()
        yield orchestrator
        await orchestrator.shutdown()
    
    async def test_application_startup_and_shutdown(self, settings, db_manager):
        """Test complete application startup and shutdown sequence."""
        
        # Test database initialization
        await db_manager.test_connection()
        stats = await db_manager.get_connection_stats()
        assert stats["database"]["connected"] is True
        
        # Test service orchestrator initialization
        orchestrator = get_service_orchestrator(settings)
        await orchestrator.initialize()
        
        # Verify services are running
        health_status = await orchestrator.get_health_status()
        assert health_status["status"] in ["healthy", "warning"]
        
        # Test graceful shutdown
        await orchestrator.shutdown()
        
        # Verify cleanup
        final_stats = await db_manager.get_connection_stats()
        assert final_stats is not None
    
    async def test_api_endpoints_integration(self, client, settings, db_manager):
        """Test API endpoints work with database integration."""
        
        # Test health endpoint
        response = await client.get("/health")
        assert response.status_code == 200
        health_data = response.json()
        assert "status" in health_data
        assert "timestamp" in health_data
        
        # Test metrics endpoint
        response = await client.get("/metrics")
        assert response.status_code == 200
        
        # Test devices endpoint
        response = await client.get("/api/v1/devices")
        assert response.status_code == 200
        devices_data = response.json()
        assert "devices" in devices_data
        assert isinstance(devices_data["devices"], list)
        
        # Test sessions endpoint
        response = await client.get("/api/v1/sessions")
        assert response.status_code == 200
        sessions_data = response.json()
        assert "sessions" in sessions_data
        assert isinstance(sessions_data["sessions"], list)
    
    @patch('src.core.router_interface.RouterInterface')
    @patch('src.core.csi_processor.CSIProcessor')
    @patch('src.core.pose_estimator.PoseEstimator')
    async def test_data_processing_pipeline(
        self, 
        mock_pose_estimator,
        mock_csi_processor, 
        mock_router_interface,
        client, 
        settings, 
        db_manager
    ):
        """Test complete data processing pipeline integration."""
        
        # Setup mocks
        mock_router = MagicMock()
        mock_router_interface.return_value = mock_router
        mock_router.connect.return_value = True
        mock_router.start_capture.return_value = True
        mock_router.get_csi_data.return_value = {
            "timestamp": time.time(),
            "csi_matrix": [[1.0, 2.0], [3.0, 4.0]],
            "rssi": -45,
            "noise_floor": -90
        }
        
        mock_processor = MagicMock()
        mock_csi_processor.return_value = mock_processor
        mock_processor.process_csi_data.return_value = {
            "processed_csi": [[1.1, 2.1], [3.1, 4.1]],
            "quality_score": 0.85,
            "phase_sanitized": True
        }
        
        mock_estimator = MagicMock()
        mock_pose_estimator.return_value = mock_estimator
        mock_estimator.estimate_pose.return_value = {
            "pose_data": {
                "keypoints": [[100, 200], [150, 250]],
                "confidence": 0.9
            },
            "processing_time": 0.05
        }
        
        # Test device registration
        device_data = {
            "name": "test_router",
            "ip_address": "192.168.1.1",
            "device_type": "router",
            "model": "test_model"
        }
        
        response = await client.post("/api/v1/devices", json=device_data)
        assert response.status_code == 201
        device_response = response.json()
        device_id = device_response["device"]["id"]
        
        # Test session creation
        session_data = {
            "device_id": device_id,
            "session_type": "pose_detection",
            "configuration": {
                "sampling_rate": 1000,
                "duration": 60
            }
        }
        
        response = await client.post("/api/v1/sessions", json=session_data)
        assert response.status_code == 201
        session_response = response.json()
        session_id = session_response["session"]["id"]
        
        # Test CSI data submission
        csi_data = {
            "session_id": session_id,
            "timestamp": time.time(),
            "csi_matrix": [[1.0, 2.0], [3.0, 4.0]],
            "rssi": -45,
            "noise_floor": -90
        }
        
        response = await client.post("/api/v1/csi-data", json=csi_data)
        assert response.status_code == 201
        
        # Test pose detection retrieval
        response = await client.get(f"/api/v1/sessions/{session_id}/pose-detections")
        assert response.status_code == 200
        
        # Test session completion
        response = await client.patch(
            f"/api/v1/sessions/{session_id}",
            json={"status": "completed"}
        )
        assert response.status_code == 200
    
    async def test_background_tasks_integration(self, settings, db_manager):
        """Test background tasks integration."""
        
        # Test cleanup manager
        cleanup_manager = get_cleanup_manager(settings)
        cleanup_stats = cleanup_manager.get_stats()
        assert "manager" in cleanup_stats
        
        # Run cleanup task
        cleanup_result = await cleanup_manager.run_all_tasks()
        assert cleanup_result["success"] is True
        
        # Test monitoring manager
        monitoring_manager = get_monitoring_manager(settings)
        monitoring_stats = monitoring_manager.get_stats()
        assert "manager" in monitoring_stats
        
        # Run monitoring task
        monitoring_result = await monitoring_manager.run_all_tasks()
        assert monitoring_result["success"] is True
        
        # Test backup manager
        backup_manager = get_backup_manager(settings)
        backup_stats = backup_manager.get_stats()
        assert "manager" in backup_stats
        
        # Run backup task
        backup_result = await backup_manager.run_all_tasks()
        assert backup_result["success"] is True
    
    async def test_error_handling_integration(self, client, settings, db_manager):
        """Test error handling across the system."""
        
        # Test invalid device creation
        invalid_device_data = {
            "name": "",  # Invalid empty name
            "ip_address": "invalid_ip",
            "device_type": "unknown_type"
        }
        
        response = await client.post("/api/v1/devices", json=invalid_device_data)
        assert response.status_code == 422
        error_response = response.json()
        assert "detail" in error_response
        
        # Test non-existent resource access
        response = await client.get("/api/v1/devices/99999")
        assert response.status_code == 404
        
        # Test invalid session creation
        invalid_session_data = {
            "device_id": "invalid_uuid",
            "session_type": "invalid_type"
        }
        
        response = await client.post("/api/v1/sessions", json=invalid_session_data)
        assert response.status_code == 422
    
    async def test_authentication_and_authorization(self, client, settings):
        """Test authentication and authorization integration."""
        
        # Test protected endpoint without authentication
        response = await client.get("/api/v1/admin/system-info")
        assert response.status_code in [401, 403]
        
        # Test with invalid token
        headers = {"Authorization": "Bearer invalid_token"}
        response = await client.get("/api/v1/admin/system-info", headers=headers)
        assert response.status_code in [401, 403]
    
    async def test_rate_limiting_integration(self, client, settings):
        """Test rate limiting integration."""
        
        # Make multiple rapid requests to test rate limiting
        responses = []
        for i in range(10):
            response = await client.get("/health")
            responses.append(response.status_code)
        
        # Should have at least some successful responses
        assert 200 in responses
        
        # Rate limiting might kick in for some requests
        # This depends on the rate limiting configuration
    
    async def test_monitoring_and_metrics_integration(self, client, settings, db_manager):
        """Test monitoring and metrics collection integration."""
        
        # Test metrics endpoint
        response = await client.get("/metrics")
        assert response.status_code == 200
        metrics_text = response.text
        
        # Check for Prometheus format metrics
        assert "# HELP" in metrics_text
        assert "# TYPE" in metrics_text
        
        # Test health check with detailed information
        response = await client.get("/health?detailed=true")
        assert response.status_code == 200
        health_data = response.json()
        
        assert "database" in health_data
        assert "services" in health_data
        assert "system" in health_data
    
    async def test_configuration_management_integration(self, settings):
        """Test configuration management integration."""
        
        # Test settings validation
        assert settings.environment == "test"
        assert settings.debug is True
        
        # Test database URL configuration
        assert "test_integration.db" in settings.database_url
        
        # Test Redis configuration
        assert settings.redis_enabled is False
        
        # Test logging configuration
        assert settings.log_level in ["DEBUG", "INFO", "WARNING", "ERROR"]
    
    async def test_database_migration_integration(self, settings, db_manager):
        """Test database migration integration."""
        
        # Test database connection
        await db_manager.test_connection()
        
        # Test table creation
        async with db_manager.get_async_session() as session:
            from sqlalchemy import text
            
            # Check if tables exist
            tables_query = text("""
                SELECT name FROM sqlite_master 
                WHERE type='table' AND name NOT LIKE 'sqlite_%'
            """)
            
            result = await session.execute(tables_query)
            tables = [row[0] for row in result.fetchall()]
            
            # Should have our main tables
            expected_tables = ["devices", "sessions", "csi_data", "pose_detections"]
            for table in expected_tables:
                assert table in tables
    
    async def test_concurrent_operations_integration(self, client, settings, db_manager):
        """Test concurrent operations integration."""
        
        async def create_device(name: str):
            device_data = {
                "name": f"test_device_{name}",
                "ip_address": f"192.168.1.{name}",
                "device_type": "router",
                "model": "test_model"
            }
            response = await client.post("/api/v1/devices", json=device_data)
            return response.status_code
        
        # Create multiple devices concurrently
        tasks = [create_device(str(i)) for i in range(5)]
        results = await asyncio.gather(*tasks)
        
        # All should succeed
        assert all(status == 201 for status in results)
        
        # Verify all devices were created
        response = await client.get("/api/v1/devices")
        assert response.status_code == 200
        devices_data = response.json()
        assert len(devices_data["devices"]) >= 5
    
    async def test_system_resource_management(self, settings, db_manager, orchestrator):
        """Test system resource management integration."""
        
        # Test connection pool management
        stats = await db_manager.get_connection_stats()
        assert "database" in stats
        assert "pool_size" in stats["database"]
        
        # Test service resource usage
        health_status = await orchestrator.get_health_status()
        assert "memory_usage" in health_status
        assert "cpu_usage" in health_status
        
        # Test cleanup of resources
        await orchestrator.cleanup_resources()
        
        # Verify resources are cleaned up
        final_stats = await db_manager.get_connection_stats()
        assert final_stats is not None


@pytest.mark.integration
class TestSystemPerformance:
    """Test system performance under load."""
    
    async def test_api_response_times(self, client):
        """Test API response times under normal load."""
        
        start_time = time.time()
        response = await client.get("/health")
        end_time = time.time()
        
        assert response.status_code == 200
        assert (end_time - start_time) < 1.0  # Should respond within 1 second
    
    async def test_database_query_performance(self, db_manager):
        """Test database query performance."""
        
        async with db_manager.get_async_session() as session:
            from sqlalchemy import text
            
            start_time = time.time()
            result = await session.execute(text("SELECT 1"))
            end_time = time.time()
            
            assert result.scalar() == 1
            assert (end_time - start_time) < 0.1  # Should complete within 100ms
    
    async def test_memory_usage_stability(self, orchestrator):
        """Test memory usage remains stable."""
        
        import psutil
        import os
        
        process = psutil.Process(os.getpid())
        initial_memory = process.memory_info().rss
        
        # Perform some operations
        for _ in range(10):
            health_status = await orchestrator.get_health_status()
            assert health_status is not None
        
        final_memory = process.memory_info().rss
        memory_increase = final_memory - initial_memory
        
        # Memory increase should be reasonable (less than 50MB)
        assert memory_increase < 50 * 1024 * 1024


if __name__ == "__main__":
    pytest.main([__file__, "-v"])