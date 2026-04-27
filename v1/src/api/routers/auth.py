"""
Authentication router for WiFi-DensePose API.
Provides logout (token blacklisting) endpoint.
"""

import logging
from typing import Optional

from fastapi import APIRouter, Request, HTTPException, status

from src.api.middleware.auth import token_blacklist

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/auth", tags=["auth"])


@router.post("/logout")
async def logout(request: Request):
    """Logout by blacklisting the current Bearer token."""
    auth_header = request.headers.get("authorization")
    if not auth_header or not auth_header.startswith("Bearer "):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing or invalid Authorization header",
        )

    token = auth_header.split(" ", 1)[1]
    token_blacklist.add_token(token)
    logger.info("Token blacklisted via /auth/logout")

    return {"success": True, "message": "Token revoked"}
