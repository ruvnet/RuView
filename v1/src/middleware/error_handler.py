"""
Global error handling middleware for WiFi-DensePose API
"""

import logging
import traceback
import time
from typing import Dict, Any, Optional, Callable, Union
from datetime import datetime

from fastapi import Request, Response, HTTPException, status
from fastapi.responses import JSONResponse
from fastapi.exceptions import RequestValidationError
from starlette.exceptions import HTTPException as StarletteHTTPException
from pydantic import ValidationError

from src.config.settings import Settings
from src.logger import get_request_context

logger = logging.getLogger(__name__)


class ErrorResponse:
    """Standardized error response format."""
    
    def __init__(
        self,
        error_code: str,
        message: str,
        details: Optional[Dict[str, Any]] = None,
        status_code: int = 500,
        request_id: Optional[str] = None,
    ):
        self.error_code = error_code
        self.message = message
        self.details = details or {}
        self.status_code = status_code
        self.request_id = request_id
        self.timestamp = datetime.utcnow().isoformat()
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON response."""
        response = {
            "error": {
                "code": self.error_code,
                "message": self.message,
                "timestamp": self.timestamp,
            }
        }
        
        if self.details:
            response["error"]["details"] = self.details
        
        if self.request_id:
            response["error"]["request_id"] = self.request_id
        
        return response
    
    def to_response(self) -> JSONResponse:
        """Convert to FastAPI JSONResponse."""
        headers = {}
        if self.request_id:
            headers["X-Request-ID"] = self.request_id
        
        return JSONResponse(
            status_code=self.status_code,
            content=self.to_dict(),
            headers=headers
        )


class ErrorHandler:
    """Central error handler for the application."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.include_traceback = settings.debug and settings.is_development
        self.log_errors = True
    
    def handle_http_exception(self, request: Request, exc: HTTPException) -> ErrorResponse:
        """Handle HTTP exceptions."""
        request_context = get_request_context()
        request_id = request_context.get("request_id")
        
        # Log the error
        if self.log_errors:
            logger.warning(
                f"HTTP {exc.status_code}: {exc.detail} - "
                f"{request.method} {request.url.path} - "
                f"Request ID: {request_id}"
            )
        
        # Determine error code
        error_code = self._get_error_code_for_status(exc.status_code)
        
        # Build error details
        details = {}
        if hasattr(exc, "headers") and exc.headers:
            details["headers"] = exc.headers
        
        if self.include_traceback and hasattr(exc, "__traceback__"):
            details["traceback"] = traceback.format_exception(
                type(exc), exc, exc.__traceback__
            )
        
        return ErrorResponse(
            error_code=error_code,
            message=str(exc.detail),
            details=details,
            status_code=exc.status_code,
            request_id=request_id
        )
    
    def handle_validation_error(self, request: Request, exc: RequestValidationError) -> ErrorResponse:
        """Handle request validation errors."""
        request_context = get_request_context()
        request_id = request_context.get("request_id")
        
        # Log the error
        if self.log_errors:
            logger.warning(
                f"Validation error: {exc.errors()} - "
                f"{request.method} {request.url.path} - "
                f"Request ID: {request_id}"
            )
        
        # Format validation errors
        validation_details = []
        for error in exc.errors():
            validation_details.append({
                "field": ".".join(str(loc) for loc in error["loc"]),
                "message": error["msg"],
                "type": error["type"],
                "input": error.get("input"),
            })
        
        details = {
            "validation_errors": validation_details,
            "error_count": len(validation_details)
        }
        
        if self.include_traceback:
            details["traceback"] = traceback.format_exception(
                type(exc), exc, exc.__traceback__
            )
        
        return ErrorResponse(
            error_code="VALIDATION_ERROR",
            message="Request validation failed",
            details=details,
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            request_id=request_id
        )
    
    def handle_pydantic_error(self, request: Request, exc: ValidationError) -> ErrorResponse:
        """Handle Pydantic validation errors."""
        request_context = get_request_context()
        request_id = request_context.get("request_id")
        
        # Log the error
        if self.log_errors:
            logger.warning(
                f"Pydantic validation error: {exc.errors()} - "
                f"{request.method} {request.url.path} - "
                f"Request ID: {request_id}"
            )
        
        # Format validation errors
        validation_details = []
        for error in exc.errors():
            validation_details.append({
                "field": ".".join(str(loc) for loc in error["loc"]),
                "message": error["msg"],
                "type": error["type"],
            })
        
        details = {
            "validation_errors": validation_details,
            "error_count": len(validation_details)
        }
        
        return ErrorResponse(
            error_code="DATA_VALIDATION_ERROR",
            message="Data validation failed",
            details=details,
            status_code=status.HTTP_400_BAD_REQUEST,
            request_id=request_id
        )
    
    def handle_generic_exception(self, request: Request, exc: Exception) -> ErrorResponse:
        """Handle generic exceptions."""
        request_context = get_request_context()
        request_id = request_context.get("request_id")
        
        # Log the error
        if self.log_errors:
            logger.error(
                f"Unhandled exception: {type(exc).__name__}: {exc} - "
                f"{request.method} {request.url.path} - "
                f"Request ID: {request_id}",
                exc_info=True
            )
        
        # Determine error details
        details = {
            "exception_type": type(exc).__name__,
        }
        
        if self.include_traceback:
            details["traceback"] = traceback.format_exception(
                type(exc), exc, exc.__traceback__
            )
        
        # Don't expose internal error details in production
        if self.settings.is_production:
            message = "An internal server error occurred"
        else:
            message = str(exc) or "An unexpected error occurred"
        
        return ErrorResponse(
            error_code="INTERNAL_SERVER_ERROR",
            message=message,
            details=details,
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            request_id=request_id
        )
    
    def handle_database_error(self, request: Request, exc: Exception) -> ErrorResponse:
        """Handle database-related errors."""
        request_context = get_request_context()
        request_id = request_context.get("request_id")
        
        # Log the error
        if self.log_errors:
            logger.error(
                f"Database error: {type(exc).__name__}: {exc} - "
                f"{request.method} {request.url.path} - "
                f"Request ID: {request_id}",
                exc_info=True
            )
        
        details = {
            "exception_type": type(exc).__name__,
            "category": "database"
        }
        
        if self.include_traceback:
            details["traceback"] = traceback.format_exception(
                type(exc), exc, exc.__traceback__
            )
        
        return ErrorResponse(
            error_code="DATABASE_ERROR",
            message="Database operation failed" if self.settings.is_production else str(exc),
            details=details,
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            request_id=request_id
        )
    
    def handle_external_service_error(self, request: Request, exc: Exception) -> ErrorResponse:
        """Handle external service errors."""
        request_context = get_request_context()
        request_id = request_context.get("request_id")
        
        # Log the error
        if self.log_errors:
            logger.error(
                f"External service error: {type(exc).__name__}: {exc} - "
                f"{request.method} {request.url.path} - "
                f"Request ID: {request_id}",
                exc_info=True
            )
        
        details = {
            "exception_type": type(exc).__name__,
            "category": "external_service"
        }
        
        return ErrorResponse(
            error_code="EXTERNAL_SERVICE_ERROR",
            message="External service unavailable" if self.settings.is_production else str(exc),
            details=details,
            status_code=status.HTTP_502_BAD_GATEWAY,
            request_id=request_id
        )
    
    def _get_error_code_for_status(self, status_code: int) -> str:
        """Get error code for HTTP status code."""
        error_codes = {
            400: "BAD_REQUEST",
            401: "UNAUTHORIZED",
            403: "FORBIDDEN",
            404: "NOT_FOUND",
            405: "METHOD_NOT_ALLOWED",
            409: "CONFLICT",
            422: "UNPROCESSABLE_ENTITY",
            429: "TOO_MANY_REQUESTS",
            500: "INTERNAL_SERVER_ERROR",
            502: "BAD_GATEWAY",
            503: "SERVICE_UNAVAILABLE",
            504: "GATEWAY_TIMEOUT",
        }
        
        return error_codes.get(status_code, "HTTP_ERROR")


class ErrorHandlingMiddleware:
    """Error handling middleware for FastAPI."""
    
    def __init__(self, app, settings: Settings):
        self.app = app
        self.settings = settings
        self.error_handler = ErrorHandler(settings)
    
    async def __call__(self, scope, receive, send):
        """Process request through error handling middleware."""
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return
            
        start_time = time.time()
        
        try:
            await self.app(scope, receive, send)
        except Exception as exc:
            # Create a mock request for error handling
            from starlette.requests import Request
            request = Request(scope, receive)
            
            # Handle different exception types
            if isinstance(exc, HTTPException):
                error_response = self.error_handler.handle_http_exception(request, exc)
            elif isinstance(exc, RequestValidationError):
                error_response = self.error_handler.handle_validation_error(request, exc)
            elif isinstance(exc, ValidationError):
                error_response = self.error_handler.handle_pydantic_error(request, exc)
            else:
                # Check for specific error types
                if self._is_database_error(exc):
                    error_response = self.error_handler.handle_database_error(request, exc)
                elif self._is_external_service_error(exc):
                    error_response = self.error_handler.handle_external_service_error(request, exc)
                else:
                    error_response = self.error_handler.handle_generic_exception(request, exc)
            
            # Send the error response
            response = error_response.to_response()
            await response(scope, receive, send)
        
        finally:
            # Log request processing time
            processing_time = time.time() - start_time
            logger.debug(f"Error handling middleware processing time: {processing_time:.3f}s")
    
    def _is_database_error(self, exc: Exception) -> bool:
        """Check if exception is database-related."""
        database_exceptions = [
            "sqlalchemy",
            "psycopg2",
            "pymongo",
            "redis",
            "ConnectionError",
            "OperationalError",
            "IntegrityError",
        ]
        
        exc_module = getattr(type(exc), "__module__", "")
        exc_name = type(exc).__name__
        
        return any(
            db_exc in exc_module or db_exc in exc_name
            for db_exc in database_exceptions
        )
    
    def _is_external_service_error(self, exc: Exception) -> bool:
        """Check if exception is external service-related."""
        external_exceptions = [
            "requests",
            "httpx",
            "aiohttp",
            "urllib",
            "ConnectionError",
            "TimeoutError",
            "ConnectTimeout",
            "ReadTimeout",
        ]
        
        exc_module = getattr(type(exc), "__module__", "")
        exc_name = type(exc).__name__
        
        return any(
            ext_exc in exc_module or ext_exc in exc_name
            for ext_exc in external_exceptions
        )


def setup_error_handling(app, settings: Settings):
    """Setup error handling for the application."""
    logger.info("Setting up error handling middleware")
    
    error_handler = ErrorHandler(settings)
    
    # Add exception handlers
    @app.exception_handler(HTTPException)
    async def http_exception_handler(request: Request, exc: HTTPException):
        error_response = error_handler.handle_http_exception(request, exc)
        return error_response.to_response()
    
    @app.exception_handler(StarletteHTTPException)
    async def starlette_http_exception_handler(request: Request, exc: StarletteHTTPException):
        # Convert Starlette HTTPException to FastAPI HTTPException
        fastapi_exc = HTTPException(status_code=exc.status_code, detail=exc.detail)
        error_response = error_handler.handle_http_exception(request, fastapi_exc)
        return error_response.to_response()
    
    @app.exception_handler(RequestValidationError)
    async def validation_exception_handler(request: Request, exc: RequestValidationError):
        error_response = error_handler.handle_validation_error(request, exc)
        return error_response.to_response()
    
    @app.exception_handler(ValidationError)
    async def pydantic_exception_handler(request: Request, exc: ValidationError):
        error_response = error_handler.handle_pydantic_error(request, exc)
        return error_response.to_response()
    
    @app.exception_handler(Exception)
    async def generic_exception_handler(request: Request, exc: Exception):
        error_response = error_handler.handle_generic_exception(request, exc)
        return error_response.to_response()
    
    # Add middleware for additional error handling
    # Note: We use exception handlers instead of custom middleware to avoid ASGI conflicts
    # The middleware approach is commented out but kept for reference
    # middleware = ErrorHandlingMiddleware(app, settings)
    # app.add_middleware(ErrorHandlingMiddleware, settings=settings)
    
    logger.info("Error handling configured")


class CustomHTTPException(HTTPException):
    """Custom HTTP exception with additional context."""
    
    def __init__(
        self,
        status_code: int,
        detail: str,
        error_code: Optional[str] = None,
        context: Optional[Dict[str, Any]] = None,
        headers: Optional[Dict[str, str]] = None,
    ):
        super().__init__(status_code=status_code, detail=detail, headers=headers)
        self.error_code = error_code
        self.context = context or {}


class BusinessLogicError(CustomHTTPException):
    """Exception for business logic errors."""
    
    def __init__(self, message: str, context: Optional[Dict[str, Any]] = None):
        super().__init__(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=message,
            error_code="BUSINESS_LOGIC_ERROR",
            context=context
        )


class ResourceNotFoundError(CustomHTTPException):
    """Exception for resource not found errors."""
    
    def __init__(self, resource: str, identifier: str):
        super().__init__(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"{resource} not found",
            error_code="RESOURCE_NOT_FOUND",
            context={"resource": resource, "identifier": identifier}
        )


class ConflictError(CustomHTTPException):
    """Exception for conflict errors."""
    
    def __init__(self, message: str, context: Optional[Dict[str, Any]] = None):
        super().__init__(
            status_code=status.HTTP_409_CONFLICT,
            detail=message,
            error_code="CONFLICT_ERROR",
            context=context
        )


class ServiceUnavailableError(CustomHTTPException):
    """Exception for service unavailable errors."""
    
    def __init__(self, service: str, reason: Optional[str] = None):
        detail = f"{service} service is unavailable"
        if reason:
            detail += f": {reason}"
        
        super().__init__(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail=detail,
            error_code="SERVICE_UNAVAILABLE",
            context={"service": service, "reason": reason}
        )