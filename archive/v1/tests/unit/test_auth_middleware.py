"""Tests for AuthMiddleware and TokenManager."""

import pytest
import os
from unittest.mock import MagicMock, AsyncMock, patch
from datetime import datetime, timedelta


class TestTokenManager:
    def test_create_token(self, mock_settings):
        from src.middleware.auth import TokenManager
        tm = TokenManager(mock_settings)
        token = tm.create_access_token({"sub": "user1"})
        assert isinstance(token, str)
        assert len(token) > 0

    def test_verify_valid_token(self, mock_settings):
        from src.middleware.auth import TokenManager
        tm = TokenManager(mock_settings)
        token = tm.create_access_token({"sub": "user1", "role": "admin"})
        payload = tm.verify_token(token)
        assert payload["sub"] == "user1"
        assert payload["role"] == "admin"

    def test_verify_invalid_token(self, mock_settings):
        from src.middleware.auth import TokenManager, AuthenticationError
        tm = TokenManager(mock_settings)
        with pytest.raises(AuthenticationError):
            tm.verify_token("invalid.token.here")

    def test_decode_claims(self, mock_settings):
        from src.middleware.auth import TokenManager
        tm = TokenManager(mock_settings)
        token = tm.create_access_token({"sub": "user1"})
        claims = tm.decode_token_claims(token)
        assert claims is not None
        assert claims["sub"] == "user1"

    def test_decode_claims_invalid(self, mock_settings):
        from src.middleware.auth import TokenManager
        tm = TokenManager(mock_settings)
        claims = tm.decode_token_claims("bad-token")
        assert claims is None

    def test_token_has_expiry(self, mock_settings):
        from src.middleware.auth import TokenManager
        tm = TokenManager(mock_settings)
        token = tm.create_access_token({"sub": "user1"})
        payload = tm.verify_token(token)
        assert "exp" in payload
        assert "iat" in payload


class TestUserManager:
    def test_create_user(self):
        from src.middleware.auth import UserManager
        um = UserManager()
        assert um.get_user("nonexistent") is None

    def test_hash_password(self):
        from src.middleware.auth import UserManager
        hashed = UserManager.hash_password("secret123")
        assert hashed != "secret123"
        assert len(hashed) > 20

    def test_verify_password(self):
        from src.middleware.auth import UserManager
        hashed = UserManager.hash_password("secret123")
        assert UserManager.verify_password("secret123", hashed) is True
        assert UserManager.verify_password("wrong", hashed) is False


class TestTokenBlacklist:
    def test_add_and_check(self):
        from src.api.middleware.auth import TokenBlacklist
        bl = TokenBlacklist()
        bl.add_token("tok123")
        assert bl.is_blacklisted("tok123") is True
        assert bl.is_blacklisted("tok456") is False

    def test_blacklisted_token_rejected(self, mock_settings):
        from src.middleware.auth import TokenManager, AuthenticationError
        from src.api.middleware.auth import token_blacklist

        tm = TokenManager(mock_settings)
        token = tm.create_access_token({"sub": "user1"})
        # Token should be valid
        tm.verify_token(token)
        # Blacklist it
        token_blacklist.add_token(token)
        with pytest.raises(AuthenticationError, match="revoked"):
            tm.verify_token(token)
        # Cleanup
        token_blacklist._blacklisted_tokens.discard(token)


class TestAuthMiddleware:
    def test_public_paths(self, mock_settings):
        with patch("src.api.middleware.auth.get_settings", return_value=mock_settings):
            from src.api.middleware.auth import AuthMiddleware
            app = MagicMock()
            mw = AuthMiddleware(app)
            assert mw._is_public_path("/health") is True
            assert mw._is_public_path("/docs") is True
            assert mw._is_public_path("/api/v1/pose/analyze") is False

    def test_protected_paths(self, mock_settings):
        with patch("src.api.middleware.auth.get_settings", return_value=mock_settings):
            from src.api.middleware.auth import AuthMiddleware
            app = MagicMock()
            mw = AuthMiddleware(app)
            assert mw._is_protected_path("/api/v1/pose/analyze") is True
            assert mw._is_protected_path("/health") is False

    def test_extract_token_from_header(self, mock_settings):
        with patch("src.api.middleware.auth.get_settings", return_value=mock_settings):
            from src.api.middleware.auth import AuthMiddleware
            app = MagicMock()
            mw = AuthMiddleware(app)
            request = MagicMock()
            request.headers = {"authorization": "Bearer mytoken123"}
            request.query_params = {}
            request.cookies = {}
            token = mw._extract_token(request)
            assert token == "mytoken123"

    def test_extract_token_missing(self, mock_settings):
        with patch("src.api.middleware.auth.get_settings", return_value=mock_settings):
            from src.api.middleware.auth import AuthMiddleware
            app = MagicMock()
            mw = AuthMiddleware(app)
            request = MagicMock()
            request.headers = {}
            request.query_params = {}
            request.cookies = {}
            token = mw._extract_token(request)
            assert token is None
