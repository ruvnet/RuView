"""
Regression tests for V-003: DELETE /clients/{client_id} authorization.

Guards that non-admin users are rejected (403) and unauthenticated requests
are rejected (401), while admin users are allowed through.
"""

import sys
from unittest.mock import MagicMock, AsyncMock

# Stub all heavy/unavailable transitive deps before any project import.
# This test environment is missing jose, psutil, and ML libs; stub them so
# the FastAPI router can be imported without installing production deps.
for _mod in ("psutil", "torch", "onnxruntime", "cv2", "jose"):
    sys.modules.setdefault(_mod, MagicMock())
sys.modules.setdefault("jose.jwt", MagicMock())
sys.modules.setdefault("jose.exceptions", MagicMock())

_fake_svc = MagicMock()
for _mod in (
    "src.services",
    "src.services.pose_service",
    "src.services.stream_service",
    "src.services.hardware_service",
    "src.services.orchestrator",
    "src.services.health_check",
    "src.services.metrics",
):
    sys.modules.setdefault(_mod, _fake_svc)

import pytest
from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient

from src.api.routers.stream import router
from src.api.dependencies import get_admin_user

ADMIN_USER = {"id": "admin-1", "username": "admin", "is_admin": True}


def _make_app():
    application = FastAPI()
    application.include_router(router, prefix="/stream")
    return application


class TestDisconnectClientAuthorization:
    def test_unauthenticated_returns_401(self):
        """No credentials → 401."""
        app = _make_app()
        client = TestClient(app, raise_server_exceptions=False)
        response = client.delete("/stream/clients/some-client-id")
        assert response.status_code == 401

    def test_non_admin_returns_403(self):
        """Authenticated but non-admin → 403."""
        app = _make_app()

        def non_admin():
            raise HTTPException(status_code=403, detail="Admin privileges required")

        app.dependency_overrides[get_admin_user] = non_admin
        client = TestClient(app, raise_server_exceptions=False)
        response = client.delete("/stream/clients/some-client-id")
        assert response.status_code == 403

    def test_admin_not_rejected_on_missing_client(self):
        """Admin with a nonexistent client_id → 404, not 401 or 403."""
        from unittest.mock import patch

        app = _make_app()
        app.dependency_overrides[get_admin_user] = lambda: ADMIN_USER
        with patch(
            "src.api.routers.stream.connection_manager.disconnect",
            new_callable=AsyncMock,
            return_value=False,
        ):
            client = TestClient(app, raise_server_exceptions=False)
            response = client.delete("/stream/clients/nonexistent-id")
        assert response.status_code not in (401, 403)

    def test_admin_can_disconnect_existing_client(self):
        """Admin with an existing client_id → 200."""
        from unittest.mock import patch

        app = _make_app()
        app.dependency_overrides[get_admin_user] = lambda: ADMIN_USER
        with patch(
            "src.api.routers.stream.connection_manager.disconnect",
            new_callable=AsyncMock,
            return_value=True,
        ):
            client = TestClient(app, raise_server_exceptions=False)
            response = client.delete("/stream/clients/real-client-id")
        assert response.status_code == 200
