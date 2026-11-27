"""
Regression test for V-003 — DELETE /clients/{client_id} must be admin-only.

Background
----------
`archive/v1/src/api/routers/stream.py::disconnect_client` previously depended on
`require_auth`, so ANY authenticated user could disconnect ANY WebSocket client by
id — there is no per-connection owner recorded (WS connections are anonymous; see
`WebSocketConnection.get_info`), so ownership cannot be checked. The fix restricts
the endpoint to admins by depending on `get_admin_user`.

These tests assert the *authorization* property (admin vs non-admin), which is the
property V-003 is about — unlike an authentication-only test, which would pass even
against the broken behaviour.

Injection point: both `require_auth` and `get_admin_user` resolve through
`get_current_active_user`, so overriding that single dependency injects the desired
identity without real JWT plumbing.

Note: designed for the v1 test harness (needs fastapi + the `src` package importable).
"""

import pytest
from unittest.mock import AsyncMock, patch

from fastapi import FastAPI
from fastapi.testclient import TestClient

from src.api.routers.stream import router
from src.api.dependencies import get_current_active_user


ADMIN = {"id": "admin-1", "username": "admin", "is_admin": True, "is_active": True}
USER = {"id": "user-1", "username": "alice", "is_admin": False, "is_active": True}


def _app_as(user):
    """Build a minimal app exposing the stream router as the given identity."""
    app = FastAPI()
    app.include_router(router)
    app.dependency_overrides[get_current_active_user] = lambda: user
    return app


def test_non_admin_cannot_disconnect_client():
    """V-003: a non-admin authenticated user is rejected BEFORE any disconnect."""
    app = _app_as(USER)
    with patch("src.api.routers.stream.connection_manager") as cm:
        cm.disconnect = AsyncMock(return_value=True)
        resp = TestClient(app).delete("/clients/victim-client-id")
    assert resp.status_code == 403
    cm.disconnect.assert_not_called()


def test_admin_can_disconnect_client():
    """An admin (operator) can disconnect any client."""
    app = _app_as(ADMIN)
    with patch("src.api.routers.stream.connection_manager") as cm:
        cm.disconnect = AsyncMock(return_value=True)
        resp = TestClient(app).delete("/clients/some-client-id")
    assert resp.status_code == 200
    cm.disconnect.assert_awaited_once_with("some-client-id")


def test_admin_disconnect_missing_client_returns_404():
    """Admin disconnecting an unknown client gets 404 (not 500, not 403)."""
    app = _app_as(ADMIN)
    with patch("src.api.routers.stream.connection_manager") as cm:
        cm.disconnect = AsyncMock(return_value=False)
        resp = TestClient(app).delete("/clients/ghost")
    assert resp.status_code == 404
