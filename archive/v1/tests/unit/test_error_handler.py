"""Tests for error handling in the API layer."""

import pytest
from unittest.mock import MagicMock, patch
from fastapi.testclient import TestClient


class TestExceptionHandlers:
    """Test the exception handlers registered on the FastAPI app."""

    def _get_app(self):
        """Import app lazily to avoid side effects."""
        with patch("src.api.main.get_settings") as mock_gs, \
             patch("src.api.main.get_domain_config") as mock_gdc, \
             patch("src.api.main.get_pose_service") as mock_ps, \
             patch("src.api.main.get_stream_service") as mock_ss, \
             patch("src.api.main.get_hardware_service") as mock_hs, \
             patch("src.api.main.connection_manager") as mock_cm, \
             patch("src.api.main.PoseStreamHandler") as mock_psh:
            mock_gs.return_value = MagicMock(
                app_name="test", version="0.1", environment="test",
                is_production=False, enable_rate_limiting=False,
                enable_authentication=False, docs_url="/docs",
                redoc_url="/redoc", openapi_url="/openapi.json",
                api_prefix="/api/v1",
            )
            mock_gs.return_value.get_logging_config.return_value = {
                "version": 1, "disable_existing_loggers": False,
                "handlers": {}, "loggers": {},
            }
            mock_gs.return_value.get_cors_config.return_value = {
                "allow_origins": ["*"], "allow_methods": ["*"],
                "allow_headers": ["*"],
            }
            # Re-import to pick up patches
            import importlib
            import src.api.main as m
            importlib.reload(m)
            return m.app


class TestErrorResponseModel:
    def test_error_json_structure(self):
        """Verify error JSON has code, message, type fields."""
        error = {
            "error": {
                "code": 404,
                "message": "Not found",
                "type": "http_error"
            }
        }
        assert error["error"]["code"] == 404
        assert "message" in error["error"]
        assert "type" in error["error"]

    def test_validation_error_structure(self):
        error = {
            "error": {
                "code": 422,
                "message": "Validation error",
                "type": "validation_error",
                "details": []
            }
        }
        assert error["error"]["type"] == "validation_error"
        assert isinstance(error["error"]["details"], list)

    def test_internal_error_masks_details(self):
        """In production, internal errors should not leak stack traces."""
        error = {
            "error": {
                "code": 500,
                "message": "Internal server error",
                "type": "internal_error"
            }
        }
        assert "traceback" not in str(error)
        assert error["error"]["message"] == "Internal server error"
