"""Tests for rate limiting middleware."""

import pytest
from unittest.mock import MagicMock, AsyncMock, patch


class TestRateLimitMiddleware:
    def test_init(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert "anonymous" in mw.rate_limits
            assert "authenticated" in mw.rate_limits
            assert "admin" in mw.rate_limits

    def test_exempt_paths(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert "/health" in mw.exempt_paths
            assert "/metrics" in mw.exempt_paths

    def test_is_exempt(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert mw._is_exempt_path("/health") is True
            assert mw._is_exempt_path("/api/v1/pose/current") is False

    def test_path_specific_limits(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert "/api/v1/pose/current" in mw.path_limits
            assert mw.path_limits["/api/v1/pose/current"]["requests"] == 60

    def test_trusted_proxies_not_blocked(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert not mw._is_client_blocked("new-client-id")


class TestRateLimitConfig:
    def test_anonymous_limit(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert mw.rate_limits["anonymous"]["burst"] == 10

    def test_admin_limit(self, mock_settings):
        with patch("src.api.middleware.rate_limit.get_settings", return_value=mock_settings):
            from src.api.middleware.rate_limit import RateLimitMiddleware
            app = MagicMock()
            mw = RateLimitMiddleware(app)
            assert mw.rate_limits["admin"]["requests"] == 10000
